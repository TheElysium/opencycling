<script lang="ts">
  import { aero } from '$lib/aero.svelte';

  let pct = $derived(Math.round(aero.aeroPct * 100));
  // Use the debounced gate state, not the raw score, so the tile matches what is
  // persisted. `unknown` while no usable frame is being scored.
  let state = $derived(
    aero.currentScore == null ? 'unknown' : aero.inAero ? 'aero' : 'upright'
  );
</script>

<div class="aero-tile" class:alert={aero.situpAlert}>
  <div class="label">Aero</div>
  <div class="state {state}">{state.toUpperCase()}</div>
  <div class="pct">{pct}% <span class="unit">in aero</span></div>
</div>

<style>
  .aero-tile { background: var(--surface); border: 1px solid var(--border); border-radius: 10px; padding: 1rem; }
  .aero-tile.alert { border-color: var(--danger); background: #fef2f2; }
  .label { font-size: .75rem; text-transform: uppercase; letter-spacing: .06em; color: var(--muted); }
  .state.aero { color: var(--accent); }
  .state.upright { color: var(--danger); }
  .state.unknown { color: var(--muted); }
  .state { font-size: 1.4rem; font-weight: 700; }
  .pct { font-variant-numeric: tabular-nums; }
  .unit { color: var(--muted); font-size: .85rem; }
</style>
