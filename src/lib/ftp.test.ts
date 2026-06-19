import { describe, it, expect } from 'vitest';
import {
  bestRollingAvg,
  estimateFtpFromRamp,
  workoutFtp,
  FTP_TEST_REFERENCE_W,
  stepDropDetector,
  INITIAL_DROP_STATE,
} from './ftp';

describe('bestRollingAvg', () => {
  it('returns the best full window average', () => {
    const power = [...Array(60).fill(200), ...Array(60).fill(300)];
    expect(bestRollingAvg(power, 60)).toBe(300);
  });

  it('averages the whole series when shorter than the window', () => {
    expect(bestRollingAvg([100, 200, 300], 60)).toBe(200);
  });

  it('does not over-credit a short partial final step', () => {
    // 60s @ 620 then only 18s @ 640: best 60s window straddles, < 640.
    const power = [...Array(60).fill(620), ...Array(18).fill(640)];
    const best = bestRollingAvg(power, 60);
    expect(best).toBeGreaterThan(620);
    expect(best).toBeLessThan(640);
  });
});

describe('estimateFtpFromRamp', () => {
  it('is 75% of the best 1-min power, rounded', () => {
    const power = [...Array(60).fill(100), ...Array(60).fill(400)];
    expect(estimateFtpFromRamp(power)).toBe(300); // 0.75 * 400
  });
});

describe('workoutFtp', () => {
  it('uses the reference for a test, the rider FTP otherwise', () => {
    expect(workoutFtp({ is_ftp_test: true }, 240)).toBe(FTP_TEST_REFERENCE_W);
    expect(workoutFtp({ is_ftp_test: false }, 240)).toBe(240);
  });

  it('falls back to the rider FTP for a missing workout', () => {
    expect(workoutFtp(null, 240)).toBe(240);
    expect(workoutFtp(undefined, 240)).toBe(240);
  });
});

describe('stepDropDetector', () => {
  const tick = (prev: typeof INITIAL_DROP_STATE, powerW: number, targetW = 300) =>
    stepDropDetector(prev, { running: true, targetW, powerW });

  it('stays clear while the rider holds the target', () => {
    expect(tick(INITIAL_DROP_STATE, 300)).toEqual(INITIAL_DROP_STATE);
  });

  it('counts down 5..1 then raises the prompt on the 6th failing tick', () => {
    let s = INITIAL_DROP_STATE;
    const seen: (number | null)[] = [];
    for (let i = 0; i < 6; i++) {
      s = tick(s, 0); // power collapsed to 0, target 300
      seen.push(s.countdown);
    }
    expect(seen).toEqual([5, 4, 3, 2, 1, null]);
    expect(s.prompt).toBe(true);
  });

  it('resets the moment the rider recovers above the threshold', () => {
    let s = tick(tick(INITIAL_DROP_STATE, 0), 0); // two failing ticks
    expect(s.countdown).toBe(4);
    s = tick(s, 300); // back on target
    expect(s).toEqual(INITIAL_DROP_STATE);
  });

  it('freezes once the prompt is up, waiting on the rider', () => {
    const up = { lowTicks: 6, countdown: null, prompt: true };
    expect(tick(up, 0)).toBe(up);
  });

  it('clears when not running or without a real target', () => {
    const mid = tick(INITIAL_DROP_STATE, 0);
    expect(stepDropDetector(mid, { running: false, targetW: 300, powerW: 0 })).toEqual(
      INITIAL_DROP_STATE,
    );
    expect(stepDropDetector(mid, { running: true, targetW: null, powerW: 0 })).toEqual(
      INITIAL_DROP_STATE,
    );
  });
});
