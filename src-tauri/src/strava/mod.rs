pub mod api;
pub mod oauth;
pub mod types;

use crate::db::DbActorHandle;
use crate::db::StravaAuth;
use crate::errors::AppError;

/// Returns a valid access token, refreshing via the proxy if it expires soon.
pub async fn ensure_fresh_token(db: &DbActorHandle) -> Result<String, AppError> {
    let auth = db
        .get_strava_auth()
        .await?
        .ok_or_else(|| AppError::StravaAuth("not connected to Strava".into()))?;

    let now = chrono::Utc::now().timestamp();
    if auth.expires_at - now > 300 {
        return Ok(auth.access_token);
    }

    let fresh = oauth::refresh_tokens(&auth.refresh_token).await?;
    let updated = StravaAuth {
        access_token: fresh.access_token.clone(),
        refresh_token: fresh.refresh_token,
        expires_at: fresh.expires_at,
        athlete_id: auth.athlete_id,
        athlete_name: auth.athlete_name,
        connected_at: auth.connected_at,
    };
    db.upsert_strava_auth(updated).await?;
    Ok(fresh.access_token)
}
