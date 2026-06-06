use crate::ble::{BleActorHandle, BleMetrics};
use crate::db::DbActorHandle;
use crate::errors::AppError;
use crate::workout::ParsedWorkout;
use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tokio::sync::{mpsc, oneshot};

#[derive(Debug)]
pub struct Session {
    pub blocks: Vec<FlatBlock>,
    pub ftp_w: u16,
    pub total_elapsed_s: u32,
    pub total_active_s: u32,
    pub current_block_idx: usize,
    pub current_block_elapsed_s: u32,
    pub last_target_w: Option<u16>,
    pub last_cadence_rpm: Option<u16>,
    pub last_power_w: Option<i16>,
    pub workout_name: Option<String>,
    pub workout_author: Option<String>,
    pub workout_description: Option<String>,
}

impl Session {
    pub fn is_finished(&self) -> bool {
        self.current_block_idx >= self.blocks.len()
    }

    pub fn current_block(&self) -> Option<&FlatBlock> {
        self.blocks.get(self.current_block_idx)
    }

    pub fn compute_target_w(&self) -> Option<u16> {
        let block = self.current_block()?;
        if block.duration_s == 0 {
            return self.last_target_w;
        }
        let start = block.power_start_w as i32;
        let end = block.power_end_w as i32;
        let t = self.current_block_elapsed_s as i32;
        let duration = block.duration_s as i32;
        let target = start + (end - start) * t / duration;
        Some(target.max(0) as u16)
    }

    pub fn advance_block(&mut self) {
        self.current_block_idx += 1;
        self.current_block_elapsed_s = 0
    }

    pub fn skip_block(&mut self) {
        let Some(block) = self.current_block() else {
            return;
        };
        self.total_elapsed_s += block.duration_s - self.current_block_elapsed_s;
        self.advance_block()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlatBlock {
    pub duration_s: u32,
    pub power_start_w: u16,
    pub power_end_w: u16,
    pub cadence_rpm: Option<u16>,
    pub label: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum StateKind {
    WaitingForRider,
    Running,
    Paused,
    Finished,
}

pub trait State: Send + 'static {
    fn kind(&self) -> StateKind;
    fn tick(self: Box<Self>, session: &mut Session) -> Box<dyn State>;
    fn pause(self: Box<Self>) -> Box<dyn State>;
    fn resume(self: Box<Self>) -> Box<dyn State>;
    fn stop(self: Box<Self>) -> Box<dyn State>;
    fn skip(self: Box<Self>, session: &mut Session) -> Box<dyn State>;
}

pub struct WaitingForRiderState;
pub struct RunningState;
pub struct PausedState;
pub struct FinishedState;

pub enum SessionCommand {
    Start {
        workout: ParsedWorkout,
        ftp_w: u16,
        reply: oneshot::Sender<Result<(), AppError>>,
    },
    Pause,
    Resume,
    Stop,
    Skip,
    Snapshot {
        reply: oneshot::Sender<Option<SessionSnapshot>>,
    },
}

#[derive(Serialize)]
pub struct SessionMetrics {
    pub state: StateKind,
    pub total_elapsed_s: u32,
    pub total_active_s: u32,
    pub current_block_idx: usize,
    pub current_block_elapsed_s: u32,
    pub target_w: Option<u16>,
    pub cadence_target_rpm: Option<u16>,
    pub power_w: Option<i16>,
    pub hr_bpm: Option<u16>,
    pub cadence_rpm: Option<u16>,
    pub ftp_w: u16,
    pub blocks_total: u32,
    pub session_id: Option<i64>,
}

#[derive(Serialize)]
pub struct SessionSnapshot {
    pub flat_blocks: Vec<FlatBlock>,
    pub ftp_w: u16,
    pub workout_name: Option<String>,
    pub workout_author: Option<String>,
    pub workout_description: Option<String>,
    pub metrics: Option<SessionMetrics>,
}

pub struct SessionActor {
    pub app_handle: AppHandle,
    pub cmd_rx: mpsc::Receiver<SessionCommand>,
    pub ble_metrics_rx: mpsc::Receiver<BleMetrics>,
    pub ble_handle: BleActorHandle,
    pub session: Option<Session>,
    pub state: Option<Box<dyn State>>,
    pub last_power_w: Option<i16>,
    pub last_hr_bpm: Option<u16>,
    pub last_cadence_rpm: Option<u16>,
    pub db_handle: DbActorHandle,
    pub current_session_id: Option<i64>,
    pub last_session_id: Option<i64>,
}
