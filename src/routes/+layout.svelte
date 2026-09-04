<script lang="ts">
  import { page } from '$app/state';
  import { onMount } from 'svelte';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import {Plug, History, Settings, Bike, FlaskConical} from '@lucide/svelte';
  import type { BleError } from '$lib/bindings';
  import { commands } from '$lib/bindings';
  import { ble, kindFromWire, type BleMetrics, type DeviceStatus, type DeviceWire, type ReconnectStatus } from '$lib/ble.svelte';
  import { session, type SessionMetrics } from '$lib/session.svelte';
  import { checkForUpdate } from '$lib/updater';
  import SimPanel from '$lib/components/SimPanel.svelte';
  import '../app.css';

  let { children } = $props();

  const navItems = [
    { href: '/',          label: 'Connection', icon: Plug      },
    { href: '/workouts',  label: 'Workouts',   icon: Bike  },
    { href: '/history',   label: 'History',    icon: History   },
    { href: '/settings',  label: 'Settings',   icon: Settings  },
  ];

  let showSidebar = $derived(page.url.pathname !== '/session');

  let simOpen  = $state(false);
  let simReady = $state(false);

  function dotColor(s: DeviceStatus): string {
    if (s === 'connected')   return 'var(--status-ok)';
    if (s === 'connecting')  return 'var(--status-warn)';
    if (s === 'error' || s === 'disconnected') return 'var(--status-error)';
    if (s === 'detected')    return 'var(--status-info)';
    return 'var(--status-idle)';
  }

  const statusLabels: Record<DeviceStatus, string> = {
    scanning:     'Scanning…',
    not_found:    'Not found',
    detected:     'Detected',
    connecting:   'Connecting…',
    connected:    'Connected',
    disconnected: 'Disconnected',
    error:        'Error',
  };

  let trainerDot = $derived(dotColor(ble.trainerStatus));
  let hrmDot     = $derived(dotColor(ble.hrmStatus));

  onMount(() => {
    checkForUpdate();

    // Own availability probe so the chip only exists in simulator builds.
    commands.simAvailable()
      .then((available: boolean) => (simReady = available))
      .catch(() => (simReady = false));

    let cancelled = false;
    const unlisteners: UnlistenFn[] = [];
    const track = (u: UnlistenFn) => {
      if (cancelled) u();
      else unlisteners.push(u);
    };

    listen<BleMetrics>('ble_metrics', (e) => {
      ble.metrics = e.payload;
    }).then(track);

    listen<BleError>('ble_error', (e) => {
      ble.setError(e.payload.device === 'trainer' ? 'Trainer' : 'Hrm', e.payload.message);
    }).then(track);

    listen<SessionMetrics>('session_metrics', (e) => {
      session.ingestMetrics(e.payload);
    }).then(track);

    listen<string>('ble_disconnected', (e) => {
      const kind = e.payload === 'trainer' ? 'Trainer' : 'Hrm';
      ble.markDisconnected(kind);
      // Show the reconnect affordance right away; the first `ble_reconnect` event
      // (with attempt 1) follows a beat later.
      ble.markReconnecting(kind);
    }).then(track);

    listen<{ device: DeviceWire; status: ReconnectStatus; attempt?: number }>('ble_reconnect', (e) => {
      ble.applyReconnect(kindFromWire(e.payload.device), e.payload.status, e.payload.attempt);
    }).then(track);

    return () => {
      cancelled = true;
      unlisteners.forEach(fn => fn());
    };
  });
</script>

<div class="shell" class:with-sidebar={showSidebar} class:fullscreen={!showSidebar}>
  {#if showSidebar}
    <nav class="sidebar">
      <div class="logo">
        <img src="/logo-source.svg" alt="" class="logo-icon" width="22" height="22" />
        <span>OpenCycling</span>
      </div>
      <ul>
        {#each navItems as item}
          {@const active = item.href === '/'
            ? page.url.pathname === '/'
            : page.url.pathname === item.href || page.url.pathname.startsWith(item.href + '/')}
          <li>
            <a href={item.href} class:active>
              <item.icon size={18} aria-hidden="true" />
              <span class="label">{item.label}</span>
              {#if item.href === '/'}
                <span class="ble-dots" aria-label="BLE status">
                  <span class="ble-dot" style="background: {trainerDot}" title="Trainer: {statusLabels[ble.trainerStatus]}"></span>
                  <span class="ble-dot" style="background: {hrmDot}" title="HRM: {statusLabels[ble.hrmStatus]}"></span>
                </span>
              {/if}
            </a>
          </li>
        {/each}
      </ul>
    </nav>
  {/if}
  <main class="content">
    {@render children()}
  </main>

  {#if simReady && page.url.pathname !== '/settings'}
    {#if simOpen}
      <div class="sim-popover">
        <SimPanel variant="floating" />
      </div>
    {/if}
    <button
      type="button"
      class="sim-chip"
      class:is-open={simOpen}
      title="Simulation controls"
      onclick={() => (simOpen = !simOpen)}
    >
      <FlaskConical size={14} aria-hidden="true" />
      SIM
    </button>
  {/if}
</div>

<!-- HRM reconnect feedback is shown inline in the Heart rate tile on the session
     page (see MetricTile `status`), not as a global toast. -->

<style>
  .shell {
    display: flex;
    min-height: 100vh;
  }

  .shell.with-sidebar {
    display: grid;
    grid-template-columns: var(--sidebar-w) 1fr;
  }

  .sidebar {
    background: var(--surface);
    border-right: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    padding: 1.25rem 0;
    position: sticky;
    top: 0;
    height: 100vh;
  }

  .logo {
    display: flex;
    align-items: center;
    gap: 0.55rem;
    font-size: 0.85rem;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--accent);
    padding: 0 1.25rem 1.5rem;
  }

  .logo-icon {
    width: 22px;
    height: 22px;
    display: block;
    flex-shrink: 0;
  }

  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  a {
    display: flex;
    align-items: center;
    gap: 0.65rem;
    padding: 0.6rem 1.25rem;
    color: var(--muted);
    font-size: 0.9rem;
    border-left: 3px solid transparent;
    transition: color 0.15s, background 0.15s;
  }

  a:hover {
    color: var(--text);
    background: var(--bg);
  }

  a.active {
    color: var(--accent);
    border-left-color: var(--accent);
    background: color-mix(in srgb, var(--accent) 8%, transparent);
  }

  .label {
    flex: 1;
  }

  .ble-dots {
    display: inline-flex;
    gap: 3px;
  }

  .ble-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    display: inline-block;
  }

  .content {
    padding: 2rem;
    min-height: 100vh;
  }

  .shell.fullscreen {
    height: 100vh;
    overflow: hidden;
  }
  .shell.fullscreen .content {
    padding: 0;
    height: 100vh;
    min-height: 0;
    overflow: hidden;
  }

  /* Above the trainer-reconnect modal and aero overlay (z-index 120) so Restore
     stays clickable during a failed-reconnect modal. */
  .sim-chip,
  .sim-popover {
    position: fixed;
    right: 1rem;
    z-index: 200;
  }

  .sim-chip {
    bottom: 1rem;
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    background: var(--surface);
    color: var(--muted);
    border: 1px solid var(--border);
    border-radius: 999px;
    padding: 0.3rem 0.7rem;
    font-size: 0.7rem;
    font-weight: 700;
    letter-spacing: 0.08em;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
  }

  .sim-chip:hover,
  .sim-chip.is-open {
    color: var(--accent);
    border-color: var(--accent);
  }

  .sim-popover {
    bottom: 3.7rem;
  }
</style>
