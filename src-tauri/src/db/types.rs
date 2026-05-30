use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct Settings {
    pub ftp_w: u16,
    pub max_hr_bpm: u16,
    pub workout_path: String,
}
