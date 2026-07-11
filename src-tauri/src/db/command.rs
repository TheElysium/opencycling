use crate::db::actor::DbActor;
use crate::db::types::{Metric, SessionCard, SessionDetail, StravaAuth};
use crate::db::Settings;
use crate::errors::AppError;
use tokio::sync::mpsc::{channel, Sender};
use tokio::sync::oneshot;

pub enum DbCommand {
    QuerySettings {
        reply: tokio::sync::oneshot::Sender<Result<Settings, AppError>>,
    },
    UpdateSettings {
        reply: oneshot::Sender<Result<(), AppError>>,
        settings: Settings,
    },
    InsertSession {
        workout_name: String,
        started_at: String,
        ftp_w_used: u16,
        flat_blocks_json: String,
        reply: oneshot::Sender<Result<i64, AppError>>,
    },
    InsertMetric {
        session_id: i64,
        metric: Metric,
    },
    FinalizeSession {
        session_id: i64,
        ended_at: String,
        duration_s: u32,
    },
    ListSessions {
        reply: oneshot::Sender<Result<Vec<SessionCard>, AppError>>,
    },
    GetSession {
        id: i64,
        reply: oneshot::Sender<Result<SessionDetail, AppError>>,
    },
    DeleteSession {
        id: i64,
        reply: oneshot::Sender<Result<(), AppError>>,
    },
    UpsertStravaAuth {
        auth: StravaAuth,
        reply: oneshot::Sender<Result<(), AppError>>,
    },
    GetStravaAuth {
        reply: oneshot::Sender<Result<Option<StravaAuth>, AppError>>,
    },
    DeleteStravaAuth {
        reply: oneshot::Sender<Result<(), AppError>>,
    },
    GetStravaAutoUpload {
        reply: oneshot::Sender<Result<bool, AppError>>,
    },
    SetStravaAutoUpload {
        enabled: bool,
        reply: oneshot::Sender<Result<(), AppError>>,
    },
    SetSessionStravaActivity {
        session_id: i64,
        activity_id: i64,
        reply: oneshot::Sender<Result<(), AppError>>,
    },
}

#[derive(Clone)]
pub struct DbActorHandle {
    sender: Sender<DbCommand>,
}

impl DbActorHandle {
    pub async fn spawn(db_path: String) -> Result<DbActorHandle, AppError> {
        // Sized for ~1 Hz metric writes + occasional UI reads; backpressure via
        // send().await is preferred over silent try_send drops.
        let (cmd_tx, cmd_rx) = channel::<DbCommand>(256);
        let mut db_actor = DbActor::new(cmd_rx, db_path)?;
        // Finalize sessions that were interrupted by a crash, power loss, or
        // forced close on a previous run. This runs before the actor loop starts
        // and before SessionActor can be created, so no live session can exist
        // during this pass -- the invariant is structural, not just by convention.
        if let Err(e) = db_actor.finalize_orphaned_sessions() {
            tracing::error!("finalize_orphaned_sessions failed: {e}");
        }
        tokio::spawn(db_actor.run());
        Ok(DbActorHandle { sender: cmd_tx })
    }

    pub async fn get_settings(&self) -> Result<Settings, AppError> {
        let (tx, rx) = oneshot::channel::<Result<Settings, AppError>>();
        self.sender
            .send(DbCommand::QuerySettings { reply: tx })
            .await
            .map_err(|_| AppError::ChannelClosed)?;
        rx.await.map_err(|_| AppError::ChannelClosed)?
    }

    pub async fn update_settings(&self, settings: Settings) -> Result<(), AppError> {
        let (tx, rx) = oneshot::channel::<Result<(), AppError>>();
        self.sender
            .send(DbCommand::UpdateSettings {
                reply: tx,
                settings,
            })
            .await
            .map_err(|_| AppError::ChannelClosed)?;
        rx.await.map_err(|_| AppError::ChannelClosed)?
    }

    pub async fn insert_session(
        &self,
        workout_name: String,
        started_at: String,
        ftp_w_used: u16,
        flat_blocks_json: String,
    ) -> Result<i64, AppError> {
        let (tx, rx) = oneshot::channel();
        self.sender
            .send(DbCommand::InsertSession {
                workout_name,
                started_at,
                ftp_w_used,
                flat_blocks_json,
                reply: tx,
            })
            .await
            .map_err(|_| AppError::ChannelClosed)?;
        rx.await.map_err(|_| AppError::ChannelClosed)?
    }

    pub async fn insert_metric(&self, session_id: i64, metric: Metric) {
        if let Err(e) = self
            .sender
            .send(DbCommand::InsertMetric { session_id, metric })
            .await
        {
            tracing::error!("insert_metric send failed: {e}");
        }
    }

    pub async fn finalize_session(&self, session_id: i64, ended_at: String, duration_s: u32) {
        if let Err(e) = self
            .sender
            .send(DbCommand::FinalizeSession {
                session_id,
                ended_at,
                duration_s,
            })
            .await
        {
            tracing::error!("finalize_session send failed: {e}");
        }
    }

    pub async fn list_sessions(&self) -> Result<Vec<SessionCard>, AppError> {
        let (tx, rx) = oneshot::channel();
        self.sender
            .send(DbCommand::ListSessions { reply: tx })
            .await
            .map_err(|_| AppError::ChannelClosed)?;
        rx.await.map_err(|_| AppError::ChannelClosed)?
    }

    pub async fn get_session(&self, id: i64) -> Result<SessionDetail, AppError> {
        let (tx, rx) = oneshot::channel();
        self.sender
            .send(DbCommand::GetSession { id, reply: tx })
            .await
            .map_err(|_| AppError::ChannelClosed)?;
        rx.await.map_err(|_| AppError::ChannelClosed)?
    }

    pub async fn delete_session(&self, id: i64) -> Result<(), AppError> {
        let (tx, rx) = oneshot::channel();
        self.sender
            .send(DbCommand::DeleteSession { id, reply: tx })
            .await
            .map_err(|_| AppError::ChannelClosed)?;
        rx.await.map_err(|_| AppError::ChannelClosed)?
    }

    pub async fn upsert_strava_auth(&self, auth: StravaAuth) -> Result<(), AppError> {
        let (tx, rx) = oneshot::channel();
        self.sender
            .send(DbCommand::UpsertStravaAuth { auth, reply: tx })
            .await
            .map_err(|_| AppError::ChannelClosed)?;
        rx.await.map_err(|_| AppError::ChannelClosed)?
    }

    pub async fn get_strava_auth(&self) -> Result<Option<StravaAuth>, AppError> {
        let (tx, rx) = oneshot::channel();
        self.sender
            .send(DbCommand::GetStravaAuth { reply: tx })
            .await
            .map_err(|_| AppError::ChannelClosed)?;
        rx.await.map_err(|_| AppError::ChannelClosed)?
    }

    pub async fn delete_strava_auth(&self) -> Result<(), AppError> {
        let (tx, rx) = oneshot::channel();
        self.sender
            .send(DbCommand::DeleteStravaAuth { reply: tx })
            .await
            .map_err(|_| AppError::ChannelClosed)?;
        rx.await.map_err(|_| AppError::ChannelClosed)?
    }

    pub async fn get_strava_auto_upload(&self) -> Result<bool, AppError> {
        let (tx, rx) = oneshot::channel();
        self.sender
            .send(DbCommand::GetStravaAutoUpload { reply: tx })
            .await
            .map_err(|_| AppError::ChannelClosed)?;
        rx.await.map_err(|_| AppError::ChannelClosed)?
    }

    pub async fn set_strava_auto_upload(&self, enabled: bool) -> Result<(), AppError> {
        let (tx, rx) = oneshot::channel();
        self.sender
            .send(DbCommand::SetStravaAutoUpload { enabled, reply: tx })
            .await
            .map_err(|_| AppError::ChannelClosed)?;
        rx.await.map_err(|_| AppError::ChannelClosed)?
    }

    pub async fn set_session_strava_activity(
        &self,
        session_id: i64,
        activity_id: i64,
    ) -> Result<(), AppError> {
        let (tx, rx) = oneshot::channel();
        self.sender
            .send(DbCommand::SetSessionStravaActivity {
                session_id,
                activity_id,
                reply: tx,
            })
            .await
            .map_err(|_| AppError::ChannelClosed)?;
        rx.await.map_err(|_| AppError::ChannelClosed)?
    }
}
