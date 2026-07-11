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
];

pub fn run(conn: &mut Connection) -> Result<(), AppError> {
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;

    let mut current: u32 = conn.query_row(
        "SELECT user_version FROM pragma_user_version",
        [],
        |r| r.get(0),
    )?;

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
