<script lang="ts">
  import { onMount } from 'svelte';
  import { page } from '$app/stores';
  import { ArrowLeft } from '@lucide/svelte';
  import { commands, type PlanWeek, type TrainingPlan } from '$lib/bindings';
  import { toMessage } from '$lib/format';
  import { formatPlanRange } from '$lib/plan-date';
  import PlanWeekRow from '$lib/components/PlanWeekRow.svelte';

  const WEEKDAYS = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'];

  let plan  = $state<TrainingPlan | null>(null);
  let weeks = $state<PlanWeek[]>([]);
  let loading = $state(true);
  let error   = $state<string | null>(null);

  let id = $derived(parseInt($page.params.id ?? '0', 10));

  onMount(async () => {
    try {
      const [p, w] = await Promise.all([commands.getPlan(id), commands.getPlanWeeks(id)]);
      plan = p;
      weeks = w;
    } catch (e) {
      error = toMessage(e);
    } finally {
      loading = false;
    }
  });
</script>

<div class="page-wide">
  <a class="back" href="/plans">
    <ArrowLeft size={18} />
    Plans
  </a>

  {#if loading}
    <p class="muted">Loading…</p>
  {:else if error}
    <p class="error-box">{error}</p>
  {:else if plan}
    <header>
      <h1>{plan.name}</h1>
      <p class="meta">{formatPlanRange(plan.start_date, plan.weeks)}</p>
    </header>

    <div class="grid">
      <div class="weekday-header">
        {#each WEEKDAYS as wd (wd)}
          <span class="weekday-label">{wd}</span>
        {/each}
      </div>
      {#each weeks as week (week.number)}
        <PlanWeekRow {week} />
      {/each}
    </div>
  {/if}
</div>

<style>
  .muted { color: var(--muted); }

  .back {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    color: var(--muted);
    text-decoration: none;
    font-size: 0.85rem;
    margin-bottom: 0.75rem;
  }
  .back:hover { color: var(--text); }

  header { margin-bottom: 1.25rem; }

  h1 {
    font-size: 1.4rem;
    font-weight: 600;
    margin: 0 0 0.25rem;
  }

  .meta {
    margin: 0;
    font-size: 0.9rem;
    color: var(--muted);
  }

  .grid {
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  .weekday-header {
    display: grid;
    grid-template-columns: repeat(7, 1fr);
    gap: 0.4rem;
    padding-bottom: 0.35rem;
  }

  .weekday-label {
    text-align: center;
    font-size: 0.75rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--muted);
  }
</style>
