<script lang="ts">
  import { onMount } from 'svelte';
  import { SvelteSet } from 'svelte/reactivity';
  import { goto } from '$app/navigation';
  import { Search, X, ArrowUp, ArrowDown } from '@lucide/svelte';
  import WorkoutThumb from '$lib/components/WorkoutThumb.svelte';
  import { commands, type FlatBlock } from '$lib/bindings';
  import { workoutSelection, type ParsedWorkout, type WorkoutFileError } from '$lib/workout.svelte';
  import {
    formatDuration,
    totalDuration,
    displayWorkoutName,
    formatRelativeDate,
    toMessage,
  } from '$lib/format';
  import { computeWorkoutMetrics, workoutTypeColor, type WorkoutType } from '$lib/metrics';
  import { workoutFtp } from '$lib/ftp';
  import { getSettings } from '$lib/settings';
  import { uniqueSortedTags, matchesSelectedTags } from '$lib/workout-filter';

  let workoutPath    = $state('');
  let ftp            = $state(200);
  let workouts       = $state<ParsedWorkout[]>([]);
  // Flat blocks per workout (same index as `workouts`), returned by
  // list_workouts_cmd; drives the card thumbnails.
  let flats          = $state<FlatBlock[][]>([]);
  // Last-used ISO timestamp per workout, same index as `workouts` (from list_workouts_cmd).
  let lastUsed       = $state<(string | null)[]>([]);
  let parseErrors    = $state<WorkoutFileError[]>([]);
  let showParseErrors = $state(true);
  let loading        = $state(true);
  let error          = $state<string | null>(null);
  let query          = $state('');
  const selectedTags = new SvelteSet<string>();

  function toggleTag(tag: string) {
    if (selectedTags.has(tag)) selectedTags.delete(tag); else selectedTags.add(tag);
  }

  type SortField = 'name' | 'zone' | 'duration' | 'lastUsed';
  let sortField = $state<SortField>('name');
  let sortDir   = $state<'asc' | 'desc'>('asc');

  const sortOptions: { field: SortField; label: string }[] = [
    { field: 'name',     label: 'Name'      },
    { field: 'zone',     label: 'Zone'      },
    { field: 'duration', label: 'Duration'  },
    { field: 'lastUsed', label: 'Last used' },
  ];

  // Zone order by ascending intensity, drives the "Zone" sort.
  const ZONE_ORDER: Record<WorkoutType, number> = {
    Recovery:     0,
    Endurance:    1,
    Tempo:        2,
    'Sweet Spot': 3,
    Threshold:    4,
    VO2max:       5,
    Anaerobic:    6,
  };

  function setSort(field: SortField) {
    if (sortField === field) {
      sortDir = sortDir === 'asc' ? 'desc' : 'asc';
    } else {
      sortField = field;
      // Sensible default direction per field.
      sortDir = field === 'name' ? 'asc' : 'desc';
    }
  }

  // Precompute metrics and display name once so filtering, sorting, and the
  // grid all share the same values instead of recomputing per render.
  let decorated = $derived(
    workouts.map((w, i) => {
      // A test renders at its reference FTP (watt == %), so metrics/thumb use that.
      const cardFtp = workoutFtp(w, ftp);
      const lastUsedAt = lastUsed[i] ?? null;
      return {
        w,
        cardFtp,
        flat: flats[i] ?? [],
        m: computeWorkoutMetrics(w.workout_blocks, cardFtp),
        name: displayWorkoutName(w.name),
        lastUsedAt,
        lastUsedLabel: formatRelativeDate(lastUsedAt),
      };
    })
  );

  let allTags = $derived(uniqueSortedTags(workouts));

  let filteredWorkouts = $derived.by(() => {
    const q = query.trim().toLowerCase();
    const afterQuery = q
      ? decorated.filter(d => d.name.toLowerCase().includes(q))
      : decorated.slice();
    const withTags = selectedTags.size === 0
      ? afterQuery
      : afterQuery.filter(d => matchesSelectedTags(d.w.tags, selectedTags));

    const dir = sortDir === 'asc' ? 1 : -1;
    withTags.sort((a, b) => {
      let cmp: number;
      if (sortField === 'zone') {
        cmp = ZONE_ORDER[a.m.type] - ZONE_ORDER[b.m.type];
        // Tie-break within a zone by intensity for a stable, intuitive order.
        if (cmp === 0) cmp = a.m.if_ - b.m.if_;
      } else if (sortField === 'duration') {
        cmp = a.m.duration_s - b.m.duration_s;
      } else if (sortField === 'lastUsed') {
        // Unparsable/null dates sort as oldest (0), so never-used workouts fall last on desc.
        cmp = (Date.parse(a.lastUsedAt ?? '') || 0) - (Date.parse(b.lastUsedAt ?? '') || 0);
      } else {
        cmp = a.name.localeCompare(b.name);
      }
      return cmp * dir;
    });
    return withTags;
  });

  onMount(async () => {
    try {
      const s = await getSettings();
      workoutPath = s.workout_path;
      ftp = s.ftp_w;
      if (workoutPath) {
        const lib = await commands.listWorkoutsCmd(workoutPath, ftp);
        // Flats come pre-flattened from the backend (one round-trip), parallel to
        // `workouts` and at each card's FTP, so thumbnails match the session exactly.
        workouts = lib.workouts;
        flats = lib.flats;
        lastUsed = lib.last_used;
        parseErrors = lib.errors;
      }
    } catch (e) {
      error = toMessage(e);
    } finally {
      loading = false;
    }
  });

  function select(w: ParsedWorkout) {
    workoutSelection.workout = w;
    goto('/workouts/detail');
  }
</script>

<div>
  <h1>
    Workouts
    {#if !loading && workouts.length > 0}
      <span class="count">{filteredWorkouts.length}{#if (query || selectedTags.size > 0) && filteredWorkouts.length !== workouts.length} / {workouts.length}{/if}</span>
    {/if}
  </h1>

  {#if !loading && workouts.length > 0}
    <div class="toolbar">
      <div class="sort">
        <span class="sort-label">Sort</span>
        {#each sortOptions as opt (opt.field)}
          <button
            class="sort-btn"
            class:active={sortField === opt.field}
            onclick={() => setSort(opt.field)}
            aria-label="Sort by {opt.label}"
          >
            {opt.label}
            {#if sortField === opt.field}
              {#if sortDir === 'asc'}
                <ArrowUp size={13} aria-hidden="true" />
              {:else}
                <ArrowDown size={13} aria-hidden="true" />
              {/if}
            {/if}
          </button>
        {/each}
      </div>
      <div class="search">
        <Search size={14} aria-hidden="true" />
        <input
          type="search"
          placeholder="Search workouts…"
          bind:value={query}
          aria-label="Search workouts"
        />
        {#if query}
          <button class="clear-btn" onclick={() => query = ''} aria-label="Clear search">
            <X size={14} />
          </button>
        {/if}
      </div>
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
  {/if}

  {#if parseErrors.length > 0 && showParseErrors}
    <div class="warn-box">
      <span>
        {parseErrors.length} file{parseErrors.length > 1 ? 's' : ''} could not be parsed:
        {parseErrors.map(e => e.file_name).join(', ')}
      </span>
      <button class="dismiss-btn" onclick={() => showParseErrors = false} aria-label="Dismiss warning">
        <X size={14} />
      </button>
    </div>
  {/if}

  {#if loading}
    <div class="workout-grid">
      {#each Array(4) as _, i (i)}
        <div class="skeleton-card"></div>
      {/each}
    </div>
  {:else if error}
    <p class="error-box">{error}</p>
  {:else if !workoutPath}
    <p class="muted">No workout folder configured. <a class="link" href="/settings">Go to Settings</a></p>
  {:else if workouts.length === 0}
    <p class="muted">No workouts found in <code>{workoutPath}</code>.</p>
  {:else if filteredWorkouts.length === 0}
    <p class="muted">
      {#if query}No workouts match "<strong>{query}</strong>".{:else}No workouts match the selected tags.{/if}
    </p>
  {:else}
    <div class="workout-grid">
      {#each filteredWorkouts as { w, m, name, cardFtp, flat, lastUsedLabel }, i (i)}
        <button class="workout-card" onclick={() => select(w)}>
          <div class="card-chart">
            <WorkoutThumb blocks={flat} ftpWatts={cardFtp} />
          </div>
          <div class="card-info">
            {#if w.tags.length > 0}
              <div class="tag-pills">
                {#each w.tags as tag (tag)}
                  <span class="tag-pill">{tag}</span>
                {/each}
              </div>
            {/if}
            {#if w.is_ftp_test}
              <span class="ftp-badge">FTP Test</span>
            {:else if m.tss > 0}
              <span class="type-badge" style="--type-color: {workoutTypeColor(m.type)}">
                <span class="type-dot"></span>{m.type}
              </span>
            {/if}
            <span class="name">{name}</span>
            <div class="card-meta">
              <span>{formatDuration(totalDuration(w.workout_blocks))}</span>
              {#if m.tss > 0 && !w.is_ftp_test}
                <span class="dot-sep">·</span>
                <span title="Training Stress Score">{Math.round(m.tss)} TSS</span>
                <span class="dot-sep">·</span>
                <span title="Intensity Factor">{m.if_.toFixed(2)} IF</span>
              {/if}
              <span class="dot-sep">·</span>
              <span class="last-used">{lastUsedLabel}</span>
            </div>
          </div>
        </button>
      {/each}
    </div>
  {/if}
</div>

<style>
  h1 {
    font-size: 1.4rem;
    font-weight: 600;
    margin: 0 0 1.25rem;
    display: flex;
    align-items: baseline;
    gap: 0.5rem;
  }

  .count {
    font-size: 0.8rem;
    font-weight: 500;
    color: var(--muted);
  }

  .toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    margin-bottom: 1.25rem;
    flex-wrap: wrap;
  }

  .sort {
    display: flex;
    align-items: center;
    gap: 0.25rem;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 0.2rem;
  }

  .sort-label {
    font-size: 0.75rem;
    color: var(--muted);
    padding: 0 0.4rem 0 0.35rem;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .sort-btn {
    display: inline-flex;
    align-items: center;
    gap: 0.25rem;
    background: none;
    border: none;
    color: var(--muted);
    font: inherit;
    font-size: 0.82rem;
    padding: 0.3rem 0.55rem;
    border-radius: 6px;
    cursor: pointer;
    transition: color 0.15s, background 0.15s;
  }

  .sort-btn:hover {
    color: var(--text);
    background: var(--bg);
  }

  .sort-btn.active {
    color: var(--accent);
    background: color-mix(in srgb, var(--accent) 12%, transparent);
  }

  .search {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 0.45rem 0.7rem;
    color: var(--text);
    transition: border-color 0.15s, box-shadow 0.15s;
    min-width: 220px;
  }

  .search :global(svg) {
    color: var(--muted);
    flex-shrink: 0;
  }

  .search:focus-within {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 18%, transparent);
  }

  .search:focus-within :global(svg) {
    color: var(--accent);
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

  .search input::placeholder {
    color: var(--muted);
  }

  .search input::-webkit-search-cancel-button { display: none; }

  .clear-btn {
    background: none;
    border: none;
    color: var(--muted);
    padding: 0.1rem;
    display: inline-flex;
    cursor: pointer;
    border-radius: 4px;
  }

  .clear-btn:hover { color: var(--text); }

  .tag-filters {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
    margin: -0.5rem 0 1.25rem;
  }

  /* .tag-pill (below) provides the shared pastille look; this adds the
     interactive-button bits (cursor, border, active state, transition). */
  .tag-filter {
    border: 1px solid transparent;
    cursor: pointer;
    transition: color 0.15s, background 0.15s, border-color 0.15s;
  }

  .tag-filter:hover {
    color: var(--text);
  }

  .tag-filter.active {
    color: var(--accent);
    background: color-mix(in srgb, var(--accent) 14%, transparent);
    border-color: color-mix(in srgb, var(--accent) 30%, transparent);
  }

  .muted { color: var(--muted); }
  .link  { color: var(--accent); text-decoration: underline; }

  .workout-card {
    display: flex;
    flex-direction: column;
    width: 100%;
    min-width: 0;
    text-align: left;
    cursor: pointer;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 10px;
    overflow: hidden;
    padding: 0;
    transition: border-color 0.15s, box-shadow 0.15s, transform 0.15s;
  }

  .workout-card:hover {
    border-color: var(--accent);
    box-shadow: 0 4px 12px rgba(0,0,0,0.08);
    transform: translateY(-1px);
  }

  .card-chart {
    background: var(--surface-dark);
    padding: 1rem 0.75rem 0.5rem;
    --chart-gap: var(--surface-dark);
  }

  .card-info {
    padding: 0.75rem 1rem 0.9rem;
  }

  .type-badge {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    font-size: 0.7rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--type-color);
    background: color-mix(in srgb, var(--type-color) 14%, transparent);
    border-radius: 4px;
    padding: 0.15rem 0.5rem;
    align-self: flex-start;
    margin-bottom: 0.35rem;
  }

  .type-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--type-color);
  }

  .tag-pills {
    display: flex;
    flex-wrap: wrap;
    gap: 0.25rem;
    margin-bottom: 0.35rem;
  }

  .tag-pill {
    font-size: 0.65rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.03em;
    padding: 0.1rem 0.4rem;
    border-radius: 4px;
    background: var(--border);
    color: var(--muted);
  }

  .ftp-badge {
    display: inline-flex;
    align-items: center;
    font-size: 0.7rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--accent);
    background: color-mix(in srgb, var(--accent) 14%, transparent);
    border-radius: 4px;
    padding: 0.15rem 0.5rem;
    align-self: flex-start;
    margin-bottom: 0.35rem;
  }

  .name {
    display: block;
    font-weight: 600;
    font-size: 0.95rem;
    margin-bottom: 0.3rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .card-meta {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.35rem;
    font-size: 0.78rem;
    color: var(--muted);
  }

  .dot-sep {
    opacity: 0.6;
  }

  .last-used {
    margin-left: auto;
  }

  .warn-box {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
    padding: 0.65rem 1rem;
    background: color-mix(in srgb, var(--warning) 10%, transparent);
    border: 1px solid color-mix(in srgb, var(--warning) 30%, transparent);
    border-radius: 8px;
    color: color-mix(in srgb, var(--warning) 80%, var(--text));
    font-size: 0.85rem;
    margin-bottom: 1rem;
  }

  .dismiss-btn {
    background: none;
    border: none;
    color: inherit;
    padding: 0.1rem;
    display: inline-flex;
    cursor: pointer;
    border-radius: 4px;
    flex-shrink: 0;
  }

  .dismiss-btn:hover {
    background: color-mix(in srgb, var(--warning) 20%, transparent);
  }

  .skeleton-card {
    height: 170px;
    background: linear-gradient(90deg, var(--surface) 0%, var(--bg) 50%, var(--surface) 100%);
    background-size: 200% 100%;
    border: 1px solid var(--border);
    border-radius: 10px;
    animation: shimmer 1.4s ease-in-out infinite;
  }

  @keyframes shimmer {
    0%   { background-position: 200% 0; }
    100% { background-position: -200% 0; }
  }
</style>
