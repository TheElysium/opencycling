<script lang="ts" module>
  import type { Component } from 'svelte';

  export type MetricTile = {
    /** Optional Lucide icon component shown next to the label */
    icon?: Component<{ size?: number }>;
    /** Top-line family label (e.g. "Avg Power", "Stress") */
    label: string;
    /** Primary value (number or pre-formatted string) */
    value: string | number;
    /** Optional unit suffix (e.g. "W", "kJ", "bpm") rendered small */
    unit?: string;
    /** Optional secondary line below the value. `value` is rendered bold; `label` and `unit` are plain. */
    secondary?: { label?: string; value?: string | number; unit?: string };
    /** Optional tooltip for the whole cell */
    title?: string;
  };
</script>

<script lang="ts">
  import type { Snippet } from 'svelte';

  let { tiles, trailing }: {
    tiles: MetricTile[];
    /** Optional content (e.g. help button) rendered after the last metric */
    trailing?: Snippet;
  } = $props();
</script>

<div class="metrics-strip" style="--cells: {tiles.length};">
  {#each tiles as t}
    <div class="metric-tile" title={t.title ?? ''}>
      <span class="metric-family">
        {#if t.icon}
          {@const Icon = t.icon}
          <Icon size={12} />
        {/if}
        {t.label}
      </span>
      <span class="metric-primary">
        {t.value}{#if t.unit}<span class="metric-unit">{t.unit}</span>{/if}
      </span>
      {#if t.secondary}
        <span class="metric-secondary">
          {#if t.secondary.label}{t.secondary.label}{/if}{#if t.secondary.value != null} <strong>{t.secondary.value}</strong>{/if}{#if t.secondary.unit} {t.secondary.unit}{/if}
        </span>
      {/if}
    </div>
  {/each}

  {#if trailing}
    <div class="metric-trailing">
      {@render trailing()}
    </div>
  {/if}
</div>

<style>
  .metrics-strip {
    display: grid;
    grid-template-columns: repeat(var(--cells), 1fr) auto;
    gap: 0;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 0.65rem 1rem;
    margin-bottom: 1rem;
  }

  .metric-tile {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    padding: 0 0.85rem;
    border-left: 1px solid var(--border);
    min-width: 0;
  }

  .metric-tile:first-child {
    border-left: none;
    padding-left: 0;
  }

  .metric-family {
    font-size: 0.62rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--muted);
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    white-space: nowrap;
  }

  .metric-primary {
    font-size: 1.35rem;
    font-weight: 600;
    line-height: 1.1;
    font-variant-numeric: tabular-nums;
    color: var(--text);
    letter-spacing: -0.01em;
  }

  .metric-unit {
    font-size: 0.78rem;
    font-weight: 500;
    color: var(--muted);
    margin-left: 0.18rem;
    letter-spacing: 0;
  }

  .metric-secondary {
    font-size: 0.75rem;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
    font-weight: 500;
    letter-spacing: 0.04em;
  }

  .metric-secondary :global(strong) {
    color: var(--text);
    font-weight: 600;
  }

  .metric-trailing {
    display: flex;
    align-items: flex-start;
    padding-left: 0.5rem;
    border-left: 1px solid var(--border);
  }
</style>
