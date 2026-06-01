use crate::errors::AppError;

mod library;
mod types;
mod zwo;

pub use types::ParsedWorkout;

pub fn parse_zwo(file_content: &str) -> Result<ParsedWorkout, AppError> {
    zwo::parse_zwo(file_content)
}

pub fn list_workouts(folder: &str) -> Result<Vec<ParsedWorkout>, AppError> {
    library::list_workouts(folder)
}
