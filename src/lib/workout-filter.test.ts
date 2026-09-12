import { describe, it, expect } from 'vitest';
import { uniqueSortedTags, matchesSelectedTags } from './workout-filter';

describe('uniqueSortedTags', () => {
  it('collects and sorts unique tags across workouts', () => {
    const workouts = [{ tags: ['climb', 'z2'] }, { tags: ['z2', 'sprint'] }, { tags: [] }];
    expect(uniqueSortedTags(workouts)).toEqual(['climb', 'sprint', 'z2']);
  });

  it('returns an empty array when no workout has tags', () => {
    expect(uniqueSortedTags([{ tags: [] }])).toEqual([]);
  });
});

describe('matchesSelectedTags', () => {
  it('matches everything when no tag is selected', () => {
    expect(matchesSelectedTags([], new Set())).toBe(true);
    expect(matchesSelectedTags(['z2'], new Set())).toBe(true);
  });

  it('matches a workout that has at least one selected tag (OR)', () => {
    expect(matchesSelectedTags(['z2', 'climb'], new Set(['climb']))).toBe(true);
  });

  it('rejects a workout with none of the selected tags', () => {
    expect(matchesSelectedTags(['z2'], new Set(['sprint']))).toBe(false);
  });
});
