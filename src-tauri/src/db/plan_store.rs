//! SQL for the `training_plans` table. Lives outside `actor.rs` to keep that
//! file under the 1000-line size gate; the DB actor is the only consumer.

use crate::errors::AppError;
use crate::plan::{NewPlan, TrainingPlan};
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
}
