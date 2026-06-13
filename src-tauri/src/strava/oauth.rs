use crate::errors::AppError;
use crate::strava::types::ProxyTokens;
use std::io::{Read, Write};
use std::net::TcpListener;
use tokio::time::{timeout, Duration};

/// Public Strava application client id (safe to ship: visible in the authorize URL).
pub const CLIENT_ID: &str = "257818";
pub const LOOPBACK_PORT: u16 = 8123;
pub const PROXY_BASE: &str = "http://127.0.0.1:8788";

/// Builds the Strava authorize URL the browser is sent to.
pub fn authorize_url() -> String {
    format!(
        "https://www.strava.com/oauth/authorize?client_id={CLIENT_ID}\
&response_type=code&redirect_uri=http://localhost:{LOOPBACK_PORT}/callback\
&approval_prompt=auto&scope=activity:write,read"
    )
}

/// Blocks on a one-shot loopback server, returning the `code` query param.
/// Runs on a blocking thread (std TcpListener). 3 min timeout overall.
pub async fn wait_for_code() -> Result<String, AppError> {
    let handle = tokio::task::spawn_blocking(|| -> Result<String, AppError> {
        let listener = TcpListener::bind(("127.0.0.1", LOOPBACK_PORT))
            .map_err(|e| AppError::StravaAuth(format!("loopback bind: {e}")))?;
        let (mut stream, _) = listener
            .accept()
            .map_err(|e| AppError::StravaAuth(format!("loopback accept: {e}")))?;
        let mut buf = [0u8; 2048];
        let n = stream
            .read(&mut buf)
            .map_err(|e| AppError::StravaAuth(format!("loopback read: {e}")))?;
        let req = String::from_utf8_lossy(&buf[..n]);
        // First line: "GET /callback?code=XXX&scope=... HTTP/1.1"
        let code = req
            .split_whitespace()
            .nth(1)
            .and_then(|path| path.split("code=").nth(1))
            .map(|rest| rest.split('&').next().unwrap_or("").to_string())
            .filter(|c| !c.is_empty())
            .ok_or_else(|| AppError::StravaAuth("no code in callback".into()))?;
        let body = "<html><body>OpenCycling connected. You can close this tab.</body></html>";
        let resp = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        let _ = stream.write_all(resp.as_bytes());
        Ok(code)
    });

    timeout(Duration::from_secs(180), handle)
        .await
        .map_err(|_| AppError::StravaAuth("authorization timed out".into()))?
        .map_err(|e| AppError::StravaAuth(e.to_string()))?
}

/// Exchanges an auth code for tokens via the proxy.
pub async fn exchange_code(code: &str) -> Result<ProxyTokens, AppError> {
    proxy_post("/exchange", serde_json::json!({ "code": code })).await
}

/// Refreshes tokens via the proxy.
pub async fn refresh_tokens(refresh_token: &str) -> Result<ProxyTokens, AppError> {
    proxy_post(
        "/refresh",
        serde_json::json!({ "refresh_token": refresh_token }),
    )
    .await
}

async fn proxy_post(path: &str, body: serde_json::Value) -> Result<ProxyTokens, AppError> {
    let url = format!("{PROXY_BASE}{path}");
    let resp = reqwest::Client::new()
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| AppError::StravaAuth(format!("proxy {path}: {e}")))?;
    let text = resp
        .text()
        .await
        .map_err(|e| AppError::StravaAuth(e.to_string()))?;
    serde_json::from_str::<ProxyTokens>(&text)
        .map_err(|_| AppError::StravaAuth(format!("proxy {path} returned: {text}")))
}
