import { describe, it, expect } from 'vitest';
import { powerScale, BASE_POWER_SCALE, POWER_CAP } from './chart-scale';

describe('powerScale', () => {
  it('keeps the base scale for ordinary workouts (peak fits under the cap)', () => {
    expect(powerScale(1.0)).toBe(BASE_POWER_SCALE);
    expect(powerScale(1.15)).toBe(BASE_POWER_SCALE);
  });

  it('compresses so a tall peak lands exactly on the cap', () => {
    expect(powerScale(6.4)).toBeCloseTo(POWER_CAP / 6.4, 5);
    // a 120% spike that used to clip now fits:
    expect(powerScale(1.2) * 1.2).toBeCloseTo(POWER_CAP, 5);
  });

  it('is safe for an empty/zero workout', () => {
    expect(powerScale(0)).toBe(BASE_POWER_SCALE);
  });
});
