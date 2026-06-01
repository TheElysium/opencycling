<script lang="ts">
  import type { WorkoutBlock } from '$lib/workout.svelte';

  type FlatSteady = { duration_s: number; power_pct: number; is_ramp: false };
  type FlatRamp   = { duration_s: number; power_start_pct: number; power_end_pct: number; is_ramp: true };
  type FlatBlock  = FlatSteady | FlatRamp;

  type ZoneSlice = { color: string; label: string; pct: number };
  type TimeMark  = { t: number; label: string };

  // Vertical compression: 1.0 = 100% FTP maps to this fraction of the chart height,
  // leaving headroom on top for the max-power label.
  const POWER_SCALE = 85;
  const FTP_Y_PCT   = 100 - 1.0 * POWER_SCALE; // = 15
  const POWER_CAP   = 99; // clamp very high power so bars stay visible

  const ZONES = [
    { max: 0.55, color: 'var(--z1)', label: 'Z1' },
    { max: 0.75, color: 'var(--z2)', label: 'Z2' },
    { max: 0.90, color: 'var(--z3)', label: 'Z3' },
    { max: 1.05, color: 'var(--z4)', label: 'Z4' },
    { max: 1.20, color: 'var(--z5)', label: 'Z5' },
    { max: Infinity, color: 'var(--z6)', label: 'Z6' },
  ] as const;

  function zone(pct: number) {
    return ZONES.find(z => pct < z.max) ?? ZONES[ZONES.length - 1];
  }

  // Unique prefix so multiple charts on the same page don't collide on gradient IDs.
  const uid = Math.random().toString(36).slice(2, 9);

  type GradStop = { offset: number; color: string };

  function rampGradientStops(pStart: number, pEnd: number): GradStop[] {
    if (pStart === pEnd) return [{ offset: 0, color: zone(pStart).color }];
    const stops: GradStop[] = [];
    const crossings: Array<{ t: number; color: string }> = [];
    for (const z of ZONES) {
      if (z.max === Infinity) continue;
      const t = (z.max - pStart) / (pEnd - pStart);
      if (t > 0 && t < 1) {
        const eps = 1e-6;
        const pAfter = pStart + (pEnd - pStart) * Math.min(1, t + eps);
        crossings.push({ t, color: zone(pAfter).color });
      }
    }
    crossings.sort((a, b) => a.t - b.t);

    stops.push({ offset: 0, color: zone(pStart).color });
    for (const c of crossings) {
      stops.push({ offset: c.t * 100, color: c.color });
    }
    stops.push({ offset: 100, color: zone(pEnd).color });
    return stops;
  }

  let { blocks, height = 80, currentPos = null, showFtpLine = false, showZones = false, ftpWatts = 0 }:
    { blocks: WorkoutBlock[]; height?: number; currentPos?: number | null; showFtpLine?: boolean; showZones?: boolean; ftpWatts?: number } = $props();

  function flatten(bs: WorkoutBlock[]): FlatBlock[] {
    const result: FlatBlock[] = [];
    for (const b of bs) {
      if ('SteadyState' in b) {
        result.push({ duration_s: b.SteadyState.duration_s, power_pct: b.SteadyState.power_pct, is_ramp: false });
      } else if ('Ramp' in b) {
        result.push({ duration_s: b.Ramp.duration_s, power_start_pct: b.Ramp.power_start_pct, power_end_pct: b.Ramp.power_end_pct, is_ramp: true });
      } else if ('IntervalsT' in b) {
        const { repeat, on, off } = b.IntervalsT;
        for (let i = 0; i < repeat; i++) {
          result.push(...flatten([on]), ...flatten([off]));
        }
      }
    }
    return result;
  }

  function computeZones(flat: FlatBlock[]): ZoneSlice[] {
    const total = flat.reduce((s, b) => s + b.duration_s, 0);
    if (total === 0) return [];
    const acc: Record<string, number> = {};
    for (const z of ZONES) acc[z.label] = 0;
    for (const b of flat) {
      const pct = b.is_ramp ? (b.power_start_pct + b.power_end_pct) / 2 : b.power_pct;
      acc[zone(pct).label] += b.duration_s;
    }
    return ZONES
      .map(z => ({ label: z.label, color: z.color, pct: acc[z.label] / total }))
      .filter(z => z.pct > 0);
  }

  function computeTimeMarks(dur: number): TimeMark[] {
    const interval = dur < 1800 ? 600 : dur < 7200 ? 900 : 1800;
    const marks: TimeMark[] = [];
    for (let t = interval; t < dur; t += interval) {
      const m = Math.floor(t / 60);
      const h = Math.floor(m / 60);
      const label = h > 0 ? `${h}h${String(m % 60).padStart(2, '0')}` : `${m}min`;
      marks.push({ t, label });
    }
    return marks;
  }

  function computePowerGridlines(ftp: number, maxPct: number): number[] {
    if (ftp <= 0) return [];
    const maxW = maxPct * ftp;
    const step = maxW > 400 ? 100 : maxW > 200 ? 50 : 25;
    const out: number[] = [];
    for (let w = step; w <= maxW + step; w += step) out.push(w);
    return out;
  }

  let flat        = $derived(flatten(blocks));
  let totalDur    = $derived(flat.reduce((s, b) => s + b.duration_s, 0));
  let zones       = $derived(showZones ? computeZones(flat) : []);
  let timeMarks   = $derived(showZones ? computeTimeMarks(totalDur) : []);
  let maxPct = $derived(flat.reduce((m, b) => {
    const p = b.is_ramp ? Math.max(b.power_start_pct, b.power_end_pct) : b.power_pct;
    return Math.max(m, p);
  }, 0));
  let maxY        = $derived(100 - Math.min(maxPct * POWER_SCALE, POWER_CAP));
  let maxLabelTop = $derived(Math.round(height * (maxY / 100)));
  let ftpLabelTop = $derived(Math.round(height * (FTP_Y_PCT / 100)));
  let xPositions  = $derived.by(() => {
    const positions: number[] = [];
    let x = 0;
    for (const b of flat) {
      positions.push(x);
      x += b.duration_s;
    }
    return positions;
  });
  let gridlines = $derived(showFtpLine ? computePowerGridlines(ftpWatts, maxPct) : []);

  function blockPoints(b: FlatRamp, x: number): string {
    const hStart = Math.min(b.power_start_pct * POWER_SCALE, POWER_CAP);
    const hEnd   = Math.min(b.power_end_pct   * POWER_SCALE, POWER_CAP);
    const w      = b.duration_s;
    return [
      `${x},100`,
      `${x + w},100`,
      `${x + w},${100 - hEnd}`,
      `${x},${100 - hStart}`,
    ].join(' ');
  }
</script>

{#if totalDur > 0}
  <div class="chart-wrap" style="height: {height}px;">
    <svg
      viewBox="0 0 {totalDur} 100"
      preserveAspectRatio="none"
      width="100%"
      height="100%"
      role="img"
      aria-label="Workout power profile"
    >
      <defs>
        {#each flat as b, i}
          {#if b.is_ramp}
            <linearGradient id="ramp-{uid}-{i}" x1="0" x2="1" y1="0" y2="0">
              {#each rampGradientStops(b.power_start_pct, b.power_end_pct) as s}
                <stop offset="{s.offset}%" stop-color={s.color} />
              {/each}
            </linearGradient>
          {/if}
        {/each}
      </defs>

      {#if showFtpLine}
        {#each gridlines as w}
          {@const y = 100 - Math.min((w / ftpWatts) * POWER_SCALE, POWER_CAP)}
          <line
            x1="0" x2={totalDur} y1={y} y2={y}
            stroke="rgba(255,255,255,0.08)"
            stroke-width="1"
            vector-effect="non-scaling-stroke"
          />
        {/each}
      {/if}

      {#each flat as b, i}
        {@const x = xPositions[i]}
        {#if b.is_ramp}
          <polygon
            points={blockPoints(b, x)}
            fill="url(#ramp-{uid}-{i})"
            stroke="var(--chart-gap, var(--bg))"
            stroke-width="1"
            vector-effect="non-scaling-stroke"
          />
        {:else}
          {@const h = Math.min(b.power_pct * POWER_SCALE, POWER_CAP)}
          <rect
            x={x}
            y={100 - h}
            width={b.duration_s}
            height={h}
            fill={zone(b.power_pct).color}
            stroke="var(--chart-gap, var(--bg))"
            stroke-width="1"
            vector-effect="non-scaling-stroke"
          />
        {/if}
      {/each}

      {#if showFtpLine}
        <line
          x1="0" x2={totalDur} y1={FTP_Y_PCT} y2={FTP_Y_PCT}
          stroke="rgba(255,255,255,0.5)"
          stroke-width="1.5"
          vector-effect="non-scaling-stroke"
        />
        <line
          x1="0" x2={totalDur} y1={maxY} y2={maxY}
          stroke="rgba(255,255,255,0.3)"
          stroke-width="1"
          stroke-dasharray="4 3"
          vector-effect="non-scaling-stroke"
        />
      {/if}

      {#if currentPos !== null}
        <line x1={currentPos} x2={currentPos} y1="0" y2="100" stroke="white" stroke-width="2" />
      {/if}
    </svg>

    {#if showFtpLine}
      <span class="ftp-tag ftp-left" style="top: {ftpLabelTop}px;">
        {ftpWatts > 0 ? `FTP ${ftpWatts}W` : 'FTP'}
      </span>
      {#if ftpWatts > 0}
        <span class="ftp-tag ftp-right" style="top: {maxLabelTop}px;">
          {Math.round(maxPct * ftpWatts)}W
        </span>
      {/if}
    {/if}
  </div>

  {#if showZones}
    {#if timeMarks.length > 0}
      <div class="time-axis">
        {#each timeMarks as m}
          <span class="time-mark" style="left: {(m.t / totalDur) * 100}%;">{m.label}</span>
        {/each}
      </div>
    {/if}

    {#if zones.length > 0}
      <div class="zone-badges">
        {#each zones as z}
          <div class="zone-badge">
            <span class="zone-circle" style="background: {z.color};">{z.label}</span>
            <span class="zone-badge-pct">{Math.round(z.pct * 100)}%</span>
          </div>
        {/each}
      </div>
    {/if}
  {/if}
{/if}

<style>
  .chart-wrap {
    position: relative;
    width: 100%;
  }

  .ftp-tag {
    position: absolute;
    transform: translateY(-50%);
    font-size: 0.7rem;
    font-weight: 600;
    color: var(--muted);
    pointer-events: none;
    line-height: 1;
    background: var(--chart-gap, var(--bg));
    padding: 0 4px;
  }

  .ftp-left { left: 0; }
  .ftp-right { right: 0; }

  .time-axis {
    position: relative;
    height: 18px;
    margin-top: 2px;
  }

  .time-mark {
    position: absolute;
    transform: translateX(-50%);
    font-size: 0.68rem;
    color: var(--muted);
    white-space: nowrap;
  }

  .zone-badges {
    display: flex;
    gap: 0.85rem;
    margin-top: 0.75rem;
    flex-wrap: wrap;
    justify-content: center;
  }

  .zone-badge {
    display: flex;
    align-items: center;
    gap: 0.4rem;
  }

  .zone-circle {
    width: 28px;
    height: 28px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 0.65rem;
    font-weight: 800;
    color: #fff;
    flex-shrink: 0;
  }

  .zone-badge-pct {
    font-size: 0.82rem;
    font-weight: 600;
    color: var(--muted);
  }
</style>
