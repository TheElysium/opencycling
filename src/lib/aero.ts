// Aero-position detection from a FRONT-facing webcam (rider seen head-on / "de face").
// Every calculation here assumes a frontal view: both left and right ears and
// shoulders are visible, the body is roughly symmetric across the image vertical,
// and depth (the rider leaning toward/away from the camera) shows up as the head
// dropping toward the shoulder line and the ears spreading horizontally. None of
// this holds for a side view — the features below are only meaningful from the front.

// COCO-17 keypoint indices (MoveNet).
export const KP = {
  nose: 0, leftEye: 1, rightEye: 2, leftEar: 3, rightEar: 4,
  leftShoulder: 5, rightShoulder: 6, leftElbow: 7, rightElbow: 8,
  leftWrist: 9, rightWrist: 10, leftHip: 11, rightHip: 12,
} as const;

export const CONF = 0.3; // keypoint confidence threshold

export type Keypoint = { x: number; y: number; score: number };

// headPitch was dropped on purpose: headPitch === headDrop - earDrop exactly
// (both share the shoulder-line reference), so it carries no independent signal and
// only reweights the projection. These three are the linearly independent set.
export const FEATURES = ['headDrop', 'earDrop', 'earSpread'] as const;
export type FeatureName = (typeof FEATURES)[number];
export type FeatureVec = Record<FeatureName, number>;

// Upper-body only (nose, ears, shoulders): lower-body keypoints drop below
// confidence / leave the frame in aero. Returns null if any required keypoint
// is low-confidence.
//
// Known limitation: in a deep tuck the head drops and an ear can fall under CONF,
// so we may drop precisely the aero frames we want. If that biases capture in
// practice, make the ear features optional instead of rejecting the whole frame.
//
// FRONT-facing view assumed: the rider faces the camera, so both ears and both
// shoulders are visible. We normalize by the shoulder width and read the
// head-vs-shoulder geometry that, head-on, distinguishes aero from upright:
//   - headDrop:  nose drops toward the shoulder line as the rider gets low (front view).
//   - earDrop:   ear line drops toward the shoulder line (front view).
//   - earSpread: horizontal ear distance grows as the head leans toward the camera
//                (front view only, meaningless side-on).
export function extractFeatures(kp: Keypoint[]): FeatureVec | null {
  const get = (i: number): Keypoint | null => (kp[i] && kp[i].score >= CONF ? kp[i] : null);
  const ls = get(KP.leftShoulder), rs = get(KP.rightShoulder);
  const lEar = get(KP.leftEar), rEar = get(KP.rightEar);
  const nose = get(KP.nose);
  if (!ls || !rs || !lEar || !rEar || !nose) return null;

  const shoulderMidY = (ls.y + rs.y) / 2;
  const earMidY = (lEar.y + rEar.y) / 2;
  // Scale unit = apparent shoulder span. Caveat: it is not perfectly invariant.
  // In aero the shoulders roll forward/inward and foreshorten, so u shrinks a bit
  // exactly when the rider gets low, i.e. the unit tracks the signal slightly. The
  // per-rider z-scoring in calibration absorbs this; it is the first place to look
  // if scores ever drift. The inter-ear distance would be a more invariant unit.
  const u = Math.hypot(ls.x - rs.x, ls.y - rs.y);
  if (u < 1e-3) return null;

  return {
    headDrop:  (nose.y - shoulderMidY) / u,
    earDrop:   (earMidY - shoulderMidY) / u,
    earSpread: Math.abs(lEar.x - rEar.x) / u,
  };
}

function mean(a: number[]): number { return a.reduce((x, y) => x + y, 0) / a.length; }
function std(a: number[], m: number): number {
  if (a.length < 2) return 0;
  const v = a.reduce((x, y) => x + (y - m) ** 2, 0) / (a.length - 1);
  return Math.sqrt(v);
}

export type Calibration = {
  mean: FeatureVec;
  std: FeatureVec;
  aeroRef: FeatureVec;     // z-scored aero centroid
  uprightRef: FeatureVec;  // z-scored upright centroid
  cohend: FeatureVec;      // per-feature separation (diagnostic)
  sep: number;             // euclidean centroid separation in z-space
};

// ~3s of capture at ~8 fps: enough frames for stable centroids/std-devs. A handful
// of frames (sub-second) gives noisy references and a badly defined axis.
export const MIN_CAPTURE_FRAMES = 24;

// Minimum z-space centroid separation for a usable axis. Below this the two captured
// poses are too similar (rider did not exaggerate the difference), the axis is noise
// and scores degrade to a clamped guess. Callers should reject and re-prompt.
export const MIN_SEP = 1.0;

// Whether a built calibration separates the two poses enough to score reliably.
// Distinct from buildCalibration returning null (too few frames): here we have
// enough frames but the poses are too close. The UI can message each case.
export function isCalibrationStrong(c: Calibration): boolean {
  return c.sep >= MIN_SEP;
}

// Above this separation the two poses are cleanly distinct and scoring is robust.
// Between MIN_SEP and here the axis is usable but noisier (rider barely changed
// position). Tunable; picked from prototype runs, not a hard statistical bound.
export const SEP_GOOD = 1.8;

export type CalibQuality = 'poor' | 'fair' | 'good';

// Three-tier verdict for UI feedback. `poor` is rejected by isCalibrationStrong;
// `fair`/`good` are both accepted, the split is purely advisory.
export function calibQuality(sep: number): CalibQuality {
  if (sep >= SEP_GOOD) return 'good';
  if (sep >= MIN_SEP) return 'fair';
  return 'poor';
}

// Build the upright->aero axis from two captured clusters. Null if either is too small.
// Both clusters are sets of FRONT-facing feature vectors (rider seen head-on); the
// learned axis is therefore only valid for scoring frames captured from the front.
export function buildCalibration(aero: FeatureVec[], upright: FeatureVec[]): Calibration | null {
  if (aero.length < MIN_CAPTURE_FRAMES || upright.length < MIN_CAPTURE_FRAMES) return null;
  const all = [...aero, ...upright];
  const meanV = {} as FeatureVec, stdV = {} as FeatureVec;
  const aeroRef = {} as FeatureVec, uprightRef = {} as FeatureVec, cohend = {} as FeatureVec;
  for (const f of FEATURES) {
    const m = mean(all.map(v => v[f]));
    const s = std(all.map(v => v[f]), m) || 1e-6;
    meanV[f] = m; stdV[f] = s;
    const aMu = mean(aero.map(v => v[f])), uMu = mean(upright.map(v => v[f]));
    aeroRef[f] = (aMu - m) / s;
    uprightRef[f] = (uMu - m) / s;
    const aS = std(aero.map(v => v[f]), aMu), uS = std(upright.map(v => v[f]), uMu);
    const pooled = Math.sqrt((aS ** 2 + uS ** 2) / 2) || 1e-6;
    cohend[f] = Math.abs(aMu - uMu) / pooled;
  }
  let sep = 0;
  for (const f of FEATURES) sep += (aeroRef[f] - uprightRef[f]) ** 2;
  return { mean: meanV, std: stdV, aeroRef, uprightRef, cohend, sep: Math.sqrt(sep) };
}

// Project a feature vector onto the upright->aero axis. 0 = upright, 1 = aero.
// `feat` must come from a FRONT-facing frame (same geometry the calibration was built on).
export function scoreFrame(feat: FeatureVec, c: Calibration): number {
  let dot = 0, axisLen2 = 0;
  for (const f of FEATURES) {
    const z = (feat[f] - c.mean[f]) / c.std[f];
    const axis = c.aeroRef[f] - c.uprightRef[f];
    dot += (z - c.uprightRef[f]) * axis;
    axisLen2 += axis ** 2;
  }
  const t = axisLen2 < 1e-9 ? 0 : dot / axisLen2;
  return Math.max(0, Math.min(1, t));
}

// Hysteresis bounds on the smoothed score: enter aero above ENTER, leave below EXIT.
// The dead band between them kills the boundary flicker a single cutoff would produce
// when the smoothed score hovers near the middle.
export const AERO_ENTER = 0.55;
export const AERO_EXIT = 0.45;

// Stateful aero/upright decision. `update` takes a smoothed score and returns the
// debounced binary state. This boolean is the single source of truth: the frontend
// reports it over the Tauri bridge and Rust simply averages it into the session
// `aero_pct`, so the persisted history matches exactly what the rider saw live.
export class AeroGate {
  private inAero = false;
  update(smoothedScore: number): boolean {
    if (this.inAero) {
      if (smoothedScore < AERO_EXIT) this.inAero = false;
    } else if (smoothedScore > AERO_ENTER) {
      this.inAero = true;
    }
    return this.inAero;
  }
  reset(): void { this.inAero = false; }
  get state(): boolean { return this.inAero; }
}

// Sliding-average smoother (~1s at ~8 fps with size 8).
export class Smoother {
  private window: number[] = [];
  constructor(private size = 8) {}
  push(v: number): number {
    this.window.push(v);
    if (this.window.length > this.size) this.window.shift();
    return this.window.reduce((a, b) => a + b, 0) / this.window.length;
  }
  reset(): void { this.window = []; }
}
