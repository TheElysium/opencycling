use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Serialize, Deserialize, Type)]
pub struct ParsedWorkout {
    pub author: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub sport_type: SportType,
    pub workout_blocks: Vec<WorkoutBlock>,
    // No #[serde(default)] on the two fields below: ParsedWorkout is only ever
    // deserialized at the Tauri boundary, where the generated bindings guarantee the
    // frontend sends complete objects, and defaults would export them as optional
    // (`field?:`) in TypeScript.
    pub is_ftp_test: bool,
    /// Source file name (basename only). None when parsed from raw content
    /// without a file context (e.g. tests or load_workout command).
    pub file_name: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub enum WorkoutBlock {
    SteadyState {
        duration_s: u32,
        power_pct: f32,
        cadence_rpm: Option<u16>,
        label: Option<String>,
    },
    Ramp {
        duration_s: u32,
        power_start_pct: f32,
        power_end_pct: f32,
        cadence_rpm: Option<u16>,
        label: Option<String>,
    },
    IntervalsT {
        repeat: u16,
        on: Box<WorkoutBlock>,
        off: Box<WorkoutBlock>,
    },
}

impl WorkoutBlock {
    pub fn duration_s(&self) -> u32 {
        match &self {
            WorkoutBlock::SteadyState { duration_s, .. } => *duration_s,
            WorkoutBlock::Ramp { duration_s, .. } => *duration_s,
            WorkoutBlock::IntervalsT { repeat, on, off } => {
                *repeat as u32 * (on.duration_s() + off.duration_s())
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum SportType {
    Bike,
    Running,
}
