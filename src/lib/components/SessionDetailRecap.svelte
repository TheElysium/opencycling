<script lang="ts">
  import { zoneOf } from '$lib/metrics';
  import { formatDuration } from '$lib/format';
  import {
    type SessionDetail,
    powerZoneDistribution,
    hrZoneDistribution,
  } from '$lib/db';
  import SessionStatsPanel from './SessionStatsPanel.svelte';
  import ZoneBar from './ZoneBar.svelte';

  let { detail, maxHr }: { detail: SessionDetail; maxHr: number } = $props();

  let powerZones = $derived(powerZoneDistribution(detail.metrics, detail.ftp_w_used));
  let hrZones    = $derived(hrZoneDistribution(detail.metrics, maxHr));

  type BlockRow = {
    label: string;
    duration_s: number;
    pill: string;
    pillTitle: string;
    avg_power_w: number | null;
    avg_hr_bpm: number | null;
    avg_cadence_rpm: number | null;
  };

  let blockRows = $derived.by<BlockRow[]>(() => {
    const ftp = detail.ftp_w_used;
    const samples = detail.metrics;
    const rows: BlockRow[] = [];
    let t0 = 0;
    let si = 0;
    for (const b of detail.flat_blocks) {
      const t1 = t0 + b.duration_s;
      let sumP = 0, nP = 0, sumH = 0, nH = 0, sumC = 0, nC = 0;
      while (si < samples.length && samples[si].t_offset_s < t1) {
        const s = samples[si];
        if (s.t_offset_s >= t0) {
          if (s.power_w     != null) { sumP += s.power_w;     nP++; }
          if (s.hr_bpm      != null) { sumH += s.hr_bpm;      nH++; }
          if (s.cadence_rpm != null) { sumC += s.cadence_rpm; nC++; }
        }
        si++;
      }
      const zS = ftp > 0 ? zoneOf(b.power_start_w / ftp) : 1;
      const zE = ftp > 0 ? zoneOf(b.power_end_w / ftp) : 1;
      const pill = zS === zE
        ? `var(--z${zS})`
        : `linear-gradient(to right, var(--z${zS}), var(--z${zE}))`;
      const pillTitle = zS === zE ? `Zone ${zS}` : `Zone ${zS} → ${zE}`;
      rows.push({
        label: b.label,
        duration_s: b.duration_s,
        pill,
        pillTitle,
        avg_power_w:     nP > 0 ? Math.round(sumP / nP) : null,
        avg_hr_bpm:      nH > 0 ? Math.round(sumH / nH) : null,
        avg_cadence_rpm: nC > 0 ? Math.round(sumC / nC) : null,
      });
      t0 = t1;
    }
    return rows;
  });
</script>

<SessionStatsPanel {detail} {maxHr} />

<h2 class="section-title">Power zones</h2>
<div class="card zone-card">
  <ZoneBar distribution={powerZones} labels={['Z1','Z2','Z3','Z4','Z5','Z6']} />
</div>

<h2 class="section-title">Heart rate zones</h2>
<div class="card zone-card">
  <ZoneBar distribution={hrZones} labels={['Z1','Z2','Z3','Z4','Z5']} />
</div>

{#if blockRows.length > 0}
  <h2 class="section-title">Block breakdown</h2>
  <div class="card blocks-card">
    <table class="block-table">
      <thead>
        <tr>
          <th>Block</th>
          <th>Duration</th>
          <th>Avg Power</th>
          <th>Avg HR</th>
          <th>Avg Cadence</th>
        </tr>
      </thead>
      <tbody>
        {#each blockRows as row}
          <tr>
            <td class="col-kind">
              <span class="zone-pill" style="background: {row.pill}" title={row.pillTitle} aria-label={row.pillTitle}></span>
              {row.label}
            </td>
            <td class="col-dur">{formatDuration(row.duration_s)}</td>
            <td class="col-num">{row.avg_power_w     != null ? `${row.avg_power_w} W`     : '—'}</td>
            <td class="col-num">{row.avg_hr_bpm      != null ? `${row.avg_hr_bpm} bpm`    : '—'}</td>
            <td class="col-num">{row.avg_cadence_rpm != null ? `${row.avg_cadence_rpm} rpm` : '—'}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
{/if}

<style>
  .section-title {
    font-size: 0.85rem;
    font-weight: 600;
    color: var(--muted);
    text-transform: uppercase;
    letter-spacing: 0.06em;
    margin: 0 0 0.5rem;
  }

  .card {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 1rem 1.25rem;
  }
  .zone-card { margin-bottom: 1.5rem; }

  .blocks-card {
    padding: 0.5rem 0.75rem;
    margin-bottom: 1.5rem;
  }
  .block-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.82rem;
  }
  .block-table thead th {
    text-align: left;
    font-size: 0.72rem;
    font-weight: 600;
    color: var(--muted);
    text-transform: uppercase;
    letter-spacing: 0.04em;
    padding: 0.35rem 0.5rem 0.5rem;
    border-bottom: 1px solid var(--border);
  }
  .block-table tbody tr { border-bottom: 1px solid var(--border); }
  .block-table tbody tr:last-child { border-bottom: none; }
  .block-table td {
    padding: 0.4rem 0.5rem;
    vertical-align: middle;
  }
  .col-kind {
    font-weight: 600;
    color: var(--text);
  }
  .zone-pill {
    display: inline-block;
    width: 1.35em;
    height: 1.35em;
    border-radius: 0.3em;
    margin-right: 0.55em;
    vertical-align: -0.2em;
    box-shadow: inset 0 0 0 1px rgba(0, 0, 0, 0.08);
  }
  .col-dur {
    white-space: nowrap;
    color: var(--muted);
    width: 6rem;
  }
  .col-num {
    white-space: nowrap;
    color: var(--text);
    font-variant-numeric: tabular-nums;
    width: 7rem;
  }
</style>
