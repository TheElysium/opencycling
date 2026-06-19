use crate::ble::{BleActorHandle, BleMetrics, DeviceInfo};
use crate::db::{DbActorHandle, SessionCard, SessionDetail, Settings, StravaAuth};
use crate::errors::AppError;
use crate::session::{SessionActorHandle, SessionSnapshot};
use crate::strava::types::StravaStatus;
use crate::workout::{list_workouts, parse_zwo, ParsedWorkout};
use tauri::Manager;
use tauri_plugin_opener::OpenerExt;
use tracing::metadata::LevelFilter;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{fmt, EnvFilter};

mod ble;
pub mod db;
pub mod errors;
mod export;
mod metrics;
mod session;
mod strava;
pub mod workout;

const DB_FILE: &str = "opencycling.db";

#[tauri::command]
fn load_workout(path: String) -> Result<ParsedWorkout, AppError> {
    let content = std::fs::read_to_string(path).map_err(|e| AppError::Io(e.to_string()))?;
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
async fn report_aero(
    state: tauri::State<'_, SessionActorHandle>,
    aero: Option<bool>,
) -> Result<(), AppError> {
    state.report_aero(aero).await
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

#[tauri::command]
async fn export_session_tcx(
    state: tauri::State<'_, DbActorHandle>,
    id: i64,
    path: String,
) -> Result<(), AppError> {
    let detail = state.get_session(id).await?;
    let tcx = export::tcx::build_tcx(&detail);
    std::fs::write(&path, tcx).map_err(|e| AppError::Io(e.to_string()))?;
    Ok(())
}

#[tauri::command]
async fn strava_status(state: tauri::State<'_, DbActorHandle>) -> Result<StravaStatus, AppError> {
    let auth = state.get_strava_auth().await?;
    let auto_upload = state.get_strava_auto_upload().await?;
    Ok(StravaStatus {
        connected: auth.is_some(),
        athlete_id: auth.as_ref().and_then(|a| a.athlete_id),
        athlete_name: auth.and_then(|a| a.athlete_name),
        auto_upload,
    })
}

#[tauri::command]
async fn strava_connect(
    app: tauri::AppHandle,
    state: tauri::State<'_, DbActorHandle>,
) -> Result<StravaStatus, AppError> {
    let proxy_url = state.get_settings().await?.strava_proxy_url;
    // CSRF nonce: passed to Strava and verified when the callback returns.
    let csrf_state = uuid::Uuid::new_v4().to_string();
    // Bind the loopback listener before opening the browser so the callback
    // can never race ahead of us listening.
    let listener = strava::oauth::bind_loopback()?;
    let authorize = strava::oauth::authorize_url(&proxy_url, &csrf_state).await?;
    app.opener()
        .open_url(authorize, None::<&str>)
        .map_err(|e| AppError::StravaAuth(e.to_string()))?;
    let code = strava::oauth::wait_for_code(listener, csrf_state).await?;
    let tokens = strava::oauth::exchange_code(&proxy_url, &code).await?;
    state
        .upsert_strava_auth(StravaAuth {
            access_token: tokens.access_token,
            refresh_token: tokens.refresh_token,
            expires_at: tokens.expires_at,
            athlete_id: tokens.athlete_id,
            athlete_name: tokens.athlete_name.clone(),
            connected_at: chrono::Utc::now().to_rfc3339(),
        })
        .await?;
    let auto_upload = state.get_strava_auto_upload().await?;
    Ok(StravaStatus {
        connected: true,
        athlete_id: tokens.athlete_id,
        athlete_name: tokens.athlete_name,
        auto_upload,
    })
}

#[tauri::command]
async fn strava_disconnect(state: tauri::State<'_, DbActorHandle>) -> Result<(), AppError> {
    state.delete_strava_auth().await
}

#[tauri::command]
async fn strava_set_auto_upload(
    state: tauri::State<'_, DbActorHandle>,
    enabled: bool,
) -> Result<(), AppError> {
    state.set_strava_auto_upload(enabled).await
}

#[tauri::command]
async fn upload_session_to_strava(
    state: tauri::State<'_, DbActorHandle>,
    session_id: i64,
    force: bool,
) -> Result<i64, AppError> {
    let detail = state.get_session(session_id).await?;
    if !force {
        if let Some(existing) = detail.strava_activity_id {
            return Ok(existing); // dedup guard: already uploaded
        }
    }
    let token = strava::ensure_fresh_token(&state).await?;
    let tcx = export::tcx::build_tcx(&detail);
    let name = format!("OpenCycling - {}", detail.workout_name);
    let description = export::tcx::workout_description(&detail);
    let activity_id = strava::api::upload_tcx(&token, tcx, &name, &description).await?;
    state
        .set_session_strava_activity(session_id, activity_id)
        .await?;
    Ok(activity_id)
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
            report_aero,
            get_session_snapshot,
            list_sessions,
            get_session,
            delete_session,
            export_session_tcx,
            strava_status,
            strava_connect,
            strava_disconnect,
            strava_set_auto_upload,
            upload_session_to_strava
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
        .plugin(tauri_plugin_process::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
