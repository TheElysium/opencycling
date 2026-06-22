<script lang="ts">
  import type { FlatBlock } from '$lib/workout.svelte';
  import type { MetricSample } from '$lib/db';
  import { powerScale, POWER_CAP } from '$lib/chart-scale';

  type ZoneSlice = { color: string; label: string; pct: number };
  type TimeMark  = { t: number; label: string };

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
    for (const c of crossings) stops.push({ offset: c.t * 100, color: c.color });
    stops.push({ offset: 100, color: zone(pEnd).color });
    return stops;
  }

  let {
    blocks,
    height = 80,
    currentPos = null,
    showFtpLine = false,
    showZones = false,
    ftpWatts,
    actualMetrics = null,
    showToggles = false,
    showTimeAxis = false,
    maxHr = 0,
    cadenceWindow = [60, 120] as [number, number],
  }: {
    blocks: FlatBlock[];
    height?: number;
    currentPos?: number | null;
    showFtpLine?: boolean;
    showZones?: boolean;
    /** Athlete FTP in watts. Required: bars/zones are computed as power_w / ftpWatts. */
    ftpWatts: number;
    actualMetrics?: MetricSample[] | null;
    showToggles?: boolean;
    showTimeAxis?: boolean;
    /** Athlete max HR, drives the HR overlay window. 0 falls back to 80-200. */
    maxHr?: number;
    /** Cadence window [min, max] for the cadence overlay. */
    cadenceWindow?: [number, number];
  } = $props();

  // Derived HR window: bottom ~ 40 % of max HR up to max HR. Falls back to
  // 80-200 when maxHr isn't provided so old call sites still render.
  let hrLo = $derived(maxHr > 0 ? Math.round(maxHr * 0.4) : 80);
  let hrHi = $derived(maxHr > 0 ? maxHr : 200);
  let cadLo = $derived(cadenceWindow[0]);
  let cadHi = $derived(cadenceWindow[1]);

  // Layers visibility (used when showToggles=true). Defaults: show what we have.
  let visible = $state({ target: true, power: true, hr: true, cad: false });

  // %FTP helpers: blocks store watts, the chart's zone/scale logic works in %FTP.
  function pStart(b: FlatBlock): number { return ftpWatts > 0 ? b.power_start_w / ftpWatts : 0; }
  function pEnd(b: FlatBlock): number   { return ftpWatts > 0 ? b.power_end_w   / ftpWatts : 0; }
  function isRamp(b: FlatBlock): boolean { return b.power_start_w !== b.power_end_w; }

  function computeZones(flat: FlatBlock[]): ZoneSlice[] {
    const total = flat.reduce((s, b) => s + b.duration_s, 0);
    if (total === 0) return [];
    const acc: Record<string, number> = {};
    for (const z of ZONES) acc[z.label] = 0;
    for (const b of flat) {
      const pct = isRamp(b) ? (pStart(b) + pEnd(b)) / 2 : pStart(b);
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

  function formatHMS(s: number): string {
    const h = Math.floor(s / 3600);
    const m = Math.floor((s % 3600) / 60);
    const sec = Math.floor(s % 60);
    if (h > 0) return `${h}:${String(m).padStart(2, '0')}:${String(sec).padStart(2, '0')}`;
    return `${m}:${String(sec).padStart(2, '0')}`;
  }

  let flat        = $derived(blocks);
  let totalDur    = $derived(flat.reduce((s, b) => s + b.duration_s, 0));
  let zones       = $derived(showZones ? computeZones(flat) : []);
  let showTime    = $derived(showZones || showTimeAxis);
  let timeMarks   = $derived(showTime ? computeTimeMarks(totalDur) : []);

  let plannedMaxPct = $derived(flat.reduce((m, b) => {
    const p = isRamp(b) ? Math.max(pStart(b), pEnd(b)) : pStart(b);
    return Math.max(m, p);
  }, 0));

  let actualMaxPct = $derived.by(() => {
    if (!actualMetrics || ftpWatts <= 0) return 0;
    let m = 0;
    for (const s of actualMetrics) {
      if (s.power_w != null && s.power_w > m) m = s.power_w;
    }
    return m / ftpWatts;
  });

  let maxPct      = $derived(Math.max(plannedMaxPct, actualMaxPct));
  // Vertical scale adapts to the tallest planned/actual fraction so tall charts
  // (VO2 spikes, the 640% ramp test) fit instead of clipping into a flat wall.
  let scale       = $derived(powerScale(maxPct));
  let ftpYPct     = $derived(100 - Math.min(1.0 * scale, POWER_CAP)); // FTP (1.0) line position
  let maxY        = $derived(100 - Math.min(maxPct * scale, POWER_CAP));
  let maxLabelTop = $derived(Math.round(height * (maxY / 100)));
  let ftpLabelTop = $derived(Math.round(height * (ftpYPct / 100)));
  let maxWatts    = $derived(ftpWatts > 0 ? Math.round(maxPct * ftpWatts) : 0);

  let xPositions  = $derived.by(() => {
    const positions: number[] = [];
    let x = 0;
    for (const b of flat) {
      positions.push(x);
      x += b.duration_s;
    }
    return positions;
  });

  // Y mapping helpers (viewBox 0..100, top=0). HR/cadence share the power band
  // mapped linearly to bpm/rpm windows; these are only used to *position the
  // line on screen*, no axis is drawn.
  function yOfPowerW(w: number): number {
    if (ftpWatts <= 0) return 100;
    return 100 - Math.min((w / ftpWatts) * scale, POWER_CAP);
  }
  function yOfHr(bpm: number): number {
    // Window driven by athlete max HR (hrLo..hrHi); falls back to 80-200.
    const span = Math.max(1, hrHi - hrLo);
    return 100 - Math.max(0, Math.min(1, (bpm - hrLo) / span)) * 100;
  }
  function yOfCad(rpm: number): number {
    const span = Math.max(1, cadHi - cadLo);
    return 100 - Math.max(0, Math.min(1, (rpm - cadLo) / span)) * 100;
  }

  function buildLine(samples: MetricSample[], gy: (s: MetricSample) => number | null): string {
    const pts: [number, number][] = [];
    for (const s of samples) {
      const y = gy(s);
      if (y == null) continue;
      pts.push([s.t_offset_s, y]);
    }
    if (pts.length === 0) return '';
    return pts.map(([x, y], i) => `${i === 0 ? 'M' : 'L'}${x.toFixed(1)},${y.toFixed(2)}`).join(' ');
  }

  function buildArea(samples: MetricSample[]): string {
    let first: number | null = null;
    let last: number | null = null;
    const pts: [number, number][] = [];
    for (const s of samples) {
      if (s.power_w == null) continue;
      const y = yOfPowerW(s.power_w);
      if (first == null) first = s.t_offset_s;
      last = s.t_offset_s;
      pts.push([s.t_offset_s, y]);
    }
    if (pts.length === 0 || first == null || last == null) return '';
    const line = pts.map(([x, y], i) => `${i === 0 ? 'M' : 'L'}${x.toFixed(1)},${y.toFixed(2)}`).join(' ');
    return `${line} L${last.toFixed(1)},100 L${first.toFixed(1)},100 Z`;
  }

  let powerLine = $derived(actualMetrics ? buildLine(actualMetrics, s => s.power_w == null ? null : yOfPowerW(s.power_w)) : '');
  let powerArea = $derived(actualMetrics ? buildArea(actualMetrics) : '');
  let hrLine    = $derived(actualMetrics ? buildLine(actualMetrics, s => s.hr_bpm == null ? null : yOfHr(s.hr_bpm)) : '');
  let cadLine   = $derived(actualMetrics ? buildLine(actualMetrics, s => s.cadence_rpm == null ? null : yOfCad(s.cadence_rpm)) : '');

  let vTarget = $derived(showToggles ? visible.target : true);
  let vPower  = $derived(showToggles ? visible.power  : true);
  let vHr     = $derived(showToggles ? visible.hr     : true);
  let vCad    = $derived(showToggles ? visible.cad    : true);

  let targetOpacity = $derived(actualMetrics ? 0.55 : 1);

  function blockPoints(b: FlatBlock, x: number): string {
    const hStart = Math.min(pStart(b) * scale, POWER_CAP);
    const hEnd   = Math.min(pEnd(b)   * scale, POWER_CAP);
    const w      = b.duration_s;
    return [
      `${x},100`,
      `${x + w},100`,
      `${x + w},${100 - hEnd}`,
      `${x},${100 - hStart}`,
    ].join(' ');
  }

  function toggle(layer: 'target'|'power'|'hr'|'cad') {
    visible = { ...visible, [layer]: !visible[layer] };
  }

  // ---------- Hover tooltip ----------
  let hoverIdx = $state<number | null>(null);

  function onMove(e: MouseEvent) {
    if (!actualMetrics || actualMetrics.length === 0 || totalDur <= 0) return;
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    const x = e.clientX - rect.left;
    const t = (x / rect.width) * totalDur;
    // Binary search for the sample whose t_offset_s is closest to t.
    let lo = 0, hi = actualMetrics.length - 1;
    while (lo < hi) {
      const mid = (lo + hi) >> 1;
      if (actualMetrics[mid].t_offset_s < t) lo = mid + 1;
      else hi = mid;
    }
    // Pick the closer of (lo) and (lo - 1).
    if (lo > 0 && Math.abs(actualMetrics[lo - 1].t_offset_s - t) < Math.abs(actualMetrics[lo].t_offset_s - t)) {
      lo = lo - 1;
    }
    hoverIdx = lo;
  }

  function onLeave() {
    hoverIdx = null;
  }

  let hoverSample = $derived(hoverIdx != null && actualMetrics ? actualMetrics[hoverIdx] : null);
  let hoverLeftPct = $derived(hoverSample ? (hoverSample.t_offset_s / totalDur) * 100 : 0);
  // Flip tooltip to the left of the cursor in the right third of the chart.
  let hoverFlip = $derived(hoverLeftPct > 66);
</script>

{#if totalDur > 0}
  {#if showToggles}
    <div class="toggles">
      <button class="toggle-btn" class:on={vTarget} onclick={() => toggle('target')}>
        <span class="swatch swatch-target"></span>Workout
      </button>
      <button class="toggle-btn" class:on={vPower} onclick={() => toggle('power')}>
        <span class="swatch swatch-power"></span>Power
      </button>
      <button class="toggle-btn" class:on={vHr} onclick={() => toggle('hr')}>
        <span class="swatch swatch-hr"></span>HR
      </button>
      <button class="toggle-btn" class:on={vCad} onclick={() => toggle('cad')}>
        <span class="swatch swatch-cad"></span>Cadence
      </button>
    </div>
  {/if}

  <div
    class="chart-wrap"
    style="height: {height}px;"
    role={actualMetrics ? 'figure' : undefined}
    onmousemove={actualMetrics ? onMove : undefined}
    onmouseleave={actualMetrics ? onLeave : undefined}
  >
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
          {#if isRamp(b)}
            <linearGradient id="ramp-{uid}-{i}" x1="0" x2="1" y1="0" y2="0">
              {#each rampGradientStops(pStart(b), pEnd(b)) as s}
                <stop offset="{s.offset}%" stop-color={s.color} />
              {/each}
            </linearGradient>
          {/if}
        {/each}
        <linearGradient id="powerFill-{uid}" x1="0" x2="0" y1="0" y2="1">
          <stop offset="0%" stop-color="#e2e8f0" stop-opacity="0.55"/>
          <stop offset="100%" stop-color="#e2e8f0" stop-opacity="0.05"/>
        </linearGradient>
      </defs>

      {#if showFtpLine && ftpWatts > 0}
        {@const step = maxWatts > 400 ? 100 : maxWatts > 200 ? 50 : 25}
        {#each Array.from({ length: Math.floor(maxWatts / step) }, (_, i) => (i + 1) * step) as w}
          {@const y = yOfPowerW(w)}
          <line x1="0" x2={totalDur} y1={y} y2={y} stroke="rgba(255,255,255,0.08)" stroke-width="1" vector-effect="non-scaling-stroke" />
        {/each}
      {/if}

      {#if vTarget}
        <g opacity={targetOpacity}>
          {#each flat as b, i}
            {@const x = xPositions[i]}
            {#if isRamp(b)}
              <polygon points={blockPoints(b, x)} fill="url(#ramp-{uid}-{i})" stroke="var(--chart-gap, var(--bg))" stroke-width="1" vector-effect="non-scaling-stroke" />
            {:else}
              {@const p = pStart(b)}
              {@const h = Math.min(p * scale, POWER_CAP)}
              <rect x={x} y={100 - h} width={b.duration_s} height={h} fill={zone(p).color} stroke="var(--chart-gap, var(--bg))" stroke-width="1" vector-effect="non-scaling-stroke" />
            {/if}
          {/each}
        </g>
      {/if}

      {#if showFtpLine}
        <line x1="0" x2={totalDur} y1={ftpYPct} y2={ftpYPct} stroke="rgba(255,255,255,0.5)" stroke-width="1.5" vector-effect="non-scaling-stroke" />
        <line x1="0" x2={totalDur} y1={maxY} y2={maxY} stroke="rgba(255,255,255,0.3)" stroke-width="1" stroke-dasharray="4 3" vector-effect="non-scaling-stroke" />
      {/if}

      {#if vPower && powerArea}
        <path d={powerArea} fill="url(#powerFill-{uid})" />
        <path d={powerLine} fill="none" stroke="#ffffff" stroke-width="2" vector-effect="non-scaling-stroke" />
      {/if}

      {#if vHr && hrLine}
        <path d={hrLine} fill="none" stroke="#f87171" stroke-width="1.8" opacity="0.95" vector-effect="non-scaling-stroke" />
      {/if}

      {#if vCad && cadLine}
        <path d={cadLine} fill="none" stroke="#22c55e" stroke-width="1.6" opacity="0.9" stroke-dasharray="4 3" vector-effect="non-scaling-stroke" />
      {/if}

      {#if currentPos !== null}
        <line x1={currentPos} x2={currentPos} y1="0" y2="100" stroke="white" stroke-width="2" vector-effect="non-scaling-stroke" />
      {/if}

      {#if hoverSample}
        <line x1={hoverSample.t_offset_s} x2={hoverSample.t_offset_s} y1="0" y2="100" stroke="white" stroke-width="1" stroke-dasharray="2 3" opacity="0.55" vector-effect="non-scaling-stroke" />
      {/if}
    </svg>

    {#if showFtpLine}
      <span class="ftp-tag ftp-left" style="top: {ftpLabelTop}px;">
        {ftpWatts > 0 ? `FTP ${ftpWatts}W` : 'FTP'}
      </span>
      {#if ftpWatts > 0}
        <span class="ftp-tag ftp-right" style="top: {maxLabelTop}px;">{maxWatts}W</span>
      {/if}
    {/if}

    {#if hoverSample}
      <div class="hover-tip" class:flip={hoverFlip} style="left: {hoverLeftPct}%;">
        <div class="hover-time">{formatHMS(hoverSample.t_offset_s)}</div>
        {#if hoverSample.power_w != null}
          <div class="hover-row"><span class="hover-dot dot-power"></span><span class="hover-val">{hoverSample.power_w}</span><span class="hover-unit">W</span></div>
        {/if}
        {#if hoverSample.hr_bpm != null}
          <div class="hover-row"><span class="hover-dot dot-hr"></span><span class="hover-val">{hoverSample.hr_bpm}</span><span class="hover-unit">bpm</span></div>
        {/if}
        {#if hoverSample.cadence_rpm != null}
          <div class="hover-row"><span class="hover-dot dot-cad"></span><span class="hover-val">{hoverSample.cadence_rpm}</span><span class="hover-unit">rpm</span></div>
        {/if}
      </div>
    {/if}
  </div>

  {#if showTime && timeMarks.length > 0}
    <div class="time-axis">
      {#each timeMarks as m}
        <span class="time-mark" style="left: {(m.t / totalDur) * 100}%;">{m.label}</span>
      {/each}
    </div>
  {/if}

  {#if showZones && zones.length > 0}
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

  /* Hover tooltip */
  .hover-tip {
    position: absolute;
    top: 0.5rem;
    transform: translateX(0.6rem);
    background: rgba(15, 23, 42, 0.95);
    color: #e2e8f0;
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 6px;
    padding: 0.4rem 0.6rem;
    font-size: 0.75rem;
    line-height: 1.3;
    pointer-events: none;
    box-shadow: 0 4px 14px rgba(0, 0, 0, 0.35);
    min-width: 90px;
    z-index: 2;
  }
  .hover-tip.flip {
    transform: translateX(calc(-100% - 0.6rem));
  }
  .hover-time {
    font-variant-numeric: tabular-nums;
    font-weight: 600;
    color: #94a3b8;
    margin-bottom: 0.25rem;
    font-size: 0.7rem;
  }
  .hover-row {
    display: flex;
    align-items: baseline;
    gap: 0.35rem;
    font-variant-numeric: tabular-nums;
  }
  .hover-val {
    font-weight: 700;
    color: #fff;
    margin-left: auto;
  }
  .hover-unit {
    font-size: 0.65rem;
    color: #94a3b8;
  }
  .hover-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    flex-shrink: 0;
  }
  .dot-power { background: #e2e8f0; }
  .dot-hr    { background: #f87171; }
  .dot-cad   { background: #22c55e; }

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
  .zone-badge { display: flex; align-items: center; gap: 0.4rem; }
  .zone-circle {
    width: 28px; height: 28px;
    border-radius: 50%;
    display: flex; align-items: center; justify-content: center;
    font-size: 0.65rem; font-weight: 800; color: #fff;
    flex-shrink: 0;
  }
  .zone-badge-pct { font-size: 0.82rem; font-weight: 600; color: var(--muted); }

  /* Toggle bar */
  .toggles {
    display: flex;
    gap: 0.4rem;
    justify-content: flex-end;
    margin-bottom: 0.7rem;
  }
  .toggle-btn {
    background: transparent;
    border: 1px solid var(--border, #334155);
    color: var(--muted, #94a3b8);
    border-radius: 6px;
    padding: 0.3rem 0.65rem;
    font-size: 0.72rem;
    font-weight: 600;
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    cursor: pointer;
  }
  .toggle-btn.on {
    color: var(--text, #fff);
    background: color-mix(in srgb, var(--text, #fff) 8%, transparent);
    border-color: color-mix(in srgb, var(--text, #fff) 22%, transparent);
  }
  .toggle-btn .swatch {
    width: 8px; height: 8px;
    border-radius: 50%;
    display: inline-block;
    background: #475569;
  }
  .toggle-btn .swatch-target { width: 14px; height: 8px; border-radius: 2px; }
  .toggle-btn.on .swatch-target { background: linear-gradient(90deg,#60a5fa 0 33%,#4ade80 33% 66%,#f87171 66% 100%); }
  .toggle-btn.on .swatch-power  { background: #e2e8f0; }
  .toggle-btn.on .swatch-hr     { background: #f87171; }
  .toggle-btn.on .swatch-cad    { background: #22c55e; }
</style>
