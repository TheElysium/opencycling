<script lang="ts">
  import type { PlanDay, PlanEntryView } from '$lib/bindings';
  import type { WorkoutType } from '$lib/metrics';
  import { dayOfMonth } from '$lib/plan-date';
  import { workoutTypeColor } from '$lib/metrics';
  import { Play, CircleCheck } from '@lucide/svelte';

  type Props = {
    day: PlanDay;
    readonly?: boolean;
    onopen?: (date: string, entries: PlanEntryView[]) => void;
    onstart?: (entry: PlanEntryView) => void;
    /** Absent when unindexed or the classification is unknown: no dot is drawn. */
    intensities?: Map<number, WorkoutType | null>;
  };

  let { day, readonly = false, onopen, onstart, intensities }: Props = $props();

  function startClick(event: MouseEvent, entry: PlanEntryView) {
    // The entry sits inside the cell's own click target: stop it from also opening the picker.
    event.stopPropagation();
    onstart?.(entry);
  }

  function cellKeydown(event: KeyboardEvent) {
    // Keydown bubbles from the nested Start button: only the div's own keydown should open the picker.
    if (event.target !== event.currentTarget) return;
    if (event.key !== 'Enter' && event.key !== ' ') return;
    event.preventDefault();
    onopen?.(day.date, day.entries);
  }
</script>

{#snippet readonlyEntries()}
  <span class="entries">
    {#each day.entries as entry (entry.entry_id)}
      <!-- Gate on the file: early rows stored an empty name and must not read as a rest day. -->
      {#if entry.file_name}
        <span class="entry" class:missing={entry.missing} title={entry.file_name}>
          {entry.workout_name || entry.file_name}
        </span>
      {/if}
      {#if entry.note}
        <span class="entry note" title={entry.note}>{entry.note}</span>
      {/if}
    {/each}
  </span>
{/snippet}

{#if readonly}
  <div class="cell readonly" class:today={day.marker === 'Today'} class:past={day.marker === 'Past'}>
    <span class="date">{dayOfMonth(day.date)}</span>
    {@render readonlyEntries()}
  </div>
{:else}
  <div
    class="cell"
    class:today={day.marker === 'Today'}
    class:past={day.marker === 'Past'}
    role="button"
    tabindex="0"
    onclick={() => onopen?.(day.date, day.entries)}
    onkeydown={cellKeydown}
    aria-label="Edit {day.date}"
  >
    <span class="date">{dayOfMonth(day.date)}</span>
    <span class="entries">
      {#each day.entries as entry (entry.entry_id)}
        {#if entry.file_name}
          {@const type = intensities?.get(entry.entry_id)}
          <span class="entry-row">
            {#if type}
              <span class="intensity-dot" style="background: {workoutTypeColor(type)}"></span>
            {/if}
            <span class="entry" class:missing={entry.missing} title={entry.file_name}>
              {entry.workout_name || entry.file_name}
            </span>
            {#if entry.session_id != null}
              <CircleCheck class="done-badge" size={12} strokeWidth={2.5} />
            {/if}
            {#if !entry.missing}
              <button
                type="button"
                class="start-btn"
                aria-label="Start {entry.workout_name || entry.file_name}"
                onclick={(event) => startClick(event, entry)}
              >
                <Play size={14} strokeWidth={2.5} />
              </button>
            {/if}
          </span>
        {/if}
        {#if entry.note}
          <span class="entry note" title={entry.note}>{entry.note}</span>
        {/if}
      {/each}
    </span>
  </div>
{/if}

<style>
  .cell {
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    aspect-ratio: 1;
    padding: 0.4rem;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--surface);
    cursor: pointer;
    transition: border-color 0.15s, background 0.15s;
    text-align: left;
    font: inherit;
    width: 100%;
  }

  .cell:not(.readonly):hover {
    border-color: var(--accent);
    background: color-mix(in srgb, var(--accent) 6%, transparent);
  }

  .cell.readonly {
    cursor: default;
    opacity: 0.85;
  }

  .date {
    align-self: flex-end;
    font-size: 0.8rem;
    font-weight: 500;
    color: var(--text);
  }

  .today {
    background: var(--accent);
    border-color: var(--accent);
  }
  .today .date { color: #fff; }
  .today .entry { color: #fff; }
  .today .entry.missing { color: #ffdad8; }

  .past { opacity: 0.55; }

  .entries {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    min-width: 0;
    /* A long note must not stretch the square cell: it truncates instead. */
    overflow: hidden;
  }

  .entry {
    font-size: 0.75rem;
    font-weight: 600;
    color: var(--text);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .entry.note {
    color: var(--muted);
    font-style: italic;
    font-weight: 400;
  }

  .today .entry.note { color: #e8eefc; }

  .entry.missing {
    color: var(--danger);
    text-decoration: line-through;
  }

  .entry-row {
    display: flex;
    align-items: center;
    gap: 0.25rem;
    min-width: 0;
  }

  .entry-row .entry {
    flex: 1;
    min-width: 0;
  }

  .intensity-dot {
    flex-shrink: 0;
    width: 0.5rem;
    height: 0.5rem;
    border-radius: 50%;
  }

  .start-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    width: 1.35rem;
    height: 1.35rem;
    padding: 0;
    border: none;
    border-radius: 50%;
    background: var(--accent);
    color: #fff;
    cursor: pointer;
    transition: filter 0.15s;
  }
  .start-btn:hover { filter: brightness(1.1); }
  .today .start-btn {
    background: color-mix(in srgb, #fff 30%, transparent);
    color: #fff;
  }

  :global(.done-badge) {
    flex-shrink: 0;
    color: var(--success);
  }
  .today :global(.done-badge) { color: #fff; }
</style>
