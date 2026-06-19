// Vertical scale for WorkoutChart power bars, extracted pure so it is unit-tested.
// The chart viewBox is 0..100; a power fraction `f` (watts / ftpWatts) is drawn at
// height `f * powerScale(maxFrac)`, clamped to POWER_CAP.

/** Height (in viewBox units) of 1.0 (= 100% of the reference) when nothing exceeds the cap. */
export const BASE_POWER_SCALE = 85;
/** Never draw above this, leaving a sliver at the top for the peak label. */
export const POWER_CAP = 99;

/** Scale so the tallest planned fraction fits under the cap. Ordinary workouts
 *  (maxFrac <= POWER_CAP / BASE_POWER_SCALE ~= 1.16) keep BASE_POWER_SCALE, so
 *  their charts are unchanged; taller charts compress. */
export function powerScale(maxFrac: number): number {
  if (maxFrac <= 0) return BASE_POWER_SCALE;
  return Math.min(BASE_POWER_SCALE, POWER_CAP / maxFrac);
}
