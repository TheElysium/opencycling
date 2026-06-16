import { invoke } from '@tauri-apps/api/core';
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
const SITUP_ALERT_MS = 3000;    // sit-up alert delay

export type CalibPhase =
  | 'idle' | 'loading' | 'ready'
  | 'capturingAero' | 'capturingUpright'
  | 'calibrated' | 'error';

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
  aeroPct = $state(0);                       // live share of valid frames in aero
  situpAlert = $state(false);                // true while sat up past the delay
  cohend = $state<FeatureVec | null>(null);  // calibration diagnostic
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
  private aeroFrames = 0;
  private validFrames = 0;
  private uprightSinceMs: number | null = null;
  private loopHandle: number | null = null;
  private previewHandle: number | null = null; // post-calibration self-test loop
  private reportable: boolean | null = null; // latest decision, null = no usable frame
  private captures = { aero: [] as FeatureVec[], upright: [] as FeatureVec[] };
  private capturing: 'aero' | 'upright' | null = null;

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
    // Clear any previous result so a re-run starts from a clean slate.
    this.captures = { aero: [], upright: [] };
    this.calib = null;
    this.sep = 0;
    this.cohend = null;
    this.strong = false;
    this.notSeen = false;

    this.step = 'prepAero';
    await this.prep(PREP_SECONDS);
    this.step = 'recAero';
    this.phase = 'capturingAero';
    await this.captureInto('aero');

    this.step = 'prepUpright';
    await this.prep(PREP_SECONDS);
    this.step = 'recUpright';
    this.phase = 'capturingUpright';
    await this.captureInto('upright');

    this.step = 'done';
    const c = buildCalibration(this.captures.aero, this.captures.upright);
    if (!c) {
      // Too few valid frames in one pose: the rider was out of frame / occluded.
      this.notSeen = true;
      this.phase = 'ready';
      return;
    }
    this.sep = c.sep;
    this.cohend = c.cohend;
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
  private async prep(seconds: number): Promise<void> {
    for (let s = seconds; s > 0; s--) {
      this.countdown = s;
      const until = performance.now() + 1000;
      while (performance.now() < until) {
        const kp = await this.estimate();
        this.liveDetected = !!(kp && extractFeatures(kp));
        await new Promise((r) => setTimeout(r, 200));
      }
    }
    this.countdown = 0;
  }

  // Capture one reference position for `CAPTURE_MS`, updating capture progress and
  // live-detection feedback as frames come in. If the rider drops out of frame for
  // more than `LOST_FRAMES` in a row, the partial capture is discarded and the timer
  // restarts from zero so the reference is built from one continuous, in-frame hold.
  private async captureInto(which: 'aero' | 'upright'): Promise<void> {
    const LOST_FRAMES = 6; // ~0.6s out of frame restarts this position
    this.captures[which] = [];
    this.capturing = which;
    this.captureFrac = 0;
    let start = performance.now();
    let misses = 0;
    while (performance.now() - start < CAPTURE_MS) {
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
      await new Promise((r) => setTimeout(r, FRAME_MS));
    }
    this.captureFrac = 1;
    this.capturing = null;
  }

  // Start the live scoring loop. Pushes the debounced boolean to Rust ~1 Hz.
  startLoop(): void {
    if (!this.calib || this.loopHandle != null) return;
    this.stopPreview();          // hand off cleanly from the self-test loop
    this.smoother.reset();       // start the session from a clean smoothing/gate state
    this.gate.reset();
    let lastReport = 0;
    const tick = async () => {
      const kp = await this.estimate();
      const f = kp && extractFeatures(kp);
      if (f && this.calib) {
        const s = this.smoother.push(scoreFrame(f, this.calib));
        this.currentScore = s;
        this.inAero = this.gate.update(s);
        this.reportable = this.inAero;
        this.validFrames++;
        if (this.inAero) {
          this.aeroFrames++;
          this.uprightSinceMs = null;
          this.situpAlert = false;
        } else {
          if (this.uprightSinceMs == null) this.uprightSinceMs = performance.now();
          if (performance.now() - this.uprightSinceMs >= SITUP_ALERT_MS) this.situpAlert = true;
        }
        this.aeroPct = this.validFrames > 0 ? this.aeroFrames / this.validFrames : 0;
      } else {
        // No usable frame (occlusion, out of frame): report nothing so a stalled
        // view does not bias aero_pct.
        this.currentScore = null;
        this.reportable = null;
      }

      const now = performance.now();
      if (now - lastReport >= REPORT_MS) {
        lastReport = now;
        invoke('report_aero', { aero: this.reportable }).catch(() => {});
      }
      this.loopHandle = window.setTimeout(tick, FRAME_MS);
    };
    this.loopHandle = window.setTimeout(tick, FRAME_MS);
  }

  stopLoop(): void {
    if (this.loopHandle != null) {
      clearTimeout(this.loopHandle);
      this.loopHandle = null;
    }
  }

  // Live self-test loop for the post-calibration check: scores frames and updates
  // currentScore/inAero for display only. No reporting to Rust and no aeroPct
  // accumulation (there is no session yet), so the rider can verify the aero/upright
  // split before committing to the ride.
  startPreview(): void {
    if (!this.calib || this.previewHandle != null) return;
    this.smoother.reset();
    this.gate.reset();
    const tick = async () => {
      const kp = await this.estimate();
      const f = kp && extractFeatures(kp);
      if (f && this.calib) {
        this.currentScore = this.smoother.push(scoreFrame(f, this.calib));
        this.inAero = this.gate.update(this.currentScore);
      } else {
        this.currentScore = null;
      }
      this.previewHandle = window.setTimeout(tick, FRAME_MS);
    };
    this.previewHandle = window.setTimeout(tick, FRAME_MS);
  }

  stopPreview(): void {
    if (this.previewHandle != null) {
      clearTimeout(this.previewHandle);
      this.previewHandle = null;
    }
  }

  teardown(): void {
    this.stopLoop();
    this.stopPreview();
    this.stream?.getTracks().forEach((t) => t.stop());
    this.stream = null;
    this.detector?.dispose();
    this.detector = null;
    this.video = null;
    this.reset();
  }

  reset(): void {
    this.phase = 'idle';
    this.error = null;
    this.currentScore = null;
    this.inAero = false;
    this.aeroPct = 0;
    this.situpAlert = false;
    this.calib = null;
    this.cohend = null;
    this.sep = 0;
    this.strong = false;
    this.step = 'idle';
    this.countdown = 0;
    this.captureFrac = 0;
    this.liveDetected = false;
    this.notSeen = false;
    this.smoother.reset();
    this.gate.reset();
    this.aeroFrames = 0;
    this.validFrames = 0;
    this.uprightSinceMs = null;
    this.reportable = null;
    this.captures = { aero: [], upright: [] };
    this.capturing = null;
  }
}

export const aero = new AeroStore();
