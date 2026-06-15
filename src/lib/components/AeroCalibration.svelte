<script lang="ts">
  import { onMount } from 'svelte';
  import { aero } from '$lib/aero.svelte';

  let { done }: { done: () => void } = $props();
  let video: HTMLVideoElement;

  onMount(() => {
    aero.init(video);
    // Camera stays alive for the live loop; teardown happens on session end.
  });

  let busy = $derived(
    aero.phase === 'loading' || aero.phase === 'capturingAero' || aero.phase === 'capturingUpright'
  );

  async function captureAero() { await aero.capture('aero'); }
  async function captureUpright() { await aero.capture('upright'); }
  function finish() { done(); }
</script>

<div class="cal-overlay">
  <div class="cal-card">
    <h2>Aero calibration</h2>
    <p class="sub">Face the camera. Hold each position for 3s so we can learn your aero vs upright.</p>

    <!-- mirror like a selfie -->
    <video bind:this={video} class="preview" playsinline muted></video>

    {#if aero.phase === 'error'}
      <p class="err">Camera/model error: {aero.error}</p>
    {/if}

    <div class="row">
      <button disabled={busy} onclick={captureAero}>
        Capture AERO (3s)
      </button>
      <button disabled={busy} onclick={captureUpright}>
        Capture UPRIGHT (3s)
      </button>
    </div>

    {#if aero.sep > 0}
      <p class="sep">
        Separation: {aero.sep.toFixed(2)} σ
        {aero.strong ? '✓' : '— too weak, exaggerate the difference and recapture'}
      </p>
    {/if}

    <div class="actions">
      <button class="ghost" onclick={finish}>Skip aero</button>
      <button class="primary" disabled={aero.phase !== 'calibrated'} onclick={finish}>
        Start session
      </button>
    </div>
  </div>
</div>

<style>
  /* Match the .waiting-overlay / .waiting-card treatment from the session page. */
  .cal-overlay { position: fixed; inset: 0; background: rgba(15,23,42,.6); backdrop-filter: blur(4px); display: grid; place-items: center; z-index: 120; }
  .cal-card { background: var(--surface); border: 1px solid var(--border); border-radius: 12px; padding: 1.5rem; width: min(560px, 92vw); box-shadow: 0 10px 30px rgba(0,0,0,.2); }
  .preview { width: 100%; border-radius: 10px; transform: scaleX(-1); background: #000; margin: .75rem 0; }
  .row, .actions { display: grid; grid-template-columns: 1fr 1fr; gap: .6rem; }
  .actions { margin-top: 1rem; }
  .sub { color: var(--muted); }
  .err { color: var(--danger); }
  .sep { font-variant-numeric: tabular-nums; }
</style>
