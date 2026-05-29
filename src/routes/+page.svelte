<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";

  type DeviceInfo = { id: string; name: string; kind: 'Trainer' | 'Hrm' | null };
  type BleMetrics = { power_w: number | null; hr_bpm: number | null; cadence_rpm: number | null };
  type BleError = { device: string; message: string };

  let devices = $state<DeviceInfo[]>([]);
  let scanning = $state(false);
  let metrics = $state<BleMetrics | null>(null);
  let bleError = $state<string | null>(null);

  async function scanDevices() {
    scanning = true;
    try {
      devices = await invoke<DeviceInfo[]>('scan_devices');
    } catch (e) {
      bleError = e as string;
    } finally {
      scanning = false;
    }
  }

  async function connectDevice(device: DeviceInfo) {
    try {
      if (device.kind === 'Trainer') {
        await invoke('connect_trainer', { deviceId: device.id });
      } else if (device.kind === 'Hrm') {
        await invoke('connect_hrm', { deviceId: device.id });
      }
    } catch (e) {
      bleError = e as string;
    }
  }

  onMount(() => {
    scanDevices();

    let unlistenMetrics: (() => void) | undefined;
    let unlistenError: (() => void) | undefined;

    listen('ble_metrics', (e) => {
      metrics = e.payload as BleMetrics;
    }).then((fn) => { unlistenMetrics = fn; });

    listen('ble_error', (e) => {
      bleError = (e.payload as BleError).message;
    }).then((fn) => { unlistenError = fn; });

    return () => { unlistenMetrics?.(); unlistenError?.(); };
  });
</script>

<main>
  <h1>OpenCycling</h1>

  <section class="devices">
    <div class="section-header">
      <h2>Devices</h2>
      <button onclick={scanDevices} disabled={scanning}>
        {scanning ? 'Scanning...' : 'Scan'}
      </button>
    </div>

    {#if devices.length === 0 && !scanning}
      <p class="empty">No devices found.</p>
    {/if}

    <ul>
      {#each devices as device}
        <li class="device">
          <span class="device-name">{device.name}</span>
          <span class="device-kind">{device.kind ?? '?'}</span>
          {#if device.kind}
            <button onclick={() => connectDevice(device)}>Connect</button>
          {/if}
        </li>
      {/each}
    </ul>
  </section>

  <section class="metrics">
    <h2>Metrics</h2>
    {#if metrics}
      <div class="metric-grid">
        <div class="metric">
          <span class="value">{metrics.power_w ?? '—'}</span>
          <span class="label">W</span>
        </div>
        <div class="metric">
          <span class="value">{metrics.cadence_rpm ?? '—'}</span>
          <span class="label">rpm</span>
        </div>
        <div class="metric">
          <span class="value">{metrics.hr_bpm ?? '—'}</span>
          <span class="label">bpm</span>
        </div>
      </div>
    {:else}
      <p class="empty">Waiting for data…</p>
    {/if}
  </section>

  {#if bleError}
    <p class="error">{bleError}</p>
  {/if}
</main>

<style>
  main {
    max-width: 600px;
    margin: 2rem auto;
    padding: 0 1rem;
    font-family: system-ui, sans-serif;
  }

  h1 {
    font-size: 1.8rem;
    margin-bottom: 2rem;
  }

  h2 {
    font-size: 1.1rem;
    margin: 0;
  }

  section {
    background: #f5f5f5;
    border-radius: 8px;
    padding: 1rem 1.25rem;
    margin-bottom: 1rem;
  }

  .section-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 0.75rem;
  }

  ul {
    list-style: none;
    padding: 0;
    margin: 0;
  }

  .device {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0.5rem 0;
    border-top: 1px solid #e0e0e0;
  }

  .device-name {
    flex: 1;
    font-size: 0.95rem;
  }

  .device-kind {
    font-size: 0.8rem;
    color: #999;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .metric-grid {
    display: flex;
    gap: 2rem;
    margin-top: 0.75rem;
  }

  .metric {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.2rem;
  }

  .value {
    font-size: 2.5rem;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    line-height: 1;
  }

  .label {
    font-size: 0.8rem;
    color: #999;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .empty {
    color: #aaa;
    font-size: 0.9rem;
    margin: 0.5rem 0 0;
  }

  button {
    padding: 0.4rem 0.9rem;
    border: 1px solid #d0d0d0;
    border-radius: 6px;
    background: #fff;
    color: #222;
    font-size: 0.85rem;
    cursor: pointer;
    transition: background 0.15s;
  }

  button:hover:not(:disabled) {
    background: #f0f0f0;
  }

  button:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .error {
    color: #c00;
    font-size: 0.9rem;
    margin-top: 1rem;
  }
</style>
