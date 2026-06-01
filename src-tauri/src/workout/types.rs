use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ParsedWorkout {
    pub author: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub sport_type: SportType,
    pub workout_blocks: Vec<WorkoutBlock>,
}
#[derive(Debug, Clone, Serialize)]
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

#[derive(Debug, PartialEq, Eq, Serialize)]
pub enum SportType {
    Bike,
    Running,
}
