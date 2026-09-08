<script lang="ts">
  import { onMount } from 'svelte';
  import { fade, slide } from 'svelte/transition';
  import { goto } from '$app/navigation';
  import { Search, LoaderCircle, Plug2, Heart, Activity, Gauge } from '@lucide/svelte';
  import { commands, type DeviceInfo, type KnownDevices } from '$lib/bindings';
  import { ble, disconnectDevice, type DeviceKind, type DeviceStatus, type ReconnectState } from '$lib/ble.svelte';
  import { autoConnectCandidates } from '$lib/devices';
  import { toMessage } from '$lib/format';

  let trainerId  = $state<string | null>(null);
  let hrmId      = $state<string | null>(null);
  let scanning   = $state(false);
  let scanError  = $state<string | null>(null);
  let known      = $state<KnownDevices | null>(null);
  let autoConnecting = $state({ trainer: false, hrm: false });

  const statusLabels: Record<DeviceStatus, string> = {
    scanning:     'Scanning…',
    not_found:    'Not found',
    detected:     'Detected',
    connecting:   'Connecting…',
    connected:    'Connected',
    disconnected: 'Disconnected',
    error:        'Error',
  };

  function statusColor(s: DeviceStatus): string {
    if (s === 'connected')   return 'var(--status-ok)';
    if (s === 'connecting')  return 'var(--status-warn)';
    if (s === 'error' || s === 'disconnected') return 'var(--status-error)';
    if (s === 'detected')    return 'var(--status-info)';
    return 'var(--status-idle)';
  }

  // Live reconnect state overrides the generic label/color on the device cards
  // ("reconnected" lingers ~1.8 s in the store, then clears itself). Shared by the
  // trainer and HRM cards so both devices surface the same wording and Retry.
  function reconnectText(r: ReconnectState, status: DeviceStatus, auto = false): string {
    if (status === 'connecting' && auto) return 'Auto-connecting…';
    if (r?.status === 'reconnecting') return r.attempt > 0 ? `Reconnecting (attempt ${r.attempt})…` : 'Reconnecting…';
    if (r?.status === 'reconnected')  return 'Reconnected';
    if (r?.status === 'failed')       return 'Unavailable';
    return statusLabels[status];
  }

  function reconnectColor(r: ReconnectState, status: DeviceStatus): string {
    if (r?.status === 'reconnecting') return statusColor('connecting');
    if (r?.status === 'reconnected')  return statusColor('connected');
    if (r?.status === 'failed')       return statusColor('error');
    return statusColor(status);
  }

  async function scanDevices() {
    scanning = true;
    scanError = null;
    ble.trainerError = null;
    ble.hrmError = null;

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
      const devices = await commands.scanDevices();
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

      await autoConnectFrom(devices);
    } catch (e) {
      scanError = toMessage(e);
    } finally {
      scanning = false;
    }
  }

  function deviceIdle(status: DeviceStatus): boolean {
    return status !== 'connected' && status !== 'connecting';
  }

  async function autoConnectFrom(devices: DeviceInfo[]) {
    if (!known?.auto_connect) return;
    const candidates = autoConnectCandidates(devices, { trainer: known.trainer, hrm: known.hrm });
    if (candidates.trainer && deviceIdle(ble.trainerStatus)) {
      await connect('Trainer', candidates.trainer, true);
    }
    if (candidates.hrm && deviceIdle(ble.hrmStatus)) {
      await connect('Hrm', candidates.hrm, true);
    }
  }

  async function connect(kind: DeviceKind, target?: DeviceInfo, auto = false) {
    const isTrainer = kind === 'Trainer';
    if (target) {
      if (isTrainer) { trainerId = target.id; ble.trainerName = target.name; }
      else           { hrmId = target.id;     ble.hrmName = target.name; }
    }
    const id = isTrainer ? trainerId : hrmId;
    if (!id) return;
    // Capture before the await: a concurrent scan resets the store name.
    const name = target?.name ?? (isTrainer ? ble.trainerName : ble.hrmName) ?? '';
    const setStatus = (s: DeviceStatus) => { if (isTrainer) ble.trainerStatus = s; else ble.hrmStatus = s; };
    const setError = (e: string | null) => { if (isTrainer) ble.trainerError = e; else ble.hrmError = e; };
    if (auto) { if (isTrainer) autoConnecting.trainer = true; else autoConnecting.hrm = true; }
    setStatus('connecting');
    setError(null);
    try {
      await (isTrainer ? commands.connectTrainer(id) : commands.connectHrm(id));
      setStatus('connected');
      if (name) commands.saveKnownDevice(kind, id, name).catch(() => {});
    } catch (e) {
      setStatus('error');
      setError(toMessage(e));
    } finally {
      if (isTrainer) autoConnecting.trainer = false; else autoConnecting.hrm = false;
    }
  }

  onMount(async () => {
    if (ble.trainerStatus !== 'connected' || ble.hrmStatus !== 'connected') {
      try {
        known = await commands.getKnownDevices();
      } catch {
        known = null;
      }
      scanDevices();
    }
  });

  let bothNotFound = $derived(
    ble.trainerStatus === 'not_found' && ble.hrmStatus === 'not_found' && !scanning
  );
  let anyConnected = $derived(
    ble.trainerStatus === 'connected' || ble.hrmStatus === 'connected'
  );
</script>

<div class="page-wide">
  <div class="header">
    <h1>Connection</h1>
    <button
      onclick={scanDevices}
      disabled={scanning}
      class="scan-btn {anyConnected ? 'btn-secondary' : 'btn-primary'}"
    >
      <span class="scan-label" class:hidden={scanning}>
        <Search size={16} /> Scan
      </span>
      <span class="scan-label" class:hidden={!scanning}>
        <LoaderCircle size={16} class="spin" /> Scanning…
      </span>
    </button>
  </div>

  {#if scanError}
    <p class="error-box scan-error">{scanError}</p>
  {/if}

  <div class="devices">
    <div class="card device-card">
      <div class="card-header">
        <div class="device-info">
          <span class="device-label">
            <Gauge size={16} aria-hidden="true" /> Home Trainer
          </span>
          {#if ble.trainerName}
            <span class="device-name">{ble.trainerName}</span>
          {/if}
        </div>
        <div class="status">
          <span class="dot" class:pulse-dot={ble.trainerStatus === 'scanning'} style="background: {reconnectColor(ble.trainerReconnect, ble.trainerStatus)}"></span>
          <span class="status-text" style="color: {reconnectColor(ble.trainerReconnect, ble.trainerStatus)}">{reconnectText(ble.trainerReconnect, ble.trainerStatus, autoConnecting.trainer)}</span>
        </div>
      </div>
      {#if ble.trainerStatus === 'not_found' && !bothNotFound}
        <p class="hint" transition:slide={{ duration: 200 }}>Make sure your trainer is powered on, then scan again.</p>
      {/if}
      {#if ble.trainerStatus === 'disconnected'}
        <p class="hint" transition:slide={{ duration: 200 }}>Trainer disconnected. Scan to reconnect.</p>
      {/if}
      {#if ble.trainerReconnect?.status === 'failed'}
        <p class="hint" transition:slide={{ duration: 200 }}>Trainer unavailable.</p>
        <div class="card-actions" transition:slide={{ duration: 200 }}>
          <button onclick={() => ble.retryReconnect('Trainer')} class="btn-primary">Retry</button>
        </div>
      {/if}
      {#if ble.trainerStatus === 'detected'}
        <div class="card-actions" transition:slide={{ duration: 200 }}>
          <button onclick={() => connect('Trainer')} class="btn-primary">Connect</button>
        </div>
      {/if}
      {#if ble.trainerStatus === 'connected'}
        <div class="card-actions" transition:slide={{ duration: 200 }}>
          <button onclick={() => disconnectDevice('Trainer')} class="btn-ghost">Disconnect</button>
        </div>
      {/if}
      {#if ble.trainerError}
        <p class="error" transition:slide={{ duration: 200 }}>{ble.trainerError}</p>
      {/if}
    </div>

    <div class="card device-card">
      <div class="card-header">
        <div class="device-info">
          <span class="device-label">
            <Heart size={16} aria-hidden="true" /> Heart Rate Monitor
            <span class="optional">Optional</span>
          </span>
          {#if ble.hrmName}
            <span class="device-name">{ble.hrmName}</span>
          {/if}
        </div>
        <div class="status">
          <span class="dot" class:pulse-dot={ble.hrmStatus === 'scanning'} style="background: {reconnectColor(ble.hrmReconnect, ble.hrmStatus)}"></span>
          <span class="status-text" style="color: {reconnectColor(ble.hrmReconnect, ble.hrmStatus)}">{reconnectText(ble.hrmReconnect, ble.hrmStatus, autoConnecting.hrm)}</span>
        </div>
      </div>
      {#if ble.hrmStatus === 'not_found'}
        <p class="hint" transition:slide={{ duration: 200 }}>No heart rate monitor detected. Sessions work without HR.</p>
      {/if}
      {#if ble.hrmStatus === 'disconnected'}
        <p class="hint" transition:slide={{ duration: 200 }}>Heart rate monitor disconnected. Scan to reconnect.</p>
      {/if}
      {#if ble.hrmReconnect?.status === 'failed'}
        <p class="hint" transition:slide={{ duration: 200 }}>Heart rate monitor unavailable.</p>
        <div class="card-actions" transition:slide={{ duration: 200 }}>
          <button onclick={() => ble.retryReconnect('Hrm')} class="btn-primary">Retry</button>
        </div>
      {/if}
      {#if ble.hrmStatus === 'detected'}
        <div class="card-actions" transition:slide={{ duration: 200 }}>
          <button onclick={() => connect('Hrm')} class="btn-primary">Connect</button>
        </div>
      {/if}
      {#if ble.hrmStatus === 'connected'}
        <div class="card-actions" transition:slide={{ duration: 200 }}>
          <button onclick={() => disconnectDevice('Hrm')} class="btn-ghost">Disconnect</button>
        </div>
      {/if}
      {#if ble.hrmError}
        <p class="error" transition:slide={{ duration: 200 }}>{ble.hrmError}</p>
      {/if}
    </div>
  </div>

  {#if bothNotFound}
    <div class="empty-state" transition:fade={{ duration: 200 }}>
      <Plug2 size={36} aria-hidden="true" />
      <div class="empty-body">
        <p class="empty-title">No devices found</p>
        <p class="empty-hint">
          Power on your trainer, place it within ~2&nbsp;m of your computer, then scan again.
        </p>
      </div>
      <button onclick={scanDevices} class="btn-primary big-scan">
        <Search size={16} /> Scan again
      </button>
    </div>
  {/if}

  {#if anyConnected && ble.metrics}
    <div class="metrics-card" transition:fade={{ duration: 200 }}>
      <div class="metric">
        <Activity size={14} class="metric-icon" />
        <span class="value">{ble.metrics.power_w ?? '—'}</span>
        <span class="unit">W</span>
      </div>
      <div class="metric">
        <span class="value">{ble.metrics.cadence_rpm ?? '—'}</span>
        <span class="unit">rpm</span>
      </div>
      <div class="metric metric-hr" class:pulse={ble.metrics.hr_bpm !== null}>
        <Heart size={14} class="metric-icon" />
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

  .scan-btn {
    display: inline-grid;
    transition: background 0.2s ease, color 0.2s ease, border-color 0.2s ease;
  }

  .scan-label {
    grid-area: 1 / 1;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 0.4rem;
    white-space: nowrap;
    transition: opacity 0.2s ease;
  }

  .scan-label.hidden {
    opacity: 0;
  }

  :global(.spin) {
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  .pulse-dot {
    animation: pulse-dot 1s ease-in-out infinite;
  }

  @keyframes pulse-dot {
    0%, 100% { transform: scale(1); opacity: 1; }
    50%      { transform: scale(1.4); opacity: 0.5; }
  }

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
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(320px, 1fr));
    gap: 0.75rem;
    margin-bottom: 1rem;
  }

  .device-card {
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
    gap: 0.45rem;
  }

  .optional {
    font-size: 0.7rem;
    font-weight: 500;
    color: var(--muted);
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 0.1rem 0.4rem;
    margin-left: 0.15rem;
  }

  .device-name {
    font-size: 0.8rem;
    color: var(--muted);
    margin-left: 1.4rem;
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
    transition: background 0.2s ease;
  }

  .status-text {
    font-size: 0.85rem;
    color: var(--muted);
    transition: color 0.2s ease;
  }

  .card-actions {
    display: flex;
    gap: 0.5rem;
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

  .empty-state {
    display: flex;
    align-items: center;
    gap: 1.25rem;
    margin: 0 0 1.25rem;
    padding: 1.1rem 1.4rem;
    background: color-mix(in srgb, var(--warning) 10%, transparent);
    border: 1px solid color-mix(in srgb, var(--warning) 40%, transparent);
    border-radius: 12px;
    color: var(--text);
  }

  .empty-state :global(svg) {
    color: var(--warning);
    flex-shrink: 0;
  }

  .empty-body {
    flex: 1;
    min-width: 0;
  }

  .empty-title {
    margin: 0 0 0.2rem;
    font-weight: 700;
    color: var(--text);
    font-size: 0.95rem;
  }

  .empty-hint {
    margin: 0;
    font-size: 0.85rem;
    line-height: 1.45;
    color: var(--text);
    opacity: 0.85;
  }

  .big-scan {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    padding: 0.55rem 1.1rem;
    font-size: 0.88rem;
    white-space: nowrap;
    flex-shrink: 0;
  }

  @media (max-width: 540px) {
    .empty-state {
      flex-direction: column;
      align-items: flex-start;
    }
  }

  .metrics-card {
    display: flex;
    gap: 2rem;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 1.1rem 1.5rem;
    margin-bottom: 1rem;
  }

  .metric {
    display: flex;
    align-items: baseline;
    gap: 0.35rem;
    color: var(--muted);
  }

  :global(.metric-icon) {
    color: var(--muted);
    align-self: center;
  }

  .value {
    font-size: 2.2rem;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
    color: var(--text);
    line-height: 1;
  }

  .unit {
    font-size: 0.8rem;
    color: var(--muted);
    margin-left: 0.15rem;
  }

  .metric-hr :global(.metric-icon) { color: var(--danger); }

  .pulse :global(.metric-icon) {
    animation: pulse 1s ease-in-out infinite;
  }

  @keyframes pulse {
    0%, 100% { transform: scale(1); }
    20%      { transform: scale(1.25); }
    40%      { transform: scale(1); }
  }

  .cta { margin-top: 1rem; }
</style>
