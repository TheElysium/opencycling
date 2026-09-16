import { describe, it, expect } from 'vitest';
import type { ParsedWorkout, PlanDay, PlanEntryView, PlanWeek } from '$lib/bindings';
import { resolvePlanStart, todayOf } from './plan-start';
import { indexByFileName } from './plan-load';
import { FTP_TEST_REFERENCE_W } from './ftp';

function workout(overrides: Partial<ParsedWorkout> = {}): ParsedWorkout {
  return {
    author: null,
    name: 'Base',
    description: null,
    sport_type: 'Bike',
    workout_blocks: [],
    is_ftp_test: false,
    tags: [],
    file_name: 'base.zwo',
    ...overrides,
  };
}

function entry(overrides: Partial<PlanEntryView> = {}): PlanEntryView {
  return {
    entry_id: 1,
    file_name: null,
    workout_name: null,
    note: null,
    session_id: null,
    missing: false,
    ...overrides,
  };
}

function day(overrides: Partial<PlanDay> = {}): PlanDay {
  return { date: '2026-09-14', marker: 'Future', entries: [], ...overrides };
}

function week(overrides: Partial<PlanWeek> = {}): PlanWeek {
  return { number: 1, days: [], ...overrides };
}

describe('resolvePlanStart', () => {
  it('fails for a note-only entry with no file to start', () => {
    const result = resolvePlanStart(entry({ file_name: null }), indexByFileName([]), 200);
    expect(result).toEqual({ ok: false, error: 'This entry has no workout to start.' });
  });

  it('fails with the rescan message when the file is not in the library index', () => {
    const result = resolvePlanStart(entry({ file_name: 'gone.zwo' }), indexByFileName([]), 200);
    expect(result).toEqual({
      ok: false,
      error: 'Workout not found in the library, rescan the library from Settings.',
    });
  });

  it('resolves the workout and the rider FTP when the file is indexed', () => {
    const w = workout({ file_name: 'base.zwo' });
    const result = resolvePlanStart(entry({ file_name: 'base.zwo' }), indexByFileName([w]), 200);
    expect(result).toEqual({ ok: true, workout: w, ftpW: 200 });
  });

  it('resolves an FTP test at the reference FTP, not the rider FTP', () => {
    const w = workout({ file_name: 'test.zwo', is_ftp_test: true });
    const result = resolvePlanStart(entry({ file_name: 'test.zwo' }), indexByFileName([w]), 200);
    expect(result).toEqual({ ok: true, workout: w, ftpW: FTP_TEST_REFERENCE_W });
  });
});

describe('todayOf', () => {
  it('returns null for an empty week list', () => {
    expect(todayOf([])).toBeNull();
  });

  it('returns null when no day across any week is marked Today', () => {
    const weeks = [week({ days: [day({ marker: 'Past' }), day({ marker: 'Future' })] })];
    expect(todayOf(weeks)).toBeNull();
  });

  it('returns the week and day carrying the Today marker', () => {
    const todayDay = day({ marker: 'Today', date: '2026-09-14' });
    const w = week({ number: 2, days: [day({ marker: 'Past' }), todayDay] });
    expect(todayOf([week({ number: 1, days: [day({ marker: 'Past' })] }), w])).toEqual({
      week: w,
      day: todayDay,
    });
  });
});
