use crate::errors::AppError;
use rusqlite::Connection;

/// Each entry is a self-contained SQL script for migrating from version N to N+1.
/// Never modify or reorder existing entries, only append new ones.
const MIGRATIONS: &[&str] = &[
    // v0 -> v1 : initial schema
    r#"
    CREATE TABLE IF NOT EXISTS sessions(
        id integer PRIMARY KEY,
        started_at TEXT NOT NULL,
        ended_at TEXT,
        workout_name text NOT NULL,
        avg_power_w integer,
        max_power_w integer,
        avg_hr_bpm integer,
        max_hr_bpm integer,
        avg_cadence_rpm integer,
        max_cadence_rpm integer,
        duration_s integer,
        flat_blocks text NOT NULL DEFAULT '[]',
        ftp_w_used integer NOT NULL DEFAULT 0,
        workout_type text
    );
    CREATE TABLE IF NOT EXISTS session_metrics(
        id integer PRIMARY KEY,
        session_id INTEGER NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
        t_offset_s integer NOT NULL,
        power_w integer,
        hr_bpm integer,
        cadence_rpm integer
    );
    CREATE INDEX IF NOT EXISTS idx_session_metrics_session
        ON session_metrics(session_id, t_offset_s);
    CREATE TABLE IF NOT EXISTS settings (
        id integer PRIMARY KEY,
        ftp_w integer NOT NULL DEFAULT 200,
        max_hr_bpm integer NOT NULL DEFAULT 190,
        workout_path text NOT NULL DEFAULT ''
    );
    INSERT OR IGNORE INTO settings (id, ftp_w, max_hr_bpm, workout_path)
        VALUES (1, 200, 190, '');
    "#,
    // v1 -> v2 : Strava integration
    r#"
    CREATE TABLE IF NOT EXISTS strava_auth(
        id            INTEGER PRIMARY KEY CHECK (id = 1),
        access_token  TEXT NOT NULL,
        refresh_token TEXT NOT NULL,
        expires_at    INTEGER NOT NULL,
        athlete_id    INTEGER,
        connected_at  TEXT NOT NULL
    );
    ALTER TABLE settings ADD COLUMN strava_auto_upload INTEGER NOT NULL DEFAULT 0;
    ALTER TABLE sessions ADD COLUMN strava_activity_id INTEGER;
    "#,
    // v2 -> v3 : store the athlete display name
    r#"
    ALTER TABLE strava_auth ADD COLUMN athlete_name TEXT;
    "#,
    // v3 -> v4 : configurable Strava auth proxy URL (each user runs their own)
    r#"
    ALTER TABLE settings ADD COLUMN strava_proxy_url TEXT NOT NULL DEFAULT 'http://127.0.0.1:8788';
    "#,
    // v4 -> v5 : aero position detection
    r#"
    ALTER TABLE session_metrics ADD COLUMN aero_score REAL;
    ALTER TABLE sessions ADD COLUMN aero_pct REAL;
    ALTER TABLE settings ADD COLUMN aero_enabled INTEGER NOT NULL DEFAULT 0;
    "#,
    // v5 -> v6 : persist NP / IF / TSS per session (single source of truth, frozen FTP)
    r#"
    ALTER TABLE sessions ADD COLUMN np_w REAL;
    ALTER TABLE sessions ADD COLUMN if_ REAL;
    ALTER TABLE sessions ADD COLUMN tss REAL;
    "#,
    // v6 -> v7 : auto-connect to known BLE devices
    r#"
    ALTER TABLE settings ADD COLUMN trainer_device_id TEXT;
    ALTER TABLE settings ADD COLUMN trainer_device_name TEXT;
    ALTER TABLE settings ADD COLUMN hrm_device_id TEXT;
    ALTER TABLE settings ADD COLUMN hrm_device_name TEXT;
    ALTER TABLE settings ADD COLUMN auto_connect INTEGER NOT NULL DEFAULT 1;
    "#,
    // v7 -> v8 : stall detection timeout (seconds of no pedaling before auto-pause)
    r#"
    ALTER TABLE settings ADD COLUMN stall_timeout_s INTEGER NOT NULL DEFAULT 5;
    "#,
    // v8 -> v9 : workout library cache (avoid reparsing unchanged .zwo files)
    r#"
    CREATE TABLE IF NOT EXISTS workouts(
        file_name   TEXT PRIMARY KEY,
        mtime_secs  INTEGER NOT NULL,
        parsed_json TEXT NOT NULL
    );
    "#,
    // v9 -> v10 : multi-week training plans (plans + their scheduled days)
    r#"
    CREATE TABLE IF NOT EXISTS training_plans(
        id          INTEGER PRIMARY KEY,
        name        TEXT    NOT NULL,
        start_date  TEXT    NOT NULL, -- always a Monday
        weeks       INTEGER NOT NULL,
        created_at  TEXT    NOT NULL,
        archived_at TEXT,             -- NULL = active
        -- Mirrors MAX_WEEKS in plan/schedule.rs: one bad row breaks list().
        CHECK (weeks BETWEEN 1 AND 52)
    );
    CREATE TABLE IF NOT EXISTS plan_entries(
        id           INTEGER PRIMARY KEY,
        plan_id      INTEGER NOT NULL REFERENCES training_plans(id) ON DELETE CASCADE,
        date         TEXT    NOT NULL,
        position     INTEGER NOT NULL DEFAULT 0,
        file_name    TEXT,   -- `workouts` cache key, NULL on a note-only day
        workout_name TEXT,
        note         TEXT,
        session_id   INTEGER REFERENCES sessions(id) ON DELETE SET NULL,
        CHECK (file_name IS NOT NULL OR note IS NOT NULL)
    );
    CREATE INDEX IF NOT EXISTS idx_plan_entries_plan
        ON plan_entries(plan_id, date, position);
    "#,
];

pub fn run(conn: &mut Connection) -> Result<(), AppError> {
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;

    let mut current: u32 =
        conn.query_row("SELECT user_version FROM pragma_user_version", [], |r| {
            r.get(0)
        })?;

    tracing::info!(
        "DB schema at version {current}, latest = {}",
        MIGRATIONS.len()
    );

    for (idx, sql) in MIGRATIONS.iter().enumerate().skip(current as usize) {
        let target = (idx + 1) as u32;
        tracing::info!("applying migration v{current} -> v{target}");
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", target)?;
        tx.commit()?;
        current = target;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::run;
    use rusqlite::Connection;

    fn migrated() -> Connection {
        let mut conn = Connection::open_in_memory().unwrap();
        run(&mut conn).unwrap();
        conn
    }

    fn table_exists(conn: &Connection, name: &str) -> bool {
        conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
            [name],
            |row| row.get::<_, i64>(0),
        )
        .unwrap()
            == 1
    }

    #[test]
    fn migrations_create_the_training_plan_tables() {
        let conn = migrated();
        assert!(table_exists(&conn, "training_plans"));
        assert!(table_exists(&conn, "plan_entries"));
    }

    #[test]
    fn deleting_a_plan_cascades_to_its_entries() {
        let conn = migrated();
        conn.execute_batch(
            "INSERT INTO training_plans (id, name, start_date, weeks, created_at)
                 VALUES (1, 'Base', '2026-09-14', 4, '2026-09-13T00:00:00+00:00');
             INSERT INTO plan_entries (plan_id, date, file_name)
                 VALUES (1, '2026-09-15', 'tempo.zwo');",
        )
        .unwrap();
        conn.execute("DELETE FROM training_plans WHERE id = 1", [])
            .unwrap();
        let remaining: i64 = conn
            .query_row("SELECT COUNT(*) FROM plan_entries", [], |row| row.get(0))
            .unwrap();
        assert_eq!(remaining, 0);
    }

    #[test]
    fn a_plan_entry_needs_a_workout_or_a_note() {
        let conn = migrated();
        conn.execute_batch(
            "INSERT INTO training_plans (id, name, start_date, weeks, created_at)
                 VALUES (1, 'Base', '2026-09-14', 4, '2026-09-13T00:00:00+00:00');",
        )
        .unwrap();
        let empty_day = conn.execute(
            "INSERT INTO plan_entries (plan_id, date) VALUES (1, ?1)",
            ["2026-09-15"],
        );
        assert!(empty_day.is_err(), "CHECK must reject a day with neither");
    }

    #[test]
    fn deleting_a_session_keeps_the_planned_entry() {
        let conn = migrated();
        conn.execute_batch(
            "INSERT INTO sessions (id, started_at, workout_name)
                 VALUES (9, '2026-09-15T08:00:00+00:00', 'Tempo');
             INSERT INTO training_plans (id, name, start_date, weeks, created_at)
                 VALUES (1, 'Base', '2026-09-14', 4, '2026-09-13T00:00:00+00:00');
             INSERT INTO plan_entries (plan_id, date, file_name, session_id)
                 VALUES (1, '2026-09-15', 'tempo.zwo', 9);",
        )
        .unwrap();
        conn.execute("DELETE FROM sessions WHERE id = 9", [])
            .unwrap();
        let orphaned: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM plan_entries WHERE session_id IS NULL",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(orphaned, 1, "the entry must survive with a NULL session_id");
    }

    fn insert_plan_with_weeks(conn: &Connection, weeks: i64) -> rusqlite::Result<usize> {
        conn.execute(
            "INSERT INTO training_plans (name, start_date, weeks, created_at)
                 VALUES ('Base', '2026-09-14', ?1, '2026-09-13T00:00:00+00:00')",
            [weeks],
        )
    }

    #[test]
    fn training_plans_reject_a_week_count_outside_one_to_fifty_two() {
        let conn = migrated();
        assert!(insert_plan_with_weeks(&conn, 0).is_err(), "0 weeks");
        assert!(insert_plan_with_weeks(&conn, 53).is_err(), "53 weeks");
    }

    #[test]
    fn training_plans_accept_the_week_count_boundaries() {
        let conn = migrated();
        assert!(insert_plan_with_weeks(&conn, 1).is_ok());
        assert!(insert_plan_with_weeks(&conn, 52).is_ok());
    }
}
