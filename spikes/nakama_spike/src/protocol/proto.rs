//! Protobuf envelope (`?format=protobuf`, binary WebSocket frames): the subset of `nakama.realtime` the spike uses, as `prost`
//! derive structs with the field numbers of heroiclabs/nakama-common `rtapi/realtime.proto`. Hand-written instead of generated so
//! no `protoc` is needed; it is what `prost-build` would emit for these messages. Unknown fields are skipped on decode.
//! Behind the `proto` feature so the JSON-only build can be size-compared against it.

use prost::Message;

#[derive(Clone, PartialEq, Message)]
pub struct UserPresence {
    #[prost(string, tag = "1")]
    pub user_id: String,
    #[prost(string, tag = "2")]
    pub session_id: String,
    #[prost(string, tag = "3")]
    pub username: String,
    #[prost(bool, tag = "4")]
    pub persistence: bool,
}

#[derive(Clone, PartialEq, Message)]
pub struct MatchCreate {
    #[prost(string, tag = "1")]
    pub name: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct MatchJoin {
    #[prost(oneof = "match_join::Id", tags = "1, 2")]
    pub id: Option<match_join::Id>,
}
pub mod match_join {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Id {
        #[prost(string, tag = "1")]
        MatchId(String),
        #[prost(string, tag = "2")]
        Token(String),
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct MatchLeave {
    #[prost(string, tag = "1")]
    pub match_id: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct Match {
    #[prost(string, tag = "1")]
    pub match_id: String,
    #[prost(bool, tag = "2")]
    pub authoritative: bool,
    #[prost(int32, tag = "4")]
    pub size: i32,
    #[prost(message, repeated, tag = "5")]
    pub presences: Vec<UserPresence>,
    #[prost(message, optional, tag = "6")]
    pub me: Option<UserPresence>,
}

#[derive(Clone, PartialEq, Message)]
pub struct MatchData {
    #[prost(string, tag = "1")]
    pub match_id: String,
    #[prost(message, optional, tag = "2")]
    pub presence: Option<UserPresence>,
    #[prost(int64, tag = "3")]
    pub op_code: i64,
    #[prost(bytes = "vec", tag = "4")]
    pub data: Vec<u8>,
    #[prost(bool, tag = "5")]
    pub reliable: bool,
}

#[derive(Clone, PartialEq, Message)]
pub struct MatchDataSend {
    #[prost(string, tag = "1")]
    pub match_id: String,
    #[prost(int64, tag = "2")]
    pub op_code: i64,
    #[prost(bytes = "vec", tag = "3")]
    pub data: Vec<u8>,
    #[prost(message, repeated, tag = "4")]
    pub presences: Vec<UserPresence>,
    #[prost(bool, tag = "5")]
    pub reliable: bool,
}

#[derive(Clone, PartialEq, Message)]
pub struct MatchPresenceEvent {
    #[prost(string, tag = "1")]
    pub match_id: String,
    #[prost(message, repeated, tag = "2")]
    pub joins: Vec<UserPresence>,
    #[prost(message, repeated, tag = "3")]
    pub leaves: Vec<UserPresence>,
}

#[derive(Clone, PartialEq, Message)]
pub struct Ping {}
#[derive(Clone, PartialEq, Message)]
pub struct Pong {}

#[derive(Clone, PartialEq, Message)]
pub struct Error {
    #[prost(int32, tag = "1")]
    pub code: i32,
    #[prost(string, tag = "2")]
    pub message: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct Envelope {
    #[prost(string, tag = "1")]
    pub cid: String,
    #[prost(oneof = "envelope::Message", tags = "11, 12, 13, 14, 15, 16, 17, 18, 32, 33")]
    pub message: Option<envelope::Message>,
}
pub mod envelope {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Message {
        #[prost(message, tag = "11")]
        Error(super::Error),
        #[prost(message, tag = "12")]
        Match(super::Match),
        #[prost(message, tag = "13")]
        MatchCreate(super::MatchCreate),
        #[prost(message, tag = "14")]
        MatchData(super::MatchData),
        #[prost(message, tag = "15")]
        MatchDataSend(super::MatchDataSend),
        #[prost(message, tag = "16")]
        MatchJoin(super::MatchJoin),
        #[prost(message, tag = "17")]
        MatchLeave(super::MatchLeave),
        #[prost(message, tag = "18")]
        MatchPresenceEvent(super::MatchPresenceEvent),
        #[prost(message, tag = "32")]
        Ping(super::Ping),
        #[prost(message, tag = "33")]
        Pong(super::Pong),
    }
}

impl Envelope {
    pub fn with(cid: &str, message: envelope::Message) -> Self {
        Envelope { cid: cid.into(), message: Some(message) }
    }
    pub fn match_create(cid: &str) -> Self {
        Self::with(cid, envelope::Message::MatchCreate(MatchCreate::default()))
    }
    pub fn match_join(cid: &str, match_id: &str) -> Self {
        Self::with(cid, envelope::Message::MatchJoin(MatchJoin { id: Some(match_join::Id::MatchId(match_id.into())) }))
    }
    pub fn data(m: MatchDataSend) -> Self {
        Self::with("", envelope::Message::MatchDataSend(m))
    }
    pub fn to_bytes(&self) -> Vec<u8> {
        self.encode_to_vec()
    }
    pub fn parse(bytes: &[u8]) -> Result<Envelope, String> {
        Envelope::decode(bytes).map_err(|e| format!("bad protobuf envelope: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_round_trip_and_size() {
        let m = MatchDataSend { match_id: "m.n".into(), op_code: 2, data: vec![7; 400], presences: vec![], reliable: true };
        let bytes = Envelope::data(m.clone()).to_bytes();
        // 400 raw bytes + a few bytes of framing: no base64 inflation
        assert!(bytes.len() < 420, "{}", bytes.len());
        assert_eq!(Envelope::parse(&bytes).unwrap().message, Some(envelope::Message::MatchDataSend(m)));
    }
}
