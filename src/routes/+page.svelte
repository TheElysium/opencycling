<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import { fade } from 'svelte/transition';
  import { goto } from '$app/navigation';
  import { ble, type DeviceStatus } from '$lib/ble.svelte';

  type DeviceInfo = { id: string; name: string; kind: 'Trainer' | 'Hrm' | null };

  let trainerId  = $state<string | null>(null);
  let hrmId      = $state<string | null>(null);
  let scanning   = $state(false);
  let scanError  = $state<string | null>(null);

  const statusConfig: Record<DeviceStatus, { color: string; label: string }> = {
    scanning:     { color: '#94a3b8', label: 'Scanning…' },
    not_found:    { color: '#94a3b8', label: 'Not found' },
    detected:     { color: '#3b82f6', label: 'Detected' },
    connecting:   { color: '#f59e0b', label: 'Connecting…' },
    connected:    { color: '#22c55e', label: 'Connected' },
    disconnected: { color: '#f59e0b', label: 'Disconnected' },
    error:        { color: '#ef4444', label: 'Error' },
  };

  async function scanDevices() {
    scanning = true;
    scanError = null;
    ble.trainerError = null;
    ble.hrmError = null;

    // Don't reset already-connected devices — the BLE actor still holds the connection.
    if (ble.trainerStatus !== 'connected') {
      ble.trainerStatus = 'scanning';
      ble.trainerName = null;
      trainerId = null;
    }
    if (ble.hrmStatus !== 'connected') {
      ble.hrmStatus = 'scanning';
      ble.hrmName = null;
      hrmId = null;
    }

    try {
      const devices = await invoke<DeviceInfo[]>('scan_devices');

      const trainer = devices.find(d => d.kind === 'Trainer');
      const hrm     = devices.find(d => d.kind === 'Hrm');

      if (trainer) {
        trainerId = trainer.id;
        ble.trainerName = trainer.name;
        if (ble.trainerStatus !== 'connected') ble.trainerStatus = 'detected';
      } else if (ble.trainerStatus !== 'connected') {
        ble.trainerStatus = 'not_found';
      }

      if (hrm) {
        hrmId = hrm.id;
        ble.hrmName = hrm.name;
        if (ble.hrmStatus !== 'connected') ble.hrmStatus = 'detected';
      } else if (ble.hrmStatus !== 'connected') {
        ble.hrmStatus = 'not_found';
      }
    } catch (e) {
      scanError = e as string;
    } finally {
      scanning = false;
    }
  }

  async function connect(kind: 'Trainer' | 'Hrm') {
    const isTrainer = kind === 'Trainer';
    const id = isTrainer ? trainerId : hrmId;
    if (!id) return;
    const setStatus = (s: DeviceStatus) => { if (isTrainer) ble.trainerStatus = s; else ble.hrmStatus = s; };
    const setError = (e: string | null) => { if (isTrainer) ble.trainerError = e; else ble.hrmError = e; };
    setStatus('connecting');
    setError(null);
    try {
      await invoke(`connect_${kind.toLowerCase()}`, { deviceId: id });
      setStatus('connected');
    } catch (e) {
      setStatus('error');
      setError(e as string);
    }
  }

  onMount(() => {
    if (ble.trainerStatus !== 'connected' || ble.hrmStatus !== 'connected') {
      scanDevices();
    }
  });
</script>

<div class="page">
  <div class="header">
    <h1>Connection</h1>
    <button onclick={scanDevices} disabled={scanning} class="btn-secondary">
      {scanning ? 'Scanning…' : 'Scan'}
    </button>
  </div>

  {#if scanError}
    <p class="error-box scan-error">{scanError}</p>
  {/if}

  <div class="devices">
    <!-- Trainer card -->
    <div class="card">
      <div class="card-header">
        <div class="device-info">
          <span class="device-label">Home Trainer</span>
          {#if ble.trainerName}
            <span class="device-name">{ble.trainerName}</span>
          {/if}
        </div>
        <div class="status">
          <span class="dot" style="background: {statusConfig[ble.trainerStatus].color}"></span>
          <span class="status-text">{statusConfig[ble.trainerStatus].label}</span>
        </div>
      </div>
      {#if ble.trainerStatus === 'not_found'}
        <p class="hint">Make sure your trainer is powered on, then scan again.</p>
      {/if}
      {#if ble.trainerStatus === 'disconnected'}
        <p class="hint">Trainer disconnected. Scan to reconnect.</p>
      {/if}
      {#if ble.trainerStatus === 'detected'}
        <button onclick={() => connect('Trainer')} class="btn-primary">Connect</button>
      {/if}
      {#if ble.trainerError}
        <p class="error">{ble.trainerError}</p>
      {/if}
    </div>

    <!-- HRM card -->
    <div class="card">
      <div class="card-header">
        <div class="device-info">
          <span class="device-label">
            Heart Rate Monitor
            <span class="optional">Optional</span>
          </span>
          {#if ble.hrmName}
            <span class="device-name">{ble.hrmName}</span>
          {/if}
        </div>
        <div class="status">
          <span class="dot" style="background: {statusConfig[ble.hrmStatus].color}"></span>
          <span class="status-text">{statusConfig[ble.hrmStatus].label}</span>
        </div>
      </div>
      {#if ble.hrmStatus === 'not_found'}
        <p class="hint">No heart rate monitor detected. Sessions work without HR.</p>
      {/if}
      {#if ble.hrmStatus === 'disconnected'}
        <p class="hint">Heart rate monitor disconnected. Scan to reconnect.</p>
      {/if}
      {#if ble.hrmStatus === 'detected'}
        <button onclick={() => connect('Hrm')} class="btn-primary">Connect</button>
      {/if}
      {#if ble.hrmError}
        <p class="error">{ble.hrmError}</p>
      {/if}
    </div>
  </div>

  <!-- Live preview -->
  {#if ble.metrics}
    <div class="card metrics-preview">
      <div class="metric">
        <span class="value">{ble.metrics.power_w ?? '—'}</span>
        <span class="unit">W</span>
      </div>
      <div class="metric">
        <span class="value">{ble.metrics.cadence_rpm ?? '—'}</span>
        <span class="unit">rpm</span>
      </div>
      <div class="metric">
        <span class="value">{ble.metrics.hr_bpm ?? '—'}</span>
        <span class="unit">bpm</span>
      </div>
    </div>
  {/if}

  {#if ble.trainerStatus === 'connected'}
    <div class="cta" transition:fade={{ duration: 300 }}>
      <button class="btn-primary" onclick={() => goto('/workouts')}>Go to Workouts →</button>
    </div>
  {/if}
</div>

<style>
  .scan-error { margin: 0 0 1rem; }

  .header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 1.5rem;
  }

  h1 {
    font-size: 1.4rem;
    font-weight: 600;
    margin: 0;
  }

  .devices {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    margin-bottom: 1.5rem;
  }

  .card {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
  }

  .card-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
  }

  .device-info {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
  }

  .device-label {
    font-weight: 600;
    font-size: 0.95rem;
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .optional {
    font-size: 0.75rem;
    font-weight: 400;
    color: var(--muted);
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 0.1rem 0.4rem;
  }

  .device-name {
    font-size: 0.8rem;
    color: var(--muted);
  }

  .status {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    flex-shrink: 0;
  }

  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    display: inline-block;
  }

  .status-text {
    font-size: 0.85rem;
    color: var(--muted);
  }

  .hint {
    margin: 0;
    font-size: 0.82rem;
    color: var(--muted);
  }

  .error {
    margin: 0;
    font-size: 0.85rem;
    color: var(--danger);
  }

  .metrics-preview {
    display: flex;
    flex-direction: row;
    gap: 2rem;
  }

  .metric {
    display: flex;
    align-items: baseline;
    gap: 0.3rem;
  }

  .value {
    font-size: 1.6rem;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }

  .unit {
    font-size: 0.8rem;
    color: var(--muted);
  }

  .btn-primary { align-self: flex-start; }

  .cta { margin-top: 1rem; }
</style>
