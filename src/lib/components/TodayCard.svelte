<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { Play, CircleCheck, CalendarDays } from '@lucide/svelte';
  import {
    commands,
    type FlatBlock,
    type ParsedWorkout,
    type PlanDay,
    type PlanEntryView,
    type TrainingPlan,
  } from '$lib/bindings';
  import { ble } from '$lib/ble.svelte';
  import { library } from '$lib/library.svelte';
  import { workoutFtp } from '$lib/ftp';
  import { currentPlan, todayIso } from '$lib/plan-date';
  import { todayOf } from '$lib/plan-start';
  import WorkoutCard from './WorkoutCard.svelte';

  let planLoading = $state(true);
  let plan = $state<TrainingPlan | null>(null);
  let weekNumber = $state(0);
  let day = $state<PlanDay | null>(null);
  let error = $state<string | null>(null);
  let starting = $state(false);

  // `flats` shares the index of `workouts` (list_workouts_cmd contract).
  let flatIndex = $derived(
    new Map(
      library.workouts.flatMap((w, i) =>
        w.file_name === null ? [] : [[w.file_name, library.flats[i] ?? []] as const],
      ),
    ),
  );

  function flatOf(workout: ParsedWorkout): FlatBlock[] {
    return workout.file_name === null ? [] : (flatIndex.get(workout.file_name) ?? []);
  }

  // A load failure falls back to the "no plan" state rather than an error box:
  // the Home page must stay usable for connecting devices.
  async function loadPlan() {
    try {
      const plans = await commands.listPlans(true);
      const p = currentPlan(plans, todayIso());
      if (!p) return;
      const weeks = await commands.getPlanWeeks(p.id);
      const found = todayOf(weeks);
      if (!found) return;
      plan = p;
      weekNumber = found.week.number;
      day = found.day;
    } catch {
      plan = null;
      day = null;
    } finally {
      planLoading = false;
    }
  }

  onMount(() => {
    // Own fetches, independent of the page's BLE scan: neither awaits the other.
    void loadPlan();
    void library.load();
  });

  // Waits for the library too, so entries do not flash from plain rows to cards.
  let loading = $derived(planLoading || library.loading);
  let trainerReady = $derived(ble.trainerStatus === 'connected');

  function libraryWorkout(entry: PlanEntryView): ParsedWorkout | null {
    if (!entry.file_name || entry.missing) return null;
    return library.index.get(entry.file_name) ?? null;
  }

  let hasWorkout = $derived(day?.entries.some((e) => libraryWorkout(e) !== null) ?? false);

  function openPlan() {
    if (plan) goto(`/plans/${plan.id}`);
  }

  async function start(entry: PlanEntryView) {
    if (starting) return;
    error = null;
    starting = true;
    try {
      const err = await library.start(entry);
      if (err) error = err;
    } finally {
      starting = false;
    }
  }
</script>

{#if loading}
  <div class="card">
    <p class="muted">Loading today's plan...</p>
  </div>
{:else if !plan || !day}
  <div class="card empty">
    <CalendarDays size={22} strokeWidth={2} class="empty-icon" />
    <div class="empty-text">
      <span class="empty-title">No plan for today</span>
      <span class="muted">Create or activate a training plan to see today's workout here.</span>
    </div>
    <button class="btn-secondary" onclick={() => goto('/plans')}>Go to Plans</button>
  </div>
{:else}
  <div class="today">
    <div class="today-head">
      <div class="today-title">
        <span class="today-plan">{plan.name}</span>
        <span class="muted">Week {weekNumber}</span>
      </div>
      <button class="btn-ghost" onclick={openPlan}>Open plan</button>
    </div>

    {#if day.entries.length === 0}
      <div class="card">
        <p class="muted">Rest day, nothing planned.</p>
      </div>
    {:else}
      <ul class="entries">
        {#each day.entries as entry (entry.entry_id)}
          {@const workout = libraryWorkout(entry)}
          <li class="entry">
            {#if workout}
              <WorkoutCard
                {workout}
                flat={flatOf(workout)}
                ftpWatts={workoutFtp(workout, library.ftp)}
                chartHeight={110}
              >
                {#snippet actions()}
                  {#if entry.session_id != null}
                    <span class="done"><CircleCheck size={16} strokeWidth={2.5} /> Done</span>
                  {/if}
                  {#if trainerReady}
                    <button class="btn-primary start-btn" disabled={starting} onclick={() => start(entry)}>
                      <Play size={16} strokeWidth={2.5} /> Start
                    </button>
                  {/if}
                {/snippet}
              </WorkoutCard>
            {:else if entry.file_name}
              <div class="card fallback">
                <span class="fallback-name" class:missing={entry.missing}>
                  {entry.workout_name || entry.file_name}
                </span>
                <span class="muted">
                  {entry.missing ? 'Not found in the library.' : 'Not in the library, rescan it from Settings.'}
                </span>
              </div>
            {/if}
            {#if entry.note}
              <p class="entry-note">{entry.note}</p>
            {/if}
          </li>
        {/each}
      </ul>
      {#if hasWorkout && !trainerReady}
        <p class="muted connect-hint">Connect the trainer below to start.</p>
      {/if}
    {/if}

    {#if error}
      <p class="error-box">{error}</p>
    {/if}
  </div>
{/if}

<style>
  /* Global .muted has no margin reset: this component's usages need one. */
  .muted { margin: 0; }

  .today-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 0.75rem;
    margin-bottom: 0.75rem;
  }

  .today-title {
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
  }

  .today-plan {
    font-size: 1.05rem;
    font-weight: 700;
    color: var(--text);
  }

  .entries {
    display: flex;
    flex-direction: column;
    gap: 0.9rem;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .entry-note {
    margin: 0.4rem 0 0 0.25rem;
    font-size: 0.9rem;
    font-style: italic;
    color: var(--muted);
  }

  .fallback {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
  }

  .fallback-name {
    font-weight: 600;
    color: var(--text);
  }

  .fallback-name.missing {
    color: var(--danger);
    text-decoration: line-through;
  }

  .done {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    font-size: 0.85rem;
    font-weight: 600;
    color: var(--success);
  }

  .start-btn {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
  }

  .connect-hint { margin-top: 0.75rem; font-size: 0.85rem; }

  .empty {
    display: flex;
    align-items: center;
    gap: 0.9rem;
  }

  .empty-text {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    flex: 1;
  }

  .empty-title {
    font-size: 1rem;
    font-weight: 600;
    color: var(--text);
  }

  :global(.empty-icon) {
    color: var(--muted);
    flex-shrink: 0;
  }
</style>
