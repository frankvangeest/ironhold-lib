//! Nakama REST protocol, as pure request builders and response parsers. No I/O here: a driver (native: blocking, wasm:
//! callback) sends the `ehttp::Request` and hands the `ehttp::Response` back, so the same protocol code runs on both targets.

use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use base64::Engine;
use serde_json::{json, Value};

pub fn basic_auth(server_key: &str) -> String {
    format!("Basic {}", STANDARD.encode(format!("{server_key}:")))
}

fn json_request(method: &str, url: String, authorization: String, body: Option<Value>) -> ehttp::Request {
    let bytes = body.map(|b| b.to_string().into_bytes()).unwrap_or_default();
    let mut req = ehttp::Request::post(url, bytes);
    req.method = ehttp::Method::parse(method).expect("a valid HTTP method");
    req.headers.insert("Content-Type", "application/json");
    req.headers.insert("Authorization", authorization);
    req
}

#[derive(Debug, Clone)]
pub struct Session {
    pub token: String,
    pub refresh_token: String,
    pub created: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct Claims {
    pub iat: i64,
    pub exp: i64,
}

#[derive(Debug)]
pub struct ApiError {
    pub status: u16,
    pub body: String,
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HTTP {}: {}", self.status, self.body.chars().take(300).collect::<String>())
    }
}

fn ok_json(resp: &ehttp::Response) -> Result<Value, ApiError> {
    let text = resp.text().unwrap_or("").to_string();
    if !(200..300).contains(&resp.status) {
        return Err(ApiError { status: resp.status, body: text });
    }
    serde_json::from_str(&text).map_err(|e| ApiError { status: resp.status, body: format!("not JSON ({e}): {text}") })
}

/// The payload of a Nakama session JWT (`uid`, `iat`, `exp`). Only used to learn the token lifetime and the user id.
pub fn claims(token: &str) -> Option<(Claims, String)> {
    let payload = token.split('.').nth(1)?;
    let json: Value = serde_json::from_slice(&URL_SAFE_NO_PAD.decode(payload).ok()?).ok()?;
    let c = Claims { iat: json.get("iat")?.as_i64()?, exp: json.get("exp")?.as_i64()? };
    Some((c, json.get("uid")?.as_str()?.to_string()))
}

// ---- E1: sessions -----------------------------------------------------------------------------------------------

pub fn authenticate_device(base: &str, server_key: &str, device_id: &str, create: bool) -> ehttp::Request {
    json_request(
        "POST",
        format!("{base}/v2/account/authenticate/device?create={create}"),
        basic_auth(server_key),
        Some(json!({ "id": device_id })),
    )
}

pub fn refresh_session(base: &str, server_key: &str, refresh_token: &str) -> ehttp::Request {
    json_request(
        "POST",
        format!("{base}/v2/account/session/refresh"),
        basic_auth(server_key),
        Some(json!({ "token": refresh_token })),
    )
}

pub fn parse_session(resp: &ehttp::Response) -> Result<Session, ApiError> {
    let v = ok_json(resp)?;
    let get = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
    match (get("token"), get("refresh_token")) {
        (Some(token), Some(refresh_token)) => {
            Ok(Session { token, refresh_token, created: v.get("created").and_then(Value::as_bool).unwrap_or(false) })
        }
        _ => Err(ApiError { status: resp.status, body: format!("session response without tokens: {v}") }),
    }
}

// ---- E2: storage ------------------------------------------------------------------------------------------------

/// Write one object. `value` is a JSON *document as a string* (Nakama stores it as JSON text). `version` of `*` means
/// "only if it does not exist yet"; a concrete version means "only if it is still that version" (optimistic locking).
pub fn storage_write(base: &str, token: &str, collection: &str, key: &str, value: &str, version: Option<&str>) -> ehttp::Request {
    storage_write_with(base, token, collection, key, value, version, 1)
}

/// `permission_read`: 0 = nobody (server only), 1 = the owner, 2 = any signed-in user (public read). Writes stay owner-only.
pub fn storage_write_with(
    base: &str,
    token: &str,
    collection: &str,
    key: &str,
    value: &str,
    version: Option<&str>,
    permission_read: i32,
) -> ehttp::Request {
    let mut obj = json!({
        "collection": collection, "key": key, "value": value,
        "permission_read": permission_read, "permission_write": 1
    });
    if let Some(v) = version {
        obj["version"] = json!(v);
    }
    json_request("PUT", format!("{base}/v2/storage"), format!("Bearer {token}"), Some(json!({ "objects": [obj] })))
}

pub fn storage_read(base: &str, token: &str, collection: &str, key: &str, user_id: &str) -> ehttp::Request {
    json_request(
        "POST",
        format!("{base}/v2/storage"),
        format!("Bearer {token}"),
        Some(json!({ "object_ids": [{ "collection": collection, "key": key, "user_id": user_id }] })),
    )
}

/// The `version` of the first acknowledged write.
pub fn parse_write_ack(resp: &ehttp::Response) -> Result<String, ApiError> {
    let v = ok_json(resp)?;
    v.pointer("/acks/0/version")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| ApiError { status: resp.status, body: format!("no ack in {v}") })
}

/// `(value, version)` of the first object read, `None` when it does not exist.
pub fn parse_read(resp: &ehttp::Response) -> Result<Option<(String, String)>, ApiError> {
    let v = ok_json(resp)?;
    Ok(v.pointer("/objects/0").map(|o| {
        let s = |k: &str| o.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        (s("value"), s("version"))
    }))
}

// ---- E4b / E4c: listings ----------------------------------------------------------------------------------------

/// Public-read objects of one collection, across all users (no `user_id` filter): the "lobby list without server code" idea.
pub fn storage_list_public(base: &str, token: &str, collection: &str, limit: u32) -> ehttp::Request {
    let mut req = ehttp::Request::get(format!("{base}/v2/storage/{collection}?limit={limit}"));
    req.headers.insert("Authorization", format!("Bearer {token}"));
    req
}

/// `(user_id, key, value)` of every object in a storage listing.
pub fn parse_storage_list(resp: &ehttp::Response) -> Result<Vec<(String, String, String)>, ApiError> {
    let v = ok_json(resp)?;
    let s = |o: &Value, k: &str| o.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    Ok(v.get("objects")
        .and_then(Value::as_array)
        .map(|a| a.iter().map(|o| (s(o, "user_id"), s(o, "key"), s(o, "value"))).collect())
        .unwrap_or_default())
}

/// `GET /v2/match`: what Nakama lists as joinable matches. `authoritative=false` asks for relayed ones.
pub fn match_list(base: &str, token: &str, authoritative: bool, limit: u32) -> ehttp::Request {
    let mut req = ehttp::Request::get(format!("{base}/v2/match?limit={limit}&authoritative={authoritative}"));
    req.headers.insert("Authorization", format!("Bearer {token}"));
    req
}

/// The raw status and body of any response, for experiments that record exactly what the server said.
pub fn raw(resp: &ehttp::Response) -> (u16, String) {
    (resp.status, resp.text().unwrap_or("").to_string())
}

// ---- E3: leaderboard (created by server code, used by clients) ---------------------------------------------------

/// Submit a score. `score` is an int64, which the JSON API takes as a string.
pub fn leaderboard_submit(base: &str, token: &str, leaderboard: &str, score: i64) -> ehttp::Request {
    json_request("POST", format!("{base}/v2/leaderboard/{leaderboard}"), format!("Bearer {token}"), Some(json!({ "score": score.to_string() })))
}

pub fn leaderboard_top(base: &str, token: &str, leaderboard: &str, limit: u32) -> ehttp::Request {
    let mut req = ehttp::Request::get(format!("{base}/v2/leaderboard/{leaderboard}?limit={limit}"));
    req.headers.insert("Authorization", format!("Bearer {token}"));
    req
}

/// `(owner_id, score, rank)` of every record; the JSON API writes `score` and `rank` as strings.
pub fn parse_leaderboard(resp: &ehttp::Response) -> Result<Vec<(String, i64, i64)>, ApiError> {
    let v = ok_json(resp)?;
    let num = |o: &Value, k: &str| match o.get(k) {
        Some(Value::String(s)) => s.parse().unwrap_or(0),
        Some(n) => n.as_i64().unwrap_or(0),
        None => 0,
    };
    Ok(v.get("records")
        .and_then(Value::as_array)
        .map(|a| a.iter().map(|o| (o.get("owner_id").and_then(Value::as_str).unwrap_or("").to_string(), num(o, "score"), num(o, "rank"))).collect())
        .unwrap_or_default())
}
