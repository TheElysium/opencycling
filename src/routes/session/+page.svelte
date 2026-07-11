<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { commands } from '$lib/bindings';
  import { goto, beforeNavigate } from '$app/navigation';
  import { Pause, Play, Square, ArrowLeft, ArrowRight, RotateCw, Heart } from '@lucide/svelte';
  import { confirm } from '@tauri-apps/plugin-dialog';
  import { session, type SessionMetrics } from '$lib/session.svelte';
  import { ble } from '$lib/ble.svelte';
  import { aero } from '$lib/aero.svelte';
  import AeroCalibration from '$lib/components/AeroCalibration.svelte';
  import AeroPanel from '$lib/components/AeroPanel.svelte';
  import { beepShort, beepLong, beepLow } from '$lib/audio';
  import { toMessage } from '$lib/format';
  import { type SessionDetail } from '$lib/db';
  import { getSettings } from '$lib/settings';
  import { stravaStatus, uploadSessionToStrava } from '$lib/strava';
  import CurrentBlockCard from '$lib/components/CurrentBlockCard.svelte';
  import SessionFinishedCard from '$lib/components/SessionFinishedCard.svelte';
  import PowerTile from '$lib/components/PowerTile.svelte';
  import MetricTile from '$lib/components/MetricTile.svelte';
  import SessionTimeline from '$lib/components/SessionTimeline.svelte';
  import BlocksList from '$lib/components/BlocksList.svelte';
  import SessionStatsPanel from '$lib/components/SessionStatsPanel.svelte';
  import FtpTestResult from '$lib/components/FtpTestResult.svelte';

  let snapError   = $state<string | null>(null);
  let detail      = $state<SessionDetail | null>(null);
  let maxHr       = $state(190);
  let oldFtp      = $state(0); // the rider's real FTP, captured before the result screen

  let calibrating = $state(false); // aero calibration overlay is up, session not yet armed
  let aeroActive  = $state(false); // live aero loop running for this session

  // HR sensor reconnect feedback, shown inline in the Heart rate tile (no toast).
  // Null while connected -> the tile shows the live bpm value.
  let hrStatus = $derived(
    ble.hrmReconnect?.status === 'reconnecting' ? 'Reconnecting…'
    : ble.hrmReconnect?.status === 'failed'     ? 'Unavailable'
    : null
  );

  async function startSessionFlow() {
    try {
      await session.startPending();
    } catch (e) {
      snapError = toMessage(e);
    }
  }

  // Fired by AeroCalibration for both "Start session" and "Skip aero". The session
  // is armed only now, so pedaling during calibration could not have started it.
  async function onCalibrationDone() {
    calibrating = false;
    await startSessionFlow();
    if (aero.phase === 'calibrated') {
      aeroActive = true;
      aero.startLoop();
    } else {
      // Skipped (or too-weak calibration): release the camera, no reporting.
      aero.teardown();
    }
  }

  onMount(async () => {
    if (session.hasPendingStart) {
      // Fresh start coming from the workout detail page.
      if (session.aeroEnabled) {
        calibrating = true; // start_session happens once calibration closes
        return;
      }
      await startSessionFlow();
      return;
    }
    // No pending start: re-entering an already-running session (e.g. reload).
    try {
      await session.loadSnapshot();
    } catch (e) {
      snapError = toMessage(e);
    }
  });

  // Stop the camera/loop when the session ends.
  $effect(() => {
    if (isFinished && aeroActive) {
      aero.teardown();
      aeroActive = false;
    }
  });

  // Safety net: release the camera if we leave the page for any reason.
  onDestroy(() => aero.teardown());

  let m          = $derived<SessionMetrics | null>(session.metrics);
  let isWaiting  = $derived(m?.state === 'WaitingForRider');
  let isPaused   = $derived(m?.state === 'Paused');
  let isFinished = $derived(m?.state === 'Finished');
  let isActive   = $derived(m != null && !isFinished);

  let prevState: SessionMetrics['state'] | null = null;
  let prevBlockIdx: number | null = null;
  let prevBlockRemaining: number | null = null;
  let autoUploaded = false;

  /// Fires once when a session finishes: uploads to Strava if connected and the
  /// auto-upload setting is on. The backend dedup guard covers any re-entry.
  async function maybeAutoUpload(sessionId: number | null) {
    if (sessionId == null || autoUploaded) return;
    autoUploaded = true;
    try {
      const status = await stravaStatus();
      if (status.connected && status.auto_upload) {
        await uploadSessionToStrava(sessionId);
      }
    } catch (e) {
      console.error('auto-upload failed', e);
    }
  }

  $effect(() => {
    if (!m) return;
    const curBlock = session.flat_blocks[m.current_block_idx];
    const remaining = curBlock ? curBlock.duration_s - m.current_block_elapsed_s : null;

    // A fresh session resets the one-shot auto-upload guard.
    if (m.state === 'WaitingForRider') autoUploaded = false;

    if (prevState !== null && prevState !== 'Running' && m.state === 'Running') beepLow();
    if (prevState !== null && prevState !== 'Finished' && m.state === 'Finished') {
      beepLow();
      maybeAutoUpload(m.session_id);
    }
    if (m.state === 'Running' && prevBlockIdx !== null && prevBlockIdx !== m.current_block_idx) beepLong();
    if (
      m.state === 'Running' &&
      remaining !== null && remaining >= 1 && remaining <= 3 &&
      remaining !== prevBlockRemaining
    ) beepShort();

    prevState = m.state;
    prevBlockIdx = m.current_block_idx;
    prevBlockRemaining = remaining;
  });

  $effect(() => {
    if (isFinished && m?.session_id != null && detail == null) {
      const id = m.session_id;
      (async () => {
        try {
          const [d, s] = await Promise.all([
            commands.getSession(id),
            getSettings(),
          ]);
          detail = d;
          maxHr = s.max_hr_bpm;
          oldFtp = s.ftp_w;
        } catch (e) {
          snapError = toMessage(e);
        }
      })();
    }
  });

  beforeNavigate(async (nav) => {
    if (!isActive) return;
    if (nav.to?.url.pathname === '/session') return;
    nav.cancel();
    const target = nav.to?.url;
    const ok = await confirm(
      'A session is in progress. Stop it before leaving this page?',
      { title: 'Stop session', kind: 'warning' },
    );
    if (!ok) return;
    try {
      await session.stop();
      if (target) await goto(target.pathname + target.search + target.hash);
    } catch (e) {
      snapError = toMessage(e);
    }
  });

  async function togglePause() {
    try {
      if (isPaused) await session.resume();
      else          await session.pause();
    } catch (e) {
      snapError = toMessage(e);
    }
  }

  async function onStop() {
    try {
      const ok = await confirm('Stop the current session? Progress so far will be saved to your history.', {
        title: 'Stop session',
        kind: 'warning',
      });
      if (!ok) return;
      await session.stop();
    } catch (e) {
      snapError = toMessage(e);
    }
  }

  // From the FTP-test stop prompt: stop directly (the prompt is already the confirmation).
  async function confirmStop() {
    try {
      await session.confirmStopFromPrompt();
    } catch (e) {
      snapError = toMessage(e);
    }
  }

  // From the trainer "failed" reconnect modal: stop directly (the modal is already the
  // decision point) and clear the affordance. Whatever was recorded is saved.
  async function stopFromReconnect() {
    try {
      await session.stop();
      ble.clearReconnect('Trainer');
    } catch (e) {
      snapError = toMessage(e);
    }
  }
</script>

<main class="session">
  {#if !m || session.flat_blocks.length === 0}
    <div class="empty">
      <p>No active session.</p>
      <button class="btn-secondary" onclick={() => goto('/workouts')}>
        <ArrowLeft size={16} /> Back to workouts
      </button>
    </div>
  {:else}
    <!-- LEFT -->
    <section class="left">
      {#if isFinished}
        {#if session.isFtpTest && detail}
          <FtpTestResult {detail} {oldFtp} />
        {:else}
          <SessionFinishedCard total_active_s={m.total_active_s} />
          {#if detail}
            <SessionStatsPanel {detail} {maxHr} />
          {:else}
            <p class="muted">Loading recap…</p>
          {/if}
        {/if}
      {:else}
        <CurrentBlockCard metrics={m} flat_blocks={session.flat_blocks} />
        <PowerTile power_w={m.power_w} target_w={m.target_w} />
        <div class="metrics" class:with-aero={aeroActive}>
          <MetricTile label="Cadence"    value={m.cadence_rpm} unit="rpm" target={m.cadence_target_rpm} icon={RotateCw} />
          <MetricTile label="Heart rate" value={m.hr_bpm}      unit="bpm" icon={Heart} status={hrStatus} />
          {#if aeroActive}<AeroPanel />{/if}
        </div>
      {/if}
    </section>

    <!-- RIGHT -->
    <section class="right">
      <SessionTimeline flat_blocks={session.flat_blocks} metrics={m} />
      <BlocksList     flat_blocks={session.flat_blocks} metrics={m} />

      <div class="controls">
        {#if isFinished}
          <button
            class="btn btn-primary full"
            disabled={m.session_id == null}
            onclick={() => m.session_id != null && goto(`/history/${m.session_id}`)}
          >
            View details <ArrowRight size={16} />
          </button>
        {:else}
          <button class="btn btn-pause" onclick={togglePause}>
            {#if isPaused}<Play size={16} /> Resume{:else}<Pause size={16} /> Pause{/if}
          </button>
          <button class="btn btn-stop" onclick={onStop}>
            <Square size={16} /> Stop
          </button>
        {/if}
      </div>
    </section>
  {/if}

  {#if isWaiting}
    <div class="waiting-overlay">
      <div class="waiting-card">
        <div class="waiting-title">Start pedaling to begin</div>
        <div class="waiting-sub">The session will start automatically when you start riding.</div>
      </div>
    </div>
  {/if}

  {#if session.dropCountdown != null && !session.stopPromptVisible}
    <div class="drop-countdown" aria-live="assertive">
      <div class="drop-count">{session.dropCountdown}</div>
      <div class="drop-msg">Push the watts or the test stops!</div>
    </div>
  {/if}

  {#if session.stopPromptVisible}
    <div class="stop-prompt-overlay">
      <div class="stop-prompt-card">
        <div class="stop-prompt-title">Can't hold the power?</div>
        <div class="stop-prompt-sub">
          You've been below target for a few seconds. End the ramp test now, or keep pushing.
        </div>
        <div class="stop-prompt-actions">
          <button class="btn btn-pause" onclick={() => session.dismissStopPrompt()}>
            Keep going
          </button>
          <button class="btn btn-stop" onclick={confirmStop}>
            <Square size={16} /> Stop test
          </button>
        </div>
      </div>
    </div>
  {/if}

  {#if ble.trainerReconnect && isActive}
    <div class="reconnect-overlay">
      <div class="reconnect-card">
        {#if ble.trainerReconnect.status === 'reconnecting'}
          <div class="reconnect-spinner" aria-hidden="true"></div>
          <div class="reconnect-title">Trainer disconnected</div>
          <div class="reconnect-sub">
            Reconnecting{ble.trainerReconnect.attempt > 0 ? `, attempt ${ble.trainerReconnect.attempt}` : ''}…
          </div>
          <div class="reconnect-hint">Your workout is paused and will resume automatically.</div>
        {:else if ble.trainerReconnect.status === 'reconnected'}
          <div class="reconnect-title ok">Trainer reconnected</div>
          <div class="reconnect-sub">Resuming your session…</div>
        {:else}
          <div class="reconnect-title fail">Couldn't reconnect the trainer</div>
          <div class="reconnect-sub">
            Automatic reconnection gave up. Retry, or stop the session. Everything recorded so far is saved.
          </div>
          <div class="reconnect-actions">
            <button class="btn btn-pause" onclick={() => ble.retryReconnect('Trainer')}>
              <RotateCw size={16} /> Retry
            </button>
            <button class="btn btn-stop" onclick={stopFromReconnect}>
              <Square size={16} /> Stop session
            </button>
          </div>
        {/if}
      </div>
    </div>
  {/if}

  {#if snapError}
    <div class="error-box error-floating">{snapError}</div>
  {/if}
</main>

{#if calibrating}
  <AeroCalibration done={onCalibrationDone} />
{/if}

<style>
  .session {
    display: grid;
    grid-template-columns: 2.4fr 1fr;
    gap: 1rem;
    padding: 0;
    height: 100vh;
    max-height: 100vh;
    width: 100vw;
    box-sizing: border-box;
    overflow: hidden;
  }
  .left  { padding: 1rem 0 1rem 1rem; overflow-y: auto; }
  .right { padding: 1rem 1rem 1rem 0; }

  .left, .right {
    display: grid;
    grid-template-rows: auto 1fr auto;
    gap: 1rem;
    min-height: 0;
    height: 100%;
  }

  .metrics {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 1rem;
  }
  /* Cadence + HR + Aero share one row when aero detection is on. */
  .metrics.with-aero {
    grid-template-columns: 1fr 1fr 1fr;
  }

  .controls {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0.7rem;
  }
  .btn {
    padding: 0.85rem;
    border-radius: 8px;
    border: 1px solid var(--border);
    background: var(--surface);
    cursor: pointer;
    font-size: 1rem;
    font-weight: 500;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.4rem;
    color: var(--text);
  }
  .btn-pause:hover { background: var(--bg); }
  .btn-stop {
    background: #fef2f2;
    border-color: #fecaca;
    color: var(--danger);
    grid-column: 2;
  }
  .btn-stop:hover { background: #fee2e2; }
  .btn-primary {
    background: var(--accent);
    color: white;
    border-color: var(--accent);
    font-weight: 600;
    box-shadow: 0 1px 2px rgba(0,0,0,0.05), 0 4px 14px rgba(59,130,246,0.3);
  }
  .btn-primary:hover:not(:disabled) { background: #2563eb; }
  .btn-primary:disabled { opacity: 0.5; cursor: not-allowed; }
  .btn.full { grid-column: 1 / -1; }

  .empty {
    grid-column: 1 / -1;
    display: grid;
    place-items: center;
    align-content: center;
    gap: 1rem;
    color: var(--muted);
  }
  .muted { color: var(--muted); }

  .error-floating {
    position: fixed;
    bottom: 1rem;
    right: 1rem;
    max-width: 340px;
    z-index: 50;
  }

  .waiting-overlay {
    position: fixed;
    inset: 0;
    /* Light scrim so the live session stays visible behind the prompt. */
    background: rgba(15, 23, 42, 0.25);
    backdrop-filter: blur(1.5px);
    display: grid;
    place-items: center;
    z-index: 100;
  }
  .waiting-card {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 12px;
    padding: 2rem 2.5rem;
    text-align: center;
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.2);
    max-width: 420px;
  }
  .waiting-title {
    font-size: 1.6rem;
    font-weight: 600;
    color: var(--text);
    margin-bottom: 0.6rem;
  }
  .waiting-sub {
    color: var(--muted);
    font-size: 0.95rem;
  }

  /* Non-blocking countdown shown while the rider is dropping below target. */
  .drop-countdown {
    position: fixed;
    top: 1.5rem;
    left: 50%;
    transform: translateX(-50%);
    z-index: 90;
    display: grid;
    justify-items: center;
    gap: 0.3rem;
    padding: 1rem 2rem;
    border-radius: 14px;
    background: rgba(127, 29, 29, 0.92);
    color: white;
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.35);
    pointer-events: none;
  }
  .drop-count {
    font-size: 3rem;
    font-weight: 800;
    line-height: 1;
    font-variant-numeric: tabular-nums;
    animation: drop-pulse 1s ease-in-out infinite;
  }
  .drop-msg {
    font-size: 0.9rem;
    font-weight: 600;
    letter-spacing: 0.02em;
  }
  @keyframes drop-pulse {
    0%, 100% { transform: scale(1);   opacity: 1;   }
    50%      { transform: scale(1.12); opacity: 0.85; }
  }

  /* Blocking prompt once the grace period elapses: the rider decides. */
  .stop-prompt-overlay {
    position: fixed;
    inset: 0;
    background: rgba(15, 23, 42, 0.45);
    backdrop-filter: blur(2px);
    display: grid;
    place-items: center;
    z-index: 110;
  }
  .stop-prompt-card {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 12px;
    padding: 2rem 2.25rem;
    text-align: center;
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.25);
    max-width: 440px;
  }
  .stop-prompt-title {
    font-size: 1.4rem;
    font-weight: 700;
    color: var(--text);
    margin-bottom: 0.5rem;
  }
  .stop-prompt-sub {
    color: var(--muted);
    font-size: 0.95rem;
    margin-bottom: 1.5rem;
  }
  .stop-prompt-actions {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0.75rem;
  }
  .stop-prompt-actions .btn-stop { grid-column: auto; }

  /* Blocking trainer reconnect modal. Highest z-index so it sits above the waiting
     and stop-prompt overlays. */
  .reconnect-overlay {
    position: fixed;
    inset: 0;
    background: rgba(15, 23, 42, 0.55);
    backdrop-filter: blur(2px);
    display: grid;
    place-items: center;
    z-index: 120;
  }
  .reconnect-card {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 12px;
    padding: 2rem 2.25rem;
    text-align: center;
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.3);
    max-width: 440px;
    display: grid;
    justify-items: center;
    gap: 0.5rem;
  }
  .reconnect-title {
    font-size: 1.4rem;
    font-weight: 700;
    color: var(--text);
  }
  .reconnect-title.ok   { color: var(--status-ok, #16a34a); }
  .reconnect-title.fail { color: var(--danger); }
  .reconnect-sub {
    color: var(--muted);
    font-size: 0.95rem;
  }
  .reconnect-hint {
    color: var(--muted);
    font-size: 0.85rem;
    opacity: 0.8;
  }
  .reconnect-spinner {
    width: 36px;
    height: 36px;
    border-radius: 50%;
    border: 3px solid var(--border);
    border-top-color: var(--accent);
    animation: reconnect-spin 0.9s linear infinite;
    margin-bottom: 0.4rem;
  }
  @keyframes reconnect-spin {
    to { transform: rotate(360deg); }
  }
  .reconnect-actions {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0.75rem;
    margin-top: 0.75rem;
    width: 100%;
  }
  .reconnect-actions .btn-stop { grid-column: auto; }
</style>
