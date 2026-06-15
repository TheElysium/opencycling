import type { FlatBlock } from './session.svelte';
import { zoneOf, type WorkoutType } from './metrics';

export type SessionCard = {
  id: number;
  started_at: string;
  workout_name: string;
  duration_s: number | null;
  avg_power_w: number | null;
  avg_hr_bpm: number | null;
  avg_cadence_rpm: number | null;
  ftp_w_used: number;
  workout_type: WorkoutType | null;
};

export type MetricSample = {
  t_offset_s: number;
  power_w: number | null;
  hr_bpm: number | null;
  cadence_rpm: number | null;
};

export type SessionDetail = {
  id: number;
  strava_activity_id: number | null;
  started_at: string;
  ended_at: string | null;
  workout_name: string;
  duration_s: number | null;
  avg_power_w: number | null;
  max_power_w: number | null;
  avg_hr_bpm: number | null;
  max_hr_bpm: number | null;
  avg_cadence_rpm: number | null;
  max_cadence_rpm: number | null;
  ftp_w_used: number;
  workout_type: WorkoutType | null;
  flat_blocks: FlatBlock[];
  metrics: MetricSample[];
};

export type SessionMetricsComputed = {
  tss: number;
  if_: number;
  np_w: number;
};

// Workout type is authoritative on the backend (computed at finalize from the
// full series). Read it from `s.workout_type` directly — do not infer here.
export function computeSessionMetrics(s: SessionCard | SessionDetail): SessionMetricsComputed {
  const ftp = s.ftp_w_used;
  if (ftp <= 0) return { tss: 0, if_: 0, np_w: 0 };

  if ('metrics' in s && s.metrics.length > 0) {
    let sum4 = 0;
    let n = 0;
    for (const m of s.metrics) {
      if (m.power_w == null) continue;
      sum4 += m.power_w ** 4;
      n++;
    }
    if (n > 0) {
      const np_w = Math.pow(sum4 / n, 0.25);
      const if_  = np_w / ftp;
      const dur  = s.duration_s ?? 0;
      const tss  = (dur / 3600) * if_ * if_ * 100;
      return { tss, if_, np_w };
    }
  }

  // SessionCard fallback: aggregates only. Approximate IF/TSS from avg power.
  const dur = s.duration_s ?? 0;
  const avg = s.avg_power_w ?? 0;
  if (dur <= 0 || avg <= 0) return { tss: 0, if_: 0, np_w: 0 };
  const if_ = avg / ftp;
  const tss = (dur / 3600) * if_ * if_ * 100;
  return { tss, if_, np_w: avg };
}

// Power zone distribution Z1..Z6 from samples (returns 6 percentages summing to 1)
export function powerZoneDistribution(samples: MetricSample[], ftp: number): number[] {
  const acc = [0, 0, 0, 0, 0, 0];
  if (ftp <= 0) return acc;
  let n = 0;
  for (const s of samples) {
    if (s.power_w == null) continue;
    acc[zoneOf(s.power_w / ftp) - 1]++;
    n++;
  }
  if (n === 0) return acc;
  return acc.map(v => v / n);
}

// Heart rate zone distribution Z1..Z5 from samples, given max HR
// Zones: Z1 <60%, Z2 60-70%, Z3 70-80%, Z4 80-90%, Z5 >=90%
export function hrZoneDistribution(samples: MetricSample[], maxHr: number): number[] {
  const acc = [0, 0, 0, 0, 0];
  if (maxHr <= 0) return acc;
  let n = 0;
  for (const s of samples) {
    if (s.hr_bpm == null) continue;
    const r = s.hr_bpm / maxHr;
    let z = 0;
    if (r < 0.60) z = 0;
    else if (r < 0.70) z = 1;
    else if (r < 0.80) z = 2;
    else if (r < 0.90) z = 3;
    else z = 4;
    acc[z]++;
    n++;
  }
  if (n === 0) return acc;
  return acc.map(v => v / n);
}

// -------------------- grouping --------------------

export type SessionGroup = {
  label: string;
  sessions: SessionCard[];
  agg: { count: number; total_s: number; total_tss: number };
};

const MONTHS = [
  'January', 'February', 'March', 'April', 'May', 'June',
  'July', 'August', 'September', 'October', 'November', 'December',
];

function startOfWeek(d: Date): Date {
  const x = new Date(d);
  x.setHours(0, 0, 0, 0);
  // Monday = 1, Sunday = 0 → treat Sunday as day 7 so week starts Monday
  const day = x.getDay() === 0 ? 7 : x.getDay();
  x.setDate(x.getDate() - (day - 1));
  return x;
}

function periodKey(date: Date, now: Date): string {
  const thisWeek = startOfWeek(now);
  const lastWeek = new Date(thisWeek);
  lastWeek.setDate(lastWeek.getDate() - 7);
  if (date >= thisWeek) return 'this-week';
  if (date >= lastWeek) return 'last-week';
  return `month-${date.getFullYear()}-${date.getMonth()}`;
}

function periodLabel(key: string): string {
  if (key === 'this-week') return 'This week';
  if (key === 'last-week') return 'Last week';
  const m = key.match(/^month-(\d+)-(\d+)$/);
  if (m) {
    const year = parseInt(m[1], 10);
    const month = parseInt(m[2], 10);
    return `${MONTHS[month]} ${year}`;
  }
  return key;
}

export function groupByPeriod(sessions: SessionCard[], now: Date = new Date()): SessionGroup[] {
  const map = new Map<string, SessionCard[]>();
  const order: string[] = [];
  for (const s of sessions) {
    const k = periodKey(new Date(s.started_at), now);
    if (!map.has(k)) { map.set(k, []); order.push(k); }
    map.get(k)!.push(s);
  }
  return order.map(k => {
    const list = map.get(k)!;
    let total_s = 0;
    let total_tss = 0;
    for (const s of list) {
      total_s += s.duration_s ?? 0;
      total_tss += computeSessionMetrics(s).tss;
    }
    return {
      label: periodLabel(k),
      sessions: list,
      agg: { count: list.length, total_s, total_tss },
    };
  });
}

// -------------------- date formatting --------------------

export function formatDayNum(iso: string): string {
  return String(new Date(iso).getDate()).padStart(2, '0');
}

export function formatWeekdayShort(iso: string): string {
  return new Date(iso).toLocaleDateString('en-US', { weekday: 'short' });
}

export function formatHourMinute(iso: string): string {
  const d = new Date(iso);
  return `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`;
}

export function formatLongDate(iso: string): string {
  return new Date(iso).toLocaleDateString('en-US', {
    weekday: 'long', day: '2-digit', month: 'short', year: 'numeric',
  });
}

export function formatHmsShort(s: number): string {
  if (s <= 0) return '0min';
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  if (h > 0) return m > 0 ? `${h}h ${String(m).padStart(2, '0')}min` : `${h}h`;
  return `${m}min`;
}
