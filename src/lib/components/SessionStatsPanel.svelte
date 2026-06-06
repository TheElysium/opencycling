<script lang="ts">
  import { Zap, Heart, RotateCw, Flame, Target, Battery, HelpCircle } from '@lucide/svelte';
  import { type SessionDetail, computeSessionMetrics } from '$lib/db';
  import MetricsStrip, { type MetricTile } from './MetricsStrip.svelte';
  import SessionChart from './SessionChart.svelte';

  let { detail, maxHr }: { detail: SessionDetail; maxHr: number } = $props();

  let m = $derived(computeSessionMetrics(detail));

  let metricTiles = $derived<MetricTile[]>([
    { icon: Zap,      label: 'Avg Power',      value: detail.avg_power_w     ?? '—', unit: detail.avg_power_w     != null ? 'W'   : undefined, secondary: { label: 'Max', value: detail.max_power_w     ?? '—', unit: 'W' } },
    { icon: Heart,    label: 'Avg Heart rate', value: detail.avg_hr_bpm      ?? '—', unit: detail.avg_hr_bpm      != null ? 'bpm' : undefined, secondary: { label: 'Max', value: detail.max_hr_bpm      ?? '—' } },
    { icon: RotateCw, label: 'Avg Cadence',    value: detail.avg_cadence_rpm ?? '—', unit: detail.avg_cadence_rpm != null ? 'rpm' : undefined, secondary: { label: 'Max', value: detail.max_cadence_rpm ?? '—' } },
    { icon: Flame,    label: 'Stress',         value: Math.round(m.tss),                                                                       secondary: { label: 'TSS' } },
    { icon: Target,   label: 'Intensity',      value: m.if_.toFixed(2),                                                                        secondary: { label: 'IF'  } },
    { icon: Battery,  label: 'Work',           value: detail.avg_power_w != null && detail.duration_s != null
                                                       ? Math.round(detail.avg_power_w * detail.duration_s / 1000)
                                                       : '—',
                                               unit: detail.avg_power_w != null && detail.duration_s != null ? 'kJ' : undefined,
                                                                                                                                              secondary: { label: 'kJ'  } },
  ]);

  let helpOpen = $state(false);
  function onDocClick(e: MouseEvent) {
    if (!(e.target as HTMLElement).closest('.metrics-help')) helpOpen = false;
  }
  $effect(() => {
    if (helpOpen) {
      document.addEventListener('click', onDocClick);
      return () => document.removeEventListener('click', onDocClick);
    }
  });
</script>

<div class="stats-panel">
<MetricsStrip tiles={metricTiles}>
  {#snippet trailing()}
    <div class="metrics-help">
      <button
        type="button"
        class="help-btn"
        aria-label="What do these metrics mean?"
        aria-expanded={helpOpen}
        onclick={() => helpOpen = !helpOpen}
      >
        <HelpCircle size={16} />
      </button>
      {#if helpOpen}
        <div class="help-popover" role="dialog">
          <dl>
            <dt>Avg Power <span class="acro">W</span></dt>
            <dd>Average power over the session. Max shown alongside.</dd>
            <dt>Avg Heart rate <span class="acro">bpm</span></dt>
            <dd>Average heart rate over the session. Max shown alongside.</dd>
            <dt>Avg Cadence <span class="acro">rpm</span></dt>
            <dd>Average pedalling cadence. Max shown alongside.</dd>
            <dt>Stress <span class="acro">TSS</span></dt>
            <dd>Total cost of the session. 100 = exactly 1h at your FTP. Lets you compare workout difficulty.</dd>
            <dt>Intensity <span class="acro">IF</span></dt>
            <dd>Average intensity relative to FTP. 0.65 = recovery, 0.85 = tempo, 1.0 = threshold, &gt;1.05 = VO2max.</dd>
            <dt>Work <span class="acro">kJ</span></dt>
            <dd>Total energy produced. Good calorie proxy (1 kJ ≈ 1 kcal in cycling).</dd>
          </dl>
        </div>
      {/if}
    </div>
  {/snippet}
</MetricsStrip>

<SessionChart
  metrics={detail.metrics}
  blocks={detail.flat_blocks}
  ftpWatts={detail.ftp_w_used}
  maxHr={maxHr}
/>
</div>

<style>
  .stats-panel {
    display: grid;
    grid-template-rows: auto 1fr;
    gap: 1rem;
    min-height: 0;
  }
  .stats-panel :global(.metrics-strip) { margin-bottom: 0; }

  .metrics-help { position: relative; }
  .help-btn {
    background: none;
    border: none;
    color: var(--muted);
    padding: 0.15rem;
    border-radius: 50%;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    transition: color 0.15s, background 0.15s;
  }
  .help-btn:hover { color: var(--text); background: var(--bg); }
  .help-popover {
    position: absolute;
    top: calc(100% + 0.5rem);
    right: 0;
    width: 320px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 0.85rem 1rem;
    box-shadow: 0 6px 24px rgba(0,0,0,0.12);
    z-index: 20;
  }
  .help-popover dl {
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
  }
  .help-popover dt {
    font-size: 0.8rem;
    font-weight: 700;
    color: var(--text);
    display: flex;
    align-items: baseline;
    gap: 0.4rem;
  }
  .help-popover dt .acro {
    font-size: 0.65rem;
    font-weight: 600;
    color: var(--muted);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }
  .help-popover dd {
    margin: 0.1rem 0 0;
    font-size: 0.8rem;
    color: var(--muted);
    line-height: 1.45;
  }
</style>
