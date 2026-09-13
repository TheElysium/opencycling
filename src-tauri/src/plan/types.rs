use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct TrainingPlan {
    pub id: i64,
    pub name: String,
    /// ISO `YYYY-MM-DD`, always a Monday.
    pub start_date: String,
    pub weeks: u32,
    /// RFC 3339.
    pub created_at: String,
    /// `None` = active.
    pub archived_at: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct NewPlan {
    pub name: String,
    pub start_date: String,
    pub weeks: u32,
}
