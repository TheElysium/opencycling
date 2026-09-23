<script lang="ts">
  import type { Snippet } from 'svelte';
  import { SvelteSet } from 'svelte/reactivity';
  import { Search, X } from '@lucide/svelte';
  import { commands, type ParsedWorkout, type WorkoutLibrary } from '$lib/bindings';
  import { computeWorkoutMetrics } from '$lib/metrics';
  import { workoutFtp } from '$lib/ftp';
  import { getSettings } from '$lib/settings';
  import { displayWorkoutName, formatDuration, toMessage } from '$lib/format';
  import { uniqueSortedTags, matchesSelectedTags } from '$lib/workout-filter';

  type Props = {
    open: boolean;
    mode: 'create' | 'replace';
    currentName: string | null;
    disabled?: boolean;
    /** Day editors rendered above the list; the overlay shell lives here only. */
    aside?: Snippet;
    onpick: (workout: ParsedWorkout) => void;
    onremove?: () => void;
    onclose: () => void;
  };

  let {
    open,
    mode,
    currentName,
    disabled = false,
    aside,
    onpick,
    onremove,
    onclose,
  }: Props = $props();

  let workouts = $state<ParsedWorkout[]>([]);
  let noFolder = $state(false);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let query = $state('');
  let searchInput = $state<HTMLInputElement | null>(null);
  const selectedTags = new SvelteSet<string>();

  // Recompute display values once per render; do not duplicate metrics logic.
  let decorated = $derived(
    workouts.map((w) => {
      const cardFtp = workoutFtp(w, ftp);
      return {
        w,
        name: displayWorkoutName(w.name),
        m: computeWorkoutMetrics(w.workout_blocks, cardFtp),
        tags: w.tags,
      };
    })
  );

  let allTags = $derived(uniqueSortedTags(workouts));

  let filtered = $derived.by(() => {
    const q = query.trim().toLowerCase();
    const afterQuery = q
      ? decorated.filter((d) => d.name.toLowerCase().includes(q))
      : decorated.slice();
    const withTags =
      selectedTags.size === 0
        ? afterQuery
        : afterQuery.filter((d) => matchesSelectedTags(d.w.tags, selectedTags));
    withTags.sort((a, b) => a.name.localeCompare(b.name));
    return withTags;
  });

  let ftp = $state(200);

  async function load() {
    loading = true;
    error = null;
    try {
      const s = await getSettings();
      ftp = s.ftp_w;
      noFolder = !s.workout_path;
      if (s.workout_path) {
        const lib: WorkoutLibrary = await commands.listWorkoutsCmd(s.workout_path, ftp);
        // An entry references a library file: a workout without a file_name
        // (unscanned/foreign) cannot be assigned.
        workouts = lib.workouts.filter((w) => w.file_name !== null);
      }
    } catch (e) {
      error = toMessage(e);
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    if (open) {
      selectedTags.clear();
      query = '';
      load();
      // Focus the search input once the modal renders.
      queueMicrotask(() => searchInput?.focus());
    }
  });

  function toggleTag(tag: string) {
    if (selectedTags.has(tag)) selectedTags.delete(tag);
    else selectedTags.add(tag);
  }

  // The consumer decides when to close: a rejected write must keep the modal open.
  function pick(w: ParsedWorkout) {
    if (disabled) return;
    onpick(w);
  }

  // Blocked while the picker loads its own list or the caller is busy applying
  // a previous action: closing mid-mutation would discard its result silently.
  function close() {
    if (!loading && !disabled) onclose();
  }

  // Keep Tab cycling inside the modal while it is open.
  function trapFocus(e: KeyboardEvent) {
    if (e.key !== 'Tab') return;
    const focusables = (e.currentTarget as HTMLElement).querySelectorAll<HTMLElement>(
      'button:not([disabled]), input:not([disabled]), textarea:not([disabled])',
    );
    if (focusables.length === 0) return;
    const first = focusables[0];
    const last = focusables[focusables.length - 1];
    if (e.shiftKey && document.activeElement === first) {
      e.preventDefault();
      last.focus();
    } else if (!e.shiftKey && document.activeElement === last) {
      e.preventDefault();
      first.focus();
    }
  }

  function onScrimKey(e: KeyboardEvent) {
    if (e.key === 'Escape') close();
  }
</script>

{#if open}
  <div class="scrim" onclick={close} onkeydown={onScrimKey} role="presentation" tabindex="-1">
    <div
      class="dialog"
      role="dialog"
      aria-modal="true"
      tabindex="-1"
      onclick={(e) => e.stopPropagation()}
      onkeydown={(e) => (e.key === 'Escape' ? close() : trapFocus(e))}
    >
      <header>
        <h2>{mode === 'replace' ? `Replace ${currentName ?? 'workout'}` : 'Assign workout'}</h2>
        <button class="close-btn" onclick={close} aria-label="Close">
          <X size={18} />
        </button>
      </header>

      {#if error}
        <p class="error-box">{error}</p>
      {/if}

      {@render aside?.()}

      <div class="search">
        <Search size={14} aria-hidden="true" />
        <input
          bind:this={searchInput}
          type="search"
          placeholder="Search workouts…"
          bind:value={query}
          aria-label="Search workouts"
        />
        {#if query}
          <button class="clear-btn" onclick={() => (query = '')} aria-label="Clear search">
            <X size={14} />
          </button>
        {/if}
      </div>

      {#if allTags.length > 0}
        <div class="tag-filters">
          {#each allTags as tag (tag)}
            <button
              class="tag-pill tag-filter"
              class:active={selectedTags.has(tag)}
              onclick={() => toggleTag(tag)}
              aria-pressed={selectedTags.has(tag)}
            >
              {tag}
            </button>
          {/each}
        </div>
      {/if}

      <div class="list">
        {#if loading}
          <p class="muted">Loading…</p>
        {:else if noFolder}
          <p class="muted">No workout library folder configured — set it in Settings.</p>
        {:else if filtered.length === 0}
          <p class="muted">
            {#if query || selectedTags.size > 0}No workouts match.{:else}No workouts found.{/if}
          </p>
        {:else}
          {#each filtered as { w, name, m, tags } (w.file_name ?? w.name)}
            <button class="row" disabled={disabled} onclick={() => pick(w)}>
              <span class="name">{name}</span>
              <span class="meta">{formatDuration(m.duration_s)} · {m.type}</span>
              {#if tags.length > 0}
                <span class="tags">{tags.join(' · ')}</span>
              {/if}
            </button>
          {/each}
        {/if}
      </div>

      <div class="actions">
        {#if mode === 'replace' && onremove}
          <button class="btn-delete" disabled={disabled} onclick={onremove}>Remove</button>
        {/if}
        <button class="btn-secondary" disabled={disabled} onclick={close}>Cancel</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .scrim {
    position: fixed;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(0, 0, 0, 0.55);
    z-index: 100;
    padding: 1rem;
  }

  .dialog {
    width: min(560px, 100%);
    max-height: min(720px, 90vh);
    display: flex;
    flex-direction: column;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 12px;
    padding: 1.25rem;
    box-shadow: 0 16px 48px rgba(0, 0, 0, 0.25);
    outline: none;
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 1rem;
  }

  h2 {
    font-size: 1.1rem;
    font-weight: 600;
    margin: 0;
  }

  .close-btn {
    background: none;
    border: none;
    color: var(--muted);
    cursor: pointer;
    display: inline-flex;
    padding: 0.2rem;
    border-radius: 6px;
  }
  .close-btn:hover { color: var(--text); }

  .search {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 0 0.7rem;
    height: 2.25rem;
    color: var(--text);
    margin-bottom: 0.75rem;
  }

  .search :global(svg) {
    color: var(--muted);
    flex-shrink: 0;
  }

  .search input {
    border: none;
    outline: none;
    background: transparent;
    color: var(--text);
    font: inherit;
    font-size: 0.88rem;
    flex: 1;
    min-width: 0;
  }

  .search input::placeholder { color: var(--muted); }
  .search input::-webkit-search-cancel-button { display: none; }

  .clear-btn {
    background: none;
    border: none;
    color: var(--muted);
    cursor: pointer;
    display: inline-flex;
    padding: 0.1rem;
    border-radius: 4px;
  }
  .clear-btn:hover { color: var(--text); }

  .tag-filters {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
    margin-bottom: 0.75rem;
  }

  .list {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    overflow-y: auto;
    min-height: 120px;
    margin-bottom: 1rem;
  }

  .row {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 0.15rem;
    text-align: left;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 0.65rem 0.85rem;
    color: var(--text);
    font: inherit;
    cursor: pointer;
    transition: border-color 0.15s, background 0.15s;
  }

  .row:hover {
    border-color: var(--accent);
    background: color-mix(in srgb, var(--accent) 6%, transparent);
  }

  .name {
    font-weight: 600;
    font-size: 0.92rem;
  }

  .meta {
    font-size: 0.78rem;
    color: var(--muted);
  }

  .tags {
    font-size: 0.7rem;
    color: var(--muted);
    margin-top: 0.15rem;
  }

  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
    margin-top: auto;
  }

  /* Smaller footprint than the default .btn-secondary/.btn-delete: this
     dialog's actions row is compact. */
  .actions .btn-secondary,
  .actions .btn-delete {
    padding: 0.45rem 0.85rem;
  }
</style>
