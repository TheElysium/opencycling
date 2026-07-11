import type { ParsedWorkout } from './bindings';

// All bridge types are generated from the Rust structs (see src-tauri/src/lib.rs,
// `export_typescript_bindings`). Re-exported here so existing import sites keep working.
export type {
  ParsedWorkout,
  WorkoutBlock,
  WorkoutFileError,
  WorkoutLibrary,
  FlatBlock,
} from './bindings';

// Flattening (intervals unfolded, power in watts, labels synthesized) lives in Rust
// only: call `commands.flattenWorkoutCmd(workout, ftpW)` from an async context.

class WorkoutSelection {
  workout = $state<ParsedWorkout | null>(null);
}

export const workoutSelection = new WorkoutSelection();
