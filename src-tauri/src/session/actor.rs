use crate::db::Metric;
use crate::errors::AppError;
use crate::session::state::TICK_S;
use crate::session::types::{
    FlatBlock, Session, SessionActor, SessionCommand, SessionMetrics, SessionSnapshot, StateKind,
    WaitingForRiderState,
};
use crate::workout::{ParsedWorkout, WorkoutBlock};
use tauri::Emitter;
use tracing::info;

impl SessionActor {
    pub async fn run(mut self) {
        let mut ticker = tokio::time::interval(tokio::time::Duration::from_secs(TICK_S as u64));
        loop {
            tokio::select! {
                cmd = self.cmd_rx.recv() => {
                    match cmd {
                        None => {info!("session actor shutting down"); break}
                        Some(cmd) => {self.handle_command(cmd).await}
                    }
                }
                _ = ticker.tick() => {
                    self.handle_tick().await;
                }
                Some(ble) = self.ble_metrics_rx.recv() => {
                    self.last_power_w = ble.power_w;
                    self.last_cadence_rpm = ble.cadence_rpm;
                    self.last_hr_bpm = ble.hr_bpm;
                    if let Some(session) = self.session.as_mut() {
                        session.last_power_w = ble.power_w;
                        session.last_cadence_rpm = ble.cadence_rpm;
                    }
                }
            }
        }
    }

    async fn handle_command(&mut self, cmd: SessionCommand) {
        match cmd {
            SessionCommand::Start {
                workout,
                ftp_w,
                reply,
            } => {
                let active = matches!(
                    self.state.as_ref().map(|s| s.kind()),
                    Some(k) if k != StateKind::Finished
                );
                if active {
                    let _ = reply.send(Err(AppError::SessionAlreadyActive));
                    return;
                }
                let workout_name = workout.name.clone();
                let workout_author = workout.author.clone();
                let workout_description = workout.description.clone();
                let flattened_workout = flatten_workout(workout, ftp_w);
                self.session = Some(Session {
                    blocks: flattened_workout,
                    ftp_w,
                    total_elapsed_s: 0,
                    total_active_s: 0,
                    current_block_idx: 0,
                    current_block_elapsed_s: 0,
                    last_target_w: None,
                    last_cadence_rpm: None,
                    last_power_w: None,
                    workout_name,
                    workout_author,
                    workout_description,
                });
                self.state = Some(Box::new(WaitingForRiderState));
                self.last_power_w = None;
                self.last_hr_bpm = None;
                self.last_cadence_rpm = None;
                self.current_session_id = None;
                self.last_session_id = None;
                if let Some(s) = self.session.as_ref() {
                    info!(
                        workout = s.workout_name.as_deref().unwrap_or("Untitled"),
                        ftp_w = s.ftp_w,
                        blocks = s.blocks.len(),
                        "session created, waiting for rider"
                    );
                }
                let _ = reply.send(Ok(()));
                self.emit_metrics();
            }
            SessionCommand::Pause => {
                info!("pause requested");
                if let Some(state) = self.state.take() {
                    self.state = Some(state.pause())
                }
                self.emit_metrics();
            }
            SessionCommand::Resume => {
                info!("resume requested");
                if let Some(state) = self.state.take() {
                    self.state = Some(state.resume())
                }
                self.emit_metrics();
            }
            SessionCommand::Stop => {
                info!("stop requested");
                if let Some(state) = self.state.take() {
                    self.state = Some(state.stop());
                }
                self.finalize_db_session().await;
                self.emit_metrics();
            }
            SessionCommand::Skip => {
                info!("skip requested");
                if let (Some(state), Some(session)) = (self.state.take(), self.session.as_mut()) {
                    self.state = Some(state.skip(session))
                }
                self.emit_metrics();
            }
            SessionCommand::Snapshot { reply } => {
                let snapshot = self.build_snapshot();
                let _ = reply.send(snapshot);
            }
        }
    }

    async fn handle_tick(&mut self) {
        let Some(state) = self.state.take() else {
            return;
        };
        let Some(session) = self.session.as_mut() else {
            self.state = Some(state);
            return;
        };

        let prev_kind = state.kind();
        let prev_block_idx = session.current_block_idx;

        let new_state = state.tick(session);
        let kind = new_state.kind();
        let target_w = session.last_target_w;
        let block_idx = session.current_block_idx;

        self.state = Some(new_state);

        if kind != prev_kind {
            info!(from = ?prev_kind, to = ?kind, "state transition");
        }
        if kind == StateKind::Running && block_idx != prev_block_idx {
            let label = self
                .session
                .as_ref()
                .and_then(|s| s.blocks.get(block_idx))
                .map(|b| b.label.as_str())
                .unwrap_or("");
            info!(block_idx, label, target_w = ?target_w, "advanced to block");
        }

        match kind {
            StateKind::WaitingForRider => {}
            StateKind::Running => {
                if self.current_session_id.is_none() {
                    let session = self.session.as_ref().expect("session exists in Running");
                    let workout_name = session
                        .workout_name
                        .clone()
                        .unwrap_or_else(|| "Untitled workout".to_string());
                    let started_at = chrono::Utc::now().to_rfc3339();
                    let ftp_w_used = session.ftp_w;
                    let flat_blocks_json = serde_json::to_string(&session.blocks)
                        .expect("FlatBlock is plain Serialize, cannot fail");
                    match self
                        .db_handle
                        .insert_session(workout_name, started_at, ftp_w_used, flat_blocks_json)
                        .await
                    {
                        Ok(id) => {
                            info!(session_id = id, "rider started, recording session");
                            self.current_session_id = Some(id);
                        }
                        Err(e) => tracing::error!("insert_session failed: {e}"),
                    }
                }
                if let Some(session_id) = self.current_session_id {
                    let session = self.session.as_ref().expect("session exists in Running");
                    self.db_handle
                        .insert_metric(
                            session_id,
                            Metric {
                                t_offset_s: session.total_active_s,
                                power_w: self.last_power_w.map(|p| p.max(0) as u16),
                                hr_bpm: self.last_hr_bpm,
                                cadence_rpm: self.last_cadence_rpm,
                            },
                        )
                        .await;
                }
                if let Some(target) = target_w {
                    if let Err(e) = self.ble_handle.set_target_power(target as i16).await {
                        tracing::error!("set_target_power failed: {e}");
                    }
                }
            }
            StateKind::Paused => {}
            StateKind::Finished => {
                self.finalize_db_session().await;
            }
        }

        self.emit_metrics();
    }

    fn build_metrics(&self) -> Option<SessionMetrics> {
        let state = self.state.as_ref()?;
        let session = self.session.as_ref()?;
        let cadence_target_rpm = session
            .blocks
            .get(session.current_block_idx)
            .and_then(|b| b.cadence_rpm);
        Some(SessionMetrics {
            state: state.kind(),
            total_elapsed_s: session.total_elapsed_s,
            total_active_s: session.total_active_s,
            current_block_idx: session.current_block_idx,
            current_block_elapsed_s: session.current_block_elapsed_s,
            target_w: session.last_target_w,
            cadence_target_rpm,
            power_w: self.last_power_w,
            hr_bpm: self.last_hr_bpm,
            cadence_rpm: self.last_cadence_rpm,
            ftp_w: session.ftp_w,
            blocks_total: session.blocks.len() as u32,
            session_id: self.current_session_id.or(self.last_session_id),
        })
    }

    fn emit_metrics(&self) {
        if let Some(metrics) = self.build_metrics() {
            let _ = self.app_handle.emit("session_metrics", &metrics);
        }
    }

    fn build_snapshot(&self) -> Option<SessionSnapshot> {
        let session = self.session.as_ref()?;
        Some(SessionSnapshot {
            flat_blocks: session.blocks.clone(),
            ftp_w: session.ftp_w,
            workout_name: session.workout_name.clone(),
            workout_author: session.workout_author.clone(),
            workout_description: session.workout_description.clone(),
            metrics: self.build_metrics(),
        })
    }

    async fn finalize_db_session(&mut self) {
        let Some(session_id) = self.current_session_id.take() else {
            return;
        };
        let Some(session) = self.session.as_ref() else {
            return;
        };
        let ended_at = chrono::Utc::now().to_rfc3339();
        let active_s = session.total_active_s;
        self.db_handle
            .finalize_session(session_id, ended_at, active_s)
            .await;
        info!(session_id, active_s, "session finalized");
        self.last_session_id = Some(session_id);
    }
}

fn flatten_workout(parsed_workout: ParsedWorkout, ftp_w: u16) -> Vec<FlatBlock> {
    let mut out = Vec::new();
    for block in parsed_workout.workout_blocks {
        flatten_block(block, ftp_w, &mut out);
    }
    out
}

fn flatten_block(block: WorkoutBlock, ftp_w: u16, flattened: &mut Vec<FlatBlock>) {
    flatten_block_with_label(block, ftp_w, None, flattened)
}

fn flatten_block_with_label(
    block: WorkoutBlock,
    ftp_w: u16,
    override_label: Option<String>,
    flattened: &mut Vec<FlatBlock>,
) {
    match block {
        WorkoutBlock::SteadyState {
            duration_s,
            power_pct,
            cadence_rpm,
            label,
        } => {
            let w = (power_pct * ftp_w as f32).round() as u16;
            let resolved = override_label
                .or(label)
                .unwrap_or_else(|| fallback_label(w, w, ftp_w));
            flattened.push(FlatBlock {
                duration_s,
                power_start_w: w,
                power_end_w: w,
                cadence_rpm,
                label: resolved,
            })
        }
        WorkoutBlock::Ramp {
            duration_s,
            power_start_pct,
            power_end_pct,
            cadence_rpm,
            label,
        } => {
            let start_w = (power_start_pct * ftp_w as f32).round() as u16;
            let end_w = (power_end_pct * ftp_w as f32).round() as u16;
            let resolved = override_label
                .or(label)
                .unwrap_or_else(|| fallback_label(start_w, end_w, ftp_w));
            flattened.push(FlatBlock {
                duration_s,
                power_start_w: start_w,
                power_end_w: end_w,
                cadence_rpm,
                label: resolved,
            });
        }
        WorkoutBlock::IntervalsT { repeat, on, off } => {
            for i in 0..repeat {
                let on_label = format!("Interval {}/{} ON", i + 1, repeat);
                let off_label = format!("Interval {}/{} OFF", i + 1, repeat);
                flatten_block_with_label(*on.clone(), ftp_w, Some(on_label), flattened);
                flatten_block_with_label(*off.clone(), ftp_w, Some(off_label), flattened);
            }
        }
    }
}

const ZONE_THRESHOLDS: [f32; 5] = [0.55, 0.75, 0.90, 1.05, 1.20];

fn zone_of(pct: f32) -> u8 {
    for (i, t) in ZONE_THRESHOLDS.iter().enumerate() {
        if pct < *t {
            return (i as u8) + 1;
        }
    }
    6
}

fn zone_name(z: u8) -> &'static str {
    match z {
        1 => "Recovery",
        2 => "Endurance",
        3 => "Tempo",
        4 => "Threshold",
        5 => "VO2max",
        _ => "Anaerobic",
    }
}

fn fallback_label(start_w: u16, end_w: u16, ftp_w: u16) -> String {
    if ftp_w == 0 {
        return "Block".to_string();
    }
    let ftp = ftp_w as f32;
    let zs = zone_of(start_w as f32 / ftp);
    let ze = zone_of(end_w as f32 / ftp);
    if zs != ze {
        format!("Ramp {}→{}", zone_name(zs), zone_name(ze))
    } else {
        format!("Steady {}", zone_name(zs))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workout::SportType;

    #[test]
    fn flatten_workout_expands_intervals_t_and_synthesizes_labels() {
        let workout = ParsedWorkout {
            author: None,
            name: Some("Test".to_string()),
            description: None,
            sport_type: SportType::Bike,
            workout_blocks: vec![
                WorkoutBlock::SteadyState {
                    duration_s: 300,
                    power_pct: 0.5,
                    cadence_rpm: None,
                    label: Some("Warmup".to_string()),
                },
                WorkoutBlock::IntervalsT {
                    repeat: 3,
                    on: Box::new(WorkoutBlock::SteadyState {
                        duration_s: 60,
                        power_pct: 1.2,
                        cadence_rpm: Some(95),
                        label: None,
                    }),
                    off: Box::new(WorkoutBlock::SteadyState {
                        duration_s: 60,
                        power_pct: 0.6,
                        cadence_rpm: None,
                        label: None,
                    }),
                },
                WorkoutBlock::SteadyState {
                    duration_s: 120,
                    power_pct: 0.4,
                    cadence_rpm: None,
                    label: Some("Cooldown".to_string()),
                },
            ],
        };

        let flat = flatten_workout(workout, 200);

        // 1 warmup + 3 * (on + off) + 1 cooldown = 8
        assert_eq!(flat.len(), 8);
        assert_eq!(flat[0].label, "Warmup");
        assert_eq!(flat[0].power_start_w, 100);
        assert_eq!(flat[1].label, "Interval 1/3 ON");
        assert_eq!(flat[1].power_start_w, 240);
        assert_eq!(flat[1].cadence_rpm, Some(95));
        assert_eq!(flat[2].label, "Interval 1/3 OFF");
        assert_eq!(flat[5].label, "Interval 3/3 ON");
        assert_eq!(flat[6].label, "Interval 3/3 OFF");
        assert_eq!(flat[7].label, "Cooldown");
    }

    #[test]
    fn flatten_workout_ramp_interpolates_power_bounds() {
        let workout = ParsedWorkout {
            author: None,
            name: None,
            description: None,
            sport_type: SportType::Bike,
            workout_blocks: vec![WorkoutBlock::Ramp {
                duration_s: 600,
                power_start_pct: 0.5,
                power_end_pct: 1.0,
                cadence_rpm: None,
                label: Some("Build".to_string()),
            }],
        };

        let flat = flatten_workout(workout, 200);

        assert_eq!(flat.len(), 1);
        assert_eq!(flat[0].power_start_w, 100);
        assert_eq!(flat[0].power_end_w, 200);
        assert_eq!(flat[0].label, "Build");
    }
}
