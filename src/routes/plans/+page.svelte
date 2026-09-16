<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { confirm } from '@tauri-apps/plugin-dialog';
  import { commands, type NewPlan, type PlanEntryView, type TrainingPlan } from '$lib/bindings';
  import { toMessage } from '$lib/format';
  import { currentPlan, mondayOf, formatPlanRange, todayMonday, todayIso } from '$lib/plan-date';
  import { library } from '$lib/library.svelte';
  import CurrentPlanWeek from '$lib/components/CurrentPlanWeek.svelte';

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
  let plan     = $derived(currentPlan(plans, todayIso()));

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

  onMount(async () => {
    await Promise.all([load(), library.load()]);
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

  // Mirrors /plans/[id]'s startEntry: same tile, same launch path.
  async function startEntry(entry: PlanEntryView) {
    if (busy || !entry.file_name) return;
    error = null;
    const err = await library.start(entry);
    if (err) error = err;
  }

  // Editing a day belongs to the full plan view (it owns the picker modal): jump there.
  function openInPlan() {
    if (plan) goto(`/plans/${plan.id}`);
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
          {formatPlanRange(plan.start_date, plan.end_date)}
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

  <h2 class="section-title">Current plan</h2>
  <CurrentPlanWeek {plan} {loading} onopen={openInPlan} onstart={startEntry} />

  <h2 class="section-title">Create plan</h2>
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

  <h2 class="section-title">Plans</h2>
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
      <h3 class="list-section">Archived</h3>
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

  .section-title {
    font-size: 1rem;
    font-weight: 600;
    color: var(--text);
    margin: 0 0 0.6rem;
  }

  .create-card { margin-bottom: 1.25rem; }

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
</style>
