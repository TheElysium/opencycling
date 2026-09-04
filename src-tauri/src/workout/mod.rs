use crate::errors::AppError;

mod library;
mod types;
mod zwo;

#[cfg(test)]
pub use types::SportType;
pub use library::{WorkoutFileError, WorkoutLibrary};
pub use types::{ParsedWorkout, WorkoutBlock};

pub fn parse_zwo(file_content: &str) -> Result<ParsedWorkout, AppError> {
    zwo::parse_zwo(file_content)
}

pub fn list_workouts(folder: &str, ftp_w: u16) -> Result<WorkoutLibrary, AppError> {
    library::list_workouts(folder, ftp_w)
}
