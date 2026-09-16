import type { EntryContent, PlanEntryView } from '$lib/bindings';
import { displayWorkoutName } from './format';

/**
 * What saving a day amounts to. The backend rejects an entry with neither
 * workout nor note, so clearing the only content deletes the entry.
 */
export type DayAction =
  | { kind: 'none' }
  | { kind: 'create'; content: EntryContent }
  | { kind: 'update'; entryId: number; content: EntryContent }
  | { kind: 'delete'; entryId: number };

/** The part of a picked library workout an entry stores. */
export type PickedWorkout = {
  file_name: string;
  /** Raw `<name>` of the .zwo: absent or blank for a nameless workout. */
  workout_name: string | null;
};

// Mirrors `normalize_entry` in src-tauri/src/plan/entry.rs: blank means absent.
function normalize(note: string): string | null {
  const trimmed = note.trim();
  return trimmed === '' ? null : trimmed;
}

/**
 * The name to persist beside a file: authored names survive verbatim, and the
 * file stem stands in for the empty ones early rows and nameless .zwo produce.
 */
export function entryWorkoutName(
  fileName: string | null,
  workoutName: string | null,
): string | null {
  if (fileName === null) return null;
  return workoutName?.trim() || displayWorkoutName(fileName);
}

function write(entry: PlanEntryView | null, content: EntryContent): DayAction {
  return entry === null
    ? { kind: 'create', content }
    : { kind: 'update', entryId: entry.entry_id, content };
}

export function noteAction(entry: PlanEntryView | null, note: string): DayAction {
  const next = normalize(note);
  if (entry === null) {
    return next === null
      ? { kind: 'none' }
      : write(null, { file_name: null, workout_name: null, note: next });
  }
  if (next === entry.note) return { kind: 'none' };
  if (next === null && entry.file_name === null) return { kind: 'delete', entryId: entry.entry_id };
  return write(entry, {
    file_name: entry.file_name,
    workout_name: entryWorkoutName(entry.file_name, entry.workout_name),
    note: next,
  });
}

/** A picked workout lands on the day's existing entry: one entry, not a stack. */
export function assignAction(
  entry: PlanEntryView | null,
  picked: PickedWorkout,
  note: string,
): DayAction {
  return write(entry, {
    file_name: picked.file_name,
    workout_name: entryWorkoutName(picked.file_name, picked.workout_name),
    note: normalize(note),
  });
}

/** Takes the note being edited, not the stored one: Remove must not resurrect it. */
export function removeWorkoutAction(entry: PlanEntryView, note: string): DayAction {
  const next = normalize(note);
  if (next === null) return { kind: 'delete', entryId: entry.entry_id };
  return write(entry, { file_name: null, workout_name: null, note: next });
}
