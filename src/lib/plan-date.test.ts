import { describe, it, expect } from 'vitest';
import type { TrainingPlan } from '$lib/bindings';
import { currentPlan, dayOfMonth, formatPlanRange, mondayOf, todayIso, todayMonday } from './plan-date';

describe('mondayOf', () => {
  it('leaves a Monday untouched', () => {
    expect(mondayOf('2026-09-14')).toBe('2026-09-14');
  });

  it('walks a Sunday back six days (ISO week starts on Monday)', () => {
    expect(mondayOf('2026-09-20')).toBe('2026-09-14');
  });

  it('crosses a month boundary', () => {
    expect(mondayOf('2026-10-01')).toBe('2026-09-28');
  });

  it('crosses a year boundary', () => {
    expect(mondayOf('2027-01-01')).toBe('2026-12-28');
  });

  it('returns an unparseable date unchanged instead of throwing', () => {
    expect(mondayOf('')).toBe('');
    expect(mondayOf('not-a-date')).toBe('not-a-date');
  });
});

describe('formatPlanRange', () => {
  it('prints the month once when start and end share it', () => {
    expect(formatPlanRange('2026-09-07', '2026-09-14')).toBe('7 – 13 Sep 2026');
  });

  it('prints both months but one year within the same year', () => {
    expect(formatPlanRange('2026-09-14', '2026-10-12')).toBe('14 Sep – 11 Oct 2026');
  });

  it('prints both years when the plan straddles two of them', () => {
    expect(formatPlanRange('2026-12-28', '2027-01-11')).toBe('28 Dec 2026 – 10 Jan 2027');
  });

  it('returns an unparseable start date unchanged instead of throwing', () => {
    expect(formatPlanRange('', '2026-09-14')).toBe('');
    expect(formatPlanRange('not-a-date', '2026-09-14')).toBe('not-a-date');
  });
});

describe('dayOfMonth', () => {
  it('extracts the UTC day-of-month number for a day cell', () => {
    expect(dayOfMonth('2026-09-01')).toBe(1);
    expect(dayOfMonth('2026-12-31')).toBe(31);
  });
});

describe('todayIso', () => {
  it('reads the rider local calendar date, not UTC', () => {
    expect(todayIso(new Date(2026, 8, 14, 0, 30, 0))).toBe('2026-09-14');
  });
});

describe('todayMonday', () => {
  it('snaps the rider local date back to its Monday', () => {
    expect(todayMonday(new Date(2026, 8, 20, 12, 0, 0))).toBe('2026-09-14');
  });

  // Just after local midnight the UTC date is still the day before east of Greenwich.
  it('reads the local calendar date just after midnight', () => {
    expect(todayMonday(new Date(2026, 8, 14, 0, 30, 0))).toBe('2026-09-14');
  });

  // Late in the local evening the UTC date is already the next day west of Greenwich.
  it('reads the local calendar date late in the evening', () => {
    expect(todayMonday(new Date(2026, 8, 20, 23, 30, 0))).toBe('2026-09-14');
  });
});

function plan(overrides: Partial<TrainingPlan> = {}): TrainingPlan {
  return {
    id: 1,
    name: 'Base 1',
    start_date: '2026-09-07',
    weeks: 4,
    end_date: '2026-10-05',
    created_at: '2026-09-01T00:00:00Z',
    archived_at: null,
    ...overrides,
  };
}

describe('currentPlan', () => {
  it('returns null for an empty plan list', () => {
    expect(currentPlan([], '2026-09-14')).toBeNull();
  });

  it('returns the active plan whose range contains today', () => {
    const p = plan({ start_date: '2026-09-07', end_date: '2026-10-05' });
    expect(currentPlan([p], '2026-09-14')).toBe(p);
  });

  it('returns null for an otherwise-matching plan that is archived', () => {
    const p = plan({ start_date: '2026-09-07', end_date: '2026-10-05', archived_at: '2026-09-10T00:00:00Z' });
    expect(currentPlan([p], '2026-09-14')).toBeNull();
  });

  it('returns null for a plan entirely in the future', () => {
    const p = plan({ start_date: '2026-10-05', end_date: '2026-11-02' });
    expect(currentPlan([p], '2026-09-14')).toBeNull();
  });

  it('returns null for a plan entirely in the past', () => {
    const p = plan({ start_date: '2026-01-05', end_date: '2026-02-02' });
    expect(currentPlan([p], '2026-09-14')).toBeNull();
  });

  it('matches when today equals the start date exactly', () => {
    const p = plan({ start_date: '2026-09-07', end_date: '2026-10-05' });
    expect(currentPlan([p], '2026-09-07')).toBe(p);
  });

  it('excludes today when it lands exactly on the exclusive end date', () => {
    const p = plan({ start_date: '2026-09-07', end_date: '2026-10-05' });
    expect(currentPlan([p], '2026-10-05')).toBeNull();
  });

  it('matches the day before the exclusive end date', () => {
    const p = plan({ start_date: '2026-09-07', end_date: '2026-10-05' });
    expect(currentPlan([p], '2026-10-04')).toBe(p);
  });
});
