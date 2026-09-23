import { describe, it, expect } from 'vitest';
import type { PlanEntryView } from '$lib/bindings';
import { assignAction, entryWorkoutName, noteAction, removeWorkoutAction } from './plan-entry';

function entry(overrides: Partial<PlanEntryView> = {}): PlanEntryView {
  return {
    entry_id: 7,
    file_name: null,
    workout_name: null,
    note: null,
    session_id: null,
    missing: false,
    ...overrides,
  };
}

const workoutEntry = entry({ file_name: 'base.zwo', workout_name: 'Base', note: null });
const noteEntry = entry({ note: 'swim 45min' });

describe('noteAction', () => {
  it('creates a note-only entry on a blank day', () => {
    expect(noteAction(null, 'swim 45min')).toEqual({
      kind: 'create',
      content: { file_name: null, workout_name: null, note: 'swim 45min' },
    });
  });

  it('does nothing when a blank day gets a blank note', () => {
    expect(noteAction(null, '   ')).toEqual({ kind: 'none' });
  });

  it('adds a note to a day that already holds a workout', () => {
    expect(noteAction(workoutEntry, 'easy gearing')).toEqual({
      kind: 'update',
      entryId: 7,
      content: { file_name: 'base.zwo', workout_name: 'Base', note: 'easy gearing' },
    });
  });

  it('edits the note of a note-only entry', () => {
    expect(noteAction(noteEntry, 'swim 60min')).toEqual({
      kind: 'update',
      entryId: 7,
      content: { file_name: null, workout_name: null, note: 'swim 60min' },
    });
  });

  it('clears the note of a workout entry without deleting the workout', () => {
    expect(noteAction(entry({ file_name: 'base.zwo', workout_name: 'Base', note: 'old' }), '')).toEqual({
      kind: 'update',
      entryId: 7,
      content: { file_name: 'base.zwo', workout_name: 'Base', note: null },
    });
  });

  it('deletes the entry when the only content it had was the cleared note', () => {
    expect(noteAction(noteEntry, '')).toEqual({ kind: 'delete', entryId: 7 });
    expect(noteAction(noteEntry, '  \n ')).toEqual({ kind: 'delete', entryId: 7 });
  });

  it('does nothing when the note is unchanged (trimming aside)', () => {
    expect(noteAction(noteEntry, '  swim 45min  ')).toEqual({ kind: 'none' });
    expect(noteAction(workoutEntry, '')).toEqual({ kind: 'none' });
  });

  it('names a legacy row whose workout_name was stored empty', () => {
    const legacy = entry({ file_name: 'old_base.zwo', workout_name: '', note: null });
    expect(noteAction(legacy, 'easy gearing')).toEqual({
      kind: 'update',
      entryId: 7,
      content: { file_name: 'old_base.zwo', workout_name: 'Old Base', note: 'easy gearing' },
    });
  });

  it('trims the note it sends, mirroring the Rust normalize_entry', () => {
    expect(noteAction(null, '  swim 45min \n')).toEqual({
      kind: 'create',
      content: { file_name: null, workout_name: null, note: 'swim 45min' },
    });
  });
});

describe('assignAction', () => {
  const picked = { file_name: 'vo2.zwo', workout_name: 'VO2 max' };

  it('creates an entry on a blank day, carrying the note typed alongside it', () => {
    expect(assignAction(null, picked, ' ride easy ')).toEqual({
      kind: 'create',
      content: { file_name: 'vo2.zwo', workout_name: 'VO2 max', note: 'ride easy' },
    });
  });

  it('creates a workout-only entry when no note was typed', () => {
    expect(assignAction(null, picked, '')).toEqual({
      kind: 'create',
      content: { file_name: 'vo2.zwo', workout_name: 'VO2 max', note: null },
    });
  });

  it('replaces the workout of an existing entry and keeps the note being edited', () => {
    expect(assignAction(workoutEntry, picked, 'easy gearing')).toEqual({
      kind: 'update',
      entryId: 7,
      content: { file_name: 'vo2.zwo', workout_name: 'VO2 max', note: 'easy gearing' },
    });
  });

  it('attaches the workout to a note-only entry instead of stacking a second one', () => {
    expect(assignAction(noteEntry, picked, 'swim 45min')).toEqual({
      kind: 'update',
      entryId: 7,
      content: { file_name: 'vo2.zwo', workout_name: 'VO2 max', note: 'swim 45min' },
    });
  });

  // Naming itself (verbatim/trim/fallback) is entryWorkoutName's own contract, tested below;
  // this only pins that assignAction wires the picked workout through it.
  it('falls back to the file stem when the .zwo carries no <name>', () => {
    expect(assignAction(null, { file_name: 'run_3.zwo', workout_name: null }, '')).toEqual({
      kind: 'create',
      content: { file_name: 'run_3.zwo', workout_name: 'Run 3', note: null },
    });
  });
});

describe('removeWorkoutAction', () => {
  it('deletes an entry that holds nothing but the workout', () => {
    expect(removeWorkoutAction(workoutEntry, '')).toEqual({ kind: 'delete', entryId: 7 });
  });

  it('keeps the day as a note when the entry also carries one', () => {
    const both = entry({ file_name: 'base.zwo', workout_name: 'Base', note: 'easy gearing' });
    expect(removeWorkoutAction(both, 'easy gearing')).toEqual({
      kind: 'update',
      entryId: 7,
      content: { file_name: null, workout_name: null, note: 'easy gearing' },
    });
  });

  it('keeps the note being edited rather than the one last saved', () => {
    const both = entry({ file_name: 'base.zwo', workout_name: 'Base', note: 'easy gearing' });
    expect(removeWorkoutAction(both, ' rollers instead ')).toEqual({
      kind: 'update',
      entryId: 7,
      content: { file_name: null, workout_name: null, note: 'rollers instead' },
    });
  });

  it('deletes the entry when the note being edited was cleared too', () => {
    expect(removeWorkoutAction(noteEntry, '   ')).toEqual({ kind: 'delete', entryId: 7 });
  });
});

describe('entryWorkoutName', () => {
  it('leaves a note-only entry nameless', () => {
    expect(entryWorkoutName(null, null)).toBe(null);
    expect(entryWorkoutName(null, '')).toBe(null);
  });

  it('keeps the authored name, trimmed', () => {
    expect(entryWorkoutName('ss.zwo', ' sweet spot 2x20 ')).toBe('sweet spot 2x20');
  });

  it('falls back to the file stem when there is no usable name', () => {
    expect(entryWorkoutName('run_3.zwo', null)).toBe('Run 3');
    expect(entryWorkoutName('run_3.zwo', '  ')).toBe('Run 3');
  });
});
