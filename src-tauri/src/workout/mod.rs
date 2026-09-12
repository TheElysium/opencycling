use crate::errors::AppError;

mod library;
mod types;
mod zwo;

pub use library::{WorkoutFileError, WorkoutLibrary};
pub(crate) use library::{attach_last_used, list_workouts_cached};
#[cfg(test)]
pub use types::SportType;
pub use types::{ParsedWorkout, WorkoutBlock};

pub fn parse_zwo(file_content: &str) -> Result<ParsedWorkout, AppError> {
    zwo::parse_zwo(file_content)
}
