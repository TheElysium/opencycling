//! Content rules for a plan entry (workout, note, or both). Lives outside
//! `schedule.rs`, which owns dates and grid shape only.

use crate::errors::AppError;
use crate::plan::types::EntryContent;

/// Normalization is a domain rule, not a UI one: the store must never see the
/// padding the frontend happened to leave in.
pub fn normalize_entry(content: &EntryContent) -> EntryContent {
    EntryContent {
        file_name: trimmed(content.file_name.as_deref()),
        workout_name: trimmed(content.workout_name.as_deref()),
        note: trimmed(content.note.as_deref()),
    }
}

fn trimmed(value: Option<&str>) -> Option<String> {
    present(value).map(String::from)
}

fn present(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|v| !v.is_empty())
}

/// The file a write adds to a day, if any: only that one has to exist in the
/// library, so editing the note of a day whose `.zwo` was deleted still saves.
pub fn introduced_file<'a>(previous: Option<&str>, next: Option<&'a str>) -> Option<&'a str> {
    let next = present(next);
    next.filter(|file_name| Some(*file_name) != present(previous))
}

/// Run on the normalized value: mirrors the `plan_entries` CHECK constraint with
/// a message the rider can act on.
pub fn validate_entry_content(content: &EntryContent) -> Result<(), AppError> {
    match (&content.file_name, &content.workout_name, &content.note) {
        (None, None, None) => Err(invalid(
            "a day entry needs a workout or a note; remove the entry instead".to_string(),
        )),
        (Some(file_name), None, _) => Err(invalid(format!(
            "workout `{file_name}` has no name and cannot be planned"
        ))),
        (None, Some(workout_name), _) => Err(invalid(format!(
            "`{workout_name}` has no workout file to run"
        ))),
        _ => Ok(()),
    }
}

fn invalid(message: String) -> AppError {
    AppError::PlanValidation(message)
}

/// A session fulfilled the workout it was started for: editing a note must keep
/// the link, replacing or removing the workout must drop it.
pub fn clears_session(previous: Option<&str>, next: Option<&str>) -> bool {
    previous != next
}

#[cfg(test)]
mod tests {
    use super::*;

    fn content(
        file_name: Option<&str>,
        workout_name: Option<&str>,
        note: Option<&str>,
    ) -> EntryContent {
        EntryContent {
            file_name: file_name.map(String::from),
            workout_name: workout_name.map(String::from),
            note: note.map(String::from),
        }
    }

    fn is_validation_error(result: Result<(), AppError>) -> bool {
        matches!(result, Err(AppError::PlanValidation(_)))
    }

    #[test]
    fn normalize_trims_note_and_workout_name() {
        let out = normalize_entry(&content(
            Some(" base.zwo "),
            Some("  Base ride "),
            Some("  swim 45min\n"),
        ));
        assert_eq!(out.file_name.as_deref(), Some("base.zwo"));
        assert_eq!(out.workout_name.as_deref(), Some("Base ride"));
        assert_eq!(out.note.as_deref(), Some("swim 45min"));
    }

    #[test]
    fn normalize_keeps_inner_whitespace_of_a_multiline_note() {
        let out = normalize_entry(&content(None, None, Some("swim 45min\nthen sauna")));
        assert_eq!(out.note.as_deref(), Some("swim 45min\nthen sauna"));
    }

    #[test]
    fn normalize_maps_blank_fields_to_none() {
        let out = normalize_entry(&content(Some("   "), Some(""), Some(" \t\n")));
        assert_eq!(out.file_name, None);
        assert_eq!(out.workout_name, None);
        assert_eq!(out.note, None);
    }

    #[test]
    fn validate_accepts_a_note_only_entry() {
        assert!(validate_entry_content(&content(None, None, Some("swim 45min"))).is_ok());
    }

    #[test]
    fn validate_accepts_a_workout_only_entry() {
        assert!(validate_entry_content(&content(Some("base.zwo"), Some("Base"), None)).is_ok());
    }

    #[test]
    fn validate_accepts_a_workout_with_a_note() {
        let c = content(Some("base.zwo"), Some("Base"), Some("easy gearing"));
        assert!(validate_entry_content(&c).is_ok());
    }

    #[test]
    fn validate_rejects_an_entry_with_neither_workout_nor_note() {
        match validate_entry_content(&content(None, None, None)) {
            Err(AppError::PlanValidation(message)) => {
                assert!(message.contains("remove"), "unhelpful message: {message}");
            }
            other => panic!("expected a validation error, got {other:?}"),
        }
    }

    #[test]
    fn validate_rejects_a_workout_file_without_a_name() {
        let result = validate_entry_content(&content(Some("base.zwo"), None, None));
        assert!(is_validation_error(result));
    }

    #[test]
    fn validate_rejects_a_workout_name_without_a_file() {
        let result = validate_entry_content(&content(None, Some("Base"), Some("swim")));
        assert!(is_validation_error(result));
    }

    #[test]
    fn a_newly_assigned_file_must_be_checked_against_the_library() {
        assert_eq!(introduced_file(None, Some("base.zwo")), Some("base.zwo"));
        assert_eq!(
            introduced_file(Some("base.zwo"), Some("vo2.zwo")),
            Some("vo2.zwo")
        );
    }

    #[test]
    fn re_sending_the_entrys_own_file_introduces_nothing() {
        assert_eq!(introduced_file(Some("base.zwo"), Some("base.zwo")), None);
        assert_eq!(introduced_file(Some("base.zwo"), Some(" base.zwo ")), None);
    }

    #[test]
    fn dropping_or_omitting_a_file_introduces_nothing() {
        assert_eq!(introduced_file(Some("base.zwo"), None), None);
        assert_eq!(introduced_file(None, None), None);
        assert_eq!(introduced_file(None, Some("  ")), None);
    }

    #[test]
    fn note_edit_on_a_note_only_entry_keeps_the_session_link() {
        assert!(!clears_session(None, None));
    }

    #[test]
    fn removing_the_workout_clears_the_session_link() {
        assert!(clears_session(Some("base.zwo"), None));
    }
}
