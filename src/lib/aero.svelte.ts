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
const REPORT_MS = 1000;         // align reports with the backend's 1 Hz sampling
const SITUP_ALERT_MS = 3000;    // sit-up alert delay

export type CalibPhase =
  | 'idle' | 'loading' | 'ready'
  | 'capturingAero' | 'capturingUpright'
  | 'calibrated' | 'error';

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

  private async estimate(): Promise<Keypoint[] | null> {
    if (!this.detector || !this.video) return null;
    try {
      const poses = await this.detector.estimatePoses(this.video, { flipHorizontal: false });
      return (poses[0]?.keypoints as Keypoint[]) ?? null;
    } catch { return null; }
  }

  // Capture one reference position for `CAPTURE_MS`, then (re)build calibration.
  async capture(which: 'aero' | 'upright'): Promise<void> {
    this.captures[which] = [];
    this.capturing = which;
    this.phase = which === 'aero' ? 'capturingAero' : 'capturingUpright';
    const until = performance.now() + CAPTURE_MS;
    while (performance.now() < until) {
      const kp = await this.estimate();
      const f = kp && extractFeatures(kp);
      if (f) this.captures[which].push(f);
      await new Promise((r) => setTimeout(r, FRAME_MS));
    }
    this.capturing = null;

    const c = buildCalibration(this.captures.aero, this.captures.upright);
    this.sep = c?.sep ?? 0;
    this.cohend = c?.cohend ?? null;
    this.strong = c ? isCalibrationStrong(c) : false;
    // Only accept a calibration that separates the two poses; a weak axis is noise.
    if (c && this.strong) {
      this.calib = c;
      this.phase = 'calibrated';
    } else {
      this.calib = null;
      this.phase = 'ready';
    }
  }

  // Start the live scoring loop. Pushes the debounced boolean to Rust ~1 Hz.
  startLoop(): void {
    if (!this.calib || this.loopHandle != null) return;
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

  teardown(): void {
    this.stopLoop();
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
