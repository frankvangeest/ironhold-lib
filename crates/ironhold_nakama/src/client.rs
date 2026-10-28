//! The adapter: an `ehttp`-backed poll backend (see the crate docs).
//!
//! Every engine request becomes a small chain of HTTP calls (the call, a session renewal after a 401, the retried call, a rank
//! lookup). The chain lives in `pending`, keyed by the engine's request id, and ends in exactly one reply. Three rules keep
//! the adapter and the engine's view of the session from drifting apart:
//! - `SignOut` answers every request still in flight with a failure ("signed out") and forgets them, so a late sign-in,
//!   refresh or leaderboard answer can never bring the old session or the old player's board back;
//! - the session is only ever dropped by `SignOut`; a failed renewal re-authenticates with the device id, and if that also
//!   fails the call fails but the next call tries the whole chain again;
//! - `cancel` (the engine timed the request out) removes the chain, so no retried write happens after `score.submit_failed`.

use std::collections::BTreeMap;
use std::sync::mpsc::{channel, Receiver, Sender};

use ironhold_core::runtime::backend::{
    Backend, BackendEvent, BackendFactory, BackendReply, BackendRequest, LeaderRow, RequestKind, ResolvedBackendConfig,
};

use crate::ns::GameNs;
use crate::protocol::{self, Failure, Session};

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

/// Engine request ids start at 1; 0 is the one shared session renewal (refresh, then re-authenticate).
const RENEWAL_ID: u64 = 0;

/// Where a request stands.
enum Stage {
    SignIn,
    /// The real call; `retried` once a session renewal already happened.
    Call { retried: bool },
    /// The list is parsed but the player's rank is missing: this answer is the rank lookup. Any failure delivers the list as is.
    Rank { listing: BackendReply },
    /// Renewing the session with the refresh token.
    Refresh,
    /// The refresh token was refused: signing in again with the device id.
    Reauth,
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
    /// Calls that got a 401 and wait for the one shared session renewal.
    waiting: Vec<(u64, RequestKind)>,
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
            waiting: Vec::new(),
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

    fn authenticate(&mut self, id: u64, kind: RequestKind, stage: Stage) {
        self.send(id, protocol::authenticate_device(&self.base, &self.server_key, &self.device_id));
        self.pending.insert(id, Pending { kind, stage });
    }

    /// A call needs a fresh session: join the one renewal in flight, or start it (refresh token first, device id if that fails).
    fn renew_for(&mut self, id: u64, kind: RequestKind) {
        self.waiting.push((id, kind));
        if self.pending.contains_key(&RENEWAL_ID) {
            return;
        }
        match self.session.as_ref().map(|s| s.refresh_token.clone()) {
            Some(refresh) => {
                self.send(RENEWAL_ID, protocol::refresh_session(&self.base, &self.server_key, &refresh));
                self.pending.insert(RENEWAL_ID, Pending { kind: RequestKind::SignIn, stage: Stage::Refresh });
            }
            None => self.authenticate(RENEWAL_ID, RequestKind::SignIn, Stage::Reauth),
        }
    }

    fn resume_waiting(&mut self) {
        for (id, kind) in std::mem::take(&mut self.waiting) {
            self.start_call(id, kind, true);
        }
    }

    fn fail_waiting(&mut self, reason: &str) {
        for (id, kind) in std::mem::take(&mut self.waiting) {
            self.reply(id, kind.failure(reason));
        }
    }

    fn handle_renewal(&mut self, stage: Stage, result: Result<ehttp::Response, String>) {
        let resp = match result {
            Ok(r) => r,
            // The server could not be reached: keep the session, fail the waiting calls, a later call tries again.
            Err(_) => return self.fail_waiting("server unreachable"),
        };
        match (stage, protocol::parse_session(&resp)) {
            (_, Ok(session)) => {
                self.session = Some(session);
                self.resume_waiting();
            }
            // The refresh token is no longer good (expired, rotated): the device id is the real credential.
            (Stage::Refresh, Err(_)) => self.authenticate(RENEWAL_ID, RequestKind::SignIn, Stage::Reauth),
            (_, Err(f)) => self.fail_waiting(&reason(f)),
        }
    }

    fn handle(&mut self, answer: Answer) {
        let id = answer.id;
        let Some(Pending { kind, stage }) = self.pending.remove(&id) else { return };
        if id == RENEWAL_ID {
            return self.handle_renewal(stage, answer.result);
        }
        let resp = match answer.result {
            Ok(r) => r,
            // The rank is a nicety: the list itself already succeeded.
            Err(_) => {
                return match stage {
                    Stage::Rank { listing } => self.reply(id, listing),
                    _ => self.reply(id, unreachable_reply(&kind)),
                };
            }
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
            Stage::Refresh | Stage::Reauth => {}
            Stage::Rank { listing } => {
                let rank = self.session.as_ref().and_then(|s| protocol::parse_rank(&resp, &s.user_id));
                let reply = match listing {
                    BackendReply::Leaderboard { leaderboard, rows, my_best, .. } => {
                        BackendReply::Leaderboard { leaderboard, rows, my_rank: rank, my_best }
                    }
                    other => other,
                };
                self.reply(id, reply);
            }
            Stage::Call { retried } => {
                let result = match &kind {
                    RequestKind::SubmitScore { leaderboard, .. } => protocol::parse_submit(&resp)
                        .map(|()| BackendReply::ScoreSubmitted { leaderboard: leaderboard.clone() }),
                    RequestKind::ListLeaderboard { leaderboard, .. } => {
                        let user_id = self.session.as_ref().map(|s| s.user_id.clone()).unwrap_or_default();
                        match protocol::parse_listing(&resp, &user_id) {
                            Ok(l) => {
                                let reply = BackendReply::Leaderboard {
                                    leaderboard: leaderboard.clone(),
                                    rows: l.rows.into_iter().map(|(name, score)| LeaderRow { name, score }).collect(),
                                    my_rank: l.my_rank,
                                    my_best: l.my_best,
                                };
                                if l.needs_rank {
                                    if let (Some(s), Ok(board)) = (self.session.clone(), self.ns.leaderboard(leaderboard)) {
                                        self.send(id, protocol::rank_around_owner(&self.base, &s.token, &board, &s.user_id));
                                        self.pending.insert(id, Pending { kind, stage: Stage::Rank { listing: reply } });
                                        return;
                                    }
                                }
                                Ok(reply)
                            }
                            Err(f) => Err(f),
                        }
                    }
                    RequestKind::SignIn | RequestKind::SignOut => return,
                };
                match result {
                    Ok(reply) => self.reply(id, reply),
                    Err(Failure::Unauthorized) if !retried => self.renew_for(id, kind),
                    Err(f) => self.reply(id, kind.failure(&reason(f))),
                }
            }
        }
    }

    /// Answers everything still in flight with a failure and forgets it (sign-out).
    fn cancel_all(&mut self, why: &str) {
        for (id, p) in std::mem::take(&mut self.pending) {
            if id == RENEWAL_ID {
                continue;
            }
            let reply = match p.kind {
                RequestKind::SignIn => BackendReply::SignInFailed { reason: why.to_string() },
                other => other.failure(why),
            };
            self.reply(id, reply);
        }
        self.fail_waiting(why);
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
            RequestKind::SignIn => self.authenticate(req.id, RequestKind::SignIn, Stage::SignIn),
            RequestKind::SignOut => {
                self.cancel_all("signed out");
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

    fn cancel(&mut self, request_id: u64) {
        self.pending.remove(&request_id);
        self.waiting.retain(|(id, _)| *id != request_id);
    }
}
