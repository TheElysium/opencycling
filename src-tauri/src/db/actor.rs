use crate::db::command::DbCommand;
use crate::db::Settings;
use crate::errors::AppError;
use rusqlite::Connection;
use tokio::sync::mpsc::Receiver;
use tracing::info;

pub struct DbActor {
    conn: Connection,
    cmd_rx: Receiver<DbCommand>,
}

impl DbActor {
    pub fn new(cmd_rx: Receiver<DbCommand>, db_path: String) -> Result<Self, AppError> {
        let conn = Connection::open(&db_path).map_err(|e| AppError::DbError(e.to_string()))?;
        let actor = DbActor { conn, cmd_rx };
        actor.init_schema()?;
        Ok(actor)
    }

    fn init_schema(&self) -> Result<(), AppError> {
        self.conn
            .execute_batch(r#"
                PRAGMA foreign_keys = ON;
                CREATE TABLE IF NOT EXISTS sessions(
                    id integer PRIMARY KEY,
                    started_at TEXT NOT NULL,
                    ended_at TEXT,
                    workout_name text NOT NULL,
                    avg_power_w integer,
                    max_power_w integer,
                    avg_hr_bpm integer,
                    max_hr_bpm integer,
                    duration_s integer
                );
                CREATE TABLE IF NOT EXISTS session_metrics(
                     id integer PRIMARY KEY,
                     session_id INTEGER NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
                     t_offset_s integer,
                     power_w integer,
                     hr_bpm integer,
                     cadence_rpm integer
                );
                CREATE TABLE IF NOT EXISTS settings (
                    id integer PRIMARY KEY,
                    ftp_w integer NOT NULL DEFAULT 200,
                    max_hr_bpm integer NOT NULL DEFAULT 190,
                    workout_path text NOT NULL DEFAULT ''
                );
                INSERT OR IGNORE INTO settings (id, ftp_w, max_hr_bpm, workout_path) VALUES (1, 200, 190, '')
            "#)
            .map_err(|e| AppError::DbError(e.to_string()))?;
        Ok(())
    }

    pub async fn run(mut self) {
        loop {
            match self.cmd_rx.recv().await {
                None => {
                    info!("DbActor shutting down");
                    break;
                }
                Some(cmd) => match cmd {
                    DbCommand::QuerySettings { reply } => {
                        let _ = reply.send(self.query_settings());
                    }
                    DbCommand::UpdateSettings { reply, settings } => {
                        let _ = reply.send(self.update_settings(settings));
                    }
                },
            }
        }
    }

    fn query_settings(&self) -> Result<Settings, AppError> {
        self.conn
            .query_row(
                "SELECT ftp_w, max_hr_bpm, workout_path FROM settings WHERE id = 1",
                [],
                |row| {
                    Ok(Settings {
                        ftp_w: row.get(0)?,
                        max_hr_bpm: row.get(1)?,
                        workout_path: row.get(2)?,
                    })
                },
            )
            .map_err(|e| AppError::DbError(e.to_string()))
    }

    fn update_settings(&mut self, settings: Settings) -> Result<(), AppError> {
        let mut stmt = self
            .conn
            .prepare(
                "UPDATE settings SET ftp_w=(?1), max_hr_bpm=(?2), workout_path=(?3) WHERE id = 1",
            )
            .map_err(|e| AppError::DbError(e.to_string()))?;
        stmt.execute((settings.ftp_w, settings.max_hr_bpm, settings.workout_path))
            .map_err(|e| AppError::DbError(e.to_string()))?;
        Ok(())
    }
}
