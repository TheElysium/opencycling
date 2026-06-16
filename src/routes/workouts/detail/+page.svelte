<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { ArrowLeft, HelpCircle, Flame, Target, Zap, Battery, RotateCw } from '@lucide/svelte';
  import WorkoutPreview from '$lib/components/WorkoutPreview.svelte';
  import MetricsStrip, { type MetricTile } from '$lib/components/MetricsStrip.svelte';
  import { workoutSelection, flattenWorkout, type WorkoutBlock } from '$lib/workout.svelte';
  import { ble } from '$lib/ble.svelte';
  import { blockDuration, totalDuration, formatDuration, displayWorkoutName, stripHtml, toMessage } from '$lib/format';
  import { computeWorkoutMetrics, workoutTypeColor, zoneOf } from '$lib/metrics';
  import { getSettings } from '$lib/settings';
  import { session } from '$lib/session.svelte';

  let ftp = $state(200);
  let aeroFeature = $state(false); // master switch from Settings; gates the per-ride toggle
  let aeroEnabled = $state(false);
  let starting = $state(false);
  let startError = $state<string | null>(null);

  async function startRide() {
    if (!w) return;
    starting = true;
    startError = null;
    try {
      // Defer start_session to the session page: when aero is on it must run the
      // calibration first, and the session must stay unarmed until calibration ends.
      session.prepare(w, ftp, aeroEnabled);
      await goto('/session');
    } catch (e) {
      startError = toMessage(e);
    } finally {
      starting = false;
    }
  }

  onMount(async () => {
    if (!workoutSelection.workout) {
      goto('/workouts');
      return;
    }
    try {
      const s = await getSettings();
      ftp = s.ftp_w;
      // Settings is the master: only expose the per-ride toggle when the feature is
      // on, pre-checked so it applies by default.
      aeroFeature = s.aero_enabled;
      aeroEnabled = s.aero_enabled;
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
    const onCad  = 'SteadyState' in on  ? on.SteadyState.cadence_rpm  : 'Ramp' in on  ? on.Ramp.cadence_rpm  : null;
    const offCad = 'SteadyState' in off ? off.SteadyState.cadence_rpm : 'Ramp' in off ? off.Ramp.cadence_rpm : null;
    let cadence: string | null = null;
    if (onCad != null && offCad != null) cadence = `${onCad} rpm on / ${offCad} rpm off`;
    else if (onCad != null) cadence = `${onCad} rpm on`;
    else if (offCad != null) cadence = `${offCad} rpm off`;
    return {
      kind: `${repeat}×`,
      duration: `${formatDuration(blockDuration(on))} on · ${formatDuration(blockDuration(off))} off`,
      power: `${pwr(onPct)} on / ${pwr(offPct)} off`,
      cadence,
      pill: p.bg,
      pillTitle: p.title,
    };
  }

  let w         = $derived(workoutSelection.workout);
  let blockRows = $derived(w ? w.workout_blocks.map(describeBlock) : []);
  let metrics   = $derived(w ? computeWorkoutMetrics(w.workout_blocks, ftp) : null);

  // Duration-weighted avg cadence across blocks that specify one. null if none specified.
  let avgCadence = $derived.by<number | null>(() => {
    if (!w) return null;
    let num = 0;
    let den = 0;
    for (const f of flattenWorkout(w.workout_blocks, ftp)) {
      if (f.cadence_rpm != null) {
        num += f.cadence_rpm * f.duration_s;
        den += f.duration_s;
      }
    }
    return den > 0 ? Math.round(num / den) : null;
  });

  let metricTiles = $derived<MetricTile[]>(metrics ? [
    { icon: Zap,     label: 'Avg Power',   value: Math.round(metrics.np_pct * ftp), unit: 'W',                            secondary: { label: 'NP'  }, title: 'Normalized Power — physiological average' },
    { icon: RotateCw,label: 'Avg Cadence', value: avgCadence ?? '—',                unit: avgCadence != null ? 'rpm' : undefined },
    { icon: Flame,   label: 'Stress',      value: Math.round(metrics.tss),                                                secondary: { label: 'TSS' }, title: 'Training Stress Score — 100 ≈ 1h at FTP' },
    { icon: Target,  label: 'Intensity',   value: metrics.if_.toFixed(2),                                                 secondary: { label: 'IF'  }, title: 'Intensity Factor — average intensity (1.0 = FTP)' },
    { icon: Battery, label: 'Work',        value: Math.round(metrics.kj),           unit: 'kJ',                           secondary: { label: 'kJ'  }, title: 'Total energy produced' },
  ] : []);

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
        {/snippet}
      </MetricsStrip>
    {/if}

    <div class="chart-section">
      <WorkoutPreview blocks={flattenWorkout(w.workout_blocks, ftp)} ftpWatts={ftp} />
    </div>

    <div class="cta-row">
      {#if ble.trainerStatus === 'connected'}
        <button class="btn-start" onclick={startRide} disabled={starting}>
          {starting ? 'Starting…' : 'Start Ride'}
        </button>
      {:else}
        <button class="btn-start" onclick={() => goto('/')}>Connect Trainer</button>
      {/if}
      {#if startError}
        <span class="error-box">{startError}</span>
      {/if}
    </div>

    {#if ble.trainerStatus === 'connected' && aeroFeature}
      <label class="aero-opt">
        <span class="switch">
          <input type="checkbox" bind:checked={aeroEnabled} />
          <span class="slider"></span>
        </span>
        <span class="aero-opt-text">
          <span class="aero-opt-title">Detect aero position</span>
          <span class="aero-opt-sub">Track how much of this ride you spend in your aero position.</span>
        </span>
      </label>
    {/if}

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

  .metrics-help {
    position: relative;
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
    margin-bottom: 1rem;
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

  /* Per-ride aero option on its own line below the CTA, not orphaned beside it. */
  .aero-opt {
    display: inline-flex;
    align-items: center;
    gap: 0.7rem;
    margin-bottom: 1.5rem;
    cursor: pointer;
  }
  .aero-opt-text {
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
  }
  .aero-opt-title {
    font-size: 0.9rem;
    font-weight: 500;
    color: var(--text);
  }
  .aero-opt-sub {
    font-size: 0.78rem;
    color: var(--muted);
  }

  /* Shared .switch/.slider styles live in app.css. */

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
  }
</style>
