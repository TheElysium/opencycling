use serde::{Deserialize, Serialize};
use specta::Type;

/// Tokens returned by the proxy /exchange and /refresh routes.
#[derive(Deserialize, Debug, Clone)]
pub struct ProxyTokens {
    pub access_token: String,
    pub refresh_token: String,
    /// Epoch seconds (UTC).
    pub expires_at: i64,
    #[serde(default)]
    pub athlete_id: Option<i64>,
    #[serde(default)]
    pub athlete_name: Option<String>,
}

/// Status surfaced to the frontend.
#[derive(Serialize, Debug, Clone, Type)]
pub struct StravaStatus {
    pub connected: bool,
    pub athlete_id: Option<i64>,
    pub athlete_name: Option<String>,
    pub auto_upload: bool,
}

/// Strava upload polling response (GET /uploads/{id}).
#[derive(Deserialize, Debug, Clone)]
pub struct UploadStatus {
    pub id: i64,
    pub error: Option<String>,
    pub activity_id: Option<i64>,
}
