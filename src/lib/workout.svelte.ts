import { zoneOf } from './metrics';

export type SteadyStateBlock = {
  SteadyState: { duration_s: number; power_pct: number; cadence_rpm: number | null; label: string | null };
};

export type RampBlock = {
  Ramp: { duration_s: number; power_start_pct: number; power_end_pct: number; cadence_rpm: number | null; label: string | null };
};

export type IntervalsTBlock = {
  IntervalsT: { repeat: number; on: WorkoutBlock; off: WorkoutBlock };
};

export type WorkoutBlock = SteadyStateBlock | RampBlock | IntervalsTBlock;

export type ParsedWorkout = {
  author: string | null;
  name: string | null;
  description: string | null;
  sport_type: 'Bike' | 'Running';
  workout_blocks: WorkoutBlock[];
  is_ftp_test: boolean;
};

// Canonical display block: intervals unfolded, power in watts.
// Mirrors the Rust `FlatBlock` returned by SessionActor, used everywhere we
// render a planned workout (charts, lists, timeline).
export type FlatBlock = {
  duration_s: number;
  power_start_w: number;
  power_end_w: number;
  cadence_rpm: number | null;
  label: string;
};

// Must mirror Rust `zone_name` in src-tauri/src/session/actor.rs.
function zoneName(z: number): string {
  switch (z) {
    case 1: return 'Recovery';
    case 2: return 'Endurance';
    case 3: return 'Tempo';
    case 4: return 'Threshold';
    case 5: return 'VO2max';
    default: return 'Anaerobic';
  }
}

// Must mirror Rust `fallback_label` in src-tauri/src/session/actor.rs.
function fallbackLabel(startW: number, endW: number, ftpW: number): string {
  if (ftpW === 0) return 'Block';
  const zs = zoneOf(startW / ftpW);
  const ze = zoneOf(endW / ftpW);
  return zs !== ze ? `Ramp ${zoneName(zs)}→${zoneName(ze)}` : `Steady ${zoneName(zs)}`;
}

function flattenBlockWithLabel(
  block: WorkoutBlock,
  ftpW: number,
  overrideLabel: string | null,
  out: FlatBlock[],
): void {
  if ('SteadyState' in block) {
    const w = Math.round(block.SteadyState.power_pct * ftpW);
    const label = overrideLabel ?? block.SteadyState.label ?? fallbackLabel(w, w, ftpW);
    out.push({
      duration_s: block.SteadyState.duration_s,
      power_start_w: w,
      power_end_w: w,
      cadence_rpm: block.SteadyState.cadence_rpm,
      label,
    });
  } else if ('Ramp' in block) {
    const startW = Math.round(block.Ramp.power_start_pct * ftpW);
    const endW   = Math.round(block.Ramp.power_end_pct   * ftpW);
    const label = overrideLabel ?? block.Ramp.label ?? fallbackLabel(startW, endW, ftpW);
    out.push({
      duration_s: block.Ramp.duration_s,
      power_start_w: startW,
      power_end_w: endW,
      cadence_rpm: block.Ramp.cadence_rpm,
      label,
    });
  } else if ('IntervalsT' in block) {
    const { repeat, on, off } = block.IntervalsT;
    for (let i = 0; i < repeat; i++) {
      flattenBlockWithLabel(on,  ftpW, `Interval ${i + 1}/${repeat} ON`,  out);
      flattenBlockWithLabel(off, ftpW, `Interval ${i + 1}/${repeat} OFF`, out);
    }
  }
}

export function flattenWorkout(blocks: WorkoutBlock[], ftpW: number): FlatBlock[] {
  const out: FlatBlock[] = [];
  for (const b of blocks) flattenBlockWithLabel(b, ftpW, null, out);
  return out;
}

class WorkoutSelection {
  workout = $state<ParsedWorkout | null>(null);
}

export const workoutSelection = new WorkoutSelection();
