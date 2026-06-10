use crate::ble::{BleActorHandle, BleMetrics, DeviceInfo};
use crate::db::{DbActorHandle, SessionCard, SessionDetail, Settings};
use crate::errors::AppError;
use crate::session::{SessionActorHandle, SessionSnapshot};
use crate::workout::{list_workouts, parse_zwo, ParsedWorkout};
use tauri::Manager;
use tracing::metadata::LevelFilter;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{fmt, EnvFilter};

mod ble;
pub mod db;
pub mod errors;
mod metrics;
mod session;
pub mod workout;

const DB_FILE: &str = "opencycling.db";

#[tauri::command]
fn load_workout(path: String) -> Result<ParsedWorkout, AppError> {
    let content = std::fs::read_to_string(path).map_err(|e| AppError::Other(e.to_string()))?;
    parse_zwo(&content)
}

#[tauri::command]
async fn scan_devices(
    state: tauri::State<'_, BleActorHandle>,
) -> Result<Vec<DeviceInfo>, AppError> {
    state.scan().await
}

#[tauri::command]
async fn connect_trainer(
    state: tauri::State<'_, BleActorHandle>,
    device_id: String,
) -> Result<(), AppError> {
    state.connect_trainer(device_id).await
}

#[tauri::command]
async fn connect_hrm(
    state: tauri::State<'_, BleActorHandle>,
    device_id: String,
) -> Result<(), AppError> {
    state.connect_hrm(device_id).await
}

#[tauri::command]
async fn set_target_power(
    state: tauri::State<'_, BleActorHandle>,
    watts: i16,
) -> Result<(), AppError> {
    state.set_target_power(watts).await
}

#[tauri::command]
async fn get_settings(state: tauri::State<'_, DbActorHandle>) -> Result<Settings, AppError> {
    state.get_settings().await
}

#[tauri::command]
async fn update_settings(
    state: tauri::State<'_, DbActorHandle>,
    settings: Settings,
) -> Result<(), AppError> {
    state.update_settings(settings).await
}

#[tauri::command]
fn list_workouts_cmd(folder: String) -> Result<Vec<ParsedWorkout>, AppError> {
    list_workouts(&folder)
}

#[tauri::command]
async fn start_session(
    state: tauri::State<'_, SessionActorHandle>,
    workout: ParsedWorkout,
    ftp_w: u16,
) -> Result<(), AppError> {
    state.start(workout, ftp_w).await
}

#[tauri::command]
async fn pause_session(state: tauri::State<'_, SessionActorHandle>) -> Result<(), AppError> {
    state.pause().await
}

#[tauri::command]
async fn resume_session(state: tauri::State<'_, SessionActorHandle>) -> Result<(), AppError> {
    state.resume().await
}

#[tauri::command]
async fn stop_session(state: tauri::State<'_, SessionActorHandle>) -> Result<(), AppError> {
    state.stop().await
}

#[tauri::command]
async fn skip_block(state: tauri::State<'_, SessionActorHandle>) -> Result<(), AppError> {
    state.skip().await
}

#[tauri::command]
async fn get_session_snapshot(
    state: tauri::State<'_, SessionActorHandle>,
) -> Result<Option<SessionSnapshot>, AppError> {
    state.snapshot().await
}

#[tauri::command]
async fn list_sessions(
    state: tauri::State<'_, DbActorHandle>,
) -> Result<Vec<SessionCard>, AppError> {
    state.list_sessions().await
}

#[tauri::command]
async fn get_session(
    state: tauri::State<'_, DbActorHandle>,
    id: i64,
) -> Result<SessionDetail, AppError> {
    state.get_session(id).await
}

#[tauri::command]
async fn delete_session(state: tauri::State<'_, DbActorHandle>, id: i64) -> Result<(), AppError> {
    state.delete_session(id).await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            load_workout,
            list_workouts_cmd,
            scan_devices,
            connect_trainer,
            connect_hrm,
            set_target_power,
            get_settings,
            update_settings,
            start_session,
            pause_session,
            resume_session,
            stop_session,
            skip_block,
            get_session_snapshot,
            list_sessions,
            get_session,
            delete_session
        ])
        .setup(|app| {
            let log_dir = app.path().app_log_dir().expect("no app log dir");
            std::fs::create_dir_all(&log_dir).expect("failed to create log dir");
            let stamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S");
            let file_appender =
                tracing_appender::rolling::never(&log_dir, format!("opencycling_{stamp}.log"));
            tracing_subscriber::registry()
                .with(
                    EnvFilter::builder()
                        .with_default_directive(LevelFilter::INFO.into())
                        .from_env_lossy(),
                )
                .with(fmt::layer())
                .with(fmt::layer().with_ansi(false).with_writer(file_appender))
                .init();
            tracing::info!("logging to {}", log_dir.display());

            let (ble_metrics_tx, ble_metrics_rx) = tokio::sync::mpsc::channel::<BleMetrics>(8);
            let ble_handle = tauri::async_runtime::block_on(BleActorHandle::spawn(
                app.handle().clone(),
                ble_metrics_tx,
            ))
            .expect("BLE init failed");
            let app_data_dir = app.path().app_data_dir().expect("no app data dir");
            std::fs::create_dir_all(&app_data_dir).expect("failed to create app data dir");
            let db_path = app_data_dir.join(DB_FILE).to_string_lossy().to_string();
            let db_handle = tauri::async_runtime::block_on(DbActorHandle::spawn(db_path))
                .expect("DB init failed");
            let session_handle = tauri::async_runtime::block_on(SessionActorHandle::spawn(
                app.handle().clone(),
                ble_handle.clone(),
                ble_metrics_rx,
                db_handle.clone(),
            ));
            app.manage(ble_handle);
            app.manage(session_handle);
            app.manage(db_handle);
            Ok(())
        })
        .plugin(tauri_plugin_updater::Builder::new().build())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
