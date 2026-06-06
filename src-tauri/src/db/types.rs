use crate::metrics::WorkoutType;
use crate::session::FlatBlock;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Settings {
    pub ftp_w: u16,
    pub max_hr_bpm: u16,
    pub workout_path: String,
}

#[derive(Serialize, Debug, Clone)]
pub struct SessionCard {
    pub id: i64,
    pub started_at: String,
    pub workout_name: String,
    pub duration_s: Option<u32>,
    pub avg_power_w: Option<u16>,
    pub avg_hr_bpm: Option<u16>,
    pub avg_cadence_rpm: Option<u16>,
    pub ftp_w_used: u16,
    pub workout_type: Option<WorkoutType>,
}

#[derive(Serialize, Debug, Clone)]
pub struct Metric {
    pub t_offset_s: u32,
    pub power_w: Option<u16>,
    pub hr_bpm: Option<u16>,
    pub cadence_rpm: Option<u16>,
}

#[derive(Serialize, Debug, Clone)]
pub struct SessionDetail {
    pub id: i64,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub workout_name: String,
    pub duration_s: Option<u32>,
    pub avg_power_w: Option<u16>,
    pub max_power_w: Option<u16>,
    pub avg_hr_bpm: Option<u16>,
    pub max_hr_bpm: Option<u16>,
    pub avg_cadence_rpm: Option<u16>,
    pub max_cadence_rpm: Option<u16>,
    pub ftp_w_used: u16,
    pub workout_type: Option<WorkoutType>,
    pub flat_blocks: Vec<FlatBlock>,
    pub metrics: Vec<Metric>,
}
