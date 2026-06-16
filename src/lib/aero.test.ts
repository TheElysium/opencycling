import { describe, it, expect } from 'vitest';
import {
  extractFeatures, KP, type Keypoint, FEATURES, type FeatureVec,
  buildCalibration, scoreFrame, type Calibration,
  Smoother, AeroGate, isCalibrationStrong,
} from './aero';

// Helper: build a 17-slot keypoint array with confident defaults, then override.
function kps(overrides: Partial<Record<keyof typeof KP, Partial<Keypoint>>> = {}): Keypoint[] {
  const arr: Keypoint[] = Array.from({ length: 17 }, () => ({ x: 0, y: 0, score: 0.9 }));
  // sensible upright defaults: shoulders 100px apart, ears above shoulders, nose below ears
  arr[KP.leftShoulder]  = { x: 250, y: 300, score: 0.9 };
  arr[KP.rightShoulder] = { x: 150, y: 300, score: 0.9 };
  arr[KP.leftEar]       = { x: 230, y: 200, score: 0.9 };
  arr[KP.rightEar]      = { x: 170, y: 200, score: 0.9 };
  arr[KP.nose]          = { x: 200, y: 230, score: 0.9 };
  for (const [name, ov] of Object.entries(overrides)) {
    const i = KP[name as keyof typeof KP];
    arr[i] = { ...arr[i], ...ov };
  }
  return arr;
}

describe('extractFeatures', () => {
  it('returns all features for a valid front pose', () => {
    const f = extractFeatures(kps());
    expect(f).not.toBeNull();
    for (const name of FEATURES) expect(typeof f![name]).toBe('number');
  });

  it('normalizes by shoulder width (scale-invariant)', () => {
    const a = extractFeatures(kps())!;
    // double every coordinate -> features unchanged (same geometry, bigger image)
    const b = extractFeatures(kps({
      leftShoulder:  { x: 500, y: 600 }, rightShoulder: { x: 300, y: 600 },
      leftEar: { x: 460, y: 400 }, rightEar: { x: 340, y: 400 }, nose: { x: 400, y: 460 },
    }))!;
    expect(b.headDrop).toBeCloseTo(a.headDrop, 5);
    expect(b.earSpread).toBeCloseTo(a.earSpread, 5);
  });

  it('returns null when a required keypoint is below confidence', () => {
    expect(extractFeatures(kps({ nose: { score: 0.1 } }))).toBeNull();
    expect(extractFeatures(kps({ leftEar: { score: 0.1 } }))).toBeNull();
    expect(extractFeatures(kps({ rightShoulder: { score: 0.1 } }))).toBeNull();
  });
});

// Two clusters separated along every feature: a small linear ramp gives within-cluster
// jitter while the base vectors set the between-cluster separation.
function cluster(base: FeatureVec, jitter: number, n: number): FeatureVec[] {
  const out: FeatureVec[] = [];
  for (let i = 0; i < n; i++) {
    const k = (i / (n - 1) - 0.5) * 2 * jitter; // -jitter..+jitter
    out.push({ headDrop: base.headDrop + k, earDrop: base.earDrop + k,
               earSpread: base.earSpread + k });
  }
  return out;
}

const UPRIGHT = { headDrop: -0.7, earDrop: -1.0, earSpread: 0.6 };
const AERO    = { headDrop:  0.2, earDrop: -0.2, earSpread: 0.9 };

describe('calibration + scoring', () => {
  it('returns null with too few frames', () => {
    expect(buildCalibration(cluster(AERO, 0.05, 3), cluster(UPRIGHT, 0.05, 3))).toBeNull();
  });

  it('scores upright reference near 0 and aero reference near 1', () => {
    const c = buildCalibration(cluster(AERO, 0.05, 30), cluster(UPRIGHT, 0.05, 30)) as Calibration;
    expect(c).not.toBeNull();
    expect(scoreFrame(AERO, c)).toBeGreaterThan(0.8);
    expect(scoreFrame(UPRIGHT, c)).toBeLessThan(0.2);
  });

  it('clamps scores to [0,1]', () => {
    const c = buildCalibration(cluster(AERO, 0.05, 30), cluster(UPRIGHT, 0.05, 30)) as Calibration;
    const beyond = { headDrop: 1, earDrop: 1, earSpread: 1.5 };
    const s = scoreFrame(beyond, c);
    expect(s).toBeGreaterThanOrEqual(0);
    expect(s).toBeLessThanOrEqual(1);
  });
});

describe('calibration strength', () => {
  it('accepts well-separated poses, rejects near-identical ones', () => {
    const strong = buildCalibration(cluster(AERO, 0.05, 30), cluster(UPRIGHT, 0.05, 30))!;
    expect(isCalibrationStrong(strong)).toBe(true);
    // same pose captured twice: axis is noise, must be rejected.
    const weak = buildCalibration(cluster(AERO, 0.05, 30), cluster(AERO, 0.05, 30))!;
    expect(isCalibrationStrong(weak)).toBe(false);
  });
});

describe('smoothing', () => {
  it('averages a sliding window', () => {
    const s = new Smoother(4);
    expect(s.push(1)).toBeCloseTo(1);
    expect(s.push(0)).toBeCloseTo(0.5);
    expect(s.push(0)).toBeCloseTo(1 / 3);
  });
  it('drops oldest beyond window size', () => {
    const s = new Smoother(2);
    s.push(1); s.push(1);
    expect(s.push(0)).toBeCloseTo(0.5); // window now [1,0]
  });
});

describe('AeroGate hysteresis', () => {
  it('enters above ENTER, exits below EXIT, holds through the dead band', () => {
    const g = new AeroGate();
    expect(g.update(0.5)).toBe(false);   // below ENTER, still upright
    expect(g.update(0.56)).toBe(true);   // crosses ENTER -> aero
    expect(g.update(0.5)).toBe(true);    // dead band, holds aero
    expect(g.update(0.46)).toBe(true);   // still above EXIT, holds
    expect(g.update(0.44)).toBe(false);  // crosses EXIT -> upright
    expect(g.update(0.5)).toBe(false);   // dead band, holds upright
  });
});
