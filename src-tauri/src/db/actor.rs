use crate::db::command::DbCommand;
use crate::db::{Metric, SessionCard, SessionDetail, Settings, StravaAuth};
use crate::errors::AppError;
use crate::metrics::{classify, normalized_power, WorkoutType};
use crate::session::FlatBlock;
use rusqlite::Connection;
use tokio::sync::mpsc::Receiver;
use tracing::info;

/// Compute the ISO-8601 `ended_at` timestamp for a session given its
/// `started_at` string (RFC 3339) and the reconstructed `duration_s`.
///
/// This is a pure function so it can be unit-tested without a database.
pub(crate) fn ended_at_from_parts(started_at: &str, duration_s: u32) -> Option<String> {
    use chrono::{DateTime, Duration, Utc};
    let start: DateTime<Utc> = started_at.parse().ok()?;
    Some((start + Duration::seconds(duration_s as i64)).to_rfc3339())
}

/// Aggregated session metrics: (avg_power, max_power, avg_hr, max_hr, avg_cad, max_cad).
type SessionAggregates = (
    Option<i64>,
    Option<i64>,
    Option<i64>,
    Option<i64>,
    Option<i64>,
    Option<i64>,
);

/// Metrics derived from the full power series at finalize. All fields are `None`
/// together when no power was recorded (or FTP is unknown).
#[derive(Default)]
struct DerivedMetrics {
    workout_type: Option<WorkoutType>,
    np_w: Option<f32>,
    if_: Option<f32>,
    tss: Option<f32>,
}

pub struct DbActor {
    conn: Connection,
    cmd_rx: Receiver<DbCommand>,
}

impl DbActor {
    pub fn new(cmd_rx: Receiver<DbCommand>, db_path: String) -> Result<Self, AppError> {
        let mut conn = Connection::open(&db_path).map_err(|e| AppError::DbError(e.to_string()))?;
        crate::db::migrations::run(&mut conn)?;
        Ok(DbActor { conn, cmd_rx })
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
                    DbCommand::InsertSession {
                        workout_name,
                        started_at,
                        ftp_w_used,
                        flat_blocks_json,
                        reply,
                    } => {
                        let _ = reply.send(self.insert_session(
                            workout_name,
                            started_at,
                            ftp_w_used,
                            flat_blocks_json,
                        ));
                    }
                    DbCommand::InsertMetric { session_id, metric } => {
                        if let Err(e) = self.insert_metric(session_id, metric) {
                            tracing::error!("insert_metric failed: {e}");
                        }
                    }
                    DbCommand::FinalizeSession {
                        session_id,
                        ended_at,
                        duration_s,
                    } => {
                        if let Err(e) = self.finalize_session(session_id, ended_at, duration_s) {
                            tracing::error!("finalize_session failed: {e}");
                        }
                    }
                    DbCommand::ListSessions { reply } => {
                        let _ = reply.send(self.list_sessions());
                    }
                    DbCommand::GetSession { id, reply } => {
                        let _ = reply.send(self.get_session(id));
                    }
                    DbCommand::DeleteSession { id, reply } => {
                        let _ = reply.send(self.delete_session(id));
                    }
                    DbCommand::UpsertStravaAuth { auth, reply } => {
                        let _ = reply.send(self.upsert_strava_auth(auth));
                    }
                    DbCommand::GetStravaAuth { reply } => {
                        let _ = reply.send(self.get_strava_auth());
                    }
                    DbCommand::DeleteStravaAuth { reply } => {
                        let _ = reply.send(self.delete_strava_auth());
                    }
                    DbCommand::GetStravaAutoUpload { reply } => {
                        let _ = reply.send(self.get_strava_auto_upload());
                    }
                    DbCommand::SetStravaAutoUpload { enabled, reply } => {
                        let _ = reply.send(self.set_strava_auto_upload(enabled));
                    }
                    DbCommand::SetSessionStravaActivity {
                        session_id,
                        activity_id,
                        reply,
                    } => {
                        let _ =
                            reply.send(self.set_session_strava_activity(session_id, activity_id));
                    }
                },
            }
        }
    }

    /// Finalize any sessions left open by a previous crash, power loss, or forced
    /// close. Runs once at startup, synchronously, before the actor loop begins
    /// and before `SessionActor` can start anything -- so no live session can
    /// exist during this pass.
    ///
    /// For each orphan (ended_at IS NULL):
    ///   duration_s = MAX(t_offset_s) + 1 over its samples.
    ///   ended_at   = started_at + duration_s (approximation: samples carry only
    ///                active seconds; the true wall-clock end is unknowable after
    ///                the fact, so we reconstruct it from the last recorded offset).
    ///   Then the normal finalize_session path runs, computing aggregates / NP /
    ///   IF / TSS exactly as for a cleanly-stopped session.
    ///
    /// Edge case -- zero samples: the session is deleted. A session with no data
    /// at all provides no value in history and cannot be meaningfully finalized.
    pub(crate) fn finalize_orphaned_sessions(&mut self) -> Result<(), AppError> {
        // Collect orphan ids and started_at strings first to avoid holding the
        // query open while we mutate rows inside the loop.
        let orphans: Vec<(i64, String)> = {
            let mut stmt = self
                .conn
                .prepare("SELECT id, started_at FROM sessions WHERE ended_at IS NULL")
                .map_err(|e| AppError::DbError(e.to_string()))?;
            let rows = stmt
                .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)))
                .map_err(|e| AppError::DbError(e.to_string()))?;
            rows.collect::<Result<Vec<_>, _>>()
                .map_err(|e| AppError::DbError(e.to_string()))?
        };

        for (session_id, started_at) in orphans {
            // Compute duration from sample offsets. MAX returns NULL when there
            // are no rows, which maps to None via rusqlite's Option handling.
            let max_offset: Option<u32> = self
                .conn
                .query_row(
                    "SELECT MAX(t_offset_s) FROM session_metrics WHERE session_id = ?1",
                    [session_id],
                    |row| row.get(0),
                )
                .map_err(|e| AppError::DbError(e.to_string()))?;

            match max_offset {
                None => {
                    // No samples recorded -- session has no data. Delete it rather
                    // than leaving a hollow stub in history.
                    tracing::warn!(
                        session_id,
                        "orphaned session has no samples -- deleting"
                    );
                    self.conn
                        .execute("DELETE FROM sessions WHERE id = ?1", [session_id])
                        .map_err(|e| AppError::DbError(e.to_string()))?;
                }
                Some(max_t) => {
                    let duration_s = max_t + 1;
                    let ended_at = match ended_at_from_parts(&started_at, duration_s) {
                        Some(ts) => ts,
                        None => {
                            tracing::error!(
                                session_id,
                                started_at,
                                "orphaned session has unparseable started_at -- skipping"
                            );
                            continue;
                        }
                    };
                    tracing::info!(
                        session_id,
                        duration_s,
                        "finalizing orphaned session"
                    );
                    if let Err(e) = self.finalize_session(session_id, ended_at, duration_s) {
                        tracing::error!(session_id, "finalize_orphaned_sessions failed: {e}");
                    }
                }
            }
        }
        Ok(())
    }

    fn query_settings(&self) -> Result<Settings, AppError> {
        self.conn
            .query_row(
                "SELECT ftp_w, max_hr_bpm, workout_path, strava_proxy_url, aero_enabled \
                 FROM settings WHERE id = 1",
                [],
                |row| {
                    Ok(Settings {
                        ftp_w: row.get(0)?,
                        max_hr_bpm: row.get(1)?,
                        workout_path: row.get(2)?,
                        strava_proxy_url: row.get(3)?,
                        aero_enabled: row.get::<_, i64>(4)? != 0,
                    })
                },
            )
            .map_err(|e| AppError::DbError(e.to_string()))
    }

    fn update_settings(&mut self, settings: Settings) -> Result<(), AppError> {
        let mut stmt = self
            .conn
            .prepare(
                "UPDATE settings SET ftp_w=(?1), max_hr_bpm=(?2), workout_path=(?3), \
                 strava_proxy_url=(?4), aero_enabled=(?5) WHERE id = 1",
            )
            .map_err(|e| AppError::DbError(e.to_string()))?;
        stmt.execute((
            settings.ftp_w,
            settings.max_hr_bpm,
            settings.workout_path,
            settings.strava_proxy_url,
            settings.aero_enabled as i64,
        ))
        .map_err(|e| AppError::DbError(e.to_string()))?;
        Ok(())
    }

    fn insert_session(
        &mut self,
        workout_name: String,
        started_at: String,
        ftp_w_used: u16,
        flat_blocks_json: String,
    ) -> Result<i64, AppError> {
        self.conn
            .execute(
                "INSERT INTO sessions (workout_name, started_at, ftp_w_used, flat_blocks) \
                 VALUES (?1, ?2, ?3, ?4)",
                (&workout_name, &started_at, ftp_w_used, &flat_blocks_json),
            )
            .map_err(|e| AppError::DbError(e.to_string()))?;
        Ok(self.conn.last_insert_rowid())
    }

    fn insert_metric(&mut self, session_id: i64, metric: Metric) -> Result<(), AppError> {
        self.conn
            .execute(
                "INSERT INTO session_metrics (session_id, t_offset_s, power_w, hr_bpm, cadence_rpm, aero_score) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                (
                    session_id,
                    metric.t_offset_s,
                    metric.power_w,
                    metric.hr_bpm,
                    metric.cadence_rpm,
                    metric.aero_score,
                ),
            )
            .map_err(|e| AppError::DbError(e.to_string()))?;
        Ok(())
    }
    fn finalize_session(
        &mut self,
        session_id: i64,
        ended_at: String,
        duration_s: u32,
    ) -> Result<(), AppError> {
        let ftp_w_used: u16 = self
            .conn
            .query_row(
                "SELECT ftp_w_used FROM sessions WHERE id = ?1",
                [session_id],
                |row| row.get(0),
            )
            .map_err(|e| AppError::DbError(e.to_string()))?;

        let (avg_power, max_power, avg_hr, max_hr, avg_cad, max_cad): SessionAggregates = self
            .conn
            .query_row(
                "SELECT \
                    CAST(AVG(power_w)     AS INTEGER), MAX(power_w), \
                    CAST(AVG(hr_bpm)      AS INTEGER), MAX(hr_bpm), \
                    CAST(AVG(cadence_rpm) AS INTEGER), MAX(cadence_rpm) \
                 FROM session_metrics WHERE session_id = ?1",
                [session_id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                    ))
                },
            )
            .map_err(|e| AppError::DbError(e.to_string()))?;

        let derived = self.compute_session_metrics(session_id, ftp_w_used, duration_s)?;

        // Share of samples spent in aero. The frontend already made the binary
        // aero/upright decision (smoothing + hysteresis) and stored it as 0.0/1.0,
        // so a plain AVG reproduces exactly what the rider saw live. AVG skips NULLs
        // (samples with no aero report), yielding NULL when none were recorded.
        let aero_pct: Option<f64> = self
            .conn
            .query_row(
                "SELECT AVG(aero_score) FROM session_metrics WHERE session_id = ?1",
                [session_id],
                |row| row.get(0),
            )
            .map_err(|e| AppError::DbError(e.to_string()))?;

        self.conn
            .execute(
                "UPDATE sessions SET \
                    ended_at = ?1, duration_s = ?2, \
                    avg_power_w = ?3, max_power_w = ?4, \
                    avg_hr_bpm = ?5, max_hr_bpm = ?6, \
                    avg_cadence_rpm = ?7, max_cadence_rpm = ?8, \
                    workout_type = ?9, aero_pct = ?10, \
                    np_w = ?11, if_ = ?12, tss = ?13 \
                 WHERE id = ?14",
                (
                    &ended_at,
                    duration_s,
                    avg_power,
                    max_power,
                    avg_hr,
                    max_hr,
                    avg_cad,
                    max_cad,
                    derived.workout_type.map(|t| t.as_str()),
                    aero_pct,
                    derived.np_w,
                    derived.if_,
                    derived.tss,
                    session_id,
                ),
            )
            .map_err(|e| AppError::DbError(e.to_string()))?;
        Ok(())
    }

    /// Single source of truth for the session's derived metrics, computed once at
    /// finalize from the full 1 Hz power series against the frozen FTP. Returns all
    /// `None` when no power was recorded (or FTP is unknown), so the caller stores
    /// workout_type / NP / IF / TSS as NULL.
    fn compute_session_metrics(
        &self,
        session_id: i64,
        ftp_w_used: u16,
        duration_s: u32,
    ) -> Result<DerivedMetrics, AppError> {
        if ftp_w_used == 0 {
            return Ok(DerivedMetrics::default());
        }
        let mut stmt = self
            .conn
            .prepare(
                "SELECT power_w FROM session_metrics \
                 WHERE session_id = ?1 AND power_w IS NOT NULL \
                 ORDER BY t_offset_s",
            )
            .map_err(|e| AppError::DbError(e.to_string()))?;
        let rows = stmt
            .query_map([session_id], |row| row.get::<_, i64>(0))
            .map_err(|e| AppError::DbError(e.to_string()))?;

        let ftp = ftp_w_used as f32;
        let mut series_pcts: Vec<f32> = Vec::new();
        let mut powers: Vec<f64> = Vec::new();
        for r in rows {
            let w = r.map_err(|e| AppError::DbError(e.to_string()))? as f64;
            powers.push(w);
            series_pcts.push(w as f32 / ftp);
        }
        let Some(np_w) = normalized_power(&powers) else {
            return Ok(DerivedMetrics::default());
        };
        let if_ = np_w / ftp;
        let tss = (duration_s as f32 / 3600.0) * if_ * if_ * 100.0;
        Ok(DerivedMetrics {
            workout_type: Some(classify(&series_pcts, if_)),
            np_w: Some(np_w),
            if_: Some(if_),
            tss: Some(tss),
        })
    }

    fn list_sessions(&self) -> Result<Vec<SessionCard>, AppError> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, started_at, workout_name, duration_s, \
                        avg_power_w, avg_hr_bpm, avg_cadence_rpm, ftp_w_used, workout_type, aero_pct, \
                        np_w, if_, tss \
                 FROM sessions \
                 ORDER BY started_at DESC",
            )
            .map_err(|e| AppError::DbError(e.to_string()))?;
        let rows = stmt
            .query_map([], |row| {
                let workout_type_str: Option<String> = row.get(8)?;
                Ok(SessionCard {
                    id: row.get(0)?,
                    started_at: row.get(1)?,
                    workout_name: row.get(2)?,
                    duration_s: row.get(3)?,
                    avg_power_w: row.get(4)?,
                    avg_hr_bpm: row.get(5)?,
                    avg_cadence_rpm: row.get(6)?,
                    ftp_w_used: row.get(7)?,
                    workout_type: workout_type_str.as_deref().and_then(WorkoutType::from_str),
                    aero_pct: row.get(9)?,
                    np_w: row.get(10)?,
                    if_: row.get(11)?,
                    tss: row.get(12)?,
                })
            })
            .map_err(|e| AppError::DbError(e.to_string()))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| AppError::DbError(e.to_string()))
    }

    fn get_session(&self, id: i64) -> Result<SessionDetail, AppError> {
        // First pass: read scalar columns + raw JSON / type-string.
        // Vec fields are populated below from a second query.
        let (mut detail, flat_blocks_json) = self
            .conn
            .query_row(
                "SELECT started_at, ended_at, workout_name, duration_s, \
                        avg_power_w, max_power_w, avg_hr_bpm, max_hr_bpm, \
                        avg_cadence_rpm, max_cadence_rpm, ftp_w_used, \
                        workout_type, aero_pct, np_w, if_, tss, flat_blocks, strava_activity_id \
                 FROM sessions WHERE id = ?1",
                [id],
                |row| {
                    let workout_type_str: Option<String> = row.get("workout_type")?;
                    let flat_blocks_json: String = row.get("flat_blocks")?;
                    let detail = SessionDetail {
                        id,
                        strava_activity_id: row.get("strava_activity_id")?,
                        started_at: row.get("started_at")?,
                        ended_at: row.get("ended_at")?,
                        workout_name: row.get("workout_name")?,
                        duration_s: row.get("duration_s")?,
                        avg_power_w: row.get("avg_power_w")?,
                        max_power_w: row.get("max_power_w")?,
                        avg_hr_bpm: row.get("avg_hr_bpm")?,
                        max_hr_bpm: row.get("max_hr_bpm")?,
                        avg_cadence_rpm: row.get("avg_cadence_rpm")?,
                        max_cadence_rpm: row.get("max_cadence_rpm")?,
                        ftp_w_used: row.get("ftp_w_used")?,
                        workout_type: workout_type_str.as_deref().and_then(WorkoutType::from_str),
                        aero_pct: row.get("aero_pct")?,
                        np_w: row.get("np_w")?,
                        if_: row.get("if_")?,
                        tss: row.get("tss")?,
                        flat_blocks: Vec::new(),
                        metrics: Vec::new(),
                    };
                    Ok((detail, flat_blocks_json))
                },
            )
            .map_err(|e| AppError::DbError(e.to_string()))?;

        detail.flat_blocks = serde_json::from_str::<Vec<FlatBlock>>(&flat_blocks_json)
            .map_err(|e| AppError::DbError(e.to_string()))?;

        let mut stmt = self
            .conn
            .prepare(
                "SELECT t_offset_s, power_w, hr_bpm, cadence_rpm, aero_score \
                 FROM session_metrics WHERE session_id = ?1 ORDER BY t_offset_s",
            )
            .map_err(|e| AppError::DbError(e.to_string()))?;
        detail.metrics = stmt
            .query_map([id], |row| {
                Ok(Metric {
                    t_offset_s: row.get(0)?,
                    power_w: row.get(1)?,
                    hr_bpm: row.get(2)?,
                    cadence_rpm: row.get(3)?,
                    aero_score: row.get(4)?,
                })
            })
            .map_err(|e| AppError::DbError(e.to_string()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| AppError::DbError(e.to_string()))?;

        Ok(detail)
    }

    fn delete_session(&mut self, id: i64) -> Result<(), AppError> {
        // session_metrics rows cascade via the FK ON DELETE CASCADE.
        self.conn
            .execute("DELETE FROM sessions WHERE id = ?1", [id])
            .map_err(|e| AppError::DbError(e.to_string()))?;
        Ok(())
    }

    fn upsert_strava_auth(&mut self, auth: StravaAuth) -> Result<(), AppError> {
        self.conn
            .execute(
                "INSERT INTO strava_auth \
                   (id, access_token, refresh_token, expires_at, athlete_id, athlete_name, connected_at) \
                 VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6) \
                 ON CONFLICT(id) DO UPDATE SET \
                   access_token=excluded.access_token, \
                   refresh_token=excluded.refresh_token, \
                   expires_at=excluded.expires_at, \
                   athlete_id=excluded.athlete_id, \
                   athlete_name=excluded.athlete_name, \
                   connected_at=excluded.connected_at",
                (
                    &auth.access_token,
                    &auth.refresh_token,
                    auth.expires_at,
                    auth.athlete_id,
                    &auth.athlete_name,
                    &auth.connected_at,
                ),
            )
            .map_err(|e| AppError::DbError(e.to_string()))?;
        Ok(())
    }

    fn get_strava_auth(&self) -> Result<Option<StravaAuth>, AppError> {
        self.conn
            .query_row(
                "SELECT access_token, refresh_token, expires_at, athlete_id, athlete_name, connected_at \
                 FROM strava_auth WHERE id = 1",
                [],
                |row| {
                    Ok(StravaAuth {
                        access_token: row.get(0)?,
                        refresh_token: row.get(1)?,
                        expires_at: row.get(2)?,
                        athlete_id: row.get(3)?,
                        athlete_name: row.get(4)?,
                        connected_at: row.get(5)?,
                    })
                },
            )
            .map(Some)
            .or_else(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                other => Err(AppError::DbError(other.to_string())),
            })
    }

    fn delete_strava_auth(&mut self) -> Result<(), AppError> {
        self.conn
            .execute("DELETE FROM strava_auth WHERE id = 1", [])
            .map_err(|e| AppError::DbError(e.to_string()))?;
        Ok(())
    }

    fn get_strava_auto_upload(&self) -> Result<bool, AppError> {
        self.conn
            .query_row(
                "SELECT strava_auto_upload FROM settings WHERE id = 1",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map(|v| v != 0)
            .map_err(|e| AppError::DbError(e.to_string()))
    }

    fn set_strava_auto_upload(&mut self, enabled: bool) -> Result<(), AppError> {
        self.conn
            .execute(
                "UPDATE settings SET strava_auto_upload = ?1 WHERE id = 1",
                [enabled as i64],
            )
            .map_err(|e| AppError::DbError(e.to_string()))?;
        Ok(())
    }

    fn set_session_strava_activity(
        &mut self,
        session_id: i64,
        activity_id: i64,
    ) -> Result<(), AppError> {
        self.conn
            .execute(
                "UPDATE sessions SET strava_activity_id = ?1 WHERE id = ?2",
                (activity_id, session_id),
            )
            .map_err(|e| AppError::DbError(e.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::ended_at_from_parts;

    #[test]
    fn ended_at_adds_duration_to_started_at() {
        // A session started at midnight UTC; 90 seconds later should land at 00:01:30.
        let result = ended_at_from_parts("2024-01-15T00:00:00+00:00", 90);
        assert!(result.is_some(), "should produce a timestamp");
        let ts = result.unwrap();
        // The returned string must be parseable as RFC 3339 and encode the
        // correct instant (90 s after the start).
        let parsed: chrono::DateTime<chrono::Utc> =
            ts.parse().expect("result must be valid RFC 3339");
        assert_eq!(parsed.timestamp(), 1705276890); // 2024-01-15T00:01:30Z
    }

    #[test]
    fn ended_at_duration_zero_equals_start() {
        let result = ended_at_from_parts("2024-06-01T12:00:00+00:00", 0);
        assert!(result.is_some());
        let parsed: chrono::DateTime<chrono::Utc> =
            result.unwrap().parse().expect("valid RFC 3339");
        assert_eq!(parsed.timestamp(), 1717243200); // 2024-06-01T12:00:00Z
    }

    #[test]
    fn ended_at_rejects_garbage_started_at() {
        assert!(ended_at_from_parts("not-a-date", 100).is_none());
    }
}
