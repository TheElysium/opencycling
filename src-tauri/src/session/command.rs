use crate::ble::{BleActorHandle, BleEvent, BleMetrics};
use crate::db::DbActorHandle;
use crate::errors::AppError;
use crate::session::types::{SessionActor, SessionCommand, SessionSnapshot};
use crate::workout::ParsedWorkout;
use tauri::AppHandle;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::{mpsc, oneshot};

#[derive(Clone)]
pub struct SessionActorHandle {
    sender: Sender<SessionCommand>,
}

impl SessionActorHandle {
    pub async fn spawn(
        app_handle: AppHandle,
        ble_handle: BleActorHandle,
        ble_metrics_rx: Receiver<BleMetrics>,
        ble_event_rx: Receiver<BleEvent>,
        db_handle: DbActorHandle,
    ) -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel::<SessionCommand>(16);
        let actor = SessionActor {
            app_handle,
            cmd_rx,
            ble_metrics_rx,
            ble_event_rx,
            ble_handle,
            session: None,
            state: None,
            last_power_w: None,
            last_hr_bpm: None,
            last_cadence_rpm: None,
            db_handle,
            current_session_id: None,
            last_session_id: None,
            last_aero: None,
        };
        tokio::spawn(actor.run());
        Self { sender: cmd_tx }
    }

    pub async fn report_aero(&self, aero: Option<bool>) -> Result<(), AppError> {
        self.sender
            .send(SessionCommand::ReportAero { aero })
            .await
            .map_err(|_| AppError::ChannelClosed)
    }

    pub async fn start(&self, workout: ParsedWorkout, ftp_w: u16) -> Result<(), AppError> {
        let (tx, rx) = oneshot::channel::<Result<(), AppError>>();
        self.sender
            .send(SessionCommand::Start {
                workout,
                ftp_w,
                reply: tx,
            })
            .await
            .map_err(|_| AppError::ChannelClosed)?;
        rx.await.map_err(|_| AppError::ChannelClosed)?
    }
    pub async fn pause(&self) -> Result<(), AppError> {
        self.sender
            .send(SessionCommand::Pause)
            .await
            .map_err(|_| AppError::ChannelClosed)
    }
    pub async fn resume(&self) -> Result<(), AppError> {
        self.sender
            .send(SessionCommand::Resume)
            .await
            .map_err(|_| AppError::ChannelClosed)
    }
    pub async fn stop(&self) -> Result<(), AppError> {
        self.sender
            .send(SessionCommand::Stop)
            .await
            .map_err(|_| AppError::ChannelClosed)
    }
    pub async fn skip(&self) -> Result<(), AppError> {
        self.sender
            .send(SessionCommand::Skip)
            .await
            .map_err(|_| AppError::ChannelClosed)
    }
    pub async fn snapshot(&self) -> Result<Option<SessionSnapshot>, AppError> {
        let (tx, rx) = oneshot::channel();
        self.sender
            .send(SessionCommand::Snapshot { reply: tx })
            .await
            .map_err(|_| AppError::ChannelClosed)?;
        rx.await.map_err(|_| AppError::ChannelClosed)
    }
}
