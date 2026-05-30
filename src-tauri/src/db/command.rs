use crate::db::actor::DbActor;
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
}

pub struct DbActorHandle {
    sender: Sender<DbCommand>,
}

impl DbActorHandle {
    pub async fn spawn(db_path: String) -> Result<DbActorHandle, AppError> {
        let (cmd_tx, cmd_rx) = channel::<DbCommand>(32);
        let db_actor = DbActor::new(cmd_rx, db_path)?;
        tokio::spawn(db_actor.run());
        Ok(DbActorHandle { sender: cmd_tx })
    }

    pub async fn get_settings(&self) -> Result<Settings, AppError> {
        let (tx, rx) = oneshot::channel::<Result<Settings, AppError>>();
        self.sender
            .send(DbCommand::QuerySettings { reply: tx })
            .await
            .map_err(|e| AppError::DbError(e.to_string()))?;
        rx.await.map_err(|e| AppError::DbError(e.to_string()))?
    }

    pub async fn update_settings(&self, settings: Settings) -> Result<(), AppError> {
        let (tx, rx) = oneshot::channel::<Result<(), AppError>>();
        self.sender
            .send(DbCommand::UpdateSettings {
                reply: tx,
                settings,
            })
            .await
            .map_err(|e| AppError::DbError(e.to_string()))?;
        rx.await.map_err(|e| AppError::DbError(e.to_string()))?
    }
}
