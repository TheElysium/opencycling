<script lang="ts">
  import { commands, type PlanEntryView, type PlanWeek, type TrainingPlan } from '$lib/bindings';
  import { toMessage } from '$lib/format';
  import { library } from '$lib/library.svelte';
  import { entryIntensities, maxWeekTss, weekLoad } from '$lib/plan-load';
  import { todayOf } from '$lib/plan-start';
  import PlanWeekRow from './PlanWeekRow.svelte';

  type Props = {
    plan: TrainingPlan | null;
    loading: boolean;
    onopen: () => void;
    onstart: (entry: PlanEntryView) => void;
  };

  let { plan, loading, onopen, onstart }: Props = $props();

  let weeks = $state<PlanWeek[]>([]);
  let currentWeek = $state<PlanWeek | null>(null);
  let fetchedForPlanId = $state<number | null>(null);
  let error = $state<string | null>(null);

  // Distinguishes "still fetching" from "confirmed no current plan" so the empty state never flashes.
  let pending = $derived(plan !== null && fetchedForPlanId !== plan.id);
  let currentWeekLoad = $derived(currentWeek ? weekLoad(currentWeek, library.index, library.ftp) : undefined);
  // Relative to the whole plan (matches /plans/[id]'s planMaxTss), not just this one week.
  let maxTss = $derived(maxWeekTss(weeks.map((w) => weekLoad(w, library.index, library.ftp))));
  let intensities = $derived(entryIntensities(weeks, library.index, library.ftp));

  $effect(() => {
    const p = plan;
    if (!p || fetchedForPlanId === p.id) return;
    error = null;
    commands
      .getPlanWeeks(p.id)
      .then((w) => {
        weeks = w;
        currentWeek = todayOf(w)?.week ?? null;
      })
      .catch((e) => {
        weeks = [];
        currentWeek = null;
        error = toMessage(e);
      })
      .finally(() => {
        fetchedForPlanId = p.id;
      });
  });
</script>

<section class="card current-week-card">
  <div class="current-week-head">
    <span class="current-week-label">Current week</span>
    {#if plan}
      <a class="plan-name" href="/plans/{plan.id}">{plan.name}</a>
    {/if}
  </div>
  {#if loading || pending}
    <p class="muted">Loading…</p>
  {:else if error}
    <p class="error-box">{error}</p>
  {:else if !plan || !currentWeek}
    <p class="muted">No active plan this week.</p>
  {:else}
    <PlanWeekRow week={currentWeek} load={currentWeekLoad} {maxTss} {intensities} {onopen} {onstart} />
  {/if}
</section>

<style>
  .current-week-card { margin-bottom: 1.25rem; }

  .current-week-head {
    display: flex;
    align-items: baseline;
    gap: 0.6rem;
    margin-bottom: 0.75rem;
  }

  .current-week-label {
    font-size: 0.78rem;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--muted);
  }

  .plan-name {
    font-weight: 600;
    font-size: 0.95rem;
    color: inherit;
    text-decoration: none;
  }
  .plan-name:hover,
  .plan-name:focus-visible { color: var(--accent); }
</style>
