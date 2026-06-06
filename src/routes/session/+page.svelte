<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { goto, beforeNavigate } from '$app/navigation';
  import { Pause, Play, Square, ArrowLeft, ArrowRight, RotateCw, Heart } from '@lucide/svelte';
  import { confirm } from '@tauri-apps/plugin-dialog';
  import { session, type SessionMetrics } from '$lib/session.svelte';
  import { toMessage } from '$lib/format';
  import { type SessionDetail } from '$lib/db';
  import { getSettings } from '$lib/settings';
  import CurrentBlockCard from '$lib/components/CurrentBlockCard.svelte';
  import SessionFinishedCard from '$lib/components/SessionFinishedCard.svelte';
  import PowerTile from '$lib/components/PowerTile.svelte';
  import MetricTile from '$lib/components/MetricTile.svelte';
  import SessionTimeline from '$lib/components/SessionTimeline.svelte';
  import BlocksList from '$lib/components/BlocksList.svelte';
  import SessionStatsPanel from '$lib/components/SessionStatsPanel.svelte';

  let snapError = $state<string | null>(null);
  let detail    = $state<SessionDetail | null>(null);
  let maxHr     = $state(190);

  onMount(async () => {
    try {
      await session.loadSnapshot();
    } catch (e) {
      snapError = toMessage(e);
    }
  });

  let m          = $derived<SessionMetrics | null>(session.metrics);
  let isWaiting  = $derived(m?.state === 'WaitingForRider');
  let isPaused   = $derived(m?.state === 'Paused');
  let isFinished = $derived(m?.state === 'Finished');
  let isActive   = $derived(m != null && !isFinished);

  $effect(() => {
    if (isFinished && m?.session_id != null && detail == null) {
      const id = m.session_id;
      (async () => {
        try {
          const [d, s] = await Promise.all([
            invoke<SessionDetail>('get_session', { id }),
            getSettings(),
          ]);
          detail = d;
          maxHr = s.max_hr_bpm;
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
        <SessionFinishedCard total_active_s={m.total_active_s} />
        {#if detail}
          <SessionStatsPanel {detail} {maxHr} />
        {:else}
          <p class="muted">Loading recap…</p>
        {/if}
      {:else}
        <CurrentBlockCard metrics={m} flat_blocks={session.flat_blocks} />
        <PowerTile power_w={m.power_w} target_w={m.target_w} />
        <div class="metrics">
          <MetricTile label="Cadence"    value={m.cadence_rpm} unit="rpm" target={m.cadence_target_rpm} icon={RotateCw} />
          <MetricTile label="Heart rate" value={m.hr_bpm}      unit="bpm" icon={Heart} />
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

  {#if snapError}
    <div class="error-box error-floating">{snapError}</div>
  {/if}
</main>

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
    background: rgba(15, 23, 42, 0.6);
    backdrop-filter: blur(4px);
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
</style>
