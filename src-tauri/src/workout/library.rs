use crate::errors::AppError;
use crate::workout::{parse_zwo, ParsedWorkout};
use std::ffi::OsStr;
use std::fs::{read_dir, read_to_string};

pub(crate) fn list_workouts(folder: &str) -> Result<Vec<ParsedWorkout>, AppError> {
    Ok(read_dir(folder)
        .map_err(|e| AppError::Other(e.to_string()))?
        .flatten()
        .filter(|e| e.path().extension() == Some(OsStr::new("zwo")))
        .filter_map(|entry| {
            let content = read_to_string(entry.path()).ok()?;
            parse_zwo(&content).ok()
        })
        .collect())
}
