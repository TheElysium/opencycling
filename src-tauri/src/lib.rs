use crate::ble::{BleActorHandle, BleEvent, BleMetrics, DeviceInfo, DeviceKind};
use crate::db::{DbActorHandle, SessionCard, SessionDetail, Settings, StravaAuth};
use crate::errors::AppError;
use crate::session::{
    flatten_workout, FlatBlock, SessionActorHandle, SessionSnapshot, StateKind,
};
use crate::strava::types::StravaStatus;
use crate::workout::{list_workouts, parse_zwo, ParsedWorkout, WorkoutLibrary};
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
#[specta::specta]
fn load_workout(path: String) -> Result<ParsedWorkout, AppError> {
    let content = std::fs::read_to_string(path)?;
    parse_zwo(&content)
}

#[tauri::command]
#[specta::specta]
async fn scan_devices(
    state: tauri::State<'_, BleActorHandle>,
) -> Result<Vec<DeviceInfo>, AppError> {
    state.scan().await
}

#[tauri::command]
#[specta::specta]
async fn connect_trainer(
    state: tauri::State<'_, BleActorHandle>,
    device_id: String,
) -> Result<(), AppError> {
    state.connect_trainer(device_id).await
}

#[tauri::command]
#[specta::specta]
async fn connect_hrm(
    state: tauri::State<'_, BleActorHandle>,
    device_id: String,
) -> Result<(), AppError> {
    state.connect_hrm(device_id).await
}

// A session that is not yet finalized (WaitingForRider / Running / Paused) still holds
// the trainer under ERG control, so disconnecting it mid-session would leave the session
// driving a phantom trainer. `Finished` is inert, so it does not block. `None` metrics
// means no session at all.
async fn trainer_session_active(session: &tauri::State<'_, SessionActorHandle>) -> bool {
    match session.snapshot().await {
        Ok(Some(snapshot)) => match snapshot.metrics {
            Some(metrics) => metrics.state != StateKind::Finished,
            None => false,
        },
        // No snapshot (or the query failed): treat as not active so a stuck query never
        // wedges the disconnect affordance.
        _ => false,
    }
}

#[tauri::command]
#[specta::specta]
async fn disconnect_trainer(
    ble: tauri::State<'_, BleActorHandle>,
    session: tauri::State<'_, SessionActorHandle>,
) -> Result<(), AppError> {
    // Blocked during an active session: the trainer is under ERG control, so the rider
    // must stop the session first. Decided in the command layer to keep BleActor and
    // SessionActor decoupled.
    if trainer_session_active(&session).await {
        return Err(AppError::Other(
            "Stop the current session before disconnecting the trainer".to_string(),
        ));
    }
    ble.disconnect(DeviceKind::Trainer).await
}

#[tauri::command]
#[specta::specta]
async fn disconnect_hrm(ble: tauri::State<'_, BleActorHandle>) -> Result<(), AppError> {
    // The HRM never drives session state, so disconnecting it is always allowed.
    ble.disconnect(DeviceKind::Hrm).await
}

#[tauri::command]
#[specta::specta]
async fn retry_reconnect(
    state: tauri::State<'_, BleActorHandle>,
    kind: DeviceKind,
) -> Result<(), AppError> {
    state.retry_reconnect(kind).await
}

#[tauri::command]
#[specta::specta]
async fn set_target_power(
    state: tauri::State<'_, BleActorHandle>,
    watts: i16,
) -> Result<(), AppError> {
    state.set_target_power(watts).await
}

#[tauri::command]
#[specta::specta]
async fn get_settings(state: tauri::State<'_, DbActorHandle>) -> Result<Settings, AppError> {
    state.get_settings().await
}

#[tauri::command]
#[specta::specta]
async fn update_settings(
    state: tauri::State<'_, DbActorHandle>,
    settings: Settings,
) -> Result<(), AppError> {
    state.update_settings(settings).await
}

#[tauri::command]
#[specta::specta]
fn list_workouts_cmd(folder: String) -> Result<WorkoutLibrary, AppError> {
    list_workouts(&folder)
}

// Expand a workout into the canonical flat block list (intervals unfolded, power in
// watts, labels synthesized). Single source of truth for planned-workout rendering:
// the frontend calls this instead of duplicating the flatten/zone logic.
#[tauri::command]
#[specta::specta]
fn flatten_workout_cmd(workout: ParsedWorkout, ftp_w: u16) -> Vec<FlatBlock> {
    flatten_workout(workout, ftp_w)
}

#[tauri::command]
#[specta::specta]
async fn start_session(
    state: tauri::State<'_, SessionActorHandle>,
    workout: ParsedWorkout,
    ftp_w: u16,
) -> Result<(), AppError> {
    state.start(workout, ftp_w).await
}

#[tauri::command]
#[specta::specta]
async fn pause_session(state: tauri::State<'_, SessionActorHandle>) -> Result<(), AppError> {
    state.pause().await
}

#[tauri::command]
#[specta::specta]
async fn resume_session(state: tauri::State<'_, SessionActorHandle>) -> Result<(), AppError> {
    state.resume().await
}

#[tauri::command]
#[specta::specta]
async fn stop_session(state: tauri::State<'_, SessionActorHandle>) -> Result<(), AppError> {
    state.stop().await
}

#[tauri::command]
#[specta::specta]
async fn skip_block(state: tauri::State<'_, SessionActorHandle>) -> Result<(), AppError> {
    state.skip().await
}

#[tauri::command]
#[specta::specta]
async fn report_aero(
    state: tauri::State<'_, SessionActorHandle>,
    aero: Option<bool>,
) -> Result<(), AppError> {
    state.report_aero(aero).await
}

#[tauri::command]
#[specta::specta]
async fn get_session_snapshot(
    state: tauri::State<'_, SessionActorHandle>,
) -> Result<Option<SessionSnapshot>, AppError> {
    state.snapshot().await
}

#[tauri::command]
#[specta::specta]
async fn list_sessions(
    state: tauri::State<'_, DbActorHandle>,
) -> Result<Vec<SessionCard>, AppError> {
    state.list_sessions().await
}

#[tauri::command]
#[specta::specta]
async fn get_session(
    state: tauri::State<'_, DbActorHandle>,
    id: i64,
) -> Result<SessionDetail, AppError> {
    state.get_session(id).await
}

#[tauri::command]
#[specta::specta]
async fn delete_session(state: tauri::State<'_, DbActorHandle>, id: i64) -> Result<(), AppError> {
    state.delete_session(id).await
}

#[tauri::command]
#[specta::specta]
async fn export_session_tcx(
    state: tauri::State<'_, DbActorHandle>,
    id: i64,
    path: String,
) -> Result<(), AppError> {
    let detail = state.get_session(id).await?;
    let tcx = export::tcx::build_tcx(&detail);
    std::fs::write(&path, tcx)?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
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
#[specta::specta]
async fn strava_connect(
    app: tauri::AppHandle,
    state: tauri::State<'_, DbActorHandle>,
) -> Result<StravaStatus, AppError> {
    let proxy_url = state.get_settings().await?.strava_proxy_url;
    // CSRF nonce: passed to Strava and verified when the callback returns.
    let csrf_state = uuid::Uuid::new_v4().to_string();
    // Bind the loopback listener before opening the browser so the callback
    // can never race ahead of us listening.
    let listener = strava::oauth::bind_loopback().await?;
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
#[specta::specta]
async fn strava_disconnect(state: tauri::State<'_, DbActorHandle>) -> Result<(), AppError> {
    state.delete_strava_auth().await
}

#[tauri::command]
#[specta::specta]
async fn strava_set_auto_upload(
    state: tauri::State<'_, DbActorHandle>,
    enabled: bool,
) -> Result<(), AppError> {
    state.set_strava_auto_upload(enabled).await
}

#[tauri::command]
#[specta::specta]
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

// Single specta/tauri-specta builder describing every command that crosses the bridge.
// `run()` uses it to register the invoke handler; `export_typescript_bindings` uses it
// to generate the typed `src/lib/bindings.ts` the frontend imports. Event payload types
// (ble_metrics, session_metrics, ble_error, ble_reconnect) are pulled in with `.typ()`
// so they appear in the bindings even though the events themselves are still driven by
// manual `listen()` calls on the frontend.
fn specta_builder() -> tauri_specta::Builder<tauri::Wry> {
    use tauri_specta::{collect_commands, Builder};
    Builder::<tauri::Wry>::new()
        .commands(collect_commands![
            load_workout,
            list_workouts_cmd,
            flatten_workout_cmd,
            scan_devices,
            connect_trainer,
            connect_hrm,
            disconnect_trainer,
            disconnect_hrm,
            retry_reconnect,
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
            upload_session_to_strava,
        ])
        .typ::<crate::ble::BleMetrics>()
        .typ::<crate::ble::BleError>()
        .typ::<crate::ble::BleReconnect>()
        .typ::<crate::session::SessionMetrics>()
        // The frontend has always treated i64/u32 ids and counters as plain `number`
        // (session ids, athlete ids, durations). Emit `number` instead of `bigint` so
        // the generated bindings match the existing call sites and JSON round-trips.
        .dangerously_cast_bigints_to_number()
        // Throw, not Result-object wrapping: call sites keep the promise-rejection
        // semantics the whole frontend is written against (try/catch + toMessage).
        .error_handling(tauri_specta::ErrorHandlingMode::Throw)
        // Unified serde mode: without it, types whose serialize and deserialize JSON
        // shapes differ (e.g. skip_serializing_if fields) are split into two exported
        // types, which nothing in this frontend needs.
        .disable_serde_phases()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let specta = specta_builder();
    // Keep src/lib/bindings.ts in lock-step with the Rust bridge during development:
    // every `pnpm tauri dev` run rewrites it, so a changed command signature or shared
    // type surfaces immediately as a TypeScript error in the frontend.
    #[cfg(debug_assertions)]
    export_typescript_bindings();
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(specta.invoke_handler())
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
            // Critical BLE lifecycle events (trainer lost/reconnected). Mirror of the
            // metrics channel but sent with send().await so they are never dropped.
            let (ble_event_tx, ble_event_rx) = tokio::sync::mpsc::channel::<BleEvent>(16);
            let ble_handle = tauri::async_runtime::block_on(BleActorHandle::spawn(
                app.handle().clone(),
                ble_metrics_tx,
                ble_event_tx,
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
                ble_event_rx,
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

/// Writes the generated TypeScript bindings (commands + shared types) to
/// `src/lib/bindings.ts` at the repo root, resolved from this crate's manifest dir so
/// it works regardless of the current working directory. Called on startup in debug
/// builds (see `run()`) and by the `export_bindings` bin target for one-shot
/// regeneration (`cargo run --bin export_bindings`).
///
/// Not a `#[test]`: instantiating the Wry-typed builder inside a test executable links
/// the WebView GUI stack, which fails to load without the Windows manifest that
/// tauri-build only embeds into bin targets (STATUS_ENTRYPOINT_NOT_FOUND).
pub fn export_typescript_bindings() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/lib/bindings.ts");
    specta_builder()
        .export(specta_typescript::Typescript::default(), path)
        .expect("failed to export TypeScript bindings");
    // println, not tracing: this runs before the tracing subscriber is initialized
    // (debug startup) or without one at all (the export_bindings bin).
    println!("exported TypeScript bindings to {path}");
}
