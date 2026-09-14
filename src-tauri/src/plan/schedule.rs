use crate::errors::AppError;
use crate::plan::types::{
    DayMarker, NewPlan, PlanDay, PlanEntry, PlanEntryView, PlanWeek, TrainingPlan,
};
use chrono::{Datelike, Days, NaiveDate, Weekday};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

const MAX_WEEKS: u32 = 52;
const DAYS_PER_WEEK: u64 = 7;

type EntriesByDate<'a> = HashMap<String, Vec<&'a PlanEntry>>;

/// End is EXCLUSIVE: a 4-week plan starting 2026-09-14 ends 2026-10-12.
pub fn plan_range(start_date: &str, weeks: u32) -> Result<(NaiveDate, NaiveDate), AppError> {
    let start = parse_date(start_date)?;
    let end = start
        .checked_add_days(Days::new(u64::from(weeks) * DAYS_PER_WEEK))
        .ok_or_else(|| invalid(format!("{weeks} weeks from {start_date} is out of range")))?;
    Ok((start, end))
}

/// The grid shape of a plan: `weeks` rows of 7 dated days, Monday first.
pub fn build_weeks(
    plan: &TrainingPlan,
    entries: &[PlanEntry],
    known_files: &HashSet<String>,
    today: NaiveDate,
) -> Result<Vec<PlanWeek>, AppError> {
    // Write-time validation cannot be fully trusted: a corrupt row must not
    // render as a silently empty grid.
    if plan.weeks == 0 {
        return Err(invalid(format!("plan {} has weeks = 0", plan.id)));
    }
    let (start, end) = plan_range(&plan.start_date, plan.weeks)?;
    let entries_by_date = entries_for_days(entries, start, end);
    (0..plan.weeks)
        .map(|index| build_week(start, index, &entries_by_date, known_files, today))
        .collect()
}

fn build_week(
    start: NaiveDate,
    index: u32,
    entries_by_date: &EntriesByDate<'_>,
    known_files: &HashSet<String>,
    today: NaiveDate,
) -> Result<PlanWeek, AppError> {
    let first = u64::from(index) * DAYS_PER_WEEK;
    let days = (first..first + DAYS_PER_WEEK)
        .map(|offset| build_day(start, offset, entries_by_date, known_files, today))
        .collect::<Result<Vec<_>, AppError>>()?;
    Ok(PlanWeek {
        number: index + 1,
        days,
    })
}

fn build_day(
    start: NaiveDate,
    offset: u64,
    entries_by_date: &EntriesByDate<'_>,
    known_files: &HashSet<String>,
    today: NaiveDate,
) -> Result<PlanDay, AppError> {
    let date = start
        .checked_add_days(Days::new(offset))
        .ok_or_else(|| invalid(format!("day {offset} of {start} is out of range")))?;
    let date_str = date.format("%Y-%m-%d").to_string();
    let entries = entries_by_date
        .get(&date_str)
        .map(|list| view_entries(list, known_files))
        .unwrap_or_default();
    Ok(PlanDay {
        date: date_str,
        marker: marker_of(date, today),
        entries,
    })
}

/// Entries whose date falls outside the plan bounds are ignored: a corrupt row
/// must not fail the whole grid.
fn entries_for_days(entries: &[PlanEntry], start: NaiveDate, end: NaiveDate) -> EntriesByDate<'_> {
    let mut by_date: EntriesByDate = HashMap::new();
    for entry in entries {
        if let Ok(date) = NaiveDate::parse_from_str(&entry.date, "%Y-%m-%d")
            && date >= start
            && date < end
        {
            by_date.entry(entry.date.clone()).or_default().push(entry);
        }
    }
    by_date
}

fn view_entries(entries: &[&PlanEntry], known_files: &HashSet<String>) -> Vec<PlanEntryView> {
    let mut views: Vec<_> = entries
        .iter()
        .map(|e| PlanEntryView {
            entry_id: e.id,
            position: e.position,
            file_name: e.file_name.clone(),
            workout_name: e.workout_name.clone(),
            note: e.note.clone(),
            session_id: e.session_id,
            missing: e
                .file_name
                .as_ref()
                .is_some_and(|f| !known_files.contains(f)),
        })
        .collect();
    views.sort_by(|a, b| {
        a.position
            .cmp(&b.position)
            .then_with(|| a.entry_id.cmp(&b.entry_id))
    });
    views
}

/// Validates an entry date against the plan's half-open range [start, end).
/// Malformed dates and out-of-bounds dates both surface as PlanValidation errors.
pub fn validate_entry_date(plan: &TrainingPlan, date: &str) -> Result<(), AppError> {
    let entry = NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map_err(|_| invalid(format!("`{date}` is not a valid YYYY-MM-DD date")))?;
    let (start, end) = plan_range(&plan.start_date, plan.weeks)?;
    if entry < start || entry >= end {
        return Err(invalid(format!(
            "date `{date}` is outside the plan `{}` ({} to {})",
            plan.name,
            start,
            end.pred_opt().unwrap_or(start)
        )));
    }
    Ok(())
}

fn marker_of(date: NaiveDate, today: NaiveDate) -> DayMarker {
    match date.cmp(&today) {
        Ordering::Less => DayMarker::Past,
        Ordering::Equal => DayMarker::Today,
        Ordering::Greater => DayMarker::Future,
    }
}

/// True when two plans share at least one day. Archived plans are the caller's
/// concern -- this is a pure date-range test.
pub fn overlaps(a: &TrainingPlan, b: &TrainingPlan) -> bool {
    plan_range(&a.start_date, a.weeks).is_ok_and(|range| conflicts_with(range, b))
}

/// The one overlap predicate (`overlaps` and `check_no_overlap` both route here);
/// a corrupt row cannot be range-compared, so it never reports a conflict.
fn conflicts_with(range: DateRange, other: &TrainingPlan) -> bool {
    plan_range(&other.start_date, other.weeks).is_ok_and(|o| ranges_overlap(range, o))
}

/// `editing_id` is the id of the plan being updated (None when creating), so a
/// plan never conflicts with itself.
pub fn validate_new_plan(
    candidate: &NewPlan,
    editing_id: Option<i64>,
    active_plans: &[TrainingPlan],
) -> Result<(), AppError> {
    check_name(&candidate.name)?;
    check_weeks(candidate.weeks)?;
    let range = plan_range(&candidate.start_date, candidate.weeks)?;
    check_monday(range.0)?;
    check_no_overlap(range, editing_id, active_plans)
}

/// Normalization is a domain rule, not a UI one: the store must never see the
/// padding the frontend happened to leave in.
pub fn normalize_plan(plan: &NewPlan) -> NewPlan {
    NewPlan {
        name: plan.name.trim().to_string(),
        start_date: plan.start_date.clone(),
        weeks: plan.weeks,
    }
}

/// An archived plan is history: it keeps its shape rules but may overlap anything,
/// so it is validated against an empty set of active plans.
pub fn validate_plan_write(
    candidate: &NewPlan,
    editing_id: Option<i64>,
    archived: bool,
    active_plans: &[TrainingPlan],
) -> Result<(), AppError> {
    let peers: &[TrainingPlan] = if archived { &[] } else { active_plans };
    validate_new_plan(candidate, editing_id, peers)
}

fn invalid(message: String) -> AppError {
    AppError::PlanValidation(message)
}

fn parse_date(start_date: &str) -> Result<NaiveDate, AppError> {
    NaiveDate::parse_from_str(start_date, "%Y-%m-%d").map_err(|_| {
        invalid(format!(
            "start date `{start_date}` is not a valid YYYY-MM-DD date"
        ))
    })
}

type DateRange = (NaiveDate, NaiveDate);

fn ranges_overlap(a: DateRange, b: DateRange) -> bool {
    a.0 < b.1 && b.0 < a.1
}

fn check_name(name: &str) -> Result<(), AppError> {
    if name.trim().is_empty() {
        return Err(invalid("the plan name cannot be empty".to_string()));
    }
    Ok(())
}

fn check_weeks(weeks: u32) -> Result<(), AppError> {
    if !(1..=MAX_WEEKS).contains(&weeks) {
        return Err(invalid(format!(
            "a plan lasts between 1 and {MAX_WEEKS} weeks, got {weeks}"
        )));
    }
    Ok(())
}

fn check_monday(start: NaiveDate) -> Result<(), AppError> {
    if start.weekday() != Weekday::Mon {
        return Err(invalid(format!(
            "a plan must start on a Monday, {start} is a {}",
            start.weekday()
        )));
    }
    Ok(())
}

fn check_no_overlap(
    range: DateRange,
    editing_id: Option<i64>,
    active_plans: &[TrainingPlan],
) -> Result<(), AppError> {
    let conflict = active_plans
        .iter()
        .filter(|existing| Some(existing.id) != editing_id)
        .find(|existing| conflicts_with(range, existing));
    match conflict {
        Some(existing) => Err(invalid(format!(
            "this plan overlaps `{}` ({}, {} weeks)",
            existing.name, existing.start_date, existing.weeks
        ))),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MONDAY: &str = "2026-09-14";

    fn date(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    fn plan(id: i64, start_date: &str, weeks: u32) -> TrainingPlan {
        TrainingPlan {
            id,
            name: format!("Plan {id}"),
            start_date: start_date.to_string(),
            weeks,
            created_at: "2026-09-01T10:00:00Z".to_string(),
            archived_at: None,
        }
    }

    fn candidate(name: &str, start_date: &str, weeks: u32) -> NewPlan {
        NewPlan {
            name: name.to_string(),
            start_date: start_date.to_string(),
            weeks,
        }
    }

    fn is_validation_error(result: Result<(), AppError>) -> bool {
        matches!(result, Err(AppError::PlanValidation(_)))
    }

    #[test]
    fn plan_range_spans_four_weeks_with_exclusive_end() {
        let (start, end) = plan_range(MONDAY, 4).unwrap();
        assert_eq!(start, date("2026-09-14"));
        assert_eq!(end, date("2026-10-12"));
    }

    #[test]
    fn plan_range_spans_a_single_week() {
        let (start, end) = plan_range(MONDAY, 1).unwrap();
        assert_eq!(start, date("2026-09-14"));
        assert_eq!(end, date("2026-09-21"));
    }

    #[test]
    fn plan_range_does_not_enforce_monday_start() {
        let (start, end) = plan_range("2026-09-15", 2).unwrap();
        assert_eq!(start, date("2026-09-15"));
        assert_eq!(end, date("2026-09-29"));
    }

    #[test]
    fn plan_range_rejects_malformed_date() {
        assert!(matches!(
            plan_range("14/09/2026", 4),
            Err(AppError::PlanValidation(_))
        ));
    }

    #[test]
    fn plan_range_rejects_week_count_overflowing_the_calendar() {
        assert!(matches!(
            plan_range(MONDAY, u32::MAX),
            Err(AppError::PlanValidation(_))
        ));
    }

    #[test]
    fn consecutive_plans_do_not_overlap_on_the_exclusive_end() {
        let a = plan(1, "2026-09-14", 4);
        let b = plan(2, "2026-10-12", 4);
        assert!(!overlaps(&a, &b));
        assert!(!overlaps(&b, &a));
    }

    #[test]
    fn contained_plan_overlaps() {
        let a = plan(1, "2026-09-14", 8);
        let b = plan(2, "2026-09-28", 2);
        assert!(overlaps(&a, &b));
        assert!(overlaps(&b, &a));
    }

    #[test]
    fn partial_tail_overlap_is_detected() {
        let a = plan(1, "2026-09-14", 4);
        let b = plan(2, "2026-10-05", 4);
        assert!(overlaps(&a, &b));
        assert!(overlaps(&b, &a));
    }

    #[test]
    fn identical_ranges_overlap() {
        let a = plan(1, MONDAY, 4);
        let b = plan(2, MONDAY, 4);
        assert!(overlaps(&a, &b));
    }

    #[test]
    fn corrupt_start_date_never_reports_an_overlap() {
        let a = plan(1, "not-a-date", 4);
        let b = plan(2, MONDAY, 4);
        assert!(!overlaps(&a, &b));
        assert!(!overlaps(&b, &a));
    }

    #[test]
    fn validate_rejects_empty_name() {
        let result = validate_new_plan(&candidate("", MONDAY, 4), None, &[]);
        assert!(is_validation_error(result));
    }

    #[test]
    fn validate_rejects_whitespace_only_name() {
        let result = validate_new_plan(&candidate("   \t", MONDAY, 4), None, &[]);
        assert!(is_validation_error(result));
    }

    #[test]
    fn validate_rejects_zero_weeks() {
        let result = validate_new_plan(&candidate("Base", MONDAY, 0), None, &[]);
        assert!(is_validation_error(result));
    }

    #[test]
    fn validate_rejects_more_than_fifty_two_weeks() {
        let result = validate_new_plan(&candidate("Base", MONDAY, 53), None, &[]);
        assert!(is_validation_error(result));
    }

    #[test]
    fn validate_accepts_the_week_count_boundaries() {
        assert!(validate_new_plan(&candidate("Base", MONDAY, 1), None, &[]).is_ok());
        assert!(validate_new_plan(&candidate("Base", MONDAY, 52), None, &[]).is_ok());
    }

    #[test]
    fn validate_rejects_a_tuesday_start() {
        let result = validate_new_plan(&candidate("Base", "2026-09-15", 4), None, &[]);
        assert!(is_validation_error(result));
    }

    #[test]
    fn validate_accepts_a_monday_start() {
        assert!(validate_new_plan(&candidate("Base", MONDAY, 4), None, &[]).is_ok());
    }

    #[test]
    fn validate_rejects_a_malformed_start_date() {
        let result = validate_new_plan(&candidate("Base", "2026-13-01", 4), None, &[]);
        assert!(is_validation_error(result));
    }

    #[test]
    fn validate_rejects_overlap_with_an_active_plan() {
        let existing = [plan(7, "2026-09-14", 4)];
        let result = validate_new_plan(&candidate("Base", "2026-10-05", 4), None, &existing);
        assert!(is_validation_error(result));
    }

    #[test]
    fn validate_lets_a_plan_be_edited_against_itself() {
        let existing = [plan(7, "2026-09-14", 4)];
        let result = validate_new_plan(&candidate("Base", "2026-09-14", 6), Some(7), &existing);
        assert!(result.is_ok());
    }

    #[test]
    fn validate_accepts_a_plan_starting_the_day_the_previous_one_ends() {
        let existing = [plan(7, "2026-09-14", 4)];
        let result = validate_new_plan(&candidate("Build", "2026-10-12", 4), None, &existing);
        assert!(result.is_ok());
    }

    #[test]
    fn validate_ignores_an_active_plan_with_a_corrupt_start_date() {
        let existing = [plan(7, "septembre", 4)];
        let result = validate_new_plan(&candidate("Base", MONDAY, 4), None, &existing);
        assert!(result.is_ok());
    }

    #[test]
    fn validate_trims_the_name_before_checking_emptiness() {
        assert!(validate_new_plan(&candidate("  Base  ", MONDAY, 4), None, &[]).is_ok());
    }

    #[test]
    fn normalize_trims_the_name_and_keeps_the_schedule() {
        let normalized = normalize_plan(&candidate(
            "  Base  
",
            MONDAY,
            4,
        ));
        assert_eq!(normalized.name, "Base");
        assert_eq!(normalized.start_date, MONDAY);
        assert_eq!(normalized.weeks, 4);
    }

    #[test]
    fn normalize_leaves_an_already_clean_name_alone() {
        let normalized = normalize_plan(&candidate("Base", MONDAY, 4));
        assert_eq!(normalized.name, "Base");
    }

    #[test]
    fn an_archived_plan_may_overlap_an_active_one() {
        let active = [plan(7, "2026-09-14", 4)];
        let result =
            validate_plan_write(&candidate("Base", "2026-09-14", 4), Some(9), true, &active);
        assert!(result.is_ok(), "archived plans are history, not schedule");
    }

    #[test]
    fn an_archived_plan_is_still_checked_for_shape() {
        let archived = |c| validate_plan_write(&c, Some(9), true, &[]);
        assert!(is_validation_error(archived(candidate("  ", MONDAY, 4))));
        assert!(is_validation_error(archived(candidate("Base", MONDAY, 0))));
        assert!(is_validation_error(archived(candidate(
            "Base",
            "2026-09-15",
            4
        ))));
    }

    #[test]
    fn an_active_plan_is_still_checked_against_overlaps() {
        let active = [plan(7, "2026-09-14", 4)];
        let result =
            validate_plan_write(&candidate("Base", "2026-09-14", 4), Some(9), false, &active);
        assert!(is_validation_error(result));
    }

    fn empty_files() -> HashSet<String> {
        HashSet::new()
    }

    fn all_dates(weeks: &[PlanWeek]) -> Vec<NaiveDate> {
        weeks
            .iter()
            .flat_map(|week| week.days.iter())
            .map(|day| date(&day.date))
            .collect()
    }

    fn assert_consecutive(dates: &[NaiveDate]) {
        for pair in dates.windows(2) {
            assert_eq!(pair[1], pair[0].succ_opt().unwrap());
        }
    }

    #[test]
    fn build_weeks_lays_out_one_row_per_week_of_seven_days() {
        let weeks = build_weeks(&plan(1, MONDAY, 4), &[], &empty_files(), date(MONDAY)).unwrap();
        assert_eq!(
            weeks.iter().map(|week| week.number).collect::<Vec<_>>(),
            vec![1, 2, 3, 4]
        );
        assert!(weeks.iter().all(|week| week.days.len() == 7));
    }

    #[test]
    fn build_weeks_runs_from_the_start_date_to_the_inclusive_end() {
        let weeks = build_weeks(&plan(1, MONDAY, 4), &[], &empty_files(), date(MONDAY)).unwrap();
        let dates = all_dates(&weeks);
        assert_eq!(dates.len(), 28);
        assert_eq!(dates.first(), Some(&date(MONDAY)));
        assert_eq!(dates.last(), Some(&date("2026-10-11")));
        assert_consecutive(&dates);
    }

    #[test]
    fn build_weeks_spans_a_year_boundary_without_a_gap() {
        let weeks =
            build_weeks(&plan(1, "2026-12-28", 2), &[], &empty_files(), date(MONDAY)).unwrap();
        let dates = all_dates(&weeks);
        assert_eq!(dates.last(), Some(&date("2027-01-10")));
        assert_consecutive(&dates);
    }

    #[test]
    fn build_weeks_rejects_a_malformed_start_date() {
        assert!(matches!(
            build_weeks(&plan(1, "not-a-date", 4), &[], &empty_files(), date(MONDAY)),
            Err(AppError::PlanValidation(_))
        ));
    }

    /// Only reachable through a corrupt row: writes enforce `weeks BETWEEN 1 AND 52`.
    #[test]
    fn build_weeks_rejects_a_zero_week_plan_instead_of_an_empty_grid() {
        assert!(matches!(
            build_weeks(&plan(1, MONDAY, 0), &[], &empty_files(), date(MONDAY)),
            Err(AppError::PlanValidation(_))
        ));
    }

    fn markers_of(weeks: &[PlanWeek], marker: DayMarker) -> Vec<NaiveDate> {
        weeks
            .iter()
            .flat_map(|week| week.days.iter())
            .filter(|day| day.marker == marker)
            .map(|day| date(&day.date))
            .collect()
    }

    #[test]
    fn today_inside_the_plan_splits_it_into_past_today_and_future() {
        let weeks =
            build_weeks(&plan(1, MONDAY, 4), &[], &empty_files(), date("2026-09-24")).unwrap();
        assert_eq!(
            markers_of(&weeks, DayMarker::Today),
            vec![date("2026-09-24")]
        );
        assert_eq!(markers_of(&weeks, DayMarker::Past).len(), 10);
        assert_eq!(markers_of(&weeks, DayMarker::Future).len(), 17);
        assert!(
            markers_of(&weeks, DayMarker::Past)
                .iter()
                .all(|day| day < &date("2026-09-24"))
        );
        assert!(
            markers_of(&weeks, DayMarker::Future)
                .iter()
                .all(|day| day > &date("2026-09-24"))
        );
    }

    #[test]
    fn a_plan_that_has_not_started_is_all_future() {
        let weeks =
            build_weeks(&plan(1, MONDAY, 4), &[], &empty_files(), date("2026-09-13")).unwrap();
        assert_eq!(markers_of(&weeks, DayMarker::Future).len(), 28);
    }

    #[test]
    fn a_finished_plan_is_all_past() {
        let weeks =
            build_weeks(&plan(1, MONDAY, 4), &[], &empty_files(), date("2026-11-02")).unwrap();
        assert_eq!(markers_of(&weeks, DayMarker::Past).len(), 28);
    }

    #[test]
    fn today_on_the_first_day_marks_only_that_day() {
        let weeks = build_weeks(&plan(1, MONDAY, 4), &[], &empty_files(), date(MONDAY)).unwrap();
        assert_eq!(markers_of(&weeks, DayMarker::Today), vec![date(MONDAY)]);
        assert!(markers_of(&weeks, DayMarker::Past).is_empty());
        assert_eq!(markers_of(&weeks, DayMarker::Future).len(), 27);
    }

    #[test]
    fn today_on_the_last_day_marks_only_that_day() {
        let weeks =
            build_weeks(&plan(1, MONDAY, 4), &[], &empty_files(), date("2026-10-11")).unwrap();
        assert_eq!(
            markers_of(&weeks, DayMarker::Today),
            vec![date("2026-10-11")]
        );
        assert_eq!(markers_of(&weeks, DayMarker::Past).len(), 27);
        assert!(markers_of(&weeks, DayMarker::Future).is_empty());
    }

    /// `plan_range` ends exclusive: the day after the plan is outside it, never `Today`.
    #[test]
    fn today_on_the_exclusive_end_date_is_outside_the_plan() {
        let weeks =
            build_weeks(&plan(1, MONDAY, 4), &[], &empty_files(), date("2026-10-12")).unwrap();
        assert!(markers_of(&weeks, DayMarker::Today).is_empty());
        assert_eq!(markers_of(&weeks, DayMarker::Past).len(), 28);
    }

    fn entry(
        id: i64,
        date: &str,
        position: i32,
        file_name: Option<&str>,
        workout_name: Option<&str>,
    ) -> PlanEntry {
        PlanEntry {
            id,
            plan_id: 1,
            date: date.to_string(),
            position,
            file_name: file_name.map(String::from),
            workout_name: workout_name.map(String::from),
            note: None,
            session_id: None,
        }
    }

    #[test]
    fn build_weeks_attaches_entries_to_their_day() {
        let entries = vec![entry(
            1,
            "2026-09-15",
            0,
            Some("base.zwo"),
            Some("Base Endurance"),
        )];
        let weeks =
            build_weeks(&plan(1, MONDAY, 1), &entries, &empty_files(), date(MONDAY)).unwrap();
        let tuesday = &weeks[0].days[1];
        assert_eq!(tuesday.entries.len(), 1);
        assert_eq!(tuesday.entries[0].entry_id, 1);
        assert_eq!(
            tuesday.entries[0].workout_name,
            Some("Base Endurance".to_string())
        );
        assert!(tuesday.entries[0].missing);
    }

    #[test]
    fn build_weeks_orders_entries_by_position_then_id() {
        let entries = vec![
            entry(5, "2026-09-15", 1, None, Some("second")),
            entry(2, "2026-09-15", 0, None, Some("first")),
            entry(3, "2026-09-15", 0, None, Some("tie-broken-by-id")),
        ];
        let weeks =
            build_weeks(&plan(1, MONDAY, 1), &entries, &empty_files(), date(MONDAY)).unwrap();
        let names: Vec<_> = weeks[0].days[1]
            .entries
            .iter()
            .map(|e| e.workout_name.clone())
            .collect();
        assert_eq!(
            names,
            vec![
                Some("first".to_string()),
                Some("tie-broken-by-id".to_string()),
                Some("second".to_string())
            ]
        );
    }

    #[test]
    fn build_weeks_marks_entries_missing_when_file_not_in_known_files() {
        let entries = vec![entry(1, "2026-09-15", 0, Some("gone.zwo"), Some("Gone"))];
        let mut files = empty_files();
        files.insert("present.zwo".to_string());
        let weeks = build_weeks(&plan(1, MONDAY, 1), &entries, &files, date(MONDAY)).unwrap();
        assert!(weeks[0].days[1].entries[0].missing);

        files.insert("gone.zwo".to_string());
        let weeks = build_weeks(&plan(1, MONDAY, 1), &entries, &files, date(MONDAY)).unwrap();
        assert!(!weeks[0].days[1].entries[0].missing);
    }

    #[test]
    fn build_weeks_ignores_entries_outside_the_plan_bounds() {
        let entries = vec![
            entry(1, "2026-09-13", 0, None, Some("before")),
            entry(2, "2026-09-21", 0, None, Some("after")),
        ];
        let weeks =
            build_weeks(&plan(1, MONDAY, 1), &entries, &empty_files(), date(MONDAY)).unwrap();
        assert!(weeks[0].days.iter().all(|d| d.entries.is_empty()));
    }

    #[test]
    fn build_weeks_keeps_note_only_entries_without_a_file_name() {
        let entries = vec![entry(1, "2026-09-15", 0, None, None)];
        let weeks =
            build_weeks(&plan(1, MONDAY, 1), &entries, &empty_files(), date(MONDAY)).unwrap();
        assert_eq!(weeks[0].days[1].entries.len(), 1);
        assert_eq!(weeks[0].days[1].entries[0].file_name, None);
        assert!(!weeks[0].days[1].entries[0].missing);
    }

    fn with_note(mut entry: PlanEntry, note: &str) -> PlanEntry {
        entry.note = Some(note.to_string());
        entry
    }

    #[test]
    fn build_weeks_surfaces_the_note_of_an_entry() {
        let entries = vec![
            with_note(entry(1, "2026-09-15", 0, None, None), "swim 45min"),
            with_note(
                entry(2, "2026-09-16", 0, Some("base.zwo"), Some("Base")),
                "easy gearing",
            ),
        ];
        let weeks =
            build_weeks(&plan(1, MONDAY, 1), &entries, &empty_files(), date(MONDAY)).unwrap();
        assert_eq!(
            weeks[0].days[1].entries[0].note.as_deref(),
            Some("swim 45min")
        );
        assert!(!weeks[0].days[1].entries[0].missing);
        assert_eq!(
            weeks[0].days[2].entries[0].note.as_deref(),
            Some("easy gearing")
        );
    }

    #[test]
    fn validate_entry_date_accepts_start_date() {
        assert!(validate_entry_date(&plan(1, MONDAY, 4), MONDAY).is_ok());
    }

    #[test]
    fn validate_entry_date_rejects_exclusive_end_date() {
        let result = validate_entry_date(&plan(1, MONDAY, 1), "2026-09-21");
        assert!(is_validation_error(result));
    }

    #[test]
    fn validate_entry_date_rejects_day_before_start() {
        let result = validate_entry_date(&plan(1, MONDAY, 1), "2026-09-13");
        assert!(is_validation_error(result));
    }

    #[test]
    fn validate_entry_date_rejects_malformed_date() {
        let result = validate_entry_date(&plan(1, MONDAY, 1), "not-a-date");
        assert!(is_validation_error(result));
    }
}
