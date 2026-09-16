<script lang="ts">
  import type { PlanDay, PlanEntryView, PlanWeek } from '$lib/bindings';
  import type { WeekLoad } from '$lib/plan-load';
  import type { WorkoutType } from '$lib/metrics';
  import PlanDayCell from './PlanDayCell.svelte';
  import WeekLoadSummary from './WeekLoadSummary.svelte';

  type Props = {
    week: PlanWeek;
    readonly?: boolean;
    onopen?: (date: string, entries: PlanDay['entries']) => void;
    onstart?: (entry: PlanEntryView) => void;
    /** Absent when the library/settings could not be loaded: the grid still renders. */
    load?: WeekLoad;
    maxTss?: number;
    intensities?: Map<number, WorkoutType | null>;
  };

  let { week, readonly = false, onopen, onstart, load, maxTss = 0, intensities }: Props = $props();
</script>

<div class="row">
  <div class="label-line">
    <span class="week-label">Week {week.number}</span>
    {#if load}
      <WeekLoadSummary {load} {maxTss} />
    {/if}
  </div>
  <div class="days">
    {#each week.days as day (day.date)}
      <PlanDayCell {day} {readonly} {onopen} {onstart} {intensities} />
    {/each}
  </div>
</div>

<style>
  .row {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }

  .label-line {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
  }

  .week-label {
    font-size: 0.8rem;
    font-weight: 600;
    color: var(--muted);
  }

  .days {
    display: grid;
    /* minmax(0, 1fr), not 1fr: caps each column's min size at 0 instead of
       its content, in case a child ever reintroduces a nowrap min-width. */
    grid-template-columns: repeat(7, minmax(0, 1fr));
    gap: 0.4rem;
  }
</style>
