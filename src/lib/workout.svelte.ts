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
};

class WorkoutSelection {
  workout = $state<ParsedWorkout | null>(null);
}

export const workoutSelection = new WorkoutSelection();
