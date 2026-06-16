<script lang="ts">
  import { onMount } from 'svelte';
  import { Wind } from '@lucide/svelte';
  import { aero } from '$lib/aero.svelte';

  // Host the live camera for the session loop: the calibration overlay's <video>
  // is gone by now, so the pose loop needs an in-DOM element to read frames from.
  let video: HTMLVideoElement;
  onMount(() => aero.attachVideo(video));

  // Live debounced state: green only while a usable frame is scored as aero.
  let active = $derived(aero.currentScore != null && aero.inAero);
</script>

<div class="card metric">
  <video bind:this={video} class="hidden-cam" playsinline muted aria-hidden="true"></video>
  <div class="lbl"><Wind size={12} aria-hidden="true" /> Aero position</div>
  <div class="val" class:active>AERO</div>
</div>

<style>
  .metric { text-align: center; position: relative; }
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
    font-size: 2.5rem;
    font-weight: 700;
    line-height: 1.1;
    color: var(--muted);
    transition: color 0.4s ease;
  }
  .val.active { color: #22c55e; }
  /* Off-screen but still playing so MoveNet keeps getting fresh frames. */
  .hidden-cam { position: absolute; width: 1px; height: 1px; opacity: 0; pointer-events: none; }
</style>
