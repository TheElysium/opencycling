use crate::errors::AppError;
use crate::strava::types::ProxyTokens;
use serde::Deserialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::time::{timeout, Duration};

/// Maximum number of bytes we read from the callback request before giving up.
/// The callback is a single short GET line plus headers; this cap keeps a
/// misbehaving or malicious client from making us buffer unbounded memory.
const MAX_REQUEST_BYTES: usize = 8 * 1024;

/// Fallback proxy URL when the user has not customised it in Settings.
pub const DEFAULT_PROXY_URL: &str = "http://127.0.0.1:8788";
pub const LOOPBACK_PORT: u16 = 8123;

/// Trims a user-provided proxy URL, stripping a trailing slash so route
/// concatenation never yields `//path`, and falls back to the default when empty.
fn normalized_proxy_url(proxy_url: &str) -> String {
    let trimmed = proxy_url.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        DEFAULT_PROXY_URL.to_string()
    } else {
        trimmed.to_string()
    }
}

#[derive(Deserialize)]
struct ProxyConfig {
    client_id: String,
}

/// Fetches the user's Strava client id from their local proxy's `/config` route.
/// Keeping the client id on the proxy means it lives in exactly one place
/// (the proxy `.env`) instead of being duplicated in the desktop app.
async fn fetch_client_id(proxy_url: &str) -> Result<String, AppError> {
    let url = format!("{}/config", normalized_proxy_url(proxy_url));
    let resp = reqwest::Client::new()
        .get(&url)
        .send()
        .await
        .map_err(|e| AppError::StravaAuth(format!("proxy /config: {e}")))?;
    if !resp.status().is_success() {
        return Err(AppError::StravaAuth(format!(
            "proxy /config returned {}",
            resp.status()
        )));
    }
    resp.json::<ProxyConfig>()
        .await
        .map(|c| c.client_id)
        .map_err(|e| AppError::StravaAuth(format!("proxy /config: {e}")))
}

/// Builds the Strava authorize URL the browser is sent to.
/// `state` is an opaque nonce echoed back in the callback for CSRF protection.
/// The client id is fetched from the user's proxy so it always matches the app
/// the proxy will use for the token exchange.
pub async fn authorize_url(proxy_url: &str, state: &str) -> Result<String, AppError> {
    let client_id = fetch_client_id(proxy_url).await?;
    Ok(format!(
        "https://www.strava.com/oauth/authorize?client_id={client_id}\
&response_type=code&redirect_uri=http://localhost:{LOOPBACK_PORT}/callback\
&approval_prompt=auto&scope=activity:write,read&state={state}"
    ))
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

/// Returns the byte length of the request headers once the `\r\n\r\n`
/// terminator has been seen, or `None` if the buffer does not yet contain a
/// complete header block. Pure so it can be unit-tested without a socket.
fn header_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|pos| pos + 4)
}

/// Binds the one-shot loopback server. Call this *before* opening the browser
/// so the callback can never arrive before we are listening.
///
/// On `AddrInUse` the message tells the user another authorization may still be
/// pending or another application is holding port 8123.
pub async fn bind_loopback() -> Result<TcpListener, AppError> {
    TcpListener::bind(("127.0.0.1", LOOPBACK_PORT))
        .await
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::AddrInUse {
                AppError::StravaAuth(format!(
                    "port {LOOPBACK_PORT} is already in use: another authorization may still \
                     be pending, or another application is using it. Wait a moment or close \
                     the other application, then try again."
                ))
            } else {
                AppError::StravaAuth(format!("loopback bind: {e}"))
            }
        })
}

/// Awaits the loopback callback, returning the `code` query param.
/// Fully async, so the wrapping `timeout` genuinely cancels the accept and the
/// listener is dropped (freeing the port) when the deadline elapses.
pub async fn wait_for_code(
    listener: TcpListener,
    expected_state: String,
) -> Result<String, AppError> {
    timeout(
        Duration::from_secs(180),
        handle_callback(listener, expected_state),
    )
    .await
    .map_err(|_| AppError::StravaAuth("authorization timed out".into()))?
}

/// Accepts a single connection, parses the callback, replies, and returns the code.
async fn handle_callback(
    listener: TcpListener,
    expected_state: String,
) -> Result<String, AppError> {
    let (mut stream, _) = listener
        .accept()
        .await
        .map_err(|e| AppError::StravaAuth(format!("loopback accept: {e}")))?;

    // Read until the header terminator, bounded so a client cannot make us
    // buffer unbounded memory. A single GET request line plus headers is tiny.
    let mut buf = Vec::with_capacity(2048);
    let mut chunk = [0u8; 1024];
    loop {
        if header_end(&buf).is_some() {
            break;
        }
        if buf.len() >= MAX_REQUEST_BYTES {
            return Err(AppError::StravaAuth("callback request too large".into()));
        }
        let n = stream
            .read(&mut chunk)
            .await
            .map_err(|e| AppError::StravaAuth(format!("loopback read: {e}")))?;
        if n == 0 {
            break; // peer closed before sending a full header block
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    let req = String::from_utf8_lossy(&buf);

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

    // Always answer the browser so the tab shows the outcome, and use an honest
    // status: 200 when we got a code, 400 when the callback was missing/denied.
    let (status, body) = match &result {
        Ok(_) => (
            "200 OK",
            "<html><body>OpenCycling connected. You can close this tab.</body></html>",
        ),
        Err(_) => (
            "400 Bad Request",
            "<html><body>OpenCycling authorization failed. \
             You can close this tab and try again.</body></html>",
        ),
    };
    let resp = format!(
        "HTTP/1.1 {}\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{}",
        status,
        body.len(),
        body
    );
    let _ = stream.write_all(resp.as_bytes()).await;
    result
}

/// Exchanges an auth code for tokens via the proxy.
pub async fn exchange_code(proxy_url: &str, code: &str) -> Result<ProxyTokens, AppError> {
    proxy_post(proxy_url, "/exchange", serde_json::json!({ "code": code })).await
}

/// Refreshes tokens via the proxy.
pub async fn refresh_tokens(proxy_url: &str, refresh_token: &str) -> Result<ProxyTokens, AppError> {
    proxy_post(
        proxy_url,
        "/refresh",
        serde_json::json!({ "refresh_token": refresh_token }),
    )
    .await
}

async fn proxy_post(
    proxy_url: &str,
    path: &str,
    body: serde_json::Value,
) -> Result<ProxyTokens, AppError> {
    let url = format!("{}{path}", normalized_proxy_url(proxy_url));
    let resp = reqwest::Client::new()
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| AppError::StravaAuth(format!("proxy {path}: {e}")))?;
    let text = resp.text().await?;
    serde_json::from_str::<ProxyTokens>(&text)
        .map_err(|_| AppError::StravaAuth(format!("proxy {path} returned: {text}")))
}

#[cfg(test)]
mod tests {
    use super::{header_end, query_param};

    #[test]
    fn header_end_found_after_terminator() {
        let req = b"GET /callback?code=x HTTP/1.1\r\nHost: localhost\r\n\r\n";
        assert_eq!(header_end(req), Some(req.len()));
    }

    #[test]
    fn header_end_none_without_terminator() {
        // A partial request (no blank line yet) must not be treated as complete.
        assert_eq!(header_end(b"GET /callback?code=x HTTP/1.1\r\nHost: loc"), None);
    }

    #[test]
    fn header_end_ignores_body_after_terminator() {
        let req = b"GET / HTTP/1.1\r\n\r\nBODYBYTES";
        assert_eq!(header_end(req), Some(req.len() - "BODYBYTES".len()));
    }


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
