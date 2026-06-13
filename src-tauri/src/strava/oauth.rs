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
/// `state` is an opaque nonce echoed back in the callback for CSRF protection.
pub fn authorize_url(state: &str) -> String {
    format!(
        "https://www.strava.com/oauth/authorize?client_id={CLIENT_ID}\
&response_type=code&redirect_uri=http://localhost:{LOOPBACK_PORT}/callback\
&approval_prompt=auto&scope=activity:write,read&state={state}"
    )
}

/// Extracts a query parameter from the request's first line.
/// Values are assumed unencoded (Strava `code`/`state` are url-safe).
fn query_param(req: &str, key: &str) -> Option<String> {
    let path = req.split_whitespace().nth(1)?; // "/callback?code=..&state=.."
    let query = path.split('?').nth(1)?;
    query
        .split('&')
        .filter_map(|kv| kv.split_once('='))
        .find(|(k, _)| *k == key)
        .map(|(_, v)| v.to_string())
}

/// Binds the one-shot loopback server. Call this *before* opening the browser
/// so the callback can never arrive before we are listening.
pub fn bind_loopback() -> Result<TcpListener, AppError> {
    TcpListener::bind(("127.0.0.1", LOOPBACK_PORT))
        .map_err(|e| AppError::StravaAuth(format!("loopback bind: {e}")))
}

/// Blocks on the loopback server, returning the `code` query param.
/// Runs on a blocking thread (std TcpListener). 3 min timeout overall.
pub async fn wait_for_code(
    listener: TcpListener,
    expected_state: String,
) -> Result<String, AppError> {
    let handle = tokio::task::spawn_blocking(move || -> Result<String, AppError> {
        let (mut stream, _) = listener
            .accept()
            .map_err(|e| AppError::StravaAuth(format!("loopback accept: {e}")))?;
        let mut buf = [0u8; 2048];
        let n = stream
            .read(&mut buf)
            .map_err(|e| AppError::StravaAuth(format!("loopback read: {e}")))?;
        let req = String::from_utf8_lossy(&buf[..n]);
        // First line: "GET /callback?code=XXX&state=YYY&scope=... HTTP/1.1"
        // CSRF guard: the callback must echo the state we sent.
        let result = (|| {
            let state = query_param(&req, "state")
                .filter(|s| !s.is_empty())
                .ok_or_else(|| AppError::StravaAuth("no state in callback".into()))?;
            if state != expected_state {
                return Err(AppError::StravaAuth(
                    "state mismatch (possible CSRF)".into(),
                ));
            }
            query_param(&req, "code")
                .filter(|c| !c.is_empty())
                .ok_or_else(|| AppError::StravaAuth("no code in callback".into()))
        })();

        // Always answer the browser so the tab shows the outcome.
        let body = match &result {
            Ok(_) => "<html><body>OpenCycling connected. You can close this tab.</body></html>",
            Err(_) => {
                "<html><body>OpenCycling authorization failed. \
                 You can close this tab and try again.</body></html>"
            }
        };
        let resp = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        let _ = stream.write_all(resp.as_bytes());
        result
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

#[cfg(test)]
mod tests {
    use super::query_param;

    const REQ: &str =
        "GET /callback?code=abc123&state=nonce42&scope=read HTTP/1.1\r\nHost: localhost\r\n\r\n";

    #[test]
    fn extracts_code_and_state() {
        assert_eq!(query_param(REQ, "code").as_deref(), Some("abc123"));
        assert_eq!(query_param(REQ, "state").as_deref(), Some("nonce42"));
    }

    #[test]
    fn returns_none_for_missing_key() {
        assert_eq!(query_param(REQ, "error"), None);
    }

    #[test]
    fn returns_none_when_no_query_string() {
        let req = "GET /callback HTTP/1.1\r\n\r\n";
        assert_eq!(query_param(req, "code"), None);
    }

    #[test]
    fn does_not_match_substring_keys() {
        // Guards against the old `split("code=")` behaviour: a param whose name
        // merely contains another key must not be returned for that key.
        let req = "GET /callback?scope=read&qrcode=x HTTP/1.1\r\n\r\n";
        assert_eq!(query_param(req, "code"), None);
    }
}
