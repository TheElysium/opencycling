<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { Search, X } from '@lucide/svelte';
  import WorkoutChart from '$lib/components/WorkoutChart.svelte';
  import { workoutSelection, type ParsedWorkout } from '$lib/workout.svelte';
  import { formatDuration, totalDuration, displayWorkoutName, toMessage } from '$lib/format';
  import { computeWorkoutMetrics, workoutTypeColor } from '$lib/metrics';
  import { getSettings } from '$lib/settings';

  let workoutPath = $state('');
  let ftp         = $state(200);
  let workouts    = $state<ParsedWorkout[]>([]);
  let loading     = $state(true);
  let error       = $state<string | null>(null);
  let query       = $state('');

  let filteredWorkouts = $derived(
    query.trim()
      ? workouts.filter(w =>
          displayWorkoutName(w.name).toLowerCase().includes(query.trim().toLowerCase())
        )
      : workouts
  );

  onMount(async () => {
    try {
      const s = await getSettings();
      workoutPath = s.workout_path;
      ftp = s.ftp_w;
      if (workoutPath) {
        workouts = await invoke<ParsedWorkout[]>('list_workouts_cmd', { folder: workoutPath });
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
  <div class="header">
    <h1>
      Workouts
      {#if !loading && workouts.length > 0}
        <span class="count">{filteredWorkouts.length}{#if query && filteredWorkouts.length !== workouts.length} / {workouts.length}{/if}</span>
      {/if}
    </h1>
    {#if !loading && workouts.length > 0}
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
    {/if}
  </div>

  {#if loading}
    <div class="workout-grid">
      {#each Array(4) as _}
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
    <p class="muted">No workouts match "<strong>{query}</strong>".</p>
  {:else}
    <div class="workout-grid">
      {#each filteredWorkouts as w}
        {@const m = computeWorkoutMetrics(w.workout_blocks, ftp)}
        <button class="workout-card" onclick={() => select(w)}>
          <div class="card-chart">
            <WorkoutChart blocks={w.workout_blocks} height={72} />
          </div>
          <div class="card-info">
            {#if m.tss > 0}
              <span class="type-badge" style="--type-color: {workoutTypeColor(m.type)}">
                <span class="type-dot"></span>{m.type}
              </span>
            {/if}
            <span class="name">{displayWorkoutName(w.name)}</span>
            <div class="card-meta">
              <span>{formatDuration(totalDuration(w.workout_blocks))}</span>
              {#if m.tss > 0}
                <span class="dot-sep">·</span>
                <span title="Training Stress Score">{Math.round(m.tss)} TSS</span>
                <span class="dot-sep">·</span>
                <span title="Intensity Factor">{m.if_.toFixed(2)} IF</span>
              {/if}
            </div>
          </div>
        </button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    margin-bottom: 1.25rem;
    flex-wrap: wrap;
  }

  h1 {
    font-size: 1.4rem;
    font-weight: 600;
    margin: 0;
    display: flex;
    align-items: baseline;
    gap: 0.5rem;
  }

  .count {
    font-size: 0.8rem;
    font-weight: 500;
    color: var(--muted);
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
