use crate::metrics::WorkoutType;
use crate::session::FlatBlock;
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct Settings {
    pub ftp_w: u16,
    pub max_hr_bpm: u16,
    pub workout_path: String,
    /// Base URL of the user's local Strava auth proxy (holds their client secret).
    pub strava_proxy_url: String,
    /// Global default for front-camera aero detection.
    pub aero_enabled: bool,
}

#[derive(Serialize, Debug, Clone, Type)]
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
    pub aero_pct: Option<f32>,
    pub np_w: Option<f32>,
    pub if_: Option<f32>,
    pub tss: Option<f32>,
}

#[derive(Serialize, Debug, Clone, Type)]
pub struct Metric {
    pub t_offset_s: u32,
    pub power_w: Option<u16>,
    pub hr_bpm: Option<u16>,
    pub cadence_rpm: Option<u16>,
    pub aero_score: Option<f32>,
}

#[derive(Serialize, Debug, Clone, Type)]
pub struct SessionDetail {
    pub id: i64,
    pub strava_activity_id: Option<i64>,
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
    pub aero_pct: Option<f32>,
    pub np_w: Option<f32>,
    pub if_: Option<f32>,
    pub tss: Option<f32>,
    pub flat_blocks: Vec<FlatBlock>,
    pub metrics: Vec<Metric>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StravaAuth {
    pub access_token: String,
    pub refresh_token: String,
    /// Token expiry, epoch seconds (UTC).
    pub expires_at: i64,
    pub athlete_id: Option<i64>,
    pub athlete_name: Option<String>,
    pub connected_at: String,
}
