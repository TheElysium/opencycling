use crate::errors::AppError;
pub use zwo::{ParsedWorkout};

mod zwo;

pub fn parse_zwo(file_content: &str) -> Result<ParsedWorkout, AppError> {
    zwo::parse_zwo(file_content)
}