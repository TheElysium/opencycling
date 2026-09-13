<script lang="ts">
  import type { PlanDay, PlanWeek } from '$lib/bindings';
  import PlanDayCell from './PlanDayCell.svelte';

  type Props = {
    week: PlanWeek;
    readonly?: boolean;
    onopen?: (date: string, entries: PlanDay['entries']) => void;
  };

  let { week, readonly = false, onopen }: Props = $props();
</script>

<div class="row">
  <span class="week-label">Week {week.number}</span>
  <div class="days">
    {#each week.days as day (day.date)}
      <PlanDayCell {day} {readonly} {onopen} />
    {/each}
  </div>
</div>

<style>
  .row {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
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
