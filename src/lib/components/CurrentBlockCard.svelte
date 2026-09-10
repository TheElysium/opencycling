<script lang="ts">
  import { SkipForward } from '@lucide/svelte';
  import { session, formatClock, flatBlockAvgPct, type SessionMetrics, type FlatBlock } from '$lib/session.svelte';
  import { zoneOf } from '$lib/metrics';
  import { tintBg } from '$lib/session-visuals';
  import { toMessage } from '$lib/format';

  let { metrics, flat_blocks }: { metrics: SessionMetrics; flat_blocks: FlatBlock[] } = $props();

  let block       = $derived<FlatBlock | undefined>(flat_blocks[metrics.current_block_idx]);
  let remainingS  = $derived(block ? Math.max(0, block.duration_s - metrics.current_block_elapsed_s) : 0);
  let durationS   = $derived(block?.duration_s ?? 0);
  let progressPct = $derived(durationS > 0 ? Math.min(100, (metrics.current_block_elapsed_s / durationS) * 100) : 0);
  let targetW     = $derived(metrics.target_w);
  let cadenceT    = $derived(metrics.cadence_target_rpm);
  let zone        = $derived(block ? zoneOf(flatBlockAvgPct(block, metrics.ftp_w)) : 1);
  let isRamping        = $derived(metrics.state === 'Ramping' && metrics.ramp_remaining_s != null);
  let isPausedByStall  = $derived(metrics.state === 'Paused' && metrics.paused_by_stall);
  let isStalled        = $derived(isPausedByStall || (isRamping && metrics.ramp_stalled));
  // Mirror: RAMP_S in src-tauri/src/session/state.rs (ramp total duration).
  const RAMP_TOTAL_S = 15;
  // While ramping, the main bar tracks ramp completion instead of block completion.
  let displayPct = $derived(
    isRamping && metrics.ramp_remaining_s != null
      ? ((RAMP_TOTAL_S - metrics.ramp_remaining_s) / RAMP_TOTAL_S) * 100
      : progressPct
  );
  let barColor = $derived(isStalled ? 'var(--warning)' : `var(--z${zone})`);

  let skipError = $state<string | null>(null);
  async function onSkip() {
    try { await session.skip(); skipError = null; }
    catch (e) { skipError = toMessage(e); }
  }
</script>

{#if !block}
  <div class="card current-block skeleton">
    <div class="info">
      <div class="label">Current block</div>
      <h2 class="name">…</h2>
    </div>
  </div>
{:else}
  <div
    class="card current-block"
    style="--zone-color: var(--z{zone}); background: {tintBg(zone)};"
  >
    <div class="info">
      <div class="label">Current block · {metrics.current_block_idx + 1} / {metrics.blocks_total}</div>
      <h2 class="name">{block.label}</h2>
      <div class="target">
        {#if targetW != null}<strong>{targetW} W</strong>{:else}—{/if}
        {#if cadenceT != null} · <strong>{cadenceT} rpm</strong>{/if}
      </div>
    </div>
    <button class="skip-btn" onclick={onSkip} aria-label="Skip block">
      Skip <SkipForward size={14} />
    </button>
    <div
      class="remaining"
      class:resuming={isRamping && !isStalled}
      class:stalled={isStalled}
      aria-live={isStalled ? 'assertive' : 'off'}
    >
      {#if isStalled}
        Dropped out! Start pedaling again to resume
      {:else if isRamping}
        Resuming… <strong>{Math.ceil(metrics.ramp_remaining_s ?? 0)}s</strong>
      {:else}
        Remaining <strong>{formatClock(remainingS)}</strong> / {formatClock(durationS)}
      {/if}
    </div>
    <div class="progress"><div style="width: {displayPct}%; background: {barColor};"></div></div>
    {#if skipError}<div class="skip-err">{skipError}</div>{/if}
  </div>
{/if}

<style>
  .current-block {
    border-left: 6px solid var(--zone-color);
    display: grid;
    grid-template-columns: 1fr auto;
    grid-template-rows: auto auto auto;
    gap: 0.5rem 1rem;
    align-items: center;
  }
  .current-block.skeleton {
    opacity: 0.6;
    border-left-color: var(--border);
  }
  .label {
    font-size: 0.75rem;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--muted);
  }
  .name {
    font-size: 1.4rem;
    font-weight: 600;
    margin: 0.15rem 0 0.2rem;
  }
  .target {
    color: var(--muted);
    font-size: 0.95rem;
  }
  .target strong { color: var(--text); }
  .remaining {
    font-variant-numeric: tabular-nums;
    font-size: 1.1rem;
    font-weight: 500;
  }
  .remaining.resuming { color: var(--accent); }
  .remaining.stalled {
    color: var(--danger);
    font-weight: 700;
    font-size: 1.15rem;
    animation: pulse-stall 1.2s ease-in-out infinite;
  }
  @keyframes pulse-stall {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.5; }
  }
  @media (prefers-reduced-motion: reduce) {
    .remaining.stalled { animation: none; }
  }
  .skip-btn {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 0.5rem 0.9rem;
    cursor: pointer;
    font-size: 0.9rem;
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    color: var(--text);
  }
  .skip-btn:hover { background: var(--bg); }
  .progress {
    grid-column: 1 / -1;
    height: 8px;
    background: rgba(255, 255, 255, 0.7);
    border-radius: 999px;
    overflow: hidden;
    border: 1px solid rgba(0, 0, 0, 0.05);
  }
  .progress > div {
    height: 100%;
    /* Matches the 1s session_metrics tick so the bar glides continuously
       instead of jumping then holding still until the next tick. */
    transition: width 1s linear;
  }
  .skip-err {
    grid-column: 1 / -1;
    font-size: 0.8rem;
    color: var(--danger);
  }
</style>
