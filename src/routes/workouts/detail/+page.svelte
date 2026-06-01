<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { ArrowLeft, HelpCircle } from '@lucide/svelte';
  import WorkoutChart from '$lib/components/WorkoutChart.svelte';
  import { workoutSelection, type WorkoutBlock } from '$lib/workout.svelte';
  import { ble } from '$lib/ble.svelte';
  import { blockDuration, totalDuration, formatDuration, displayWorkoutName, stripHtml } from '$lib/format';
  import { computeWorkoutMetrics, workoutTypeColor, zoneOf } from '$lib/metrics';
  import { getSettings } from '$lib/settings';

  let ftp = $state(200);

  onMount(async () => {
    if (!workoutSelection.workout) {
      goto('/workouts');
      return;
    }
    try {
      const s = await getSettings();
      ftp = s.ftp_w;
    } catch {
      // fallback ftp already set
    }
  });

  function pwr(pct: number): string {
    return `${Math.round(pct * 100)}% · ${Math.round(pct * ftp)}W`;
  }

  type BlockRow = { kind: string; duration: string; power: string; cadence: string | null; pill: string; pillTitle: string };

  function pillFor(b: WorkoutBlock): { bg: string; title: string } {
    if ('SteadyState' in b) {
      const z = zoneOf(b.SteadyState.power_pct);
      return { bg: `var(--z${z})`, title: `Zone ${z}` };
    }
    if ('Ramp' in b) {
      const zS = zoneOf(b.Ramp.power_start_pct);
      const zE = zoneOf(b.Ramp.power_end_pct);
      if (zS === zE) return { bg: `var(--z${zS})`, title: `Zone ${zS}` };
      return {
        bg: `linear-gradient(to right, var(--z${zS}), var(--z${zE}))`,
        title: `Zone ${zS} → ${zE}`,
      };
    }
    const { on, off } = b.IntervalsT;
    const onPct  = 'SteadyState' in on  ? on.SteadyState.power_pct  : 'Ramp' in on  ? on.Ramp.power_start_pct  : 0;
    const offPct = 'SteadyState' in off ? off.SteadyState.power_pct : 'Ramp' in off ? off.Ramp.power_start_pct : 0;
    const zOn = zoneOf(onPct);
    const zOff = zoneOf(offPct);
    if (zOn === zOff) return { bg: `var(--z${zOn})`, title: `Zone ${zOn}` };
    return {
      bg: `linear-gradient(to right, var(--z${zOn}) 50%, var(--z${zOff}) 50%)`,
      title: `Zone ${zOn} on / Zone ${zOff} off`,
    };
  }

  function describeBlock(b: WorkoutBlock): BlockRow {
    const p = pillFor(b);
    if ('SteadyState' in b) {
      const { duration_s, power_pct, cadence_rpm, label } = b.SteadyState;
      return {
        kind: label ?? 'Steady',
        duration: formatDuration(duration_s),
        power: pwr(power_pct),
        cadence: cadence_rpm ? `${cadence_rpm} rpm` : null,
        pill: p.bg,
        pillTitle: p.title,
      };
    }
    if ('Ramp' in b) {
      const { duration_s, power_start_pct, power_end_pct, cadence_rpm, label } = b.Ramp;
      return {
        kind: label ?? 'Ramp',
        duration: formatDuration(duration_s),
        power: `${Math.round(power_start_pct * 100)}→${Math.round(power_end_pct * 100)}% · ${Math.round(power_start_pct * ftp)}→${Math.round(power_end_pct * ftp)}W`,
        cadence: cadence_rpm ? `${cadence_rpm} rpm` : null,
        pill: p.bg,
        pillTitle: p.title,
      };
    }
    const { repeat, on, off } = b.IntervalsT;
    const onPct  = 'SteadyState' in on  ? on.SteadyState.power_pct  : 'Ramp' in on  ? on.Ramp.power_start_pct  : 0;
    const offPct = 'SteadyState' in off ? off.SteadyState.power_pct : 'Ramp' in off ? off.Ramp.power_start_pct : 0;
    return {
      kind: `${repeat}×`,
      duration: `${formatDuration(blockDuration(on))} on · ${formatDuration(blockDuration(off))} off`,
      power: `${pwr(onPct)} on / ${pwr(offPct)} off`,
      cadence: null,
      pill: p.bg,
      pillTitle: p.title,
    };
  }

  let w         = $derived(workoutSelection.workout);
  let blockRows = $derived(w ? w.workout_blocks.map(describeBlock) : []);
  let metrics   = $derived(w ? computeWorkoutMetrics(w.workout_blocks, ftp) : null);

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

{#if w}
  <div class="detail">
    <button class="back-btn" aria-label="Back to workouts" onclick={() => goto('/workouts')}>
      <ArrowLeft size={22} />
    </button>

    <header class="hero">
      {#if metrics && metrics.tss > 0}
        <span class="type-badge" style="--type-color: {workoutTypeColor(metrics.type)}">
          <span class="type-dot"></span>{metrics.type}
        </span>
      {/if}
      <h1>{displayWorkoutName(w.name)}</h1>
      <p class="meta">
        <span>{formatDuration(totalDuration(w.workout_blocks))}</span>
        {#if w.author}<span class="meta-sep">·</span><span>{w.author}</span>{/if}
      </p>
      {#if w.description}
        {@const desc = stripHtml(w.description)}
        {#if desc}<p class="description">{desc}</p>{/if}
      {/if}
    </header>

    {#if metrics && metrics.tss > 0}
      <div class="metrics-strip">
        <div class="metric-cell" title="Training Stress Score — 100 ≈ 1h at FTP">
          <span class="metric-value">{Math.round(metrics.tss)}</span>
          <span class="metric-label">Stress</span>
          <span class="metric-acro">TSS</span>
        </div>
        <div class="metric-cell" title="Intensity Factor — average intensity (1.0 = FTP)">
          <span class="metric-value">{metrics.if_.toFixed(2)}</span>
          <span class="metric-label">Intensity</span>
          <span class="metric-acro">IF</span>
        </div>
        <div class="metric-cell" title="Normalized Power — physiological average">
          <span class="metric-value">{Math.round(metrics.np_pct * ftp)}<span class="metric-unit">W</span></span>
          <span class="metric-label">Avg Power</span>
          <span class="metric-acro">NP</span>
        </div>
        <div class="metric-cell" title="Total energy produced">
          <span class="metric-value">{Math.round(metrics.kj)}<span class="metric-unit">kJ</span></span>
          <span class="metric-label">Work</span>
          <span class="metric-acro">kJ</span>
        </div>
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
                <dt>Stress <span class="acro">TSS</span></dt>
                <dd>Total cost of the session. 100 = exactly 1h at your FTP. Lets you compare workout difficulty.</dd>
                <dt>Intensity <span class="acro">IF</span></dt>
                <dd>Average intensity relative to FTP. 0.65 = recovery, 0.85 = tempo, 1.0 = threshold, &gt;1.05 = VO2max.</dd>
                <dt>Avg Power <span class="acro">NP</span></dt>
                <dd>Normalized Power — physiologically-weighted average. Smooths spikes to reflect the real cost on the body.</dd>
                <dt>Work <span class="acro">kJ</span></dt>
                <dd>Total energy produced. Good calorie proxy (1 kJ ≈ 1 kcal in cycling).</dd>
              </dl>
            </div>
          {/if}
        </div>
      </div>
    {/if}

    <div class="chart-section">
      <WorkoutChart blocks={w.workout_blocks} height={200} showFtpLine={true} showZones={true} ftpWatts={ftp} />
    </div>

    <div class="cta-row">
      {#if ble.trainerStatus === 'connected'}
        <button class="btn-start" onclick={() => goto('/session')}>Start Ride</button>
      {:else}
        <button class="btn-start" onclick={() => goto('/')}>Connect Trainer</button>
      {/if}
    </div>

    {#if blockRows.length > 0}
      <h2 class="section-title">Block breakdown</h2>
      <div class="card blocks-card">
        <table class="block-table">
          <thead>
            <tr>
              <th>Block</th>
              <th>Duration</th>
              <th>Power</th>
              <th>Cadence</th>
            </tr>
          </thead>
          <tbody>
            {#each blockRows as row}
              <tr>
                <td class="col-kind">
                  <span class="zone-pill" style="background: {row.pill}" title={row.pillTitle} aria-label={row.pillTitle}></span>
                  {row.kind}
                </td>
                <td class="col-dur">{row.duration}</td>
                <td class="col-power">{row.power}</td>
                <td class="col-cad">{row.cadence ?? ''}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
  </div>
{/if}

<style>
  .detail {
    max-width: 900px;
  }

  .back-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    background: none;
    border: none;
    color: var(--muted);
    padding: 0;
    margin-bottom: 0.5rem;
    cursor: pointer;
    transition: color 0.15s;
  }

  .back-btn:hover {
    color: var(--text);
  }

  .hero {
    margin-bottom: 1.25rem;
  }

  .type-badge {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    font-size: 0.72rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--type-color);
    background: color-mix(in srgb, var(--type-color) 14%, transparent);
    border-radius: 4px;
    padding: 0.2rem 0.6rem;
    margin-bottom: 0.5rem;
  }

  .type-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--type-color);
  }

  h1 {
    font-size: 2rem;
    font-weight: 800;
    margin: 0 0 0.35rem;
    letter-spacing: -0.01em;
  }

  .meta {
    font-size: 0.9rem;
    color: var(--muted);
    margin: 0 0 0.5rem;
    display: flex;
    gap: 0.4rem;
    align-items: center;
  }

  .meta-sep { opacity: 0.6; }

  .description {
    font-size: 0.95rem;
    color: var(--text);
    margin: 0;
    line-height: 1.55;
    max-width: 65ch;
    white-space: pre-line;
  }

  .metrics-strip {
    display: grid;
    grid-template-columns: repeat(4, 1fr) auto;
    gap: 0.5rem;
    align-items: start;
    margin-bottom: 1.25rem;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 0.85rem 1rem;
  }

  .metric-cell {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.15rem;
    cursor: help;
  }

  .metric-value {
    font-size: 1.4rem;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
    color: var(--text);
    line-height: 1;
  }

  .metric-unit {
    font-size: 0.7rem;
    font-weight: 500;
    color: var(--muted);
    margin-left: 0.1rem;
  }

  .metric-label {
    font-size: 0.72rem;
    color: var(--text);
    font-weight: 600;
    margin-top: 0.1rem;
  }

  .metric-acro {
    font-size: 0.62rem;
    color: var(--muted);
    text-transform: uppercase;
    letter-spacing: 0.06em;
    font-weight: 600;
  }

  .metrics-help {
    position: relative;
    align-self: start;
  }

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

  .help-btn:hover {
    color: var(--text);
    background: var(--bg);
  }

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

  .chart-section {
    background: var(--surface-dark);
    border-radius: 10px;
    padding: 1.25rem 1rem 1rem;
    margin-bottom: 1.25rem;
    --chart-gap: var(--surface-dark);
    --muted: #94a3b8;
    --text: #e2e8f0;
  }

  .cta-row {
    display: flex;
    align-items: center;
    gap: 0.85rem;
    margin-bottom: 1.5rem;
  }

  .btn-start {
    background: var(--accent);
    color: #fff;
    font-size: 1rem;
    font-weight: 700;
    padding: 0.75rem 2.25rem;
    border-radius: 10px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    transition: opacity 0.15s, transform 0.1s;
  }

  .btn-start:hover { opacity: 0.9; }
  .btn-start:active { transform: scale(0.98); }

  .section-title {
    font-size: 0.85rem;
    font-weight: 600;
    color: var(--muted);
    text-transform: uppercase;
    letter-spacing: 0.06em;
    margin: 0 0 0.5rem;
  }

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

  .block-table tbody tr {
    border-bottom: 1px solid var(--border);
  }

  .block-table tbody tr:last-child { border-bottom: none; }

  .block-table td {
    padding: 0.4rem 0.5rem;
    vertical-align: middle;
  }

  .col-kind {
    font-weight: 600;
    white-space: nowrap;
    color: var(--text);
    width: 6rem;
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
    width: 7rem;
  }

  .col-power { color: var(--text); }

  .col-cad {
    white-space: nowrap;
    color: var(--muted);
    text-align: right;
  }
</style>
