<script lang="ts">
  import type { Component } from 'svelte';
  import { targetKind, kindColor } from '$lib/session-visuals';

  let { label, value, unit, target = null, icon: Icon = undefined, status = null, onretry = null }:
    { label: string; value: number | null; unit: string; target?: number | null; icon?: Component; status?: string | null; onretry?: (() => void) | null } = $props();

  let color = $derived(kindColor(targetKind(value, target)));
</script>

<div class="card metric">
  <div class="lbl">
    {#if Icon}<Icon size={12} aria-hidden="true" />{/if}
    {label}
  </div>
  {#if status}
    <div class="status-body">
      <div class="status"><span class="status-dot" aria-hidden="true"></span>{status}</div>
      {#if onretry}
        <button class="retry" type="button" onclick={onretry} aria-label="Retry {label}">Retry</button>
      {/if}
    </div>
  {:else}
    <div class="val" style="color: {color};">{value ?? '—'}</div>
    <div class="unit">{unit}</div>
  {/if}
</div>

<style>
  .metric { text-align: center; }
  .lbl {
    font-size: 0.75rem;
    letter-spacing: 0.12em;
    color: var(--muted);
    text-transform: uppercase;
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
  }
  .val {
    font-size: 3rem;
    font-weight: 600;
    line-height: 1.1;
    font-variant-numeric: tabular-nums;
  }
  .unit {
    font-size: 0.75rem;
    color: var(--muted);
    letter-spacing: 0.1em;
    text-transform: uppercase;
  }
  /* Replaces value + unit while a device is reconnecting / unavailable. Sized to
     keep the tile height stable next to the 3rem value of sibling tiles. */
  .status {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.45rem;
    min-height: calc(3rem * 1.1 + 0.75rem);
    font-size: 0.9rem;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--muted);
  }
  .status-body {
    display: flex;
    flex-direction: column;
    align-items: center;
  }
  /* Discreet ghost action under the status line (only wired by callers passing
     `onretry`, e.g. the failed-reconnect state of the Heart rate tile). */
  .retry {
    margin-top: 0.2rem;
    padding: 0.1rem 0.55rem;
    font-size: 0.65rem;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--muted);
    background: none;
    border: 1px solid var(--border);
    border-radius: 6px;
    cursor: pointer;
    transition: color 0.15s, border-color 0.15s;
  }
  .retry:hover {
    color: var(--text);
    border-color: var(--text);
  }
  .status-dot {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    background: var(--status-warn);
    animation: status-pulse 1.1s ease-in-out infinite;
  }
  @keyframes status-pulse {
    0%, 100% { opacity: 1; transform: scale(1); }
    50%      { opacity: 0.35; transform: scale(0.7); }
  }
</style>
