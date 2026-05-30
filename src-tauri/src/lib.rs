use crate::ble::{BleActorHandle, DeviceInfo};
use crate::db::{DbActorHandle, Settings};
use crate::errors::AppError;
use crate::workout::{parse_zwo, ParsedWorkout};
use tauri::Manager;
use tracing::metadata::LevelFilter;
use tracing_subscriber::EnvFilter;

mod ble;
mod db;
mod errors;
mod workout;

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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            load_workout,
            scan_devices,
            connect_trainer,
            connect_hrm,
            set_target_power,
            get_settings,
            update_settings
        ])
        .setup(|app| {
            let ble_handle =
                tauri::async_runtime::block_on(BleActorHandle::spawn(app.handle().clone()))
                    .expect("BLE init failed");
            app.manage(ble_handle);
            let app_data_dir = app.path().app_data_dir().expect("no app data dir");
            std::fs::create_dir_all(&app_data_dir).expect("failed to create app data dir");
            let db_path = app_data_dir.join(DB_FILE).to_string_lossy().to_string();
            let db_handle = tauri::async_runtime::block_on(DbActorHandle::spawn(db_path))
                .expect("DB init failed");
            app.manage(db_handle);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
