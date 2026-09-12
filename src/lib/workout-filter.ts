/** Pure helpers for the /workouts tag filter, kept separate so they stay unit-testable. */

export function uniqueSortedTags(workouts: { tags: string[] }[]): string[] {
  return [...new Set(workouts.flatMap(w => w.tags))].sort();
}

// OR semantics: a workout matches if it carries any of the selected tags.
export function matchesSelectedTags(tags: string[], selected: ReadonlySet<string>): boolean {
  return selected.size === 0 || tags.some(t => selected.has(t));
}
