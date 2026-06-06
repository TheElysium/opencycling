<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { Zap, Heart, RotateCw } from '@lucide/svelte';
  import { workoutTypeColor } from '$lib/metrics';
  import { toMessage } from '$lib/format';
  import {
    type SessionCard, type SessionGroup,
    computeSessionMetrics, groupByPeriod,
    formatDayNum, formatWeekdayShort, formatHourMinute, formatHmsShort,
  } from '$lib/db';

  let sessions = $state<SessionCard[]>([]);
  let loading  = $state(true);
  let error    = $state<string | null>(null);

  let groups = $derived<SessionGroup[]>(groupByPeriod(sessions));

  onMount(async () => {
    try {
      sessions = await invoke<SessionCard[]>('list_sessions');
    } catch (e) {
      error = toMessage(e);
    } finally {
      loading = false;
    }
  });

  function open(id: number) {
    goto(`/history/${id}`);
  }
</script>

<div>
  <h1>
    History
    {#if !loading && sessions.length > 0}
      <span class="count">{sessions.length} sessions</span>
    {/if}
  </h1>

  {#if loading}
    <p class="muted">Loading…</p>
  {:else if error}
    <p class="error-box">{error}</p>
  {:else if sessions.length === 0}
    <p class="muted">No sessions yet. Go run a workout.</p>
  {:else}
    {#each groups as g}
      <h2 class="list-section">
        {g.label}
        <span class="agg">
          {g.agg.count} {g.agg.count === 1 ? 'session' : 'sessions'}
          · {formatHmsShort(g.agg.total_s)}
          · {Math.round(g.agg.total_tss)} TSS
        </span>
      </h2>
      <div class="session-list">
        {#each g.sessions as s}
          {@const m = computeSessionMetrics(s)}
          <button class="session-card" onclick={() => open(s.id)}>
            <div class="day-block">
              <div class="day-num">{formatDayNum(s.started_at)}</div>
              <div class="day-weekday">{formatWeekdayShort(s.started_at)}</div>
            </div>
            <div class="session-mid">
              <div class="session-top">
                {#if s.workout_type}
                  <span class="type-badge" style="--type-color: {workoutTypeColor(s.workout_type)}">
                    <span class="dot"></span>{s.workout_type}
                  </span>
                {/if}
                <span class="session-name">{s.workout_name}</span>
              </div>
              <div class="session-meta">
                <span>{formatHourMinute(s.started_at)}</span>
                {#if s.duration_s != null}
                  <span class="sep">·</span>
                  <span>{formatHmsShort(s.duration_s)}</span>
                {/if}
                {#if m.tss > 0}
                  <span class="sep">·</span>
                  <span>{Math.round(m.tss)} TSS</span>
                {/if}
              </div>
            </div>
            <div class="session-metrics">
              <div class="metric">
                <div class="metric-top"><Zap size={13} /><span class="metric-val">{s.avg_power_w ?? '—'}</span></div>
                <div class="metric-lbl">Avg W</div>
              </div>
              <div class="metric">
                <div class="metric-top"><Heart size={13} /><span class="metric-val">{s.avg_hr_bpm ?? '—'}</span></div>
                <div class="metric-lbl">Avg HR</div>
              </div>
              <div class="metric">
                <div class="metric-top"><RotateCw size={13} /><span class="metric-val">{s.avg_cadence_rpm ?? '—'}</span></div>
                <div class="metric-lbl">Avg rpm</div>
              </div>
            </div>
          </button>
        {/each}
      </div>
    {/each}
  {/if}
</div>

<style>
  h1 {
    font-size: 1.4rem;
    font-weight: 600;
    margin: 0 0 1.25rem;
    display: flex;
    align-items: baseline;
    gap: 0.5rem;
  }
  .count {
    font-size: 0.8rem;
    color: var(--muted);
    font-weight: 500;
  }
  .muted { color: var(--muted); }

  .list-section {
    font-size: 0.78rem;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--muted);
    margin: 1.5rem 0 0.6rem;
    padding-bottom: 0.4rem;
    border-bottom: 1px solid var(--border);
    display: flex;
    justify-content: space-between;
    align-items: baseline;
  }
  .list-section .agg {
    font-size: 0.72rem;
    font-weight: 500;
    color: var(--muted);
    text-transform: none;
    letter-spacing: 0;
  }

  .session-list {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
  }

  .session-card {
    display: grid;
    grid-template-columns: 50px 1fr auto;
    gap: 1rem;
    align-items: center;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 0.85rem 1.1rem;
    cursor: pointer;
    transition: border-color 0.15s, box-shadow 0.15s, transform 0.15s;
    text-align: left;
    font: inherit;
    color: inherit;
    width: 100%;
  }
  .session-card:hover {
    border-color: var(--accent);
    box-shadow: 0 4px 12px rgba(0,0,0,0.06);
    transform: translateY(-1px);
  }

  .day-block {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    background: var(--bg);
    border-radius: 8px;
    padding: 0.4rem 0.3rem;
    min-width: 50px;
  }
  .day-num {
    font-size: 1.15rem;
    font-weight: 700;
    line-height: 1;
  }
  .day-weekday {
    font-size: 0.65rem;
    color: var(--muted);
    text-transform: uppercase;
    letter-spacing: 0.05em;
    margin-top: 0.15rem;
  }

  .session-mid { min-width: 0; }
  .session-top {
    display: flex;
    align-items: center;
    gap: 0.55rem;
    margin-bottom: 0.3rem;
    flex-wrap: wrap;
  }
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
  .session-name {
    font-weight: 600;
    font-size: 0.95rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .session-meta {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
    font-size: 0.78rem;
    color: var(--muted);
    align-items: center;
  }
  .session-meta .sep { opacity: 0.5; }

  .session-metrics {
    display: flex;
    gap: 1.1rem;
    align-items: center;
  }
  .metric {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    min-width: 54px;
  }
  .metric-top {
    display: flex;
    align-items: center;
    gap: 0.3rem;
    color: var(--muted);
  }
  .metric-val {
    font-weight: 700;
    font-size: 1rem;
    line-height: 1.1;
    color: var(--text);
  }
  .metric-lbl {
    font-size: 0.62rem;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--muted);
    margin-top: 0.15rem;
  }
</style>
