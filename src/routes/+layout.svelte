<script lang="ts">
  import { page } from '$app/state';
  import { onMount } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import { Plug, Dumbbell, History, Settings } from '@lucide/svelte';
  import { ble } from '$lib/ble.svelte';
  import '../app.css';

  let { children } = $props();

  const navItems = [
    { href: '/',          label: 'Connection', icon: Plug      },
    { href: '/workouts',  label: 'Workouts',   icon: Dumbbell  },
    { href: '/history',   label: 'History',    icon: History   },
    { href: '/settings',  label: 'Settings',   icon: Settings  },
  ];

  let showSidebar = $derived(page.url.pathname !== '/session');

  onMount(() => {
    const unlisteners: Array<() => void> = [];

    listen<{ power_w: number | null; hr_bpm: number | null; cadence_rpm: number | null }>('ble_metrics', (e) => {
      ble.metrics = e.payload;
    }).then(fn => unlisteners.push(fn));

    listen<{ device: string; message: string }>('ble_error', (e) => {
      const { device, message } = e.payload;
      if (device === 'trainer') {
        ble.trainerError = message;
      } else {
        ble.hrmError = message;
      }
    }).then(fn => unlisteners.push(fn));

    listen<string>('ble_disconnected', (e) => {
      const device = e.payload;
      if (device === 'trainer') {
        ble.trainerStatus = 'disconnected';
        ble.metrics = ble.metrics ? { ...ble.metrics, power_w: null, cadence_rpm: null } : null;
      } else {
        ble.hrmStatus = 'disconnected';
        ble.metrics = ble.metrics ? { ...ble.metrics, hr_bpm: null } : null;
      }
    }).then(fn => unlisteners.push(fn));

    return () => unlisteners.forEach(fn => fn());
  });
</script>

<div class="shell" class:with-sidebar={showSidebar}>
  {#if showSidebar}
    <nav class="sidebar">
      <div class="logo">OpenCycling</div>
      <ul>
        {#each navItems as item}
          {@const active = page.url.pathname === item.href}
          <li>
            <a href={item.href} class:active>
              <item.icon size={18} />
              <span>{item.label}</span>
            </a>
          </li>
        {/each}
      </ul>
    </nav>
  {/if}
  <main class="content">
    {@render children()}
  </main>
</div>

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
    font-size: 0.85rem;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--accent);
    padding: 0 1.25rem 1.5rem;
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

  .content {
    padding: 2rem;
    min-height: 100vh;
  }
</style>
