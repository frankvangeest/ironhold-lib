//! Nakama REST as pure request builders and response parsers (ported from the throwaway spike, tag `spike/nakama_backend`).
//! No I/O: `lib.rs` sends the `ehttp::Request` and hands the `ehttp::Response` back, so the same code runs on both targets.

use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use base64::Engine;
use serde_json::{json, Value};

use crate::ns::NsLeaderboardId;

fn basic_auth(server_key: &str) -> String {
    format!("Basic {}", STANDARD.encode(format!("{server_key}:")))
}

fn post(url: String, authorization: String, body: Value) -> ehttp::Request {
    let mut req = ehttp::Request::post(url, body.to_string().into_bytes());
    req.headers.insert("Content-Type", "application/json");
    req.headers.insert("Authorization", authorization);
    req
}

#[derive(Debug, Clone, PartialEq)]
pub struct Session {
    pub token: String,
    pub refresh_token: String,
    pub user_id: String,
    pub username: String,
}

/// The payload of a session JWT: `(user id, username)`. Nakama puts both in the token, so no account call is needed.
fn claims(token: &str) -> Option<(String, String)> {
    let payload = token.split('.').nth(1)?;
    let json: Value = serde_json::from_slice(&URL_SAFE_NO_PAD.decode(payload).ok()?).ok()?;
    let s = |k: &str| json.get(k).and_then(Value::as_str).map(str::to_string);
    Some((s("uid")?, s("usn").unwrap_or_default()))
}

pub fn authenticate_device(base: &str, server_key: &str, device_id: &str) -> ehttp::Request {
    post(
        format!("{base}/v2/account/authenticate/device?create=true"),
        basic_auth(server_key),
        json!({ "id": device_id }),
    )
}

pub fn refresh_session(base: &str, server_key: &str, refresh_token: &str) -> ehttp::Request {
    post(format!("{base}/v2/account/session/refresh"), basic_auth(server_key), json!({ "token": refresh_token }))
}

/// Best effort: tells the server to drop the session. The caller ignores the answer.
pub fn logout(base: &str, session: &Session) -> ehttp::Request {
    post(
        format!("{base}/v2/session/logout"),
        format!("Bearer {}", session.token),
        json!({ "token": session.token, "refresh_token": session.refresh_token }),
    )
}

pub fn submit_score(base: &str, token: &str, board: &NsLeaderboardId, score: i64) -> ehttp::Request {
    // The JSON API takes an int64 as a string.
    post(format!("{base}/v2/leaderboard/{}", board.as_str()), format!("Bearer {token}"), json!({ "score": score.to_string() }))
}

pub fn list_leaderboard(base: &str, token: &str, board: &NsLeaderboardId, limit: u32, user_id: &str) -> ehttp::Request {
    let mut req = ehttp::Request::get(format!("{base}/v2/leaderboard/{}?limit={limit}&owner_ids={user_id}", board.as_str()));
    req.headers.insert("Authorization", format!("Bearer {token}"));
    req
}

/// The records around one owner, whose first record carries the owner's rank (`owner_records` of a plain list omit it).
pub fn rank_around_owner(base: &str, token: &str, board: &NsLeaderboardId, user_id: &str) -> ehttp::Request {
    let mut req = ehttp::Request::get(format!("{base}/v2/leaderboard/{}/owner/{user_id}?limit=1", board.as_str()));
    req.headers.insert("Authorization", format!("Bearer {token}"));
    req
}

/// A server answer, boiled down to what the adapter acts on.
#[derive(Debug, PartialEq)]
pub enum Failure {
    /// The server rejected the session token (expired or revoked): refresh and retry once.
    Unauthorized,
    /// A short, screen-sized reason (also used for `backend.last_error`).
    Other(String),
}

fn short(text: &str) -> String {
    let t: String = text.chars().take(80).collect();
    t.trim().to_string()
}

/// `Ok(json)` for a 2xx answer, otherwise the failure. Nakama errors are `{"code":..,"message":".."}`.
fn ok_json(resp: &ehttp::Response) -> Result<Value, Failure> {
    let text = resp.text().unwrap_or("");
    if resp.status == 401 {
        return Err(Failure::Unauthorized);
    }
    if !(200..300).contains(&resp.status) {
        let msg = serde_json::from_str::<Value>(text)
            .ok()
            .and_then(|v| v.get("message").and_then(Value::as_str).map(short))
            .filter(|m| !m.is_empty())
            .unwrap_or_else(|| format!("server answered HTTP {}", resp.status));
        return Err(Failure::Other(msg));
    }
    serde_json::from_str(text).map_err(|_| Failure::Other("the server sent an unreadable answer".to_string()))
}

pub fn parse_session(resp: &ehttp::Response) -> Result<Session, Failure> {
    // A sign-in or refresh with a bad key or token is a rejection, never "refresh and retry".
    let v = ok_json(resp).map_err(|f| match f {
        Failure::Unauthorized => Failure::Other("the server rejected the sign-in".to_string()),
        other => other,
    })?;
    let get = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
    let (Some(token), Some(refresh_token)) = (get("token"), get("refresh_token")) else {
        return Err(Failure::Other("the server sent no session".to_string()));
    };
    let (user_id, username) = claims(&token).ok_or_else(|| Failure::Other("the session token is unreadable".to_string()))?;
    Ok(Session { token, refresh_token, user_id, username })
}

pub fn parse_submit(resp: &ehttp::Response) -> Result<(), Failure> {
    ok_json(resp).map(|_| ())
}

#[derive(Debug, PartialEq)]
pub struct Listing {
    pub rows: Vec<(String, i64)>,
    pub my_rank: Option<u32>,
    pub my_best: Option<i64>,
    /// The player has a record but is not in the rows and the list carried no rank: ask `rank_around_owner`.
    pub needs_rank: bool,
}

fn num(o: &Value, key: &str) -> Option<i64> {
    match o.get(key) {
        Some(Value::String(s)) => s.parse().ok(),
        Some(n) => n.as_i64(),
        None => None,
    }
}

pub fn parse_listing(resp: &ehttp::Response, user_id: &str) -> Result<Listing, Failure> {
    let v = ok_json(resp)?;
    let array = |key: &str| v.get(key).and_then(Value::as_array).cloned().unwrap_or_default();
    let rows = array("records")
        .iter()
        .map(|o| {
            let name = o.get("username").and_then(Value::as_str).filter(|n| !n.is_empty()).unwrap_or("?").to_string();
            (name, num(o, "score").unwrap_or(0))
        })
        .collect();
    let owner_records = array("owner_records");
    let mine = owner_records.first();
    // The rank sits on the player's row when they are in the top list; `owner_records` themselves carry none.
    let in_rows = array("records").iter().find(|o| o.get("owner_id").and_then(Value::as_str) == Some(user_id)).and_then(|o| num(o, "rank"));
    let my_rank = in_rows.or_else(|| mine.and_then(|o| num(o, "rank"))).and_then(|r| u32::try_from(r).ok());
    Ok(Listing { rows, my_rank, my_best: mine.and_then(|o| num(o, "score")), needs_rank: mine.is_some() && my_rank.is_none() })
}

/// The player's rank from a `rank_around_owner` answer.
pub fn parse_rank(resp: &ehttp::Response, user_id: &str) -> Option<u32> {
    let v = ok_json(resp).ok()?;
    let records = v.get("records")?.as_array()?;
    let mine = records.iter().find(|o| o.get("owner_id").and_then(Value::as_str) == Some(user_id))?;
    u32::try_from(num(mine, "rank")?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resp(status: u16, body: &str) -> ehttp::Response {
        ehttp::Response {
            url: String::new(),
            ok: (200..300).contains(&status),
            status,
            status_text: String::new(),
            headers: Default::default(),
            bytes: body.as_bytes().to_vec(),
        }
    }

    pub fn token(uid: &str, usn: &str) -> String {
        let payload = URL_SAFE_NO_PAD.encode(json!({ "uid": uid, "usn": usn, "exp": 1 }).to_string());
        format!("h.{payload}.s")
    }

    #[test]
    fn a_session_carries_the_user_id_and_name_from_the_token() {
        let body = json!({ "token": token("u1", "Brave Otter"), "refresh_token": "r", "created": true }).to_string();
        let s = parse_session(&resp(200, &body)).unwrap();
        assert_eq!((s.user_id.as_str(), s.username.as_str(), s.refresh_token.as_str()), ("u1", "Brave Otter", "r"));
    }

    #[test]
    fn a_rejected_sign_in_is_a_failure_not_a_refresh() {
        let f = parse_session(&resp(401, r#"{"code":16,"message":"Server key invalid"}"#)).unwrap_err();
        assert!(matches!(f, Failure::Other(_)));
        assert_eq!(parse_submit(&resp(401, r#"{"message":"Auth token invalid"}"#)).unwrap_err(), Failure::Unauthorized);
    }

    #[test]
    fn server_messages_become_short_reasons() {
        let f = parse_submit(&resp(404, r#"{"code":5,"message":"Leaderboard not found."}"#)).unwrap_err();
        assert_eq!(f, Failure::Other("Leaderboard not found.".to_string()));
        let f = parse_submit(&resp(500, "<html>")).unwrap_err();
        assert_eq!(f, Failure::Other("server answered HTTP 500".to_string()));
    }

    #[test]
    fn a_listing_reads_string_numbers_and_my_record() {
        let body = json!({
            "records": [{"owner_id": "me", "username": "Ada", "score": "900", "rank": "1"}, {"username": "", "score": 5}],
            "owner_records": [{"owner_id": "me", "score": "900"}]
        })
        .to_string();
        let l = parse_listing(&resp(200, &body), "me").unwrap();
        assert_eq!(l.rows, vec![("Ada".to_string(), 900), ("?".to_string(), 5)]);
        assert_eq!((l.my_rank, l.my_best, l.needs_rank), (Some(1), Some(900), false));
        let empty = parse_listing(&resp(200, "{}"), "me").unwrap();
        assert_eq!((empty.rows.len(), empty.my_rank, empty.my_best, empty.needs_rank), (0, None, None, false));
    }

    #[test]
    fn a_player_outside_the_top_list_needs_a_rank_lookup() {
        let body = json!({ "records": [{"owner_id": "x", "score": "9", "rank": "1"}], "owner_records": [{"owner_id": "me", "score": "2"}] }).to_string();
        let l = parse_listing(&resp(200, &body), "me").unwrap();
        assert_eq!((l.my_rank, l.my_best, l.needs_rank), (None, Some(2), true));
        let around = json!({ "records": [{"owner_id": "me", "score": "2", "rank": "42"}] }).to_string();
        assert_eq!(parse_rank(&resp(200, &around), "me"), Some(42));
        assert_eq!(parse_rank(&resp(404, "{}"), "me"), None);
    }
}
