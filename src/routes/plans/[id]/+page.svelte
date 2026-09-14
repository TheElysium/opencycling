<script lang="ts">
  import { onMount } from 'svelte';
  import { page } from '$app/stores';
  import { goto } from '$app/navigation';
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
  import {
    assignAction,
    entryWorkoutName,
    noteAction,
    removeWorkoutAction,
    type DayAction,
  } from '$lib/plan-entry';
  import { getSettings } from '$lib/settings';
  import { entryIntensities, indexByFileName, maxWeekTss, weekLoad } from '$lib/plan-load';
  import { workoutFtp } from '$lib/ftp';
  import { session } from '$lib/session.svelte';
  import PlanWeekRow from '$lib/components/PlanWeekRow.svelte';
  import PlanNoteField from '$lib/components/PlanNoteField.svelte';
  import WorkoutPicker from '$lib/components/WorkoutPicker.svelte';

  const WEEKDAYS = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'];

  let plan = $state<TrainingPlan | null>(null);
  let weeks = $state<PlanWeek[]>([]);
  let loading = $state(true);
  let busy = $state(false);
  let error = $state<string | null>(null);

  // Library/settings feed the week summaries only: their failure must not break the grid.
  let libraryFtp = $state(0);
  let libraryAero = $state(false);
  let libraryWorkouts = $state<ParsedWorkout[]>([]);

  let pickerOpen = $state(false);
  let pickerDate = $state<string | null>(null);
  let dayEntry = $state<PlanEntryView | null>(null);
  let noteDraft = $state('');

  let id = $derived(parseInt($page.params.id ?? '0', 10));
  let archived = $derived(plan?.archived_at !== null);
  let pickerMode = $derived<'create' | 'replace'>(dayEntry?.file_name ? 'replace' : 'create');

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

  // Best-effort: a rider without a configured library still gets a usable plan grid.
  async function loadLibrary() {
    try {
      const s = await getSettings();
      libraryFtp = s.ftp_w;
      libraryAero = s.aero_enabled;
      if (s.workout_path) {
        const lib = await commands.listWorkoutsCmd(s.workout_path, s.ftp_w);
        libraryWorkouts = lib.workouts;
      }
    } catch {
      libraryWorkouts = [];
    }
  }

  let workoutIndex = $derived(indexByFileName(libraryWorkouts));
  let weekLoads = $derived(weeks.map((w) => weekLoad(w, workoutIndex, libraryFtp)));
  let planMaxTss = $derived(maxWeekTss(weekLoads));
  let planIntensities = $derived(entryIntensities(weeks, workoutIndex, libraryFtp));

  onMount(async () => {
    await Promise.all([load(), loadLibrary()]);
    loading = false;
  });

  function openPicker(date: string, entries: PlanEntryView[]) {
    if (archived || busy) return;
    pickerDate = date;
    dayEntry = entries[0] ?? null;
    noteDraft = dayEntry?.note ?? '';
    pickerOpen = true;
  }

  function closePicker() {
    pickerOpen = false;
    pickerDate = null;
    dayEntry = null;
    noteDraft = '';
  }

  async function startEntry(entry: PlanEntryView) {
    if (busy || !entry.file_name) return;
    error = null;
    const workout = workoutIndex.get(entry.file_name);
    if (!workout) {
      error = 'Workout not found in the library, rescan the library from Settings.';
      return;
    }
    session.prepare(workout, workoutFtp(workout, libraryFtp), libraryAero, entry.entry_id);
    await goto('/session');
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
    return entryWorkoutName(dayEntry?.file_name ?? null, dayEntry?.workout_name ?? null);
  }

  // Sends one decided action to the backend; a `none` action never reaches it.
  async function apply(action: DayAction, planId: number, date: string) {
    // A no-op is not a failure: it must not keep a stale error on screen.
    if (action.kind === 'none') {
      error = null;
      return;
    }
    await mutate(async () => {
      if (action.kind === 'create') {
        await commands.createPlanEntry({ plan_id: planId, date, ...action.content });
      } else if (action.kind === 'update') {
        await commands.updatePlanEntry(action.entryId, action.content);
      } else {
        await commands.deletePlanEntry(action.entryId);
      }
    });
  }

  async function assign(workout: ParsedWorkout) {
    // file_name is the library cache key: a scan can yield workouts without one.
    if (busy || !plan || !pickerDate || !workout.file_name) return;
    const picked = { file_name: workout.file_name, workout_name: workout.name };
    const action = assignAction(dayEntry, picked, noteDraft);
    await apply(action, plan.id, pickerDate);
    // A refused save must keep the modal open: closing it would discard the draft.
    if (!error) closePicker();
  }

  async function saveNote() {
    if (busy || !plan || !pickerDate) return;
    const action = noteAction(dayEntry, noteDraft);
    await apply(action, plan.id, pickerDate);
    if (!error) closePicker();
  }

  async function remove() {
    if (busy || !plan || !pickerDate || !dayEntry) return;
    // Captured before the dialog: closing the picker meanwhile nulls dayEntry.
    const entry = dayEntry;
    const ok = await confirm('Remove this workout from the plan?', {
      title: 'Remove workout',
      kind: 'warning',
    });
    if (!ok) return;
    await apply(removeWorkoutAction(entry, noteDraft), plan.id, pickerDate);
    if (!error) closePicker();
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
      {#each weeks as week, i (week.number)}
        <PlanWeekRow
          {week}
          readonly={archived || busy}
          onopen={openPicker}
          onstart={startEntry}
          load={weekLoads[i]}
          maxTss={planMaxTss}
          intensities={planIntensities}
        />
      {/each}
    </div>
  {/if}
</div>

{#snippet noteEditor()}
  <PlanNoteField bind:note={noteDraft} disabled={busy} onsave={saveNote} />
{/snippet}

<WorkoutPicker
  open={pickerOpen}
  mode={pickerMode}
  currentName={currentPickerName()}
  disabled={busy}
  aside={noteEditor}
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
