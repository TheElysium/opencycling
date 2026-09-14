<script lang="ts">
  import type { WeekLoad } from '$lib/plan-load';
  import { loadBarRatio } from '$lib/plan-load';
  import { formatDuration } from '$lib/format';

  type Props = {
    load: WeekLoad;
    maxTss: number;
  };

  let { load, maxTss }: Props = $props();

  let ratio = $derived(loadBarRatio(load.tss, maxTss));
  // Rounding first: a sub-0.5 TSS week must read as no load, not as "0 TSS".
  let tssShown = $derived(Math.round(load.tss));
  let missingTitle = $derived(
    load.missing === 1
      ? '1 planned workout is missing from this total'
      : `${load.missing} planned workouts are missing from this total`,
  );
</script>

{#if load.workouts > 0 || load.missing > 0}
  <span class="summary">
    {#if load.durationS > 0}
      <span class="duration">{formatDuration(load.durationS)}</span>
    {/if}
    {#if tssShown > 0}
      <span class="tss">{tssShown} TSS</span>
      <span class="bar"><span class="bar-fill" style:width="{ratio * 100}%"></span></span>
    {/if}
    {#if load.missing > 0}
      <span class="incomplete" title={missingTitle}>*</span>
    {/if}
  </span>
{/if}

<style>
  .summary {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    font-size: 0.75rem;
    color: var(--muted);
  }

  .duration,
  .tss {
    white-space: nowrap;
  }

  .bar {
    display: inline-block;
    width: 3rem;
    height: 0.35rem;
    border-radius: 999px;
    background: color-mix(in srgb, var(--muted) 20%, transparent);
    overflow: hidden;
  }

  .bar-fill {
    display: block;
    height: 100%;
    background: var(--accent);
    border-radius: inherit;
  }

  .incomplete {
    color: var(--danger);
    font-weight: 700;
  }
</style>
