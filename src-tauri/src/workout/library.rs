use crate::errors::AppError;
use crate::session::{FlatBlock, flatten_workout};
use crate::workout::{parse_zwo, ParsedWorkout};
use serde::Serialize;
use specta::Type;
use std::ffi::OsStr;
use std::fs::{read_dir, read_to_string};

/// FTP used to flatten FTP-test workouts, so watts equal percent on their cards.
/// Must match FTP_TEST_REFERENCE_W in src/lib/ftp.ts.
const FTP_TEST_REFERENCE_W: u16 = 100;

/// A file that could not be read or parsed, returned alongside successful workouts.
#[derive(Debug, Serialize, Type)]
pub struct WorkoutFileError {
    pub file_name: String,
    pub message: String,
}

/// Result of listing a workout folder: successfully parsed workouts plus any
/// files that failed (parse errors, unreadable files), so the frontend can
/// show a warning without hiding the workouts that did load.
#[derive(Debug, Serialize, Type)]
pub struct WorkoutLibrary {
    pub workouts: Vec<ParsedWorkout>,
    /// Flat blocks parallel to `workouts`, flattened at each card's FTP (reference
    /// FTP for tests) so thumbnails render exactly what the session will run.
    pub flats: Vec<Vec<FlatBlock>>,
    pub errors: Vec<WorkoutFileError>,
}

pub(crate) fn list_workouts(folder: &str, ftp_w: u16) -> Result<WorkoutLibrary, AppError> {
    let entries = read_dir(folder)?;

    let mut workouts = Vec::new();
    let mut flats = Vec::new();
    let mut errors = Vec::new();

    for entry in entries.flatten() {
        if entry.path().extension() != Some(OsStr::new("zwo")) {
            continue;
        }

        let file_name = entry
            .path()
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();

        let content = match read_to_string(entry.path()) {
            Ok(c) => c,
            Err(e) => {
                errors.push(WorkoutFileError {
                    file_name,
                    message: e.to_string(),
                });
                continue;
            }
        };

        match parse_zwo(&content) {
            Ok(mut w) => {
                w.file_name = Some(file_name);
                let card_ftp = if w.is_ftp_test { FTP_TEST_REFERENCE_W } else { ftp_w };
                flats.push(flatten_workout(w.clone(), card_ftp));
                workouts.push(w);
            }
            Err(e) => {
                errors.push(WorkoutFileError {
                    file_name,
                    message: e.to_string(),
                });
            }
        }
    }

    Ok(WorkoutLibrary { workouts, flats, errors })
}
