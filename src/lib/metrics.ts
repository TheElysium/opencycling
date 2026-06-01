import type { WorkoutBlock } from './workout.svelte';

export type WorkoutType =
  | 'Recovery'
  | 'Endurance'
  | 'Tempo'
  | 'Sweet Spot'
  | 'Threshold'
  | 'VO2max'
  | 'Anaerobic';

export type WorkoutMetrics = {
  duration_s: number;
  avg_pct: number;
  np_pct: number;
  if_: number;
  tss: number;
  kj: number;
  type: WorkoutType;
};

const ZONE_THRESHOLDS = [0.55, 0.75, 0.90, 1.05, 1.20];

function zoneOf(pct: number): number {
  for (let i = 0; i < ZONE_THRESHOLDS.length; i++) {
    if (pct < ZONE_THRESHOLDS[i]) return i + 1;
  }
  return 6;
}

export function workoutTypeColor(t: WorkoutType): string {
  switch (t) {
    case 'Recovery':   return 'var(--z1)';
    case 'Endurance':  return 'var(--z2)';
    case 'Tempo':      return 'var(--z3)';
    case 'Sweet Spot': return '#a3e635';
    case 'Threshold':  return 'var(--z4)';
    case 'VO2max':     return 'var(--z5)';
    case 'Anaerobic':  return 'var(--z6)';
  }
}

function flattenToSeconds(blocks: WorkoutBlock[]): number[] {
  const out: number[] = [];
  for (const b of blocks) {
    if ('SteadyState' in b) {
      const { duration_s, power_pct } = b.SteadyState;
      for (let i = 0; i < duration_s; i++) out.push(power_pct);
    } else if ('Ramp' in b) {
      const { duration_s, power_start_pct, power_end_pct } = b.Ramp;
      for (let i = 0; i < duration_s; i++) {
        const t = duration_s > 1 ? i / (duration_s - 1) : 0;
        out.push(power_start_pct + (power_end_pct - power_start_pct) * t);
      }
    } else if ('IntervalsT' in b) {
      const { repeat, on, off } = b.IntervalsT;
      for (let r = 0; r < repeat; r++) {
        out.push(...flattenToSeconds([on, off]));
      }
    }
  }
  return out;
}

function classify(series: number[], if_: number): WorkoutType {
  const total = series.length;
  if (total === 0 || if_ < 0.55) return 'Recovery';

  const zoneTime = [0, 0, 0, 0, 0, 0]; // Z1..Z6
  let ssTime = 0; // time spent in Sweet Spot range [0.83, 0.95)
  for (const pct of series) {
    zoneTime[zoneOf(pct) - 1]++;
    if (pct >= 0.83 && pct < 0.95) ssTime++;
  }

  // Cascade top-down by intensity priority — the highest-system block wins.
  if (zoneTime[5] / total > 0.05) return 'Anaerobic';
  if (zoneTime[4] / total > 0.10) return 'VO2max';
  if (zoneTime[3] / total > 0.15) return 'Threshold';
  if (ssTime    / total > 0.20)   return 'Sweet Spot';
  if (zoneTime[2] / total > 0.20) return 'Tempo';
  if ((zoneTime[1] + zoneTime[2]) / total > 0.40) return 'Endurance';
  return 'Recovery';
}

export function computeWorkoutMetrics(blocks: WorkoutBlock[], ftpWatts: number): WorkoutMetrics {
  const series = flattenToSeconds(blocks);
  const duration_s = series.length;
  if (duration_s === 0 || ftpWatts <= 0) {
    return { duration_s, avg_pct: 0, np_pct: 0, if_: 0, tss: 0, kj: 0, type: 'Recovery' };
  }
  let sum = 0;
  let sum4 = 0;
  for (const pct of series) {
    const w = pct * ftpWatts;
    sum  += w;
    sum4 += w ** 4;
  }
  const avg_w = sum / duration_s;
  const np_w  = Math.pow(sum4 / duration_s, 0.25);
  const if_   = np_w / ftpWatts;
  const tss   = ((duration_s / 3600) * np_w * if_ / ftpWatts) * 100;
  const kj    = (avg_w * duration_s) / 1000;
  return {
    duration_s,
    avg_pct: avg_w / ftpWatts,
    np_pct:  np_w / ftpWatts,
    if_,
    tss,
    kj,
    type: classify(series, if_),
  };
}
