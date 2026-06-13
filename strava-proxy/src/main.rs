use axum::{extract::State, routing::post, Json, Router};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone)]
struct Config {
    client_id: String,
    client_secret: String,
    http: reqwest::Client,
}

#[derive(Deserialize)]
struct ExchangeReq {
    code: String,
}

#[derive(Deserialize)]
struct RefreshReq {
    refresh_token: String,
}

#[derive(Serialize)]
struct Tokens {
    access_token: String,
    refresh_token: String,
    expires_at: i64,
    athlete_id: Option<i64>,
    athlete_name: Option<String>,
}

#[derive(Deserialize)]
struct StravaTokenResp {
    access_token: String,
    refresh_token: String,
    expires_at: i64,
    #[serde(default)]
    athlete: Option<Athlete>,
}

#[derive(Deserialize)]
struct Athlete {
    id: i64,
    #[serde(default)]
    firstname: Option<String>,
    #[serde(default)]
    lastname: Option<String>,
}

async fn exchange(
    State(cfg): State<Arc<Config>>,
    Json(req): Json<ExchangeReq>,
) -> Result<Json<Tokens>, String> {
    let resp = cfg
        .http
        .post("https://www.strava.com/api/v3/oauth/token")
        .form(&[
            ("client_id", cfg.client_id.as_str()),
            ("client_secret", cfg.client_secret.as_str()),
            ("code", req.code.as_str()),
            ("grant_type", "authorization_code"),
        ])
        .send()
        .await
        .map_err(|e| e.to_string())?;
    map_token_resp(resp).await
}

async fn refresh(
    State(cfg): State<Arc<Config>>,
    Json(req): Json<RefreshReq>,
) -> Result<Json<Tokens>, String> {
    let resp = cfg
        .http
        .post("https://www.strava.com/api/v3/oauth/token")
        .form(&[
            ("client_id", cfg.client_id.as_str()),
            ("client_secret", cfg.client_secret.as_str()),
            ("refresh_token", req.refresh_token.as_str()),
            ("grant_type", "refresh_token"),
        ])
        .send()
        .await
        .map_err(|e| e.to_string())?;
    map_token_resp(resp).await
}

async fn map_token_resp(resp: reqwest::Response) -> Result<Json<Tokens>, String> {
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Strava token error {status}: {body}"));
    }
    let t: StravaTokenResp = resp.json().await.map_err(|e| e.to_string())?;
    let athlete_id = t.athlete.as_ref().map(|a| a.id);
    let athlete_name = t.athlete.as_ref().and_then(|a| {
        let name = format!(
            "{} {}",
            a.firstname.as_deref().unwrap_or(""),
            a.lastname.as_deref().unwrap_or("")
        );
        let name = name.trim().to_string();
        (!name.is_empty()).then_some(name)
    });
    Ok(Json(Tokens {
        access_token: t.access_token,
        refresh_token: t.refresh_token,
        expires_at: t.expires_at,
        athlete_id,
        athlete_name,
    }))
}

#[tokio::main]
async fn main() {
    // Load strava-proxy/.env into the environment if present (optional).
    let _ = dotenvy::dotenv();

    let cfg = Arc::new(Config {
        client_id: std::env::var("STRAVA_CLIENT_ID").expect("STRAVA_CLIENT_ID"),
        client_secret: std::env::var("STRAVA_CLIENT_SECRET").expect("STRAVA_CLIENT_SECRET"),
        http: reqwest::Client::new(),
    });
    let port: u16 = std::env::var("PROXY_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8788);

    let app = Router::new()
        .route("/exchange", post(exchange))
        .route("/refresh", post(refresh))
        .with_state(cfg);

    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port))
        .await
        .expect("bind");
    println!("strava-proxy listening on http://127.0.0.1:{port}");
    axum::serve(listener, app).await.expect("serve");
}
