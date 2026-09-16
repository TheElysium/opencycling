use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct TrainingPlan {
    pub id: i64,
    pub name: String,
    /// ISO `YYYY-MM-DD`, always a Monday.
    pub start_date: String,
    pub weeks: u32,
    /// ISO `YYYY-MM-DD`; the end is EXCLUSIVE (see `plan::schedule::plan_range`).
    pub end_date: String,
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

/// One row of the plan grid; `number` is 1-based so the UI renders "Week 1" as is.
#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct PlanWeek {
    pub number: u32,
    pub days: Vec<PlanDay>,
}

/// No weekday name: a week always holds 7 days from Monday, so the index is the weekday.
#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct PlanDay {
    /// ISO `YYYY-MM-DD`.
    pub date: String,
    pub marker: DayMarker,
    pub entries: Vec<PlanEntryView>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct PlanEntryView {
    pub entry_id: i64,
    pub file_name: Option<String>,
    pub workout_name: Option<String>,
    pub note: Option<String>,
    pub session_id: Option<i64>,
    pub missing: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct PlanEntry {
    pub id: i64,
    pub plan_id: i64,
    /// ISO `YYYY-MM-DD`.
    pub date: String,
    pub position: i32,
    pub file_name: Option<String>,
    pub workout_name: Option<String>,
    pub note: Option<String>,
    pub session_id: Option<i64>,
}

/// The editable part of an entry: a workout, a note, or both (mirrors the
/// `plan_entries` CHECK constraint).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Type)]
pub struct EntryContent {
    pub file_name: Option<String>,
    pub workout_name: Option<String>,
    pub note: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct NewEntry {
    pub plan_id: i64,
    /// ISO `YYYY-MM-DD`.
    pub date: String,
    pub content: EntryContent,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
pub enum DayMarker {
    Past,
    Today,
    Future,
}
