//! API client, guest-token handling and error types.
//!
//! The Jan Aushadhi backend is a Spring Boot service reachable at
//! `https://janaushadhi.gov.in:8443`. Browsing anonymously requires a short-lived
//! guest JWT from `GET /auth/generateGuestToken`; every API call carries it as an
//! `Authorization: Bearer` header. This module hides all of that: it caches the
//! token on disk (nothing else is ever cached), refreshes it proactively before
//! expiry and reactively on 401/403, and normalizes the server's response envelope
//! (`{responseBody, message, responseCode}`) plus its raw Spring error shape into
//! typed errors.

use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use serde_json::{json, Value};

pub const BASE_URL: &str = "https://janaushadhi.gov.in:8443";

#[derive(Debug)]
pub enum ApiError {
    Network(String),
    Http { status: u16, body: String },
    Server { code: u64, message: String },
    Parse(String),
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiError::Network(m) => write!(f, "network error: {m}"),
            ApiError::Http { status, body } => {
                let trimmed = body.trim();
                let short: String = trimmed.chars().take(300).collect();
                if short.is_empty() {
                    write!(f, "HTTP {status}")
                } else {
                    write!(f, "HTTP {status}: {short}")
                }
            }
            ApiError::Server { code, message } => write!(f, "server returned {code}: {message}"),
            ApiError::Parse(m) => write!(f, "unexpected response: {m}"),
        }
    }
}

impl std::error::Error for ApiError {}

/// Application-level error with a mapping to process exit codes.
#[derive(Debug)]
pub enum AppErr {
    /// Bad invocation (exit code 2)
    Usage(String),
    /// Requested record does not exist (exit code 1)
    NotFound(String),
    /// Network or API failure (exit code 3)
    Api(ApiError),
}

impl AppErr {
    pub fn exit_code(&self) -> i32 {
        match self {
            AppErr::Usage(_) => 2,
            AppErr::NotFound(_) => 1,
            AppErr::Api(_) => 3,
        }
    }
}

impl std::fmt::Display for AppErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppErr::Usage(m) => write!(f, "{m}"),
            AppErr::NotFound(m) => write!(f, "{m}"),
            AppErr::Api(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for AppErr {}

impl From<ApiError> for AppErr {
    fn from(e: ApiError) -> Self {
        AppErr::Api(e)
    }
}

pub fn usage(msg: impl Into<String>) -> AppErr {
    AppErr::Usage(msg.into())
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn cache_file() -> PathBuf {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let home = std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."));
            home.join(".cache")
        });
    base.join("janaushadhi").join("token.json")
}

#[derive(Default)]
struct TokenCache {
    token: Option<String>,
    exp: Option<u64>,
    loaded: bool,
}

pub struct Client {
    agent: ureq::Agent,
    verbose: bool,
    token: TokenCache,
    cache_path: PathBuf,
}

impl Client {
    pub fn new(verbose: bool) -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(90))
            .user_agent(concat!("janaushadhi-cli/", env!("CARGO_PKG_VERSION")))
            .build();
        Self {
            agent,
            verbose,
            token: TokenCache::default(),
            cache_path: cache_file(),
        }
    }

    fn load_cached_token(&mut self) {
        if self.token.loaded {
            return;
        }
        self.token.loaded = true;
        if let Ok(raw) = fs::read_to_string(&self.cache_path) {
            if let Ok(v) = serde_json::from_str::<Value>(&raw) {
                self.token.token = v.get("token").and_then(|t| t.as_str()).map(String::from);
                self.token.exp = v.get("exp").and_then(|e| e.as_u64());
            }
        }
    }

    fn save_cached_token(&self, token: &str, exp: u64) {
        if let Some(dir) = self.cache_path.parent() {
            let _ = fs::create_dir_all(dir);
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = fs::set_permissions(dir, fs::Permissions::from_mode(0o700));
            }
        }
        let body = json!({ "token": token, "exp": exp }).to_string();
        if fs::write(&self.cache_path, body).is_ok() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = fs::set_permissions(&self.cache_path, fs::Permissions::from_mode(0o600));
            }
        }
    }

    /// Ensure a valid guest token exists and return its expiry (unix seconds).
    pub fn token_expiry(&mut self) -> Result<u64, AppErr> {
        self.ensure_token()?;
        Ok(self.token.exp.unwrap_or(0))
    }

    fn ensure_token(&mut self) -> Result<String, AppErr> {
        self.load_cached_token();
        if let (Some(t), Some(exp)) = (&self.token.token, self.token.exp) {
            if exp > now_secs() + 60 {
                return Ok(t.clone());
            }
        }
        self.fetch_token()
    }

    fn fetch_token(&mut self) -> Result<String, AppErr> {
        if self.verbose {
            eprintln!("[auth ] fetching guest token");
        }
        let url = format!("{BASE_URL}/auth/generateGuestToken");
        let v: Value = match self.agent.get(&url).call() {
            Ok(r) => r.into_json().map_err(|e| {
                AppErr::Api(ApiError::Parse(format!("token endpoint returned non-JSON: {e}")))
            })?,
            Err(ureq::Error::Status(code, r)) => {
                let body = r.into_string().unwrap_or_default();
                return Err(AppErr::Api(ApiError::Http { status: code, body }));
            }
            Err(ureq::Error::Transport(t)) => {
                return Err(AppErr::Api(ApiError::Network(t.to_string())));
            }
        };
        let token = v
            .get("responseBody")
            .and_then(|b| b.as_str())
            .ok_or_else(|| {
                AppErr::Api(ApiError::Parse(
                    "guest token missing from responseBody".to_string(),
                ))
            })?
            .to_string();
        let exp = jwt_exp(&token).unwrap_or_else(|| now_secs() + 25 * 60);
        self.save_cached_token(&token, exp);
        self.token.token = Some(token.clone());
        self.token.exp = Some(exp);
        Ok(token)
    }

    pub fn post_json(&mut self, path: &str, payload: &Value) -> Result<Value, AppErr> {
        self.request_json("POST", path, Some(payload))
    }

    pub fn get_json(&mut self, path: &str) -> Result<Value, AppErr> {
        self.request_json("GET", path, None)
    }

    fn request_json(
        &mut self,
        method: &str,
        path: &str,
        payload: Option<&Value>,
    ) -> Result<Value, AppErr> {
        let url = format!("{BASE_URL}{path}");
        let mut refreshed = false;
        let mut retried = false;
        loop {
            let token = self.ensure_token()?;
            let result = if method == "POST" {
                let body = payload.cloned().unwrap_or_else(|| json!({}));
                self.agent
                    .post(&url)
                    .set("Content-Type", "application/json")
                    .set("Accept", "application/json")
                    .set("Authorization", &format!("Bearer {token}"))
                    .send_json(body)
            } else {
                self.agent
                    .get(&url)
                    .set("Accept", "application/json")
                    .set("Authorization", &format!("Bearer {token}"))
                    .call()
            };
            match result {
                Ok(resp) => {
                    return resp.into_json::<Value>().map_err(|e| {
                        AppErr::Api(ApiError::Parse(format!("{path} returned invalid JSON: {e}")))
                    });
                }
                Err(ureq::Error::Status(code, resp)) => {
                    if (code == 401 || code == 403) && !refreshed {
                        refreshed = true;
                        self.token.token = None;
                        self.token.exp = None;
                        self.token.loaded = true;
                        if self.verbose {
                            eprintln!(
                                "[auth ] {code} shown by server — refreshing guest token and retrying"
                            );
                        }
                        continue;
                    }
                    if code >= 500 && !retried {
                        retried = true;
                        if self.verbose {
                            eprintln!("[retry] HTTP {code} — retrying once after 1s");
                        }
                        std::thread::sleep(Duration::from_secs(1));
                        continue;
                    }
                    let body = resp.into_string().unwrap_or_default();
                    return Err(AppErr::Api(ApiError::Http { status: code, body }));
                }
                Err(ureq::Error::Transport(t)) => {
                    if !retried {
                        retried = true;
                        if self.verbose {
                            eprintln!("[retry] {t} — retrying once after 1s");
                        }
                        std::thread::sleep(Duration::from_secs(1));
                        continue;
                    }
                    return Err(AppErr::Api(ApiError::Network(t.to_string())));
                }
            }
        }
    }
}

/// Decode the `exp` claim of a JWT without external dependencies.
pub fn jwt_exp(token: &str) -> Option<u64> {
    let payload = token.split('.').nth(1)?;
    let bytes = URL_SAFE_NO_PAD.decode(payload).ok()?;
    let v: Value = serde_json::from_slice(&bytes).ok()?;
    v.get("exp").and_then(|e| e.as_u64())
}

/// Normalize the `{responseBody, message, responseCode}` envelope.
///
/// Returns `Ok(None)` for `404 record not found` — that is an honest empty result,
/// not an error — and `Ok(Some(body))` otherwise.
pub fn unwrap_response(v: Value) -> Result<Option<Value>, AppErr> {
    if let Some(code) = v.get("responseCode").and_then(|c| c.as_u64()) {
        if code == 200 {
            return Ok(v.get("responseBody").cloned());
        }
        if code == 404 {
            return Ok(match v.get("responseBody").cloned() {
                Some(Value::Null) | None => None,
                other => other,
            });
        }
        let message = v
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("")
            .to_string();
        return Err(AppErr::Api(ApiError::Server { code, message }));
    }
    // Raw Spring error body: {"timestamp":..., "status":500, "error":"..."}
    if let Some(status) = v.get("status").and_then(|s| s.as_u64()) {
        let error = v.get("error").and_then(|e| e.as_str()).unwrap_or("error");
        let path = v.get("path").and_then(|p| p.as_str()).unwrap_or("");
        return Err(AppErr::Api(ApiError::Http {
            status: status as u16,
            body: format!("{error} ({path})"),
        }));
    }
    Ok(Some(v))
}

fn data_list(v: &Value) -> Option<Vec<Value>> {
    v.get("data")
        .and_then(|d| d.as_array())
        .or_else(|| v.get("responseBody").and_then(|d| d.as_array()))
        .cloned()
}

/// `GET /api/kendra/getAllStateOfIndia` — the state master list.
pub fn get_states(client: &mut Client) -> Result<Vec<Value>, AppErr> {
    let v = client.get_json("/api/kendra/getAllStateOfIndia")?;
    if let Some(code) = v.get("responseCode").and_then(|c| c.as_u64()) {
        if code != 200 {
            let message = v.get("message").and_then(|m| m.as_str()).unwrap_or("").to_string();
            return Err(AppErr::Api(ApiError::Server { code, message }));
        }
    }
    data_list(&v).ok_or_else(|| AppErr::Api(ApiError::Parse("states response has no list".to_string())))
}

/// `GET /api/kendra/getDistrictByStateId?id=<stateCode>` — districts of one state.
pub fn get_districts(client: &mut Client, state_code: u64) -> Result<Vec<Value>, AppErr> {
    let v = client.get_json(&format!("/api/kendra/getDistrictByStateId?id={state_code}"))?;
    if let Some(code) = v.get("responseCode").and_then(|c| c.as_u64()) {
        if code != 200 {
            let message = v.get("message").and_then(|m| m.as_str()).unwrap_or("").to_string();
            return Err(AppErr::Api(ApiError::Server { code, message }));
        }
    }
    data_list(&v).ok_or_else(|| AppErr::Api(ApiError::Parse("districts response has no list".to_string())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jwt_exp_parses_payload() {
        let payload = URL_SAFE_NO_PAD.encode(br#"{"role":"ROLE_GUEST","exp":1234567890}"#);
        let token = format!("aaa.{payload}.bbb");
        assert_eq!(jwt_exp(&token), Some(1234567890));
    }

    #[test]
    fn jwt_exp_rejects_garbage() {
        assert_eq!(jwt_exp("nope"), None);
        assert_eq!(jwt_exp("a.b.c"), None);
    }

    #[test]
    fn unwrap_404_is_empty() {
        let v = json!({"message":"record not found","responseCode":404,"responseBody":null});
        assert!(unwrap_response(v).unwrap().is_none());
    }

    #[test]
    fn unwrap_server_error_is_err() {
        let v = json!({"message":"server error","responseCode":500});
        assert!(unwrap_response(v).is_err());
    }

    #[test]
    fn unwrap_raw_spring_error() {
        let v = json!({"timestamp":"2026-10-02T00:00:00Z","status":500,"error":"Internal Server Error","path":"/x"});
        assert!(unwrap_response(v).is_err());
    }
}
