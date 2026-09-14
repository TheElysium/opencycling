<script lang="ts">
  import type { PlanDay, PlanEntryView, PlanWeek } from '$lib/bindings';
  import type { WeekLoad } from '$lib/plan-load';
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
  };

  let { week, readonly = false, onopen, onstart, load, maxTss = 0 }: Props = $props();
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
      <PlanDayCell {day} {readonly} {onopen} {onstart} />
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
    grid-template-columns: repeat(7, 1fr);
    gap: 0.4rem;
  }
</style>
