<script lang="ts">
  import { onMount } from 'svelte';
  import { HelpCircle } from '@lucide/svelte';
  import { aero } from '$lib/aero.svelte';
  import { calibQuality } from '$lib/aero';

  let { done }: { done: () => void } = $props();
  let video: HTMLVideoElement;

  onMount(() => {
    aero.init(video);
    // Camera stays alive for the live loop; teardown happens on session end.
  });

  // Help popover, same pattern as the workout-detail metrics help.
  let helpOpen = $state(false);
  function onDocClick(e: MouseEvent) {
    if (!(e.target as HTMLElement).closest('.metrics-help')) helpOpen = false;
  }
  $effect(() => {
    if (helpOpen) {
      document.addEventListener('click', onDocClick);
      return () => document.removeEventListener('click', onDocClick);
    }
  });

  // The auto run is in flight whenever we are in a prep/rec step.
  let running = $derived(
    aero.step === 'prepAero' || aero.step === 'recAero' ||
    aero.step === 'prepUpright' || aero.step === 'recUpright'
  );
  let capturing = $derived(aero.step === 'recAero' || aero.step === 'recUpright');
  let prepping = $derived(aero.step === 'prepAero' || aero.step === 'prepUpright');
  let target = $derived(
    aero.step === 'prepAero' || aero.step === 'recAero' ? 'AERO' : 'UPRIGHT'
  );
  let quality = $derived(aero.step === 'done' && !aero.notSeen ? calibQuality(aero.sep) : null);
  let calibrated = $derived(aero.step === 'done' && aero.phase === 'calibrated');

  // Once a usable calibration exists, run the live self-test so the rider can verify
  // the aero/upright split before starting. Torn down when leaving this state
  // (recalibrate / start / skip / unmount).
  $effect(() => {
    if (calibrated) {
      aero.startPreview();
      return () => aero.stopPreview();
    }
  });

  function start() { aero.runCalibration(); }
  function finish() { done(); }
</script>

<div class="cal-overlay">
  <div class="cal-card">
    <div class="head">
      <h2>Aero calibration</h2>
      <div class="metrics-help">
        <button
          type="button"
          class="help-btn"
          aria-label="How aero detection works"
          aria-expanded={helpOpen}
          onclick={() => (helpOpen = !helpOpen)}
        >
          <HelpCircle size={16} />
        </button>
        {#if helpOpen}
          <div class="help-popover" role="dialog">
            <dl>
              <dt>What we track</dt>
              <dd>Your head and shoulders. Keep both visible with the camera facing you.</dd>
              <dt>How calibration works</dt>
              <dd>Hold your aero position, then sit upright, so we learn the difference between the two.</dd>
              <dt>Privacy</dt>
              <dd>Everything runs locally on your machine. No video ever leaves the device.</dd>
            </dl>
          </div>
        {/if}
      </div>
    </div>
    <p class="sub">
      One tap, then hold each position while we learn your aero vs upright. Stay in
      frame and face the camera.
    </p>

    <!-- Fixed-size stage: the frame keeps its shape with or without a video stream. -->
    <div class="stage">
      <!-- mirror like a selfie -->
      <video bind:this={video} class="preview" playsinline muted></video>

      {#if prepping}
        <div class="cue">
          <div class="cue-label">Get into <strong>{target}</strong> position</div>
          <div class="count">{aero.countdown}</div>
          <div class="frame-pill" class:ok={aero.liveDetected}>
            {aero.liveDetected ? '✓ I can see you' : '✗ Get back in frame'}
          </div>
        </div>
      {:else if capturing}
        <div class="cue">
          <div class="cue-label">Hold <strong>{target}</strong>…</div>
          <div class="bar"><span style="width:{Math.round(aero.captureFrac * 100)}%"></span></div>
          <div class="frame-pill" class:ok={aero.liveDetected}>
            {aero.liveDetected ? '✓ Capturing' : '✗ Get back in frame, restarting'}
          </div>
        </div>
      {/if}
    </div>

    {#if aero.phase === 'loading'}
      <p class="muted">Starting camera…</p>
    {:else if aero.phase === 'error'}
      <p class="err">Camera/model error: {aero.error}</p>
    {/if}

    <!-- Verdict after a finished run -->
    {#if aero.step === 'done'}
      {#if aero.notSeen}
        <div class="verdict poor">
          <strong>Couldn't see you.</strong> Stay fully in frame, then recalibrate.
        </div>
      {:else if quality === 'good'}
        <div class="verdict good">
          <strong>Great calibration.</strong> Clear aero/upright split.
        </div>
      {:else if quality === 'fair'}
        <div class="verdict fair">
          <strong>Calibration OK.</strong> For best tracking, recalibrate with a bigger
          difference between the two positions.
        </div>
      {:else}
        <div class="verdict poor">
          <strong>Too weak.</strong> Exaggerate the difference between aero and upright,
          then recalibrate.
        </div>
      {/if}
    {/if}

    <!-- Live self-test: lean into aero, then sit up — the badge should follow you. -->
    {#if calibrated}
      <div class="tester">
        <div class="tester-head">Test it: get aero, then sit up. The badge should follow you.</div>
        <div
          class="pose-badge"
          class:aero={aero.currentScore != null && aero.inAero}
          class:upright={aero.currentScore != null && !aero.inAero}
        >
          {#if aero.currentScore == null}
            Out of frame
          {:else if aero.inAero}
            AERO
          {:else}
            UPRIGHT
          {/if}
        </div>
        <div class="score-bar" aria-hidden="true">
          <span style="width:{Math.round((aero.currentScore ?? 0) * 100)}%"></span>
        </div>
        <div class="score-ends"><span>Upright</span><span>Aero</span></div>
      </div>
    {/if}

    <div class="actions">
      {#if aero.step === 'idle'}
        <button class="btn-warning" onclick={finish}>Skip calibration</button>
        <button class="btn-primary" disabled={aero.phase !== 'ready'} onclick={start}>
          Start calibration
        </button>
      {:else if running}
        <button class="btn-warning" disabled>Skip calibration</button>
        <button class="btn-primary" disabled>Calibrating…</button>
      {:else}
        <!-- step === 'done' -->
        <button class="btn-secondary" onclick={start}>Recalibrate</button>
        <button class="btn-primary" disabled={aero.phase !== 'calibrated'} onclick={finish}>
          Start session <span class="arrow">→</span>
        </button>
      {/if}
    </div>
  </div>
</div>

<style>
  /* Match the .waiting-overlay / .waiting-card treatment from the session page. */
  .cal-overlay { position: fixed; inset: 0; background: rgba(15,23,42,.6); backdrop-filter: blur(4px); display: grid; place-items: center; z-index: 120; }
  .cal-card { background: var(--surface); border: 1px solid var(--border); border-radius: 12px; padding: 1.5rem; width: min(560px, 92vw); box-shadow: 0 10px 30px rgba(0,0,0,.2); }
  .head { display: flex; align-items: center; gap: .5rem; }
  .head h2 { margin: 0; }

  /* Help popover — same treatment as the workout-detail metrics help. */
  .metrics-help { position: relative; }
  .help-btn {
    background: none; border: none; color: var(--muted); padding: 0.15rem;
    border-radius: 50%; display: inline-flex; align-items: center; justify-content: center;
    cursor: pointer; transition: color 0.15s, background 0.15s;
  }
  .help-btn:hover { color: var(--text); background: var(--bg); }
  .help-popover {
    position: absolute; top: calc(100% + 0.5rem); left: 0; width: 320px;
    background: var(--surface); border: 1px solid var(--border); border-radius: 10px;
    padding: 0.85rem 1rem; box-shadow: 0 6px 24px rgba(0,0,0,0.12); z-index: 20;
  }
  .help-popover dl { margin: 0; display: flex; flex-direction: column; gap: 0.6rem; }
  .help-popover dt { font-size: 0.8rem; font-weight: 700; color: var(--text); }
  .help-popover dd { margin: 0.1rem 0 0; font-size: 0.8rem; color: var(--muted); line-height: 1.45; }

  /* Fixed aspect-ratio box so the camera area never collapses when the stream is off. */
  .stage { position: relative; width: 100%; aspect-ratio: 4 / 3; margin: .75rem 0; }
  .preview { width: 100%; height: 100%; object-fit: cover; border-radius: 10px; transform: scaleX(-1); background: #000; display: block; }
  /* Overlay sits on top of the mirrored video but is not itself mirrored. */
  .cue { position: absolute; inset: 0; display: grid; place-content: center; justify-items: center; gap: .75rem; text-align: center; color: #fff; background: rgba(15,23,42,.35); border-radius: 10px; }
  .cue-label { font-size: 1.1rem; text-shadow: 0 1px 4px rgba(0,0,0,.6); }
  .count { font-size: 4.5rem; font-weight: 800; line-height: 1; font-variant-numeric: tabular-nums; text-shadow: 0 2px 8px rgba(0,0,0,.6); }
  .bar { width: 70%; height: 10px; border-radius: 999px; background: rgba(255,255,255,.25); overflow: hidden; }
  .bar span { display: block; height: 100%; background: var(--accent); transition: width .1s linear; }
  .frame-pill { font-size: .85rem; padding: .25rem .6rem; border-radius: 999px; background: rgba(239,68,68,.85); }
  .frame-pill.ok { background: rgba(34,197,94,.85); }

  .verdict { margin-top: .75rem; padding: .6rem .75rem; border-radius: 8px; font-size: .9rem; }
  .verdict.good { background: color-mix(in srgb, #22c55e 18%, transparent); border: 1px solid #22c55e; }
  .verdict.fair { background: color-mix(in srgb, var(--warning) 18%, transparent); border: 1px solid var(--warning); }
  .verdict.poor { background: color-mix(in srgb, var(--danger) 18%, transparent); border: 1px solid var(--danger); }

  .tester { margin-top: .75rem; padding: .75rem; border: 1px solid var(--border); border-radius: 8px; background: var(--bg); }
  .tester-head { font-size: .85rem; color: var(--muted); margin-bottom: .5rem; }
  .pose-badge { text-align: center; font-size: 1.4rem; font-weight: 800; letter-spacing: .08em; padding: .4rem; border-radius: 6px; color: var(--muted); background: var(--surface); border: 1px solid var(--border); transition: background .15s, color .15s; }
  .pose-badge.aero { color: #fff; background: #22c55e; border-color: #22c55e; }
  .pose-badge.upright { color: #fff; background: var(--accent); border-color: var(--accent); }
  .score-bar { margin-top: .5rem; height: 8px; border-radius: 999px; background: var(--surface); border: 1px solid var(--border); overflow: hidden; }
  .score-bar span { display: block; height: 100%; background: linear-gradient(90deg, var(--accent), #22c55e); transition: width .1s linear; }
  .score-ends { display: flex; justify-content: space-between; font-size: .7rem; color: var(--muted); margin-top: .2rem; }

  .actions { display: grid; grid-template-columns: 1fr 1fr; gap: .6rem; margin-top: 1rem; }
  .arrow { font-weight: 700; }
  /* Yellow warning button for the skip action (no shared class for this variant). */
  .btn-warning { background: color-mix(in srgb, var(--warning) 15%, transparent); color: var(--warning); border: 1px solid color-mix(in srgb, var(--warning) 45%, transparent); }
  .btn-warning:hover:not(:disabled) { background: color-mix(in srgb, var(--warning) 25%, transparent); }

  .sub { color: var(--muted); }
  .muted { color: var(--muted); }
  .err { color: var(--danger); }
</style>
