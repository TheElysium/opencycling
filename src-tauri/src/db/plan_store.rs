//! SQL for the `training_plans` table. Lives outside `actor.rs` to keep that
//! file under the 1000-line size gate; the DB actor is the only consumer.

use crate::errors::AppError;
use crate::plan::{EntryContent, NewEntry, NewPlan, PlanEntry, TrainingPlan, clears_session};
use rusqlite::{Connection, Row};

const PLAN_COLUMNS: &str = "id, name, start_date, weeks, created_at, archived_at";

fn plan_from_row(row: &Row) -> rusqlite::Result<TrainingPlan> {
    Ok(TrainingPlan {
        id: row.get(0)?,
        name: row.get(1)?,
        start_date: row.get(2)?,
        weeks: row.get(3)?,
        created_at: row.get(4)?,
        archived_at: row.get(5)?,
    })
}

const ENTRY_COLUMNS: &str =
    "id, plan_id, date, position, file_name, workout_name, note, session_id";

fn entry_from_row(row: &Row) -> rusqlite::Result<PlanEntry> {
    Ok(PlanEntry {
        id: row.get(0)?,
        plan_id: row.get(1)?,
        date: row.get(2)?,
        position: row.get(3)?,
        file_name: row.get(4)?,
        workout_name: row.get(5)?,
        note: row.get(6)?,
        session_id: row.get(7)?,
    })
}

/// A plan has no caller-meaningful timestamp other than now, so `created_at` /
/// `archived_at` are stamped here instead of crossing the command surface.
fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}

pub(crate) fn list(
    conn: &Connection,
    include_archived: bool,
) -> Result<Vec<TrainingPlan>, AppError> {
    let filter = if include_archived {
        ""
    } else {
        "WHERE archived_at IS NULL"
    };
    let mut stmt = conn.prepare(&format!(
        "SELECT {PLAN_COLUMNS} FROM training_plans {filter} ORDER BY start_date DESC, id DESC"
    ))?;
    let rows = stmt.query_map([], plan_from_row)?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

/// Missing ids surface as a `PlanValidation` message because the frontend shows
/// it raw, not as a bare `QueryReturnedNoRows`.
fn not_found(id: i64) -> AppError {
    AppError::PlanValidation(format!("plan {id} not found"))
}

/// A mutation matching no row means the plan vanished (deleted elsewhere), which
/// must not be reported to the UI as a success.
fn one_row(affected: usize, id: i64) -> Result<(), AppError> {
    if affected == 0 {
        return Err(not_found(id));
    }
    Ok(())
}

pub(crate) fn get(conn: &Connection, id: i64) -> Result<TrainingPlan, AppError> {
    conn.query_row(
        &format!("SELECT {PLAN_COLUMNS} FROM training_plans WHERE id = ?1"),
        [id],
        plan_from_row,
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => not_found(id),
        other => other.into(),
    })
}

pub(crate) fn insert(conn: &Connection, plan: &NewPlan) -> Result<i64, AppError> {
    conn.execute(
        "INSERT INTO training_plans (name, start_date, weeks, created_at) \
         VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![plan.name, plan.start_date, plan.weeks, now()],
    )?;
    Ok(conn.last_insert_rowid())
}

pub(crate) fn update(conn: &Connection, id: i64, plan: &NewPlan) -> Result<(), AppError> {
    let affected = conn.execute(
        "UPDATE training_plans SET name = ?1, start_date = ?2, weeks = ?3 WHERE id = ?4",
        rusqlite::params![plan.name, plan.start_date, plan.weeks, id],
    )?;
    one_row(affected, id)
}

pub(crate) fn set_archived(conn: &Connection, id: i64, archived: bool) -> Result<(), AppError> {
    let archived_at = archived.then(now);
    let affected = conn.execute(
        "UPDATE training_plans SET archived_at = ?1 WHERE id = ?2",
        rusqlite::params![archived_at, id],
    )?;
    one_row(affected, id)
}

pub(crate) fn delete(conn: &Connection, id: i64) -> Result<(), AppError> {
    // plan_entries rows cascade via the FK ON DELETE CASCADE.
    let affected = conn.execute("DELETE FROM training_plans WHERE id = ?1", [id])?;
    one_row(affected, id)
}

fn entry_not_found(id: i64) -> AppError {
    AppError::PlanValidation(format!("entry {id} not found"))
}

pub(crate) fn list_entries(conn: &Connection, plan_id: i64) -> Result<Vec<PlanEntry>, AppError> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {ENTRY_COLUMNS} FROM plan_entries WHERE plan_id = ?1 ORDER BY date, position, id"
    ))?;
    let rows = stmt.query_map([plan_id], entry_from_row)?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

pub(crate) fn insert_entry(conn: &Connection, entry: &NewEntry) -> Result<PlanEntry, AppError> {
    let position: i32 = conn.query_row(
        "SELECT COALESCE(MAX(position) + 1, 0) FROM plan_entries WHERE plan_id = ?1 AND date = ?2",
        rusqlite::params![entry.plan_id, entry.date],
        |row| row.get(0),
    )?;
    conn.execute(
        "INSERT INTO plan_entries (plan_id, date, position, file_name, workout_name, note) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![
            entry.plan_id,
            entry.date,
            position,
            entry.file_name,
            entry.workout_name,
            entry.note
        ],
    )?;
    let id = conn.last_insert_rowid();
    get_entry(conn, id)
}

/// Reads the row first: whether the linked session survives the edit is decided
/// by `plan::clears_session`, not by this SQL.
pub(crate) fn update_entry(
    conn: &Connection,
    id: i64,
    content: &EntryContent,
) -> Result<PlanEntry, AppError> {
    let previous = get_entry(conn, id)?;
    let keeps_link = !clears_session(previous.file_name.as_deref(), content.file_name.as_deref());
    let session_id = previous.session_id.filter(|_| keeps_link);
    conn.execute(
        "UPDATE plan_entries SET file_name = ?1, workout_name = ?2, note = ?3, session_id = ?4 WHERE id = ?5",
        rusqlite::params![
            content.file_name,
            content.workout_name,
            content.note,
            session_id,
            id
        ],
    )?;
    get_entry(conn, id)
}

pub(crate) fn delete_entry(conn: &Connection, id: i64) -> Result<(), AppError> {
    let affected = conn.execute("DELETE FROM plan_entries WHERE id = ?1", [id])?;
    if affected == 0 {
        return Err(entry_not_found(id));
    }
    Ok(())
}

pub(crate) fn entry_exists_file(conn: &Connection, file_name: &str) -> Result<bool, AppError> {
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM workouts WHERE file_name = ?1)",
        [file_name],
        |row| row.get(0),
    )?;
    Ok(exists)
}

pub(crate) fn workout_file_names(conn: &Connection) -> Result<Vec<String>, AppError> {
    let mut stmt = conn.prepare("SELECT file_name FROM workouts")?;
    let rows = stmt.query_map([], |row| row.get(0))?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

/// No archived-plan check: recording which session fulfilled a finished ride is
/// history, not a plan-content edit, unlike create/update/delete on entries.
pub(crate) fn link_session(
    conn: &Connection,
    entry_id: i64,
    session_id: i64,
) -> Result<PlanEntry, AppError> {
    let affected = conn.execute(
        "UPDATE plan_entries SET session_id = ?1 WHERE id = ?2",
        rusqlite::params![session_id, entry_id],
    )?;
    if affected == 0 {
        return Err(entry_not_found(entry_id));
    }
    get_entry(conn, entry_id)
}

pub(crate) fn get_entry(conn: &Connection, id: i64) -> Result<PlanEntry, AppError> {
    conn.query_row(
        &format!("SELECT {ENTRY_COLUMNS} FROM plan_entries WHERE id = ?1"),
        [id],
        entry_from_row,
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => entry_not_found(id),
        other => other.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations;

    fn store() -> Connection {
        let mut conn = Connection::open_in_memory().unwrap();
        migrations::run(&mut conn).unwrap();
        conn
    }

    fn new_plan(name: &str, start_date: &str, weeks: u32) -> NewPlan {
        NewPlan {
            name: name.to_string(),
            start_date: start_date.to_string(),
            weeks,
        }
    }

    fn seeded() -> (Connection, i64) {
        let conn = store();
        let id = insert(&conn, &new_plan("Base", "2026-09-14", 4)).unwrap();
        (conn, id)
    }

    fn archived_at_is_null(conn: &Connection, id: i64) -> bool {
        conn.query_row(
            "SELECT archived_at IS NULL FROM training_plans WHERE id = ?1",
            [id],
            |row| row.get(0),
        )
        .unwrap()
    }

    fn is_not_found(result: Result<(), AppError>) -> bool {
        matches!(result, Err(AppError::PlanValidation(msg)) if msg.contains("not found"))
    }

    #[test]
    fn insert_returns_the_new_id_and_preserves_every_field() {
        let (conn, id) = seeded();
        let stored = get(&conn, id).unwrap();
        assert_eq!(stored.id, id);
        assert_eq!(stored.name, "Base");
        assert_eq!(stored.start_date, "2026-09-14");
        assert_eq!(stored.weeks, 4);
        assert!(!stored.created_at.is_empty(), "created_at is stamped here");
        assert_eq!(stored.archived_at, None, "a new plan is active");
    }

    #[test]
    fn get_on_a_missing_id_is_a_plan_validation_error() {
        let conn = store();
        assert!(is_not_found(get(&conn, 404).map(|_| ())));
    }

    #[test]
    fn list_hides_archived_plans_unless_they_are_asked_for() {
        let (conn, id) = seeded();
        insert(&conn, &new_plan("Build", "2026-10-12", 4)).unwrap();
        set_archived(&conn, id, true).unwrap();

        let active = list(&conn, false).unwrap();
        assert_eq!(
            active.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(),
            ["Build"]
        );
        assert_eq!(list(&conn, true).unwrap().len(), 2);
    }

    #[test]
    fn list_orders_by_start_date_then_id_descending() {
        let conn = store();
        let old = insert(&conn, &new_plan("Old", "2026-09-14", 4)).unwrap();
        let recent = insert(&conn, &new_plan("Recent", "2026-10-12", 4)).unwrap();
        let same_day = insert(&conn, &new_plan("SameDay", "2026-09-14", 4)).unwrap();

        let ids: Vec<i64> = list(&conn, true).unwrap().iter().map(|p| p.id).collect();
        assert_eq!(ids, [recent, same_day, old]);
    }

    #[test]
    fn set_archived_true_stamps_a_timestamp() {
        let (conn, id) = seeded();
        set_archived(&conn, id, true).unwrap();
        let archived_at = get(&conn, id).unwrap().archived_at.expect("stamped");
        assert!(
            archived_at.parse::<chrono::DateTime<chrono::Utc>>().is_ok(),
            "archived_at must be RFC 3339, got `{archived_at}`"
        );
        assert!(!archived_at_is_null(&conn, id));
    }

    #[test]
    fn set_archived_false_writes_a_sql_null() {
        let (conn, id) = seeded();
        set_archived(&conn, id, true).unwrap();
        set_archived(&conn, id, false).unwrap();
        assert!(
            archived_at_is_null(&conn, id),
            "unarchiving must write NULL, not an empty string"
        );
        assert_eq!(list(&conn, false).unwrap().len(), 1);
    }

    #[test]
    fn update_changes_the_fields_and_leaves_created_at_alone() {
        let (conn, id) = seeded();
        let created_at = get(&conn, id).unwrap().created_at;
        update(&conn, id, &new_plan("Base rev2", "2026-10-12", 6)).unwrap();

        let stored = get(&conn, id).unwrap();
        assert_eq!(stored.name, "Base rev2");
        assert_eq!(stored.start_date, "2026-10-12");
        assert_eq!(stored.weeks, 6);
        assert_eq!(stored.created_at, created_at);
    }

    #[test]
    fn delete_removes_the_plan() {
        let (conn, id) = seeded();
        delete(&conn, id).unwrap();
        assert!(list(&conn, true).unwrap().is_empty());
    }

    #[test]
    fn update_on_a_missing_id_reports_not_found() {
        let conn = store();
        assert!(is_not_found(update(
            &conn,
            404,
            &new_plan("Ghost", "2026-09-14", 4)
        )));
    }

    #[test]
    fn set_archived_on_a_missing_id_reports_not_found() {
        let conn = store();
        assert!(is_not_found(set_archived(&conn, 404, true)));
    }

    #[test]
    fn delete_on_a_missing_id_reports_not_found() {
        let conn = store();
        assert!(is_not_found(delete(&conn, 404)));
    }

    fn new_entry(plan_id: i64, date: &str, file_name: &str, workout_name: &str) -> NewEntry {
        NewEntry {
            plan_id,
            date: date.to_string(),
            file_name: Some(file_name.to_string()),
            workout_name: Some(workout_name.to_string()),
            note: None,
        }
    }

    fn note_entry(plan_id: i64, date: &str, note: &str) -> NewEntry {
        NewEntry {
            plan_id,
            date: date.to_string(),
            file_name: None,
            workout_name: None,
            note: Some(note.to_string()),
        }
    }

    fn workout_content(file_name: &str, workout_name: &str, note: Option<&str>) -> EntryContent {
        EntryContent {
            file_name: Some(file_name.to_string()),
            workout_name: Some(workout_name.to_string()),
            note: note.map(String::from),
        }
    }

    fn note_content(note: &str) -> EntryContent {
        EntryContent {
            file_name: None,
            workout_name: None,
            note: Some(note.to_string()),
        }
    }

    fn seeded_with_entry() -> (Connection, i64, i64) {
        let (conn, plan_id) = seeded();
        let entry_id = insert_entry(&conn, &new_entry(plan_id, "2026-09-15", "base.zwo", "Base"))
            .unwrap()
            .id;
        (conn, plan_id, entry_id)
    }

    #[test]
    fn insert_entry_stamps_position_and_returns_the_row() {
        let (conn, plan_id, _) = seeded_with_entry();
        let e2 = insert_entry(
            &conn,
            &new_entry(plan_id, "2026-09-15", "tempo.zwo", "Tempo"),
        )
        .unwrap();
        assert_eq!(e2.position, 1);

        let entries = list_entries(&conn, plan_id).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].position, 0);
        assert_eq!(entries[1].position, 1);
    }

    #[test]
    fn list_entries_orders_by_date_position_then_id() {
        let (conn, plan_id, first_id) = seeded_with_entry();
        let third = insert_entry(&conn, &new_entry(plan_id, "2026-09-17", "vo2.zwo", "VO2"))
            .unwrap()
            .id;
        let second = insert_entry(
            &conn,
            &new_entry(plan_id, "2026-09-15", "tempo.zwo", "Tempo"),
        )
        .unwrap()
        .id;
        let ids: Vec<_> = list_entries(&conn, plan_id)
            .unwrap()
            .iter()
            .map(|e| e.id)
            .collect();
        // Same-date rows in insertion order (position), dates ascending across days.
        assert_eq!(ids, vec![first_id, second, third]);
    }

    fn insert_dummy_session(conn: &Connection) -> i64 {
        conn.execute(
            "INSERT INTO sessions (workout_name, started_at, ftp_w_used, flat_blocks) \
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params!["Dummy", "2026-09-15T00:00:00Z", 200, "[]"],
        )
        .unwrap();
        conn.last_insert_rowid()
    }

    fn seed_linked_session(conn: &Connection, entry_id: i64) -> i64 {
        let session_id = insert_dummy_session(conn);
        conn.execute(
            "UPDATE plan_entries SET session_id = ?1 WHERE id = ?2",
            rusqlite::params![session_id, entry_id],
        )
        .unwrap();
        session_id
    }

    #[test]
    fn insert_entry_persists_a_note_only_entry() {
        let (conn, plan_id) = seeded();
        let entry = insert_entry(&conn, &note_entry(plan_id, "2026-09-17", "swim 45min")).unwrap();
        assert_eq!(entry.file_name, None);
        assert_eq!(entry.workout_name, None);
        assert_eq!(entry.note, Some("swim 45min".to_string()));
        assert_eq!(entry.position, 0);
    }

    #[test]
    fn insert_entry_persists_a_note_next_to_a_workout() {
        let (conn, plan_id) = seeded();
        let mut new = new_entry(plan_id, "2026-09-15", "base.zwo", "Base");
        new.note = Some("easy gearing".to_string());
        let entry = insert_entry(&conn, &new).unwrap();
        assert_eq!(entry.file_name, Some("base.zwo".to_string()));
        assert_eq!(entry.note, Some("easy gearing".to_string()));
    }

    #[test]
    fn update_entry_changes_file_workout_and_clears_session() {
        let (conn, _, entry_id) = seeded_with_entry();
        seed_linked_session(&conn, entry_id);
        let updated = update_entry(
            &conn,
            entry_id,
            &workout_content("build.zwo", "Build", None),
        )
        .unwrap();
        assert_eq!(updated.file_name, Some("build.zwo".to_string()));
        assert_eq!(updated.workout_name, Some("Build".to_string()));
        assert_eq!(updated.session_id, None);
    }

    #[test]
    fn update_entry_keeps_the_session_link_when_only_the_note_changes() {
        let (conn, _, entry_id) = seeded_with_entry();
        let session_id = seed_linked_session(&conn, entry_id);
        let updated = update_entry(
            &conn,
            entry_id,
            &workout_content("base.zwo", "Base", Some("legs felt heavy")),
        )
        .unwrap();
        assert_eq!(updated.note, Some("legs felt heavy".to_string()));
        assert_eq!(updated.session_id, Some(session_id));
    }

    #[test]
    fn update_entry_clears_the_session_link_when_the_workout_is_dropped() {
        let (conn, _, entry_id) = seeded_with_entry();
        seed_linked_session(&conn, entry_id);
        let updated = update_entry(&conn, entry_id, &note_content("swim 45min")).unwrap();
        assert_eq!(updated.file_name, None);
        assert_eq!(updated.workout_name, None);
        assert_eq!(updated.note, Some("swim 45min".to_string()));
        assert_eq!(updated.session_id, None);
    }

    #[test]
    fn update_entry_on_missing_id_reports_not_found() {
        let conn = store();
        assert!(is_not_found(
            update_entry(&conn, 404, &workout_content("x.zwo", "X", None)).map(|_| ())
        ));
    }

    #[test]
    fn delete_entry_removes_the_row() {
        let (conn, plan_id, entry_id) = seeded_with_entry();
        delete_entry(&conn, entry_id).unwrap();
        assert!(list_entries(&conn, plan_id).unwrap().is_empty());
    }

    #[test]
    fn delete_entry_on_missing_id_reports_not_found() {
        let conn = store();
        assert!(is_not_found(delete_entry(&conn, 404)));
    }

    #[test]
    fn entry_exists_file_checks_the_workouts_cache() {
        let conn = store();
        conn.execute(
            "INSERT INTO workouts (file_name, mtime_secs, parsed_json) VALUES (?1, ?2, ?3)",
            ("known.zwo", 1, "{}"),
        )
        .unwrap();
        assert!(entry_exists_file(&conn, "known.zwo").unwrap());
        assert!(!entry_exists_file(&conn, "missing.zwo").unwrap());
    }

    #[test]
    fn workout_file_names_returns_all_cache_keys() {
        let conn = store();
        conn.execute(
            "INSERT INTO workouts (file_name, mtime_secs, parsed_json) VALUES (?1, ?2, ?3)",
            ("a.zwo", 1, "{}"),
        )
        .unwrap();
        let mut names = workout_file_names(&conn).unwrap();
        names.sort();
        assert_eq!(names, vec!["a.zwo".to_string()]);
    }

    #[test]
    fn link_session_sets_the_session_id_and_returns_the_updated_row() {
        let (conn, _, entry_id) = seeded_with_entry();
        let session_id = insert_dummy_session(&conn);
        let updated = link_session(&conn, entry_id, session_id).unwrap();
        assert_eq!(updated.session_id, Some(session_id));
        assert_eq!(
            get_entry(&conn, entry_id).unwrap().session_id,
            Some(session_id)
        );
    }

    #[test]
    fn link_session_overwrites_an_already_linked_session() {
        let (conn, _, entry_id) = seeded_with_entry();
        seed_linked_session(&conn, entry_id);
        let redo_session_id = insert_dummy_session(&conn);
        let updated = link_session(&conn, entry_id, redo_session_id).unwrap();
        assert_eq!(updated.session_id, Some(redo_session_id));
    }

    #[test]
    fn link_session_on_missing_entry_reports_not_found() {
        let conn = store();
        let session_id = insert_dummy_session(&conn);
        assert!(is_not_found(
            link_session(&conn, 404, session_id).map(|_| ())
        ));
    }
}
