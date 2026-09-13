import { describe, it, expect } from 'vitest';
import { mondayOf, planEndDate, formatPlanRange, todayMonday, dayOfMonth } from './plan-date';

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

describe('planEndDate', () => {
  it('ends a one-week plan on the following Sunday', () => {
    expect(planEndDate('2026-09-14', 1)).toBe('2026-09-20');
  });

  it('ends a four-week plan one day before the exclusive Rust end', () => {
    expect(planEndDate('2026-09-14', 4)).toBe('2026-10-11');
  });

  it('crosses a year boundary', () => {
    expect(planEndDate('2026-12-28', 2)).toBe('2027-01-10');
  });

  it('returns an unparseable start date unchanged instead of throwing', () => {
    expect(planEndDate('', 4)).toBe('');
    expect(planEndDate('not-a-date', 4)).toBe('not-a-date');
  });
});

describe('formatPlanRange', () => {
  it('prints the month once when start and end share it', () => {
    expect(formatPlanRange('2026-09-07', 1)).toBe('7 – 13 Sep 2026');
  });

  it('prints both months but one year within the same year', () => {
    expect(formatPlanRange('2026-09-14', 4)).toBe('14 Sep – 11 Oct 2026');
  });

  it('prints both years when the plan straddles two of them', () => {
    expect(formatPlanRange('2026-12-28', 2)).toBe('28 Dec 2026 – 10 Jan 2027');
  });

  it('returns an unparseable start date unchanged instead of throwing', () => {
    expect(formatPlanRange('', 4)).toBe('');
    expect(formatPlanRange('not-a-date', 4)).toBe('not-a-date');
  });
});

describe('dayOfMonth', () => {
  it('extracts the UTC day-of-month number for a day cell', () => {
    expect(dayOfMonth('2026-09-01')).toBe(1);
    expect(dayOfMonth('2026-12-31')).toBe(31);
  });

  it('returns 0 for an unparseable date instead of throwing', () => {
    expect(dayOfMonth('')).toBe(0);
    expect(dayOfMonth('not-a-date')).toBe(0);
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
