import type { ParsedWorkout, PlanDay, PlanEntryView, PlanWeek } from '$lib/bindings';
import { workoutFtp } from './ftp';

export type PlanStartResult =
  | { ok: true; workout: ParsedWorkout; ftpW: number }
  | { ok: false; error: string };

const NOTE_ONLY_ERROR = 'This entry has no workout to start.';
const NOT_IN_LIBRARY_ERROR = 'Workout not found in the library, rescan the library from Settings.';

/** Resolves an entry to its workout and run FTP, or the reason it cannot start yet.
 *  Callers already guard on `file_name`; this stays safe (no throw) either way. */
export function resolvePlanStart(
  entry: PlanEntryView,
  index: Map<string, ParsedWorkout>,
  libraryFtp: number,
): PlanStartResult {
  if (!entry.file_name) return { ok: false, error: NOTE_ONLY_ERROR };
  const workout = index.get(entry.file_name);
  if (!workout) return { ok: false, error: NOT_IN_LIBRARY_ERROR };
  return { ok: true, workout, ftpW: workoutFtp(workout, libraryFtp) };
}

/** The week and day carrying the backend's `Today` marker; null when the plan does
 *  not cover today (e.g. an empty list, or a plan whose range excludes today). */
export function todayOf(weeks: PlanWeek[]): { week: PlanWeek; day: PlanDay } | null {
  for (const week of weeks) {
    const day = week.days.find((d) => d.marker === 'Today');
    if (day) return { week, day };
  }
  return null;
}
