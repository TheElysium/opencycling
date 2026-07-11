import { commands } from './bindings';
import * as poseDetection from '@tensorflow-models/pose-detection';
import * as tf from '@tensorflow/tfjs-core';
import '@tensorflow/tfjs-backend-webgl';
import {
  extractFeatures, buildCalibration, scoreFrame, isCalibrationStrong,
  Smoother, AeroGate, type Calibration, type FeatureVec, type Keypoint,
} from './aero';

const MODEL_URL = '/models/movenet-lightning/model.json';
const FRAME_MS = 100;           // ~10 fps
const CAPTURE_MS = 3000;        // per-position capture duration (~30 frames > MIN_CAPTURE_FRAMES)
const PREP_SECONDS = 5;         // get-into-position countdown before each capture
const REPORT_MS = 1000;         // align reports with the backend's 1 Hz sampling

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

export type CalibPhase = 'idle' | 'loading' | 'ready' | 'calibrated' | 'error';

// Sub-state of the automatic calibration run. `idle` = not started / between runs,
// `done` = run finished (read `phase`/`sep`/`notSeen` for the verdict). The prep/rec
// steps drive the live countdown + progress UI.
export type CalibStep =
  | 'idle' | 'prepAero' | 'recAero' | 'prepUpright' | 'recUpright' | 'done';

// Front-camera aero detection store. Mirrors the singleton pattern of ble/session.
// Owns the MoveNet detector, the camera stream, the calibration FSM, the live loop,
// and the 1 Hz reporting to Rust. The aero/upright decision (smoothing + hysteresis)
// is made here and reported as a boolean: Rust just averages it into aero_pct, so the
// persisted history matches exactly what the rider saw live. No unit tests
// (browser/WebGL/camera I/O, per repo convention).
class AeroStore {
  enabled = $state(false);                  // set from the start checkbox
  phase = $state<CalibPhase>('idle');
  error = $state<string | null>(null);
  currentScore = $state<number | null>(null); // smoothed 0..1, for display
  inAero = $state(false);                    // debounced aero/upright state
  sep = $state(0);                           // z-space centroid separation
  strong = $state(false);                    // calibration separates poses enough

  // Automatic-run UI state (driven by runCalibration).
  step = $state<CalibStep>('idle');          // current sub-step of the auto run
  countdown = $state(0);                     // seconds left in the current prep step
  captureFrac = $state(0);                   // 0..1 progress of the current capture
  liveDetected = $state(false);              // a usable pose is visible right now
  notSeen = $state(false);                   // last run failed: rider out of frame

  private detector: poseDetection.PoseDetector | null = null;
  private stream: MediaStream | null = null;
  private video: HTMLVideoElement | null = null;
  private calib: Calibration | null = null;
  private smoother = new Smoother(8);
  private gate = new AeroGate();
  private loopHandle: number | null = null;
  private captures = { aero: [] as FeatureVec[], upright: [] as FeatureVec[] };
  // True while a calibration run is in flight. The run is a long async loop that
  // outlives the component, so it checks this flag after each await and bails the
  // moment reset() (called by teardown when leaving the page) clears it, instead of
  // resuming where it left off. Runs never overlap (the recalibrate button is
  // disabled mid-run), so a plain flag is enough; no generation counter needed.
  private calibrating = false;

  async init(video: HTMLVideoElement): Promise<void> {
    this.video = video;
    this.phase = 'loading';
    this.error = null;
    try {
      await tf.setBackend('webgl');
      await tf.ready();
      this.detector = await poseDetection.createDetector(
        poseDetection.SupportedModels.MoveNet,
        { modelType: poseDetection.movenet.modelType.SINGLEPOSE_LIGHTNING, modelUrl: MODEL_URL },
      );
      this.stream = await navigator.mediaDevices.getUserMedia({ video: { width: 640, height: 480 } });
      video.srcObject = this.stream;
      await video.play();
      this.phase = 'ready';
    } catch (e) {
      this.error = e instanceof Error ? e.message : String(e);
      this.phase = 'error';
    }
  }

  // Re-bind the live stream to a new <video> element. The calibration overlay owns
  // the original element and unmounts when the session starts; the detached element
  // stops producing frames, so the session must hand the loop a live, in-DOM video.
  attachVideo(video: HTMLVideoElement): void {
    this.video = video;
    if (this.stream) {
      video.srcObject = this.stream;
      video.play().catch(() => {});
    }
  }

  private async estimate(): Promise<Keypoint[] | null> {
    if (!this.detector || !this.video) return null;
    try {
      const poses = await this.detector.estimatePoses(this.video, { flipHorizontal: false });
      return (poses[0]?.keypoints as Keypoint[]) ?? null;
    } catch { return null; }
  }

  // Run the whole hands-free calibration: prep countdown → capture aero →
  // prep countdown → capture upright → build + score. The rider never touches the
  // screen mid-session; all feedback is on `step`/`countdown`/`captureFrac`/`quality`.
  async runCalibration(): Promise<void> {
    if (this.detector == null || this.phase === 'loading' || this.phase === 'error') return;
    this.stopPreview(); // a re-run supersedes any running self-test
    this.calibrating = true; // cleared by reset() (teardown) to cancel this run
    // Clear any previous result so a re-run starts from a clean slate.
    this.captures = { aero: [], upright: [] };
    this.calib = null;
    this.sep = 0;
    this.strong = false;
    this.notSeen = false;

    // Each step owns its own `step`/`phase` and bails on its own if cancelled, so the
    // sequence here stays a flat list of awaits. Once cancelled, every later step is a
    // no-op, so a single guard before committing the result is all we need.
    await this.prep('prepAero');
    await this.captureInto('aero');
    await this.prep('prepUpright');
    await this.captureInto('upright');
    if (!this.calibrating) return;

    this.step = 'done';
    const c = buildCalibration(this.captures.aero, this.captures.upright);
    if (!c) {
      // Too few valid frames in one pose: the rider was out of frame / occluded.
      this.notSeen = true;
      this.phase = 'ready';
      return;
    }
    this.sep = c.sep;
    this.strong = isCalibrationStrong(c);
    // Only accept a calibration that separates the two poses; a weak axis is noise.
    if (this.strong) {
      this.calib = c;
      this.phase = 'calibrated';
    } else {
      this.calib = null;
      this.phase = 'ready';
    }
  }

  // Hold a "get into position" countdown, polling the pose so the UI can show
  // whether the rider is currently in frame before the capture starts.
  private async prep(step: 'prepAero' | 'prepUpright'): Promise<void> {
    if (!this.calibrating) return; // cancelled (page left)
    this.step = step;
    for (let s = PREP_SECONDS; s > 0; s--) {
      if (!this.calibrating) return; // cancelled (page left)
      this.countdown = s;
      const until = performance.now() + 1000;
      while (performance.now() < until) {
        const kp = await this.estimate();
        this.liveDetected = !!(kp && extractFeatures(kp));
        await sleep(200);
      }
    }
    this.countdown = 0;
  }

  // Capture one reference position for `CAPTURE_MS`, updating capture progress and
  // live-detection feedback as frames come in. If the rider drops out of frame for
  // more than `LOST_FRAMES` in a row, the partial capture is discarded and the timer
  // restarts from zero so the reference is built from one continuous, in-frame hold.
  private async captureInto(which: 'aero' | 'upright'): Promise<void> {
    if (!this.calibrating) return; // cancelled (page left)
    this.step = which === 'aero' ? 'recAero' : 'recUpright';
    const LOST_FRAMES = 6; // ~0.6s out of frame restarts this position
    this.captures[which] = [];
    this.captureFrac = 0;
    let start = performance.now();
    let misses = 0;
    while (performance.now() - start < CAPTURE_MS) {
      if (!this.calibrating) return; // cancelled (page left)
      const kp = await this.estimate();
      const f = kp && extractFeatures(kp);
      this.liveDetected = !!f;
      if (f) {
        this.captures[which].push(f);
        misses = 0;
      } else if (++misses >= LOST_FRAMES) {
        this.captures[which] = [];
        start = performance.now();
        misses = 0;
      }
      this.captureFrac = Math.min(1, (performance.now() - start) / CAPTURE_MS);
      await sleep(FRAME_MS);
    }
    this.captureFrac = 1;
  }

  // Score the current frame against the calibration, updating currentScore/inAero
  // for display. Returns the smoothed score, or null when there is no usable pose
  // (occlusion / out of frame). Shared by the live loop and the self-test preview.
  private async scoreOnce(): Promise<number | null> {
    const kp = await this.estimate();
    const f = kp && extractFeatures(kp);
    if (!f || !this.calib) {
      this.currentScore = null;
      return null;
    }
    const s = this.smoother.push(scoreFrame(f, this.calib));
    this.currentScore = s;
    this.inAero = this.gate.update(s);
    return s;
  }

  // Single scoring loop. Always updates currentScore/inAero for display; when
  // `report` is set it also pushes the debounced boolean to Rust ~1 Hz. The session
  // loop reports; the post-calibration self-test (preview) only displays, so the
  // rider can verify the aero/upright split before committing to the ride.
  private startScoring(report: boolean): void {
    if (!this.calib || this.loopHandle != null) return;
    this.smoother.reset();       // start from a clean smoothing/gate state
    this.gate.reset();
    let lastReport = 0;
    const tick = async () => {
      const s = await this.scoreOnce();
      if (report) {
        const now = performance.now();
        if (now - lastReport >= REPORT_MS) {
          lastReport = now;
          // No usable frame: report null so a stalled view does not bias aero_pct.
          commands.reportAero(s != null ? this.inAero : null).catch(() => {});
        }
      }
      this.loopHandle = window.setTimeout(tick, FRAME_MS);
    };
    this.loopHandle = window.setTimeout(tick, FRAME_MS);
  }

  private stopScoring(): void {
    if (this.loopHandle != null) {
      clearTimeout(this.loopHandle);
      this.loopHandle = null;
    }
  }

  // Session loop: scores + reports to Rust. Supersedes any running self-test.
  startLoop(): void {
    this.stopScoring();
    this.startScoring(true);
  }
  stopLoop(): void { this.stopScoring(); }

  // Post-calibration self-test: scores for display only, no reporting.
  startPreview(): void { this.startScoring(false); }
  stopPreview(): void { this.stopScoring(); }

  teardown(): void {
    this.stopScoring();
    this.stream?.getTracks().forEach((t) => t.stop());
    this.stream = null;
    this.detector?.dispose();
    this.detector = null;
    this.video = null;
    this.reset();
  }

  reset(): void {
    this.calibrating = false; // cancel any in-flight calibration run (teardown -> reset on page leave)
    this.phase = 'idle';
    this.error = null;
    this.currentScore = null;
    this.inAero = false;
    this.calib = null;
    this.sep = 0;
    this.strong = false;
    this.step = 'idle';
    this.countdown = 0;
    this.captureFrac = 0;
    this.liveDetected = false;
    this.notSeen = false;
    this.smoother.reset();
    this.gate.reset();
    this.captures = { aero: [], upright: [] };
  }
}

export const aero = new AeroStore();
