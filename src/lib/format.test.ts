import { describe, it, expect } from 'vitest';
import { formatRelativeDate } from './format';

describe('formatRelativeDate', () => {
  const now = Date.parse('2024-06-10T00:00:00Z');

  it('reports never used for a null date', () => {
    expect(formatRelativeDate(null, now)).toBe('Never used');
  });

  it('reports today for a same-day timestamp', () => {
    expect(formatRelativeDate('2024-06-10T00:00:00Z', now)).toBe('Today');
  });

  it('reports singular yesterday for 1 day ago', () => {
    expect(formatRelativeDate('2024-06-09T00:00:00Z', now)).toBe('Yesterday');
  });

  it('reports N days ago', () => {
    expect(formatRelativeDate('2024-06-05T00:00:00Z', now)).toBe('5 days ago');
  });

  it('falls back to never used for an unparsable date', () => {
    expect(formatRelativeDate('not-a-date', now)).toBe('Never used');
  });
});
