<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import { fade } from 'svelte/transition';

  type Settings = { ftp_w: number; max_hr_bpm: number; workout_path: string };

  let ftp         = $state<number | null>(null);
  let maxHr       = $state<number | null>(null);
  let workoutPath = $state<string | null>(null);
  let loading     = $state(true);
  let saving      = $state(false);
  let saved       = $state(false);
  let error       = $state<string | null>(null);
  let savedTimer: ReturnType<typeof setTimeout> | null = null;

  onMount(async () => {
    try {
      const s = await invoke<Settings>('get_settings');
      ftp         = s.ftp_w;
      maxHr       = s.max_hr_bpm;
      workoutPath = s.workout_path;
    } catch (e) {
      error = e as string;
    } finally {
      loading = false;
    }
  });

  async function save() {
    saving = true;
    error  = null;
    saved  = false;
    try {
      await invoke('update_settings', {
        settings: { ftp_w: ftp, max_hr_bpm: maxHr, workout_path: workoutPath }
      });
      saved = true;
      if (savedTimer) clearTimeout(savedTimer);
      savedTimer = setTimeout(() => saved = false, 2000);
    } catch (e) {
      error = e as string;
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
        <input id="workout-path" type="text" bind:value={workoutPath} placeholder="/path/to/workouts" />
      </div>
    </div>

    {#if error}
      <p class="error-box">{error}</p>
    {/if}

    <div class="actions">
      {#if saved}
        <span class="saved-msg" transition:fade={{ duration: 100 }}>Saved</span>
      {/if}
      <button onclick={save} disabled={saving || ftp === null || maxHr === null || workoutPath === null} class="btn-primary">
        {saving ? 'Saving…' : 'Save'}
      </button>
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

  .actions {
    display: flex;
    align-items: center;
    gap: 1rem;
    justify-content: flex-end;
  }

  .saved-msg {
    font-size: 0.85rem;
    color: #22c55e;
  }
</style>