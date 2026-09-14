import { describe, it, expect } from 'vitest';
import type { TrainingPlan } from '$lib/bindings';
import { currentPlan } from './plan-current';
import { planEndDate } from './plan-date';

function plan(overrides: Partial<TrainingPlan> = {}): TrainingPlan {
  return {
    id: 1,
    name: 'Base 1',
    start_date: '2026-09-07',
    weeks: 4,
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
    const p = plan({ start_date: '2026-09-07', weeks: 4 });
    expect(currentPlan([p], '2026-09-14')).toBe(p);
  });

  it('returns null for an otherwise-matching plan that is archived', () => {
    const p = plan({ start_date: '2026-09-07', weeks: 4, archived_at: '2026-09-10T00:00:00Z' });
    expect(currentPlan([p], '2026-09-14')).toBeNull();
  });

  it('returns null for a plan entirely in the future', () => {
    const p = plan({ start_date: '2026-10-05', weeks: 4 });
    expect(currentPlan([p], '2026-09-14')).toBeNull();
  });

  it('returns null for a plan entirely in the past', () => {
    const p = plan({ start_date: '2026-01-05', weeks: 4 });
    expect(currentPlan([p], '2026-09-14')).toBeNull();
  });

  it('matches when today equals the start date exactly', () => {
    const p = plan({ start_date: '2026-09-07', weeks: 4 });
    expect(currentPlan([p], '2026-09-07')).toBe(p);
  });

  it('matches when today equals the inclusive end date exactly', () => {
    const p = plan({ start_date: '2026-09-07', weeks: 4 });
    expect(currentPlan([p], planEndDate(p.start_date, p.weeks))).toBe(p);
  });
});
