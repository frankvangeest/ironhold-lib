//! Nakama adapter for Ironhold's online backend contract (`planning/features/backend_abstraction.md`, v2).
//!
//! Request/response only (sign-in, leaderboard submit and list): every call is an `ehttp` request whose callback pushes the
//! answer into a channel that `poll()` drains. No thread of our own, no async runtime, the same code on native and wasm32.
//! The realtime socket (matchmaker, reconnect) is v3.
//!
//! The runner builds a [`NakamaOptions`] (server key and a per-install device uuid, which only the runner can persist) and
//! hands [`factory`] to `AppOptions::backend_factory`; the engine calls it once `backend.ron` resolves.

mod ns;
mod protocol;

use std::collections::BTreeMap;
use std::sync::mpsc::{channel, Receiver, Sender};

use ironhold_core::runtime::backend::{
    Backend, BackendEvent, BackendFactory, BackendReply, BackendRequest, LeaderRow, RequestKind, ResolvedBackendConfig,
};

pub use ns::{GameNs, NsLeaderboardId};
use protocol::{Failure, Session};

/// What the runner supplies: not part of `backend.ron` (the server key is a client identifier but still per deployment,
/// and the device uuid is per install).
#[derive(Debug, Clone)]
pub struct NakamaOptions {
    pub server_key: String,
    /// A stable uuid for this install; the adapter derives the per-game device id `{game_id}:{uuid}` from it.
    pub device_uuid: String,
}

/// The factory a runner puts in `AppOptions::backend_factory`.
pub fn factory(options: NakamaOptions) -> BackendFactory {
    BackendFactory::new(move |resolved| NakamaBackend::new(resolved, &options).map(|b| Box::new(b) as Box<dyn Backend>))
}

/// Where a request stands.
enum Stage {
    SignIn,
    /// The real call; `retried` once a refresh already happened.
    Call { retried: bool },
    /// Refreshing the session after a 401, then the call is retried.
    Refresh,
}

struct Pending {
    kind: RequestKind,
    stage: Stage,
}

struct Answer {
    id: u64,
    result: Result<ehttp::Response, String>,
}

pub struct NakamaBackend {
    base: String,
    server_key: String,
    ns: GameNs,
    device_id: String,
    session: Option<Session>,
    pending: BTreeMap<u64, Pending>,
    /// Replies that need no network (sign-out, refusals), delivered by the next `poll`.
    ready: Vec<BackendEvent>,
    tx: Sender<Answer>,
    rx: Receiver<Answer>,
}

impl NakamaBackend {
    pub fn new(resolved: &ResolvedBackendConfig, options: &NakamaOptions) -> Result<Self, String> {
        let base = resolved.endpoint.trim_end_matches('/').to_string();
        if !(base.starts_with("http://") || base.starts_with("https://")) {
            return Err(format!("endpoint {base:?} is not an http(s) URL"));
        }
        if options.server_key.trim().is_empty() {
            return Err("no Nakama server key was provided".to_string());
        }
        let ns = GameNs::new(&resolved.game_id)?;
        // Checked here so a bad declaration fails at startup, not at the first submit.
        for lb in &resolved.leaderboards {
            ns.leaderboard(&lb.name)?;
        }
        let device_id = ns.device_id(&options.device_uuid)?;
        let (tx, rx) = channel();
        Ok(Self {
            base,
            server_key: options.server_key.trim().to_string(),
            ns,
            device_id,
            session: None,
            pending: BTreeMap::new(),
            ready: Vec::new(),
            tx,
            rx,
        })
    }

    fn send(&self, id: u64, request: ehttp::Request) {
        let tx = self.tx.clone();
        ehttp::fetch(request, move |result| {
            let _ = tx.send(Answer { id, result });
        });
    }

    fn reply(&mut self, id: u64, reply: BackendReply) {
        self.ready.push(BackendEvent { request_id: id, reply });
    }

    /// The authenticated call for a request, or the refusal reply when there is no session or the id is unusable.
    fn call(&self, kind: &RequestKind) -> Result<ehttp::Request, BackendReply> {
        let Some(session) = &self.session else {
            return Err(kind.failure("not signed in"));
        };
        match kind {
            RequestKind::SubmitScore { leaderboard, score } => {
                let board = self.ns.leaderboard(leaderboard).map_err(|e| kind.failure(&e))?;
                Ok(protocol::submit_score(&self.base, &session.token, &board, *score))
            }
            RequestKind::ListLeaderboard { leaderboard, count } => {
                let board = self.ns.leaderboard(leaderboard).map_err(|e| kind.failure(&e))?;
                Ok(protocol::list_leaderboard(&self.base, &session.token, &board, *count, &session.user_id))
            }
            RequestKind::SignIn | RequestKind::SignOut => Err(kind.failure("not a call")),
        }
    }

    fn start_call(&mut self, id: u64, kind: RequestKind, retried: bool) {
        match self.call(&kind) {
            Ok(req) => {
                self.send(id, req);
                self.pending.insert(id, Pending { kind, stage: Stage::Call { retried } });
            }
            Err(reply) => self.reply(id, reply),
        }
    }

    fn handle(&mut self, answer: Answer) {
        let Some(Pending { kind, stage }) = self.pending.remove(&answer.id) else { return };
        let id = answer.id;
        let resp = match answer.result {
            Ok(r) => r,
            Err(_) => return self.reply(id, unreachable_reply(&kind)),
        };
        match stage {
            Stage::SignIn => match protocol::parse_session(&resp) {
                Ok(session) => {
                    let display_name = if session.username.is_empty() { "Player".to_string() } else { session.username.clone() };
                    self.session = Some(session);
                    self.reply(id, BackendReply::SignedIn { display_name });
                }
                Err(f) => self.reply(id, BackendReply::SignInFailed { reason: reason(f) }),
            },
            Stage::Refresh => match protocol::parse_session(&resp) {
                Ok(session) => {
                    self.session = Some(session);
                    self.start_call(id, kind, true);
                }
                Err(_) => {
                    self.session = None;
                    self.reply(id, kind.failure("the session expired, sign in again"));
                }
            },
            Stage::Call { retried } => {
                let result = match &kind {
                    RequestKind::SubmitScore { leaderboard, .. } => protocol::parse_submit(&resp)
                        .map(|()| BackendReply::ScoreSubmitted { leaderboard: leaderboard.clone() }),
                    RequestKind::ListLeaderboard { leaderboard, .. } => protocol::parse_listing(&resp).map(|l| BackendReply::Leaderboard {
                        leaderboard: leaderboard.clone(),
                        rows: l.rows.into_iter().map(|(name, score)| LeaderRow { name, score }).collect(),
                        my_rank: l.my_rank,
                        my_best: l.my_best,
                    }),
                    RequestKind::SignIn | RequestKind::SignOut => return,
                };
                match result {
                    Ok(reply) => self.reply(id, reply),
                    Err(Failure::Unauthorized) if !retried => match self.session.clone() {
                        Some(s) => {
                            self.send(id, protocol::refresh_session(&self.base, &self.server_key, &s.refresh_token));
                            self.pending.insert(id, Pending { kind, stage: Stage::Refresh });
                        }
                        None => self.reply(id, kind.failure("not signed in")),
                    },
                    Err(f) => self.reply(id, kind.failure(&reason(f))),
                }
            }
        }
    }
}

fn reason(f: Failure) -> String {
    match f {
        Failure::Unauthorized => "the session expired, sign in again".to_string(),
        Failure::Other(text) => text,
    }
}

/// A transport error is "could not reach the server" for every kind of request.
fn unreachable_reply(kind: &RequestKind) -> BackendReply {
    match kind {
        RequestKind::SignIn | RequestKind::SignOut => BackendReply::Unreachable { reason: "server unreachable".to_string() },
        other => other.failure("server unreachable"),
    }
}

impl Backend for NakamaBackend {
    fn submit(&mut self, req: BackendRequest) {
        match req.kind {
            RequestKind::SignIn => {
                let request = protocol::authenticate_device(&self.base, &self.server_key, &self.device_id);
                self.send(req.id, request);
                self.pending.insert(req.id, Pending { kind: RequestKind::SignIn, stage: Stage::SignIn });
            }
            RequestKind::SignOut => {
                // Best effort on the server; locally the player is signed out right away.
                if let Some(session) = self.session.take() {
                    ehttp::fetch(protocol::logout(&self.base, &session), |_| {});
                }
                self.reply(req.id, BackendReply::SignedOut);
            }
            kind => self.start_call(req.id, kind, false),
        }
    }

    fn poll(&mut self, out: &mut Vec<BackendEvent>) {
        while let Ok(answer) = self.rx.try_recv() {
            self.handle(answer);
        }
        out.append(&mut self.ready);
    }
}
