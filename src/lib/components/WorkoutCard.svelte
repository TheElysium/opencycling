<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { FlatBlock, ParsedWorkout } from '$lib/bindings';
  import WorkoutThumb from './WorkoutThumb.svelte';
  import { formatDuration, totalDuration, displayWorkoutName } from '$lib/format';
  import { computeWorkoutMetrics, workoutTypeColor } from '$lib/metrics';

  // `ftpWatts` is the run FTP (workoutFtp), so an FTP test renders at its reference.
  // Pass `onclick` or `actions`, never both: actions hold buttons, which cannot nest in one.
  let {
    workout,
    flat,
    ftpWatts,
    chartHeight = 72,
    lastUsedLabel,
    onclick,
    actions,
  }: {
    workout: ParsedWorkout;
    flat: FlatBlock[];
    ftpWatts: number;
    chartHeight?: number;
    lastUsedLabel?: string;
    onclick?: () => void;
    actions?: Snippet;
  } = $props();

  let m = $derived(computeWorkoutMetrics(workout.workout_blocks, ftpWatts));
  let name = $derived(displayWorkoutName(workout.name));
</script>

{#snippet body()}
  <div class="card-chart">
    <WorkoutThumb blocks={flat} {ftpWatts} height={chartHeight} />
  </div>
  <div class="card-info">
    <div class="card-text">
      {#if workout.is_ftp_test}
        <span class="ftp-badge">FTP Test</span>
      {:else if m.tss > 0}
        <span class="type-badge" style="--type-color: {workoutTypeColor(m.type)}">
          <span class="type-dot"></span>{m.type}
        </span>
      {/if}
      <span class="name">{name}</span>
      <div class="card-meta">
        <span>{formatDuration(totalDuration(workout.workout_blocks))}</span>
        {#if m.tss > 0 && !workout.is_ftp_test}
          <span class="dot-sep">·</span>
          <span title="Training Stress Score">{Math.round(m.tss)} TSS</span>
          <span class="dot-sep">·</span>
          <span title="Intensity Factor">{m.if_.toFixed(2)} IF</span>
        {/if}
        {#if lastUsedLabel !== undefined}
          <span class="dot-sep">·</span>
          <span class="last-used">{lastUsedLabel}</span>
        {/if}
      </div>
      {#if workout.tags.length > 0}
        <div class="tag-pills">
          {#each workout.tags as tag (tag)}
            <span class="tag-pill">{tag}</span>
          {/each}
        </div>
      {/if}
    </div>
    {#if actions}
      <div class="card-actions">{@render actions()}</div>
    {/if}
  </div>
{/snippet}

{#if onclick}
  <button class="workout-card clickable" {onclick}>{@render body()}</button>
{:else}
  <div class="workout-card">{@render body()}</div>
{/if}

<style>
  .workout-card {
    display: flex;
    flex-direction: column;
    width: 100%;
    min-width: 0;
    text-align: left;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 10px;
    overflow: hidden;
    padding: 0;
    transition: border-color 0.15s, box-shadow 0.15s, transform 0.15s;
  }

  .clickable {
    cursor: pointer;
  }

  .clickable:hover {
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
    display: flex;
    align-items: center;
    gap: 1rem;
    padding: 1rem 1.15rem 1.1rem;
  }

  .card-text {
    flex: 1;
    min-width: 0;
  }

  .card-actions {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    flex-shrink: 0;
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
    margin-bottom: 0.5rem;
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
    gap: 0.3rem;
    margin-top: 0.75rem;
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
    margin-bottom: 0.5rem;
  }

  .name {
    display: block;
    font-weight: 600;
    font-size: 0.95rem;
    margin-bottom: 0.5rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .card-meta {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.45rem;
    font-size: 0.78rem;
    color: var(--muted);
  }

  .dot-sep {
    opacity: 0.6;
  }

  .last-used {
    margin-left: auto;
  }
</style>
