<script lang="ts">
  import { onMount } from 'svelte';
  import { toMessage } from '$lib/format';
  import { commands, type DeviceKind } from '$lib/bindings';

  let { variant = 'card' }: { variant?: 'card' | 'floating' } = $props();

  let available = $state(false);
  let busy      = $state<DeviceKind | null>(null);
  let error     = $state<string | null>(null);

  onMount(() => {
    // Availability is only a hint; any failure just hides the panel.
    commands.simAvailable()
      .then((v: boolean) => (available = v))
      .catch(() => (available = false));
  });

  async function simAction(kind: DeviceKind, action: () => Promise<null>) {
    busy  = kind;
    error = null;
    try {
      await action();
    } catch (e) {
      error = toMessage(e);
    } finally {
      busy = null;
    }
  }
</script>

{#snippet simRow(kind: DeviceKind, label: string)}
  <div class="sim-row">
    <span class="sim-device">{label}</span>
    <div class="sim-actions">
      <button
        type="button"
        class="btn-ghost"
        disabled={busy !== null}
        onclick={() => simAction(kind, () => commands.simDropDevice(kind, false))}
      >
        {busy === kind ? 'Dropping…' : 'Drop (auto-recover)'}
      </button>
      <button
        type="button"
        class="btn-ghost"
        disabled={busy !== null}
        onclick={() => simAction(kind, () => commands.simDropDevice(kind, true))}
      >
        {busy === kind ? 'Dropping…' : 'Drop (stay lost)'}
      </button>
      <button
        type="button"
        class="btn-ghost"
        disabled={busy !== null}
        onclick={() => simAction(kind, () => commands.simRestoreDevice(kind))}
      >
        Restore
      </button>
    </div>
  </div>
{/snippet}

{#if available}
  {#if variant === 'card'}
    <section class="card sim-card">
      <h2>Simulation</h2>
      <p class="sim-hint">
        BLE simulator active — no hardware required. Launch the app without OPENYCLING_SIM to use
        real devices.
      </p>
      {@render simRow('Trainer', 'Home Trainer')}
      {@render simRow('Hrm', 'Heart rate monitor')}
      {#if error}
        <p class="error-box">{error}</p>
      {/if}
    </section>
  {:else}
    <div class="sim-float">
      {@render simRow('Trainer', 'Home Trainer')}
      {@render simRow('Hrm', 'Heart rate monitor')}
      {#if error}
        <p class="error-box">{error}</p>
      {/if}
    </div>
  {/if}
{/if}

<style>
  .sim-card {
    display: flex;
    flex-direction: column;
    gap: 1.1rem;
  }

  h2 {
    font-size: 1.1rem;
    font-weight: 600;
    margin: 0;
  }

  .sim-hint {
    margin: 0;
    font-size: 0.8rem;
    color: var(--muted);
    line-height: 1.5;
  }

  .sim-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    flex-wrap: wrap;
  }

  .sim-device {
    font-size: 0.95rem;
    font-weight: 600;
    color: var(--text);
  }

  .sim-actions {
    display: flex;
    gap: 0.5rem;
    flex-wrap: wrap;
  }

  .sim-float {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 10px;
    box-shadow: 0 6px 20px rgba(0, 0, 0, 0.15);
    padding: 0.85rem;
    width: 300px;
    display: flex;
    flex-direction: column;
    gap: 0.7rem;
  }

  .sim-float .sim-row {
    flex-direction: column;
    align-items: flex-start;
    gap: 0.4rem;
  }

  .sim-float .sim-device {
    font-size: 0.85rem;
  }

  .sim-float .sim-actions {
    gap: 0.35rem;
  }

  .sim-float .sim-actions :global(.btn-ghost) {
    font-size: 0.72rem;
    padding: 0.25rem 0.5rem;
  }

  .sim-float :global(.error-box) {
    font-size: 0.75rem;
    padding: 0.4rem 0.6rem;
  }
</style>
