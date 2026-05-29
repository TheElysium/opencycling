<script lang="ts">
  import { ble } from '$lib/ble.svelte';

  let canStart = $derived(ble.trainerStatus === 'connected');
</script>

<div class="page">
  <h1>Workout detail</h1>
  <p class="placeholder">Coming soon — workout detail.</p>

  <button disabled={!canStart} class="btn-start">
    Start session
  </button>
  {#if !canStart}
    <p class="hint">Connect your trainer to start.</p>
  {/if}
</div>

<style>
  h1 { font-size: 1.4rem; font-weight: 600; margin: 0 0 1rem; }
  .placeholder { color: var(--muted); margin-bottom: 1.5rem; }
  .btn-start {
    background: var(--accent);
    color: #fff;
    border: none;
    border-radius: 7px;
    padding: 0.6rem 1.5rem;
    font-size: 0.95rem;
    cursor: pointer;
  }
  .btn-start:disabled { opacity: 0.4; cursor: default; }
  .hint { font-size: 0.85rem; color: var(--muted); margin-top: 0.5rem; }
</style>
