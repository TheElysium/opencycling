<script lang="ts">
  import { onMount } from 'svelte';
  import { page } from '$app/stores';
  import { ArrowLeft } from '@lucide/svelte';
  import { confirm } from '@tauri-apps/plugin-dialog';
  import {
    commands,
    type ParsedWorkout,
    type PlanEntryView,
    type PlanWeek,
    type TrainingPlan,
  } from '$lib/bindings';
  import { toMessage } from '$lib/format';
  import { formatPlanRange } from '$lib/plan-date';
  import PlanWeekRow from '$lib/components/PlanWeekRow.svelte';
  import WorkoutPicker from '$lib/components/WorkoutPicker.svelte';

  const WEEKDAYS = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'];

  let plan = $state<TrainingPlan | null>(null);
  let weeks = $state<PlanWeek[]>([]);
  let loading = $state(true);
  let busy = $state(false);
  let error = $state<string | null>(null);

  let pickerOpen = $state(false);
  let pickerMode = $state<'create' | 'replace'>('create');
  let pickerDate = $state<string | null>(null);
  let replaceEntry = $state<PlanEntryView | null>(null);

  let id = $derived(parseInt($page.params.id ?? '0', 10));
  let archived = $derived(plan?.archived_at !== null);

  async function load() {
    error = null;
    try {
      const [p, w] = await Promise.all([commands.getPlan(id), commands.getPlanWeeks(id)]);
      plan = p;
      weeks = w;
    } catch (e) {
      error = toMessage(e);
    }
  }

  onMount(async () => {
    await load();
    loading = false;
  });

  function openPicker(date: string, entries: PlanEntryView[]) {
    if (archived || busy) return;
    pickerDate = date;
    if (entries.length > 0) {
      pickerMode = 'replace';
      replaceEntry = entries[0];
    } else {
      pickerMode = 'create';
      replaceEntry = null;
    }
    pickerOpen = true;
  }

  function closePicker() {
    pickerOpen = false;
    pickerDate = null;
    replaceEntry = null;
  }

  async function mutate(action: () => Promise<unknown>) {
    busy = true;
    error = null;
    try {
      await action();
      await load();
    } catch (e) {
      error = toMessage(e);
    } finally {
      busy = false;
    }
  }

  function currentPickerName(): string | null {
    return replaceEntry?.workout_name ?? null;
  }

  async function assign(workout: ParsedWorkout) {
    // file_name is the library cache key: a scan can yield workouts without one.
    if (busy || !plan || !pickerDate || !workout.file_name) return;
    const fileName = workout.file_name;
    const planId = plan.id;
    const date = pickerDate;
    await mutate(async () => {
      if (pickerMode === 'replace' && replaceEntry) {
        await commands.updatePlanEntry(replaceEntry.entry_id, fileName, workout.name ?? '');
      } else {
        await commands.createPlanEntry(planId, date, fileName, workout.name ?? '');
      }
    });
    closePicker();
  }

  async function remove() {
    if (busy || !replaceEntry) return;
    const ok = await confirm('Remove this workout from the plan?', {
      title: 'Remove workout',
      kind: 'warning',
    });
    if (!ok) return;
    await mutate(async () => {
      await commands.deletePlanEntry(replaceEntry!.entry_id);
    });
    closePicker();
  }
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
      <div class="title-line">
        <h1>{plan.name}</h1>
        {#if archived}
          <span class="archived-badge">Archived</span>
        {/if}
      </div>
      <p class="meta">{formatPlanRange(plan.start_date, plan.weeks)}</p>
    </header>

    <div class="grid">
      <div class="weekday-header">
        {#each WEEKDAYS as wd (wd)}
          <span class="weekday-label">{wd}</span>
        {/each}
      </div>
      {#each weeks as week (week.number)}
        <PlanWeekRow {week} readonly={archived || busy} onopen={openPicker} />
      {/each}
    </div>
  {/if}
</div>

<WorkoutPicker
  open={pickerOpen}
  mode={pickerMode}
  currentName={currentPickerName()}
  disabled={busy}
  onpick={assign}
  onremove={pickerMode === 'replace' ? remove : undefined}
  onclose={closePicker}
/>

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

  .title-line {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    margin-bottom: 0.25rem;
  }

  h1 {
    font-size: 1.4rem;
    font-weight: 600;
    margin: 0;
  }

  .archived-badge {
    font-size: 0.7rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--muted);
    background: color-mix(in srgb, var(--muted) 12%, transparent);
    padding: 0.15rem 0.5rem;
    border-radius: 4px;
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
