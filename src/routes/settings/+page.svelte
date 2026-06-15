<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog';
  import { onMount } from 'svelte';
  import { fade } from 'svelte/transition';
  import { Check } from '@lucide/svelte';
  import { getSettings, updateSettings } from '$lib/settings';
  import {
    stravaStatus,
    stravaConnect,
    stravaDisconnect,
    stravaSetAutoUpload,
    type StravaStatus
  } from '$lib/strava';
  import { toMessage } from '$lib/format';

  let ftp         = $state<number | null>(null);
  let maxHr       = $state<number | null>(null);
  let workoutPath = $state<string | null>(null);
  let stravaProxy = $state<string | null>(null);
  let loading     = $state(true);
  let saving      = $state(false);
  let saved       = $state(false);
  let error       = $state<string | null>(null);
  let savedTimer: ReturnType<typeof setTimeout> | null = null;

  let strava      = $state<StravaStatus>({ connected: false, athlete_id: null, athlete_name: null, auto_upload: false });
  let stravaBusy  = $state(false);
  let stravaError = $state<string | null>(null);

  async function browseFolder() {
    const selected = await open({ directory: true, multiple: false });
    if (typeof selected === 'string') workoutPath = selected;
  }

  async function loadStrava() {
    try {
      strava = await stravaStatus();
    } catch (e) {
      stravaError = toMessage(e);
    }
  }

  async function connectStrava() {
    stravaBusy = true;
    stravaError = null;
    try {
      strava = await stravaConnect();
    } catch (e) {
      stravaError = toMessage(e);
    } finally {
      stravaBusy = false;
    }
  }

  async function disconnectStrava() {
    stravaError = null;
    try {
      await stravaDisconnect();
      await loadStrava();
    } catch (e) {
      stravaError = toMessage(e);
    }
  }

  async function toggleAutoUpload(e: Event) {
    const enabled = (e.target as HTMLInputElement).checked;
    try {
      await stravaSetAutoUpload(enabled);
      strava.auto_upload = enabled;
    } catch (err) {
      stravaError = toMessage(err);
    }
  }

  // The proxy URL lives in the Strava tile and persists on blur, so it can be
  // set right before clicking Connect without scrolling up to the global Save.
  async function saveProxyUrl() {
    if (ftp === null || maxHr === null || workoutPath === null || stravaProxy === null) return;
    stravaError = null;
    try {
      await updateSettings({ ftp_w: ftp, max_hr_bpm: maxHr, workout_path: workoutPath, strava_proxy_url: stravaProxy });
    } catch (e) {
      stravaError = toMessage(e);
    }
  }

  onMount(async () => {
    try {
      const s = await getSettings();
      ftp         = s.ftp_w;
      maxHr       = s.max_hr_bpm;
      workoutPath = s.workout_path;
      stravaProxy = s.strava_proxy_url;
    } catch (e) {
      error = toMessage(e);
    } finally {
      loading = false;
    }
    await loadStrava();
  });

  async function save() {
    if (ftp === null || maxHr === null || workoutPath === null || stravaProxy === null) return;
    saving = true;
    error  = null;
    saved  = false;
    try {
      await updateSettings({ ftp_w: ftp, max_hr_bpm: maxHr, workout_path: workoutPath, strava_proxy_url: stravaProxy });
      saved = true;
      if (savedTimer) clearTimeout(savedTimer);
      savedTimer = setTimeout(() => saved = false, 2000);
    } catch (e) {
      error = toMessage(e);
    } finally {
      saving = false;
    }
  }
</script>

<div class="page">
  <h1>Settings</h1>

  {#if loading}
    <p class="loading">Loading…</p>
  {:else}
    <div class="card fields">
      <div class="field">
        <label for="ftp">FTP (W)</label>
        <input id="ftp" type="number" min="0" max="600" bind:value={ftp} />
      </div>
      <div class="field">
        <label for="max-hr">Max HR (bpm)</label>
        <input id="max-hr" type="number" min="100" max="250" bind:value={maxHr} />
      </div>
      <div class="field">
        <label for="workout-path">Workout folder</label>
        <div class="path-row">
          <input id="workout-path" type="text" bind:value={workoutPath} placeholder="/path/to/workouts" />
          <button type="button" class="btn-secondary browse-btn" onclick={browseFolder}>Browse</button>
        </div>
      </div>
    </div>

    {#if error}
      <p class="error-box">{error}</p>
    {/if}

    <div class="actions">
      {#if saved}
        <span class="saved-msg" transition:fade={{ duration: 100 }}>Saved</span>
      {/if}
      <button onclick={save} disabled={saving || ftp === null || maxHr === null || workoutPath === null || stravaProxy === null} class="btn-primary">
        {saving ? 'Saving…' : 'Save'}
      </button>
    </div>

    <h2>Third-party integrations</h2>
    <div class="card integrations">
      <div class="integration" class:is-connected={strava.connected}>
        <div class="integration-brand">
          <span class="integration-logo">
            <svg viewBox="0 0 24 24" aria-hidden="true">
              <path d="M15.387 17.944l-2.089-4.116h-3.065L15.387 24l5.15-10.172h-3.066l-2.084 4.116z" />
              <path d="M10.463 0l-7 13.828h4.169l2.831-5.598 2.836 5.598h4.172L10.463 0z" />
            </svg>
          </span>
          <div class="integration-info">
            <span class="integration-name">Strava</span>
            {#if strava.connected}
              <span class="status-badge" transition:fade={{ duration: 120 }}>
                <Check size={13} strokeWidth={3} />
                Connected{strava.athlete_name ? ` · ${strava.athlete_name}` : strava.athlete_id ? ` · athlete ${strava.athlete_id}` : ''}
              </span>
            {:else}
              <span class="integration-sub">Publish finished sessions as Virtual Rides</span>
            {/if}
          </div>
        </div>

        {#if strava.connected}
          <button type="button" class="btn-ghost" onclick={disconnectStrava}>Disconnect</button>
        {:else}
          <button type="button" class="btn-strava" onclick={connectStrava} disabled={stravaBusy}>
            {stravaBusy ? 'Waiting…' : 'Connect'}
          </button>
        {/if}
      </div>

      <div class="proxy-field">
        <label for="strava-proxy">Auth proxy URL</label>
        <input id="strava-proxy" type="text" bind:value={stravaProxy} onblur={saveProxyUrl} placeholder="http://127.0.0.1:8788" />
        <span class="field-hint">Endpoint of the Strava auth proxy. Saved automatically. See the setup guide.</span>
      </div>

      {#if strava.connected}
        <label class="toggle">
          <input type="checkbox" checked={strava.auto_upload} onchange={toggleAutoUpload} />
          Auto-upload sessions when they finish
        </label>
      {/if}

      {#if stravaError}
        <p class="error-box">{stravaError}</p>
      {/if}
    </div>
  {/if}
</div>

<style>
  h1 { font-size: 1.4rem; font-weight: 600; margin: 0 0 1.5rem; }

  .loading { color: var(--muted); font-size: 0.9rem; }

  .fields {
    display: flex;
    flex-direction: column;
    gap: 1rem;
    margin-bottom: 1rem;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }

  label {
    font-size: 0.85rem;
    font-weight: 500;
    color: var(--muted);
  }

  input {
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 0.45rem 0.75rem;
    font-size: 0.95rem;
    color: var(--text);
    caret-color: var(--text);
    width: 100%;
    cursor: text;
  }

  input:focus {
    outline: none;
    border-color: var(--accent);
  }

  .field-hint {
    font-size: 0.75rem;
    color: var(--muted);
  }

  .path-row {
    display: flex;
    gap: 0.5rem;
  }

  .path-row input {
    flex: 1;
  }

  .browse-btn {
    white-space: nowrap;
    flex-shrink: 0;
  }

  .actions {
    display: flex;
    align-items: center;
    gap: 1rem;
    justify-content: flex-end;
  }

  .saved-msg {
    font-size: 0.85rem;
    color: var(--success);
  }

  h2 {
    font-size: 1.1rem;
    font-weight: 600;
    margin: 2rem 0 1rem;
  }

  .integrations {
    display: flex;
    flex-direction: column;
    gap: 1.1rem;
  }

  .integration {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
  }

  .integration-brand {
    display: flex;
    align-items: center;
    gap: 0.85rem;
    min-width: 0;
  }

  .integration-logo {
    width: 40px;
    height: 40px;
    border-radius: 9px;
    background: #fc4c02;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }

  .integration-logo svg {
    width: 22px;
    height: 22px;
    fill: #fff;
  }

  .integration-info {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
    min-width: 0;
  }

  .integration-name {
    font-size: 0.95rem;
    font-weight: 600;
    color: var(--text);
  }

  .integration-sub {
    font-size: 0.8rem;
    color: var(--muted);
  }

  .status-badge {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    font-size: 0.8rem;
    font-weight: 600;
    color: var(--success);
  }

  .btn-strava {
    background: #fc4c02;
    color: #fff;
    border: 1px solid #fc4c02;
    border-radius: 7px;
    padding: 0.5rem 1.3rem;
    font-size: 0.85rem;
    font-weight: 600;
    cursor: pointer;
    flex-shrink: 0;
    transition: background 0.15s, opacity 0.15s;
  }
  .btn-strava:hover:not(:disabled) { background: #e44402; }
  .btn-strava:disabled { opacity: 0.6; cursor: default; }

  .btn-ghost {
    background: transparent;
    color: var(--muted);
    border: 1px solid var(--border);
    border-radius: 7px;
    padding: 0.5rem 1.3rem;
    font-size: 0.85rem;
    font-weight: 600;
    cursor: pointer;
    flex-shrink: 0;
    transition: color 0.15s, border-color 0.15s;
  }
  .btn-ghost:hover { color: var(--danger); border-color: color-mix(in srgb, var(--danger) 35%, transparent); }

  .proxy-field {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    border-top: 1px solid var(--border);
    padding-top: 1rem;
  }

  .toggle {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-direction: row;
    font-size: 0.9rem;
    color: var(--text);
    cursor: pointer;
    border-top: 1px solid var(--border);
    padding-top: 1rem;
  }

  .toggle input {
    width: auto;
    cursor: pointer;
  }
</style>