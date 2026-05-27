use tauri::Manager;
use crate::ble::{BleActorHandle, DeviceInfo};
use crate::errors::AppError;
use crate::workout::{parse_zwo, ParsedWorkout};

mod errors;
mod ble;
mod workout;

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
fn load_workout(path: String) -> Result<ParsedWorkout, AppError> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| AppError::Other(e.to_string()))?;
    parse_zwo(&content)
}

#[tauri::command]
async fn scan_devices(state: tauri::State<'_, BleActorHandle>) -> Result<Vec<DeviceInfo>, AppError> {
    state.scan().await
}

#[tauri::command]
async fn connect_trainer(state: tauri::State<'_, BleActorHandle>, device_id: String) -> Result<(), AppError> {
    state.connect_trainer(device_id).await
}

#[tauri::command]
async fn connect_hrm(state: tauri::State<'_, BleActorHandle>, device_id: String) -> Result<(), AppError> {
    state.connect_hrm(device_id).await
}

#[tauri::command]
async fn set_target_power(state: tauri::State<'_, BleActorHandle>, watts: i16) -> Result<(), AppError> {
    state.set_target_power(watts).await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![greet, load_workout, scan_devices])
        .setup(|app| {
            let handle = tauri::async_runtime::block_on(BleActorHandle::spawn(app.handle().clone()))
                .expect("BLE init failed");
            app.manage(handle);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
