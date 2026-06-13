<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { confirm } from '@tauri-apps/plugin-dialog';
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { page } from '$app/stores';
  import { ArrowLeft, Upload } from '@lucide/svelte';
  import { workoutTypeColor } from '$lib/metrics';
  import { toMessage } from '$lib/format';
  import {
    type SessionDetail,
    formatLongDate, formatHourMinute, formatHmsShort,
  } from '$lib/db';
  import { getSettings } from '$lib/settings';
  import { exportSessionTcx } from '$lib/export';
  import SessionDetailRecap from '$lib/components/SessionDetailRecap.svelte';

  let detail  = $state<SessionDetail | null>(null);
  let maxHr   = $state(190);
  let loading = $state(true);
  let error   = $state<string | null>(null);

  let id = $derived(parseInt($page.params.id ?? '0', 10));

  onMount(async () => {
    try {
      const [d, s] = await Promise.all([
        invoke<SessionDetail>('get_session', { id }),
        getSettings(),
      ]);
      detail = d;
      maxHr = s.max_hr_bpm;
    } catch (e) {
      error = toMessage(e);
    } finally {
      loading = false;
    }
  });

  async function onExportTcx() {
    if (!detail) return;
    try {
      await exportSessionTcx(detail.id, detail.workout_name, detail.started_at);
    } catch (e) {
      error = toMessage(e);
    }
  }

  async function onDelete() {
    if (!detail) return;
    const ok = await confirm('Delete this session? This action cannot be undone.', {
      title: 'Delete session',
      kind: 'warning',
    });
    if (!ok) return;
    try {
      await invoke('delete_session', { id: detail.id });
      await goto('/history');
    } catch (e) {
      error = toMessage(e);
    }
  }
</script>

{#if loading}
  <p class="muted">Loading…</p>
{:else if error}
  <p class="error-box">{error}</p>
{:else if detail}
  <div class="detail">
    <button class="back-btn" aria-label="Back" onclick={() => goto('/history')}>
      <ArrowLeft size={22} />
    </button>

    <header class="hero">
      {#if detail.workout_type}
        <span class="type-badge" style="--type-color: {workoutTypeColor(detail.workout_type)}">
          <span class="dot"></span>{detail.workout_type}
        </span>
      {/if}
      <h1>{detail.workout_name}</h1>
      <p class="meta">
        <span>{formatLongDate(detail.started_at)}</span>
        <span class="meta-sep">·</span>
        <span>{formatHourMinute(detail.started_at)}</span>
        {#if detail.duration_s != null}
          <span class="meta-sep">·</span>
          <span>{formatHmsShort(detail.duration_s)}</span>
        {/if}
      </p>
    </header>

    <SessionDetailRecap {detail} {maxHr} />

    <div class="actions">
      <button class="btn-export" onclick={onExportTcx}>
        <Upload size={16} />
        Export .tcx
      </button>
      <button class="btn-delete" onclick={onDelete}>Delete session</button>
    </div>
  </div>
{/if}

<style>
  .muted { color: var(--muted); }

  .detail { max-width: 1100px; }

  .back-btn {
    background: none;
    border: none;
    color: var(--muted);
    padding: 0;
    margin-bottom: 0.5rem;
    cursor: pointer;
    display: inline-flex;
  }
  .back-btn:hover { color: var(--text); }

  .hero { margin-bottom: 1.25rem; }
  .hero h1 {
    font-size: 2rem;
    font-weight: 800;
    letter-spacing: -0.01em;
    margin: 0.5rem 0 0.35rem;
  }
  .hero .meta {
    font-size: 0.9rem;
    color: var(--muted);
    margin: 0 0 0.5rem;
    display: flex;
    gap: 0.4rem;
    align-items: center;
    flex-wrap: wrap;
  }
  .meta-sep { opacity: 0.6; }

  .type-badge {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    font-size: 0.65rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    border-radius: 4px;
    padding: 0.12rem 0.45rem;
    color: var(--type-color);
    background: color-mix(in srgb, var(--type-color) 15%, transparent);
  }
  .type-badge .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--type-color);
  }

  .btn-delete {
    background: transparent;
    color: var(--danger);
    border: 1px solid color-mix(in srgb, var(--danger) 35%, transparent);
    border-radius: 7px;
    padding: 0.55rem 1.4rem;
    font-size: 0.85rem;
    font-weight: 600;
    cursor: pointer;
    transition: background 0.15s, border-color 0.15s;
  }
  .btn-delete:hover {
    background: color-mix(in srgb, var(--danger) 10%, transparent);
  }
  .btn-export {
    display: inline-flex;
    align-items: center;
    gap: 0.45rem;
    background: transparent;
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 7px;
    padding: 0.55rem 1.4rem;
    font-size: 0.85rem;
    font-weight: 600;
    cursor: pointer;
    transition: background 0.15s, border-color 0.15s;
  }
  .btn-export:hover {
    border-color: var(--accent);
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.75rem;
    margin-top: 2rem;
    padding-top: 1.25rem;
    border-top: 1px solid var(--border);
  }
</style>
