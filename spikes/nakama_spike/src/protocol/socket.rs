//! The realtime socket envelope, JSON format (`ws://<host>/ws?token=<session token>&format=json`). One struct with one optional
//! field per message kind, which is how the server's JSON envelope is shaped; unknown fields are ignored so the spike only models
//! what it uses. Nakama writes `int64` fields (`op_code`) as JSON strings and match data as base64; both are handled here.

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde::{Deserialize, Deserializer, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Presence {
    #[serde(default)]
    pub user_id: String,
    #[serde(default)]
    pub session_id: String,
    #[serde(default)]
    pub username: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MatchCreate {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MatchJoin {
    pub match_id: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MatchLeave {
    pub match_id: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Match {
    pub match_id: String,
    #[serde(default)]
    pub authoritative: bool,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub size: i32,
    #[serde(default)]
    pub presences: Vec<Presence>,
    #[serde(rename = "self", default)]
    pub me: Presence,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MatchPresenceEvent {
    pub match_id: String,
    #[serde(default)]
    pub joins: Vec<Presence>,
    #[serde(default)]
    pub leaves: Vec<Presence>,
}

/// Incoming relayed data.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct MatchData {
    pub match_id: String,
    #[serde(default)]
    pub presence: Option<Presence>,
    #[serde(default, deserialize_with = "int64")]
    pub op_code: i64,
    /// Base64 on the wire; use [`MatchData::bytes`].
    #[serde(default)]
    pub data: String,
    #[serde(default)]
    pub reliable: bool,
}

impl MatchData {
    pub fn bytes(&self) -> Vec<u8> {
        STANDARD.decode(&self.data).unwrap_or_default()
    }
}

/// Outgoing relayed data. An empty `presences` list means everyone in the match; otherwise only those recipients.
#[derive(Debug, Clone, Default, Serialize)]
pub struct MatchDataSend {
    pub match_id: String,
    pub op_code: String,
    pub data: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub presences: Vec<Presence>,
    pub reliable: bool,
}

impl MatchDataSend {
    pub fn new(match_id: &str, op_code: i64, payload: &[u8], recipients: Vec<Presence>) -> Self {
        MatchDataSend {
            match_id: match_id.to_string(),
            op_code: op_code.to_string(),
            data: STANDARD.encode(payload),
            presences: recipients,
            reliable: true,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Empty {}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SocketError {
    #[serde(default)]
    pub code: i32,
    #[serde(default)]
    pub message: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Envelope {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub cid: Option<String>,

    // client -> server
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub match_create: Option<MatchCreate>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub match_join: Option<MatchJoin>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub match_leave: Option<MatchLeave>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub ping: Option<Empty>,
    #[serde(skip_serializing_if = "Option::is_none", skip_deserializing)]
    pub match_data_send: Option<MatchDataSend>,

    // server -> client
    #[serde(rename = "match", skip_serializing_if = "Option::is_none", default)]
    pub matched: Option<Match>,
    #[serde(skip_serializing, default)]
    pub match_data: Option<MatchData>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub match_presence_event: Option<MatchPresenceEvent>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub pong: Option<Empty>,
    #[serde(skip_serializing, default)]
    pub error: Option<SocketError>,
}

impl Envelope {
    pub fn match_create(cid: &str, name: Option<&str>) -> Self {
        Envelope { cid: Some(cid.into()), match_create: Some(MatchCreate { name: name.map(str::to_string) }), ..Default::default() }
    }
    pub fn match_join(cid: &str, match_id: &str) -> Self {
        Envelope { cid: Some(cid.into()), match_join: Some(MatchJoin { match_id: match_id.into() }), ..Default::default() }
    }
    pub fn match_leave(cid: &str, match_id: &str) -> Self {
        Envelope { cid: Some(cid.into()), match_leave: Some(MatchLeave { match_id: match_id.into() }), ..Default::default() }
    }
    pub fn ping(cid: &str) -> Self {
        Envelope { cid: Some(cid.into()), ping: Some(Empty {}), ..Default::default() }
    }
    pub fn data(msg: MatchDataSend) -> Self {
        Envelope { match_data_send: Some(msg), ..Default::default() }
    }
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("an envelope always serializes")
    }
    pub fn parse(text: &str) -> Result<Envelope, String> {
        serde_json::from_str(text).map_err(|e| format!("bad envelope ({e}): {}", text.chars().take(200).collect::<String>()))
    }
    /// A short label of what this envelope is, for logs (never prints payloads or tokens).
    pub fn kind(&self) -> &'static str {
        if self.error.is_some() {
            "error"
        } else if self.matched.is_some() {
            "match"
        } else if self.match_data.is_some() {
            "match_data"
        } else if self.match_presence_event.is_some() {
            "match_presence_event"
        } else if self.pong.is_some() {
            "pong"
        } else if self.cid.is_some() {
            "ack"
        } else {
            "other"
        }
    }
}

/// `int64` arrives as a string in the JSON envelope but may be a number: accept both.
fn int64<'de, D: Deserializer<'de>>(d: D) -> Result<i64, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum N {
        S(String),
        I(i64),
    }
    match N::deserialize(d)? {
        N::S(s) => s.parse().map_err(serde::de::Error::custom),
        N::I(i) => Ok(i),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn int64_string_and_base64() {
        let e = Envelope::parse(r#"{"match_data":{"match_id":"m.n","op_code":"7","data":"aGk=","reliable":true}}"#).unwrap();
        let d = e.match_data.unwrap();
        assert_eq!(d.op_code, 7);
        assert_eq!(d.bytes(), b"hi");
    }

    #[test]
    fn create_serializes_without_nulls() {
        assert_eq!(Envelope::match_create("1", None).to_json(), r#"{"cid":"1","match_create":{}}"#);
        assert_eq!(Envelope::match_create("1", Some("x")).to_json(), r#"{"cid":"1","match_create":{"name":"x"}}"#);
    }

    #[test]
    fn send_is_a_string_op_code() {
        let m = MatchDataSend::new("m.n", 3, b"hi", vec![]);
        assert_eq!(Envelope::data(m).to_json(), r#"{"match_data_send":{"match_id":"m.n","op_code":"3","data":"aGk=","reliable":true}}"#);
    }
}
