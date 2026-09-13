<script lang="ts">
  import type { PlanWeek } from '$lib/bindings';
  import PlanDayCell from './PlanDayCell.svelte';

  let { week }: { week: PlanWeek } = $props();
</script>

<div class="row">
  <span class="week-label">Week {week.number}</span>
  <div class="days">
    {#each week.days as day (day.date)}
      <PlanDayCell {day} />
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
