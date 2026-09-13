<script lang="ts">
  import type { PlanDay } from '$lib/bindings';
  import { dayOfMonth } from '$lib/plan-date';

  let { day }: { day: PlanDay } = $props();
</script>

<div class="cell" class:today={day.marker === 'Today'} class:past={day.marker === 'Past'}>
  <span class="date">{dayOfMonth(day.date)}</span>
</div>

<style>
  .cell {
    display: flex;
    align-items: flex-start;
    justify-content: flex-end;
    aspect-ratio: 1;
    padding: 0.4rem;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--surface);
  }

  .date {
    font-size: 0.8rem;
    font-weight: 500;
    color: var(--text);
  }

  .today {
    background: var(--accent);
    border-color: var(--accent);
  }
  .today .date { color: #fff; }

  .past { opacity: 0.55; }
</style>
