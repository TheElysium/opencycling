<script lang="ts">
  import {
    formatClock, totalFlatDuration,
    type FlatBlock, type SessionMetrics,
  } from '$lib/session.svelte';
  import { stateClass, zoneBg } from '$lib/session-visuals';

  let { flat_blocks, metrics }: { flat_blocks: FlatBlock[]; metrics: SessionMetrics } = $props();

  let totalS  = $derived(totalFlatDuration(flat_blocks));
  let elapsed = $derived(Math.min(metrics.total_elapsed_s, totalS));
  let remain  = $derived(Math.max(0, totalS - elapsed));
  let progPct = $derived(totalS > 0 ? (elapsed / totalS) * 100 : 0);
  let cols    = $derived(flat_blocks.map(b => `minmax(2px, ${b.duration_s}fr)`).join(' '));
</script>

<div class="card timeline-card">
  <div class="timeline-head">
    <span class="time"><strong>{formatClock(elapsed)}</strong> / {formatClock(totalS)}</span>
    <span class="time">remaining <strong>{formatClock(remain)}</strong></span>
  </div>
  <div class="timeline" style="grid-template-columns: {cols};">
    {#each flat_blocks as b, i}
      <div class={stateClass(i, metrics.current_block_idx)} style:background={zoneBg(b, metrics.ftp_w, 'to right')}></div>
    {/each}
  </div>
  <div class="session-progress"><div style="width: {progPct}%;"></div></div>
</div>

<style>
  .timeline-card {
    display: grid;
    gap: 0.5rem;
  }
  .timeline-head {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    font-variant-numeric: tabular-nums;
  }
  .time {
    font-size: 0.95rem;
    color: var(--muted);
  }
  .time strong { color: var(--text); }
  .timeline {
    display: grid;
    gap: 2px;
    height: 22px;
  }
  .timeline > div {
    border-radius: 2px;
    opacity: 0.5;
  }
  .timeline > div.done   { opacity: 0.2; }
  .timeline > div.active { opacity: 1; }
  .session-progress {
    height: 5px;
    background: var(--border);
    border-radius: 999px;
    overflow: hidden;
  }
  .session-progress > div {
    height: 100%;
    background: var(--accent);
    transition: width 0.4s linear;
  }
</style>
