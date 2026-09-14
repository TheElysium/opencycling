<script lang="ts">
  import { onMount } from 'svelte';
  import { confirm } from '@tauri-apps/plugin-dialog';
  import { commands, type NewPlan, type ParsedWorkout, type PlanWeek, type TrainingPlan } from '$lib/bindings';
  import { toMessage } from '$lib/format';
  import { mondayOf, formatPlanRange, todayMonday, todayIso } from '$lib/plan-date';
  import { currentPlan } from '$lib/plan-current';
  import { getSettings } from '$lib/settings';
  import { indexByFileName, maxWeekTss, weekLoad } from '$lib/plan-load';
  import PlanWeekRow from '$lib/components/PlanWeekRow.svelte';

  const DEFAULT_WEEKS = 4;
  const MIN_WEEKS = 1;
  const MAX_WEEKS = 52;

  type PlanForm = { name: string; start_date: string; weeks: number };

  function emptyForm(): PlanForm {
    return { name: '', start_date: todayMonday(), weeks: DEFAULT_WEEKS };
  }

  function toNewPlan(form: PlanForm): NewPlan {
    return { name: form.name.trim(), start_date: form.start_date, weeks: form.weeks };
  }

  let plans      = $state<TrainingPlan[]>([]);
  let loading    = $state(true);
  let busy       = $state(false);
  let error      = $state<string | null>(null);
  let createForm = $state<PlanForm>(emptyForm());
  let editingId  = $state<number | null>(null);
  let editForm   = $state<PlanForm>(emptyForm());

  let active   = $derived(plans.filter(p => p.archived_at === null));
  let archived = $derived(plans.filter(p => p.archived_at !== null));

  // Current-week preview: library is best-effort (see loadLibrary), the week is
  // refetched whenever the current plan changes (tracked by weekFetchedForPlanId).
  let libraryFtp = $state(0);
  let libraryWorkouts = $state<ParsedWorkout[]>([]);
  let currentWeek = $state<PlanWeek | null>(null);
  let currentPlanWeeks = $state<PlanWeek[]>([]);
  let weekFetchedForPlanId = $state<number | null>(null);

  let plan = $derived(currentPlan(plans, todayIso()));
  // Distinguishes "still fetching" from "confirmed no current plan" so the empty state never flashes.
  let weekPending = $derived(plan !== null && weekFetchedForPlanId !== plan.id);
  let workoutIndex = $derived(indexByFileName(libraryWorkouts));
  let currentWeekLoad = $derived(currentWeek ? weekLoad(currentWeek, workoutIndex, libraryFtp) : undefined);
  // Relative to the whole plan (matches /plans/[id]'s planMaxTss), not just this one week.
  let currentPlanMaxTss = $derived(
    maxWeekTss(currentPlanWeeks.map((w) => weekLoad(w, workoutIndex, libraryFtp))),
  );

  async function load() {
    error = null;
    try {
      plans = await commands.listPlans(true);
    } catch (e) {
      error = toMessage(e);
    } finally {
      loading = false;
    }
  }

  // Best-effort: a rider without a configured library still gets a usable preview.
  async function loadLibrary() {
    try {
      const s = await getSettings();
      libraryFtp = s.ftp_w;
      if (s.workout_path) {
        const lib = await commands.listWorkoutsCmd(s.workout_path, s.ftp_w);
        libraryWorkouts = lib.workouts;
      }
    } catch {
      libraryWorkouts = [];
    }
  }

  onMount(async () => {
    await Promise.all([load(), loadLibrary()]);
  });

  $effect(() => {
    const p = plan;
    if (!p || weekFetchedForPlanId === p.id) return;
    commands
      .getPlanWeeks(p.id)
      .then((weeks) => {
        currentPlanWeeks = weeks;
        currentWeek = weeks.find((w) => w.days.some((d) => d.marker === 'Today')) ?? null;
      })
      .catch(() => {
        currentPlanWeeks = [];
        currentWeek = null;
      })
      .finally(() => {
        weekFetchedForPlanId = p.id;
      });
  });

  // Backend validation is the source of truth: its message is surfaced raw.
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

  function snapMonday(form: PlanForm) {
    form.start_date = mondayOf(form.start_date);
  }

  // bind:value yields null for a cleared number input: serde would reject it
  // with a raw type error instead of the backend's own validation message.
  function canSubmit(form: PlanForm): boolean {
    return (
      !busy &&
      form.name.trim().length > 0 &&
      form.start_date.length > 0 &&
      Number.isInteger(form.weeks) &&
      form.weeks >= MIN_WEEKS &&
      form.weeks <= MAX_WEEKS
    );
  }

  async function onCreate() {
    if (!canSubmit(createForm)) return;
    await mutate(async () => {
      await commands.createPlan(toNewPlan(createForm));
      createForm = emptyForm();
    });
  }

  function startEdit(plan: TrainingPlan) {
    editingId = plan.id;
    editForm = { name: plan.name, start_date: plan.start_date, weeks: plan.weeks };
  }

  function cancelEdit() {
    editingId = null;
    editForm = emptyForm();
  }

  async function saveEdit(id: number) {
    if (!canSubmit(editForm)) return;
    await mutate(async () => {
      await commands.updatePlan(id, toNewPlan(editForm));
      cancelEdit();
    });
  }

  async function toggleArchive(plan: TrainingPlan) {
    await mutate(() => commands.setPlanArchived(plan.id, plan.archived_at === null));
  }

  async function onDelete(plan: TrainingPlan) {
    const ok = await confirm(
      `Delete "${plan.name}"? Its scheduled days go with it. This action cannot be undone.`,
      { title: 'Delete plan', kind: 'warning' },
    );
    if (!ok) return;
    await mutate(() => commands.deletePlan(plan.id));
  }
</script>

{#snippet planFields(form: PlanForm, prefix: string)}
  <div class="fields">
    <div class="field">
      <label for="{prefix}-name">Name</label>
      <input id="{prefix}-name" type="text" placeholder="Base 1" bind:value={form.name} />
    </div>
    <div class="field">
      <label for="{prefix}-start">Start date</label>
      <input
        id="{prefix}-start"
        type="date"
        bind:value={form.start_date}
        onchange={() => snapMonday(form)}
      />
      <span class="muted field-hint">Plans start on a Monday</span>
    </div>
    <div class="field">
      <label for="{prefix}-weeks">Weeks</label>
      <input
        id="{prefix}-weeks"
        type="number"
        min={MIN_WEEKS}
        max={MAX_WEEKS}
        bind:value={form.weeks}
      />
    </div>
  </div>
{/snippet}

{#snippet planCard(plan: TrainingPlan)}
  <div class="card plan-card" class:is-archived={plan.archived_at !== null}>
    {#if editingId === plan.id}
      <div class="edit-form">
        {@render planFields(editForm, `edit-${plan.id}`)}
        <div class="actions">
          <button class="btn-primary" disabled={!canSubmit(editForm)} onclick={() => saveEdit(plan.id)}>
            Save
          </button>
          <button class="btn-secondary" onclick={cancelEdit}>Cancel</button>
        </div>
      </div>
    {:else}
      <div class="plan-head">
        <a class="plan-name" href="/plans/{plan.id}">{plan.name}</a>
        <span class="plan-meta">
          {formatPlanRange(plan.start_date, plan.weeks)}
          <span class="sep">·</span>
          {plan.weeks} {plan.weeks === 1 ? 'week' : 'weeks'}
        </span>
      </div>
      <div class="actions">
        <button class="btn-secondary" disabled={busy} onclick={() => startEdit(plan)}>Edit</button>
        <button class="btn-ghost" disabled={busy} onclick={() => toggleArchive(plan)}>
          {plan.archived_at === null ? 'Archive' : 'Unarchive'}
        </button>
        <button class="btn-delete" disabled={busy} onclick={() => onDelete(plan)}>Delete</button>
      </div>
    {/if}
  </div>
{/snippet}

<div class="page-wide">
  <h1>Plans</h1>

  <section class="card current-week-card">
    <div class="current-week-head">
      <span class="current-week-label">Current week</span>
      {#if plan}
        <a class="plan-name" href="/plans/{plan.id}">{plan.name}</a>
      {/if}
    </div>
    {#if loading || weekPending}
      <p class="muted">Loading…</p>
    {:else if !plan || !currentWeek}
      <p class="muted">No active plan this week.</p>
    {:else}
      <PlanWeekRow week={currentWeek} readonly={true} load={currentWeekLoad} maxTss={currentPlanMaxTss} />
    {/if}
  </section>

  <form
    class="card create-card"
    onsubmit={(e) => {
      e.preventDefault();
      onCreate();
    }}
  >
    {@render planFields(createForm, 'create')}
    <button class="btn-primary" type="submit" disabled={!canSubmit(createForm)}>Create plan</button>
  </form>

  {#if error}
    <p class="error-box">{error}</p>
  {/if}

  {#if loading}
    <p class="muted">Loading…</p>
  {:else if plans.length === 0}
    <p class="muted">No plans yet. Create one above to start a training block.</p>
  {:else}
    <div class="plan-list">
      {#each active as plan (plan.id)}
        {@render planCard(plan)}
      {/each}
    </div>
    {#if archived.length > 0}
      <h2 class="list-section">Archived</h2>
      <div class="plan-list">
        {#each archived as plan (plan.id)}
          {@render planCard(plan)}
        {/each}
      </div>
    {/if}
  {/if}
</div>

<style>
  h1 { font-size: 1.4rem; font-weight: 600; margin: 0 0 1.25rem; }

  .muted { color: var(--muted); }

  .create-card { margin-bottom: 1.25rem; }

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

  .fields {
    display: flex;
    flex-wrap: wrap;
    gap: 1rem;
    margin-bottom: 1rem;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    flex: 1;
    min-width: 150px;
  }

  label {
    font-size: 0.85rem;
    font-weight: 500;
    color: var(--muted);
  }

  input {
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 0.45rem 0.75rem;
    font-size: 0.95rem;
    color: var(--text);
    caret-color: var(--text);
    width: 100%;
    cursor: text;
  }

  input:focus {
    outline: none;
    border-color: var(--accent);
  }

  .field-hint { font-size: 0.75rem; }

  .plan-list {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .plan-card {
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: 0.75rem;
  }

  .plan-card.is-archived { opacity: 0.65; }

  .edit-form { width: 100%; }

  .plan-head {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }

  .plan-name {
    font-weight: 600;
    font-size: 0.95rem;
    color: inherit;
    text-decoration: none;
  }
  .plan-name:hover,
  .plan-name:focus-visible { color: var(--accent); }

  .plan-meta {
    font-size: 0.8rem;
    color: var(--muted);
  }

  .sep { opacity: 0.6; }

  .list-section {
    font-size: 0.78rem;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--muted);
    margin: 1.5rem 0 0.6rem;
    padding-bottom: 0.4rem;
    border-bottom: 1px solid var(--border);
  }

  .actions {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .btn-delete {
    background: transparent;
    color: var(--danger);
    border: 1px solid color-mix(in srgb, var(--danger) 35%, transparent);
    transition: background 0.15s, border-color 0.15s;
  }

  .btn-delete:hover:not(:disabled) {
    background: color-mix(in srgb, var(--danger) 10%, transparent);
  }
</style>
