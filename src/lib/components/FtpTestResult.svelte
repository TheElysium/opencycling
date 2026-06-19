<script lang="ts">
  import type { SessionDetail } from '$lib/db';
  import { getSettings, updateSettings } from '$lib/settings';
  import { estimateFtpFromRamp } from '$lib/ftp';

  let { detail, oldFtp }: { detail: SessionDetail; oldFtp: number } = $props();

  let applied = $state(false);
  let saving = $state(false);
  let newFtp = $derived(estimateFtpFromRamp(detail.metrics.map((m) => m.power_w ?? 0)));
  let deltaW = $derived(newFtp - oldFtp);
  let deltaPct = $derived(oldFtp > 0 ? Math.round((deltaW / oldFtp) * 100) : 0);

  async function apply(): Promise<void> {
    saving = true;
    try {
      const s = await getSettings();
      await updateSettings({ ...s, ftp_w: newFtp });
      applied = true;
    } finally {
      saving = false;
    }
  }
</script>

<div class="card ftp-result">
  <h2>Ramp Test Result</h2>
  <div class="ftp-figure">
    <span class="ftp-new">{newFtp}<span class="unit">W</span></span>
    <span class="ftp-delta" class:up={deltaW >= 0} class:down={deltaW < 0}>
      {deltaW >= 0 ? '+' : ''}{deltaW} W ({deltaPct >= 0 ? '+' : ''}{deltaPct}%)
    </span>
  </div>
  <p class="ftp-old">Previous FTP: <strong>{oldFtp} W</strong></p>
  {#if applied}
    <p class="applied">Applied. Your FTP is now {newFtp} W.</p>
  {:else}
    <button class="btn btn-primary full" onclick={apply} disabled={saving}>
      {saving ? 'Applying…' : `Apply ${newFtp} W`}
    </button>
  {/if}
</div>

<style>
  .ftp-result h2 {
    font-size: 1rem;
    font-weight: 600;
    color: var(--muted);
    text-transform: uppercase;
    letter-spacing: 0.05em;
    margin: 0 0 0.75rem;
  }

  .ftp-figure {
    display: flex;
    align-items: baseline;
    gap: 0.75rem;
    margin-bottom: 0.35rem;
  }

  .ftp-new {
    font-size: 2.6rem;
    font-weight: 800;
    line-height: 1;
    color: var(--text);
  }

  .ftp-new .unit {
    font-size: 1rem;
    font-weight: 600;
    color: var(--muted);
    margin-left: 0.15rem;
  }

  .ftp-delta {
    font-size: 0.95rem;
    font-weight: 700;
  }

  .ftp-delta.up { color: #22c55e; }
  .ftp-delta.down { color: #f87171; }

  .ftp-old {
    font-size: 0.9rem;
    color: var(--muted);
    margin: 0 0 1rem;
  }

  .applied {
    font-size: 0.9rem;
    font-weight: 600;
    color: #22c55e;
    margin: 0;
  }

  .btn.full {
    width: 100%;
  }
</style>
