<script lang="ts">
  import type { PlanDay, PlanEntryView } from '$lib/bindings';
  import { dayOfMonth } from '$lib/plan-date';

  type Props = {
    day: PlanDay;
    readonly?: boolean;
    onopen?: (date: string, entries: PlanEntryView[]) => void;
  };

  let { day, readonly = false, onopen }: Props = $props();
</script>

<!-- span blocks: a button only permits phrasing content -->
{#snippet entries()}
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
    {@render entries()}
  </div>
{:else}
  <button
    class="cell"
    class:today={day.marker === 'Today'}
    class:past={day.marker === 'Past'}
    onclick={() => onopen?.(day.date, day.entries)}
    aria-label="Edit {day.date}"
  >
    <span class="date">{dayOfMonth(day.date)}</span>
    {@render entries()}
  </button>
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
    gap: 0.1rem;
    min-width: 0;
    /* A long note must not stretch the square cell: it truncates instead. */
    overflow: hidden;
  }

  .entry {
    font-size: 0.65rem;
    font-weight: 500;
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
</style>
