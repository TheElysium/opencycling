import { describe, it, expect } from 'vitest';
import type { ParsedWorkout, PlanDay, PlanEntryView, PlanWeek, WorkoutBlock } from '$lib/bindings';
import {
  entryIntensities,
  entryIntensity,
  indexByFileName,
  loadBarRatio,
  maxWeekTss,
  weekLoad,
} from './plan-load';

function steadyBlock(duration_s: number, power_pct: number): WorkoutBlock {
  return { SteadyState: { duration_s, power_pct, cadence_rpm: null, label: null } };
}

function workout(overrides: Partial<ParsedWorkout> = {}): ParsedWorkout {
  return {
    author: null,
    name: 'Base',
    description: null,
    sport_type: 'Bike',
    workout_blocks: [steadyBlock(3600, 0.7)],
    is_ftp_test: false,
    tags: [],
    file_name: 'base.zwo',
    ...overrides,
  };
}

function entry(overrides: Partial<PlanEntryView> = {}): PlanEntryView {
  return {
    entry_id: 1,
    position: 0,
    file_name: null,
    workout_name: null,
    note: null,
    session_id: null,
    missing: false,
    ...overrides,
  };
}

function day(entries: PlanEntryView[]): PlanDay {
  return { date: '2026-09-14', marker: 'Today', entries };
}

function week(days: PlanDay[]): PlanWeek {
  return { number: 1, days };
}

describe('indexByFileName', () => {
  it('indexes workouts by file name, skipping entries without one', () => {
    const w1 = workout({ file_name: 'base.zwo' });
    const w2 = workout({ file_name: null });
    const index = indexByFileName([w1, w2]);
    expect(index.get('base.zwo')).toBe(w1);
    expect(index.size).toBe(1);
  });
});

describe('weekLoad', () => {
  const base = workout({ file_name: 'base.zwo', workout_blocks: [steadyBlock(3600, 0.7)] });
  const vo2 = workout({ file_name: 'vo2.zwo', workout_blocks: [steadyBlock(1800, 1.1)] });
  const ftpTest = workout({
    file_name: 'test.zwo',
    is_ftp_test: true,
    workout_blocks: [steadyBlock(1200, 100)],
  });

  it('sums duration and TSS across several planned days', () => {
    const index = indexByFileName([base, vo2]);
    const w = week([
      day([entry({ file_name: 'base.zwo' })]),
      day([entry({ file_name: 'vo2.zwo' })]),
    ]);
    const load = weekLoad(w, index, 200);
    expect(load.durationS).toBe(3600 + 1800);
    expect(load.workouts).toBe(2);
    expect(load.missing).toBe(0);
    expect(load.tss).toBeGreaterThan(0);
  });

  it('counts the same workout planned twice in the same week twice', () => {
    const index = indexByFileName([base]);
    const w = week([
      day([entry({ file_name: 'base.zwo', entry_id: 1 })]),
      day([entry({ file_name: 'base.zwo', entry_id: 2 })]),
    ]);
    const load = weekLoad(w, index, 200);
    expect(load.durationS).toBe(3600 * 2);
    expect(load.workouts).toBe(2);
  });

  it('gives a note-only entry no contribution and does not count it missing', () => {
    const index = indexByFileName([base]);
    const w = week([day([entry({ note: 'swim 45min' })])]);
    const load = weekLoad(w, index, 200);
    expect(load.durationS).toBe(0);
    expect(load.tss).toBe(0);
    expect(load.workouts).toBe(0);
    expect(load.missing).toBe(0);
  });

  it('counts an entry the backend flagged missing and excludes it from the total', () => {
    const index = indexByFileName([base]);
    const w = week([
      day([entry({ file_name: 'base.zwo' })]),
      day([entry({ file_name: 'gone.zwo', missing: true })]),
    ]);
    const load = weekLoad(w, index, 200);
    expect(load.durationS).toBe(3600);
    expect(load.workouts).toBe(1);
    expect(load.missing).toBe(1);
  });

  it('skips an entry absent from the index but not flagged missing, without counting it', () => {
    const w = week([day([entry({ file_name: 'base.zwo' })])]);
    const load = weekLoad(w, indexByFileName([]), 200);
    expect(load.durationS).toBe(0);
    expect(load.workouts).toBe(0);
    expect(load.missing).toBe(0);
  });

  it('trusts the backend flag over the index for a file the library still holds', () => {
    const index = indexByFileName([base]);
    const w = week([day([entry({ file_name: 'base.zwo', missing: true })])]);
    const load = weekLoad(w, index, 200);
    expect(load.durationS).toBe(0);
    expect(load.missing).toBe(1);
  });

  it('adds duration but not TSS for an FTP-test workout', () => {
    const index = indexByFileName([ftpTest]);
    const w = week([day([entry({ file_name: 'test.zwo' })])]);
    const load = weekLoad(w, index, 200);
    expect(load.durationS).toBe(1200);
    expect(load.tss).toBe(0);
    expect(load.workouts).toBe(1);
  });

  it('yields zero TSS but keeps duration when FTP is unset', () => {
    const index = indexByFileName([base]);
    const w = week([day([entry({ file_name: 'base.zwo' })])]);
    const load = weekLoad(w, index, 0);
    expect(load.durationS).toBe(3600);
    expect(load.tss).toBe(0);
  });

  it('returns an all-zero load for an empty week', () => {
    const load = weekLoad(week([day([])]), indexByFileName([]), 200);
    expect(load).toEqual({ durationS: 0, tss: 0, workouts: 0, missing: 0 });
  });
});

describe('entryIntensity', () => {
  const base = workout({ file_name: 'base.zwo', workout_blocks: [steadyBlock(3600, 0.7)] });

  it('classifies a found workout by its computed type', () => {
    const index = indexByFileName([base]);
    expect(entryIntensity(entry({ file_name: 'base.zwo' }), index, 200)).toBe('Endurance');
  });

  it('returns null for a note-only entry', () => {
    const index = indexByFileName([base]);
    expect(entryIntensity(entry({ note: 'swim 45min' }), index, 200)).toBeNull();
  });

  it('returns null for an entry the backend flagged missing, even if the file is indexed', () => {
    const index = indexByFileName([base]);
    expect(entryIntensity(entry({ file_name: 'base.zwo', missing: true }), index, 200)).toBeNull();
  });

  it('returns null for an entry whose file is absent from the index', () => {
    expect(entryIntensity(entry({ file_name: 'base.zwo' }), indexByFileName([]), 200)).toBeNull();
  });
});

describe('entryIntensities', () => {
  const base = workout({ file_name: 'base.zwo', workout_blocks: [steadyBlock(3600, 0.7)] });
  const vo2 = workout({ file_name: 'vo2.zwo', workout_blocks: [steadyBlock(1800, 1.1)] });

  it('maps every entry across all weeks by its own entry_id', () => {
    const index = indexByFileName([base, vo2]);
    const weeks = [
      week([day([entry({ entry_id: 1, file_name: 'base.zwo' })])]),
      week([day([entry({ entry_id: 2, file_name: 'vo2.zwo' })])]),
    ];
    const map = entryIntensities(weeks, index, 200);
    expect(map.get(1)).toBe('Endurance');
    expect(map.get(2)).toBe('VO2max');
    expect(map.size).toBe(2);
  });

  it('maps a note-only or unindexed entry to null rather than omitting it', () => {
    const weeks = [week([day([entry({ entry_id: 1, note: 'swim 45min' })])])];
    const map = entryIntensities(weeks, indexByFileName([]), 200);
    expect(map.get(1)).toBeNull();
  });
});

describe('maxWeekTss', () => {
  it('returns the largest TSS across the given week loads', () => {
    const loads = [
      { durationS: 0, tss: 50, workouts: 1, missing: 0 },
      { durationS: 0, tss: 120, workouts: 1, missing: 0 },
      { durationS: 0, tss: 30, workouts: 1, missing: 0 },
    ];
    expect(maxWeekTss(loads)).toBe(120);
  });

  it('returns 0 for an empty list', () => {
    expect(maxWeekTss([])).toBe(0);
  });
});

describe('loadBarRatio', () => {
  it('is 0 when the max is 0', () => {
    expect(loadBarRatio(50, 0)).toBe(0);
  });

  it('is the tss/max ratio within range', () => {
    expect(loadBarRatio(60, 120)).toBe(0.5);
  });

  it('clamps to 1 above the max', () => {
    expect(loadBarRatio(200, 120)).toBe(1);
  });
});
