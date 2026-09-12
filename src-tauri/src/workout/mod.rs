use crate::errors::AppError;

mod library;
mod types;
mod zwo;

pub(crate) use library::list_workouts_cached;
pub use library::{WorkoutFileError, WorkoutLibrary};
#[cfg(test)]
pub use types::SportType;
pub use types::{ParsedWorkout, WorkoutBlock};

pub fn parse_zwo(file_content: &str) -> Result<ParsedWorkout, AppError> {
    zwo::parse_zwo(file_content)
}
