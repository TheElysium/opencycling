import type { ParsedWorkout, PlanEntryView, PlanWeek } from '$lib/bindings';
import { computeWorkoutMetrics, type WorkoutType } from './metrics';
import { workoutFtp } from './ftp';

export type WeekLoad = {
  durationS: number;
  tss: number;
  workouts: number;
  missing: number;
};

/** Indexes a scanned library by file name; a workout without one cannot back an entry. */
export function indexByFileName(workouts: ParsedWorkout[]): Map<string, ParsedWorkout> {
  const index = new Map<string, ParsedWorkout>();
  for (const w of workouts) {
    if (w.file_name !== null) index.set(w.file_name, w);
  }
  return index;
}

/** Sums a week's planned duration and TSS from library workouts, excluding entries the
 *  backend already flagged `missing` so the summary agrees with the day cells. */
export function weekLoad(week: PlanWeek, index: Map<string, ParsedWorkout>, ftpWatts: number): WeekLoad {
  let durationS = 0;
  let tss = 0;
  let workouts = 0;
  let missing = 0;

  for (const day of week.days) {
    for (const entry of day.entries) {
      if (entry.file_name === null) continue;
      // Single source of truth for "missing": the backend flag that also drives the
      // red day-cell styling (src-tauri/src/plan/schedule.rs, build_weeks).
      if (entry.missing) {
        missing++;
        continue;
      }
      const workout = index.get(entry.file_name);
      // Library not scanned yet: contribute nothing rather than claim a missing workout.
      if (workout === undefined) continue;
      workouts++;
      const m = computeWorkoutMetrics(workout.workout_blocks, workoutFtp(workout, ftpWatts));
      durationS += m.duration_s;
      // FTP tests are authored against FTP_TEST_REFERENCE_W (see workoutFtp), so their
      // TSS is not comparable; the workouts library hides it too.
      if (!workout.is_ftp_test) tss += m.tss;
    }
  }

  return { durationS, tss, workouts, missing };
}

/** One entry's workout type for the day-cell marker; same "no data" precedence as
 *  weekLoad (missing flag beats the index, a note-only entry has no file to look up). */
export function entryIntensity(
  entry: PlanEntryView,
  index: Map<string, ParsedWorkout>,
  ftpWatts: number,
): WorkoutType | null {
  if (entry.file_name === null || entry.missing) return null;
  const workout = index.get(entry.file_name);
  if (workout === undefined) return null;
  return computeWorkoutMetrics(workout.workout_blocks, workoutFtp(workout, ftpWatts)).type;
}

/** One lookup map for a whole plan: PlanDayCell reads its own entries back by id. */
export function entryIntensities(
  weeks: PlanWeek[],
  index: Map<string, ParsedWorkout>,
  ftpWatts: number,
): Map<number, WorkoutType | null> {
  const map = new Map<number, WorkoutType | null>();
  for (const week of weeks) {
    for (const day of week.days) {
      for (const entry of day.entries) {
        map.set(entry.entry_id, entryIntensity(entry, index, ftpWatts));
      }
    }
  }
  return map;
}

export function maxWeekTss(loads: WeekLoad[]): number {
  return loads.reduce((max, l) => Math.max(max, l.tss), 0);
}

/** Fraction of the bar to fill, clamped to `[0, 1]`; 0 when the plan has no load yet. */
export function loadBarRatio(tss: number, max: number): number {
  if (max <= 0) return 0;
  return Math.min(1, Math.max(0, tss / max));
}
