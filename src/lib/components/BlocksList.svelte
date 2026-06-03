<script lang="ts">
  import { tick } from 'svelte';
  import {
    formatClock, flatBlockAvgPct, flatBlockStartPct, flatBlockEndPct, isRamp,
    type FlatBlock, type SessionMetrics,
  } from '$lib/session.svelte';
  import { zoneOf } from '$lib/metrics';
  import { stateClass, zoneBg, tintBg, pctRound } from '$lib/session-visuals';

  let { flat_blocks, metrics }: { flat_blocks: FlatBlock[]; metrics: SessionMetrics } = $props();

  function metaLine(b: FlatBlock): string {
    const ftp = metrics.ftp_w;
    const cad = b.cadence_rpm != null ? `${b.cadence_rpm} rpm` : '— rpm';
    if (isRamp(b)) {
      return `${pctRound(flatBlockStartPct(b, ftp))}% → ${pctRound(flatBlockEndPct(b, ftp))}% FTP · ${cad}`;
    }
    return `${pctRound(flatBlockAvgPct(b, ftp))}% FTP · ${cad}`;
  }

  let listEl = $state<HTMLDivElement | undefined>();

  // Auto-scroll: keep current block in view with the 2 previous blocks above it.
  $effect(() => {
    const idx = metrics.current_block_idx;
    const total = flat_blocks.length;
    if (total === 0) return;
    (async () => {
      await tick();
      const list = listEl;
      if (!list) return;
      const target = Math.max(0, idx - 2);
      const row = list.querySelector<HTMLDivElement>(`[data-idx="${target}"]`);
      if (!row) return;
      const top = row.getBoundingClientRect().top - list.getBoundingClientRect().top + list.scrollTop;
      list.scrollTo({ top, behavior: 'smooth' });
    })();
  });
</script>

<div class="card blocks-list" bind:this={listEl}>
  {#each flat_blocks as b, i (i)}
    {@const cls = stateClass(i, metrics.current_block_idx)}
    {@const z = zoneOf(flatBlockAvgPct(b, metrics.ftp_w))}
    <div
      data-idx={i}
      class="block-row {cls}"
      style:--zone-color={`var(--z${z})`}
      style:background={cls === 'active' ? tintBg(z) : 'var(--surface)'}
    >
      <div class="swatch" style:background={zoneBg(b, metrics.ftp_w, 'to bottom')}></div>
      <div class="info">
        <div class="name">{b.label}</div>
        <div class="meta">{metaLine(b)}</div>
      </div>
      <div class="dur">{formatClock(b.duration_s)}</div>
    </div>
  {/each}
</div>

<style>
  .blocks-list {
    overflow-y: auto;
    display: grid;
    gap: 0.5rem;
    align-content: start;
    scroll-behavior: smooth;
    scrollbar-width: thin;
    scrollbar-color: rgba(148, 163, 184, 0.35) transparent;
  }
  .blocks-list::-webkit-scrollbar { width: 6px; }
  .blocks-list::-webkit-scrollbar-track { background: transparent; }
  .blocks-list::-webkit-scrollbar-thumb {
    background: rgba(148, 163, 184, 0.35);
    border-radius: 999px;
  }
  .blocks-list::-webkit-scrollbar-thumb:hover {
    background: rgba(148, 163, 184, 0.55);
  }
  .block-row {
    display: grid;
    grid-template-columns: 6px 1fr auto;
    gap: 0.7rem;
    align-items: center;
    padding: 0.6rem 0.7rem;
    border: 1px solid var(--border);
    border-radius: 8px;
  }
  .block-row.active { border-color: var(--zone-color); }
  .block-row.done   { opacity: 0.5; }
  .swatch {
    width: 6px;
    height: 100%;
    min-height: 30px;
    border-radius: 3px;
  }
  .info { display: grid; gap: 2px; }
  .info .name {
    font-size: 0.9rem;
    font-weight: 500;
  }
  .info .meta {
    font-size: 0.75rem;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }
  .dur {
    font-size: 0.85rem;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }
</style>
