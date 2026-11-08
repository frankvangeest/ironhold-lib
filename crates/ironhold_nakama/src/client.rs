//! The adapter: an `ehttp`-backed poll backend (see the crate docs).
//!
//! Every engine request becomes a small chain of HTTP calls (the call, a session renewal after a 401, the retried call, a rank
//! lookup). The chain lives in `pending`, keyed by the engine's request id, and ends in exactly one reply. Three rules keep
//! the adapter and the engine's view of the session from drifting apart:
//! - `SignOut` answers every request still in flight with a failure ("signed out") and forgets them, so a late sign-in,
//!   refresh or leaderboard answer can never bring the old session or the old player's board back. The one exception is a lobby
//!   create, join or leave: the server may already have put the player in a lobby, so those finish with the **old session** (kept
//!   privately in `Teardown`, never in `self.session`), then the logout is sent, and a `SignIn` that arrives meanwhile is held
//!   until the logout is on the wire. `SignedOut` is still answered at once. Every step ends on its answer or on the engine's
//!   `cancel`, so a dead server cannot trap the player; a lobby it never heard about expires on the server after a minute;
//! - the session is only ever dropped by `SignOut`; a failed renewal re-authenticates with the device id, and if that also
//!   fails the call fails but the next call tries the whole chain again;
//! - `cancel` (the engine timed the request out) removes the chain, so no retried write happens after `score.submit_failed`.

use std::collections::BTreeMap;
use std::sync::mpsc::{channel, Receiver, Sender};

use ironhold_core::schema::backend::is_valid_server_key;
use ironhold_core::runtime::backend::{
    Backend, BackendEvent, BackendFactory, BackendReply, BackendRequest, LeaderRow, RequestKind, ResolvedBackendConfig,
};
use serde_json::json;

use crate::ns::GameNs;
use crate::protocol::{self, Failure, RpcOutcome, Session};

/// What the adapter needs to talk to one server: the (public) server key and this install's device secret.
#[derive(Debug, Clone)]
pub struct NakamaOptions {
    pub server_key: String,
    /// A stable secret for this install (32 lowercase hex characters, see `ns::is_valid_device_secret`). It is never sent: the
    /// adapter derives the per-game wire id `{game_id}:{hash}` from it and the game id.
    pub device_secret: String,
}

/// Produces this install's device secret, or the reason it cannot (shown on screen). Called only when a real server is about to
/// be used, so a project that runs offline never creates or stores one.
pub type DeviceSecretSource = std::sync::Arc<dyn Fn() -> Result<String, String> + Send + Sync>;

/// What a runner supplies to [`factory`]; the rest comes from `backend.ron` once it resolves.
#[derive(Clone)]
pub struct RunnerOptions {
    /// A server key from outside the project (`?nakama_key=`, `NAKAMA_SERVER_KEY`, a key file), if any. It only fills in when
    /// `backend.ron` names none, see [`resolve_server_key`].
    pub runner_key: Option<String>,
    /// Shown as `backend.last_error` when no key is found anywhere.
    pub missing_key_hint: String,
    pub device_secret: DeviceSecretSource,
}

/// Which server key to use. The file's key belongs to the file's own endpoint, so it wins; `file_key` is `None` when a
/// runner override replaced that endpoint (see `ResolvedBackendConfig::server_key`), and then only the runner's key applies.
/// Blank values and keys the adapter could not send (spaces, `:`, non-ASCII, too long: the same rule `backend.ron` is held to) count
/// as absent.
pub fn resolve_server_key(file_key: Option<&str>, runner_key: Option<&str>) -> Option<String> {
    [file_key, runner_key].into_iter().flatten().map(str::trim).find(|k| is_valid_server_key(k)).map(str::to_string)
}

/// The checks that need neither the network nor storage, so a config that can never connect does not get as far as creating a
/// device id.
fn precheck(resolved: &ResolvedBackendConfig) -> Result<(), String> {
    // Same rule as `NakamaBackend::new`, checked here too so a config that can never connect does not make the runner create
    // a device secret (and delete the old id) first.
    let base = resolved.endpoint.trim_end_matches('/');
    if !(base.starts_with("http://") || base.starts_with("https://")) {
        return Err(format!("endpoint {base:?} is not an http(s) URL"));
    }
    let ns = GameNs::new(&resolved.game_id)?;
    for lb in &resolved.leaderboards {
        ns.leaderboard(&lb.name)?;
    }
    Ok(())
}

/// The factory a runner puts in `AppOptions::backend_factory`. It resolves the key and the device secret itself, so the runner
/// does not need `backend.ron` to be loaded.
pub fn factory(options: RunnerOptions) -> BackendFactory {
    BackendFactory::new(move |resolved| {
        let Some(server_key) = resolve_server_key(resolved.server_key.as_deref(), options.runner_key.as_deref()) else {
            let supplied = options.runner_key.as_deref().is_some_and(|k| !k.trim().is_empty());
            return Err(if supplied {
                "the Nakama server key from the page address or environment is not valid (1-64 printable characters, no spaces or ':')".to_string()
            } else {
                options.missing_key_hint.clone()
            });
        };
        precheck(resolved)?;
        let device_secret = (options.device_secret)()?;
        NakamaBackend::new(resolved, &NakamaOptions { server_key, device_secret }).map(|b| Box::new(b) as Box<dyn Backend>)
    })
}

/// Engine request ids start at 1; 0 is the one shared session renewal (refresh, then re-authenticate).
const RENEWAL_ID: u64 = 0;
/// The renewal of the old session during a sign-out teardown (one refresh, never a re-authentication, never `self.session`).
const LEGACY_RENEWAL_ID: u64 = u64::MAX;

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
    /// Belongs to a session that was signed out meanwhile: uses `Teardown::session` and never touches `self.session`.
    legacy: bool,
}

impl Pending {
    fn new(kind: RequestKind, stage: Stage) -> Self {
        Self { kind, stage, legacy: false }
    }
}

/// The old session, kept only to finish the lobby requests that were in flight when the player signed out, and then to log out.
struct Teardown {
    session: Session,
    /// Lobby requests parked behind this teardown's own renewal.
    parked: Vec<(u64, RequestKind)>,
}

/// Lobby requests that must reach the server before the logout (a leave, and the create/join that may have put the player in).
fn is_lobby_chain(kind: &RequestKind) -> bool {
    matches!(kind, RequestKind::CreateLobby { .. } | RequestKind::JoinLobby { .. } | RequestKind::LeaveLobby)
}

/// A request that puts the player in a lobby: a leave waits for it to finish.
fn is_entering(kind: &RequestKind) -> bool {
    matches!(kind, RequestKind::CreateLobby { .. } | RequestKind::JoinLobby { .. })
}

fn is_lobby_kind(kind: &RequestKind) -> bool {
    matches!(
        kind,
        RequestKind::ListLobbies
            | RequestKind::CreateLobby { .. }
            | RequestKind::JoinLobby { .. }
            | RequestKind::LeaveLobby
            | RequestKind::StartLobby { .. }
            | RequestKind::LobbyHeartbeat { .. }
    )
}

/// The failure reply of a lobby request with the given fixed reason (core's own helper always writes `server`). A leave that could not
/// be confirmed still leaves, so it never fails.
fn lobby_failure(kind: &RequestKind, reason: &str) -> BackendReply {
    let reason = reason.to_string();
    match kind {
        RequestKind::ListLobbies => BackendReply::LobbyListFailed { reason },
        RequestKind::CreateLobby { .. } => BackendReply::LobbyCreateFailed { reason },
        RequestKind::JoinLobby { .. } => BackendReply::LobbyJoinFailed { reason },
        RequestKind::LeaveLobby => BackendReply::LobbyLeft,
        RequestKind::StartLobby { .. } => BackendReply::LobbyStartFailed { reason },
        RequestKind::LobbyHeartbeat { .. } => BackendReply::LobbyHeartbeatFailed { reason },
        other => other.failure(&reason),
    }
}

/// A failure that has no fixed reason of its own: lobby requests say `server`, the others carry the short text.
fn fail_reply(kind: &RequestKind, text: &str) -> BackendReply {
    if is_lobby_kind(kind) { lobby_failure(kind, "server") } else { kind.failure(text) }
}

/// What a lobby RPC's answer means for the engine.
fn lobby_reply(kind: &RequestKind, outcome: RpcOutcome) -> BackendReply {
    match (kind, outcome) {
        (RequestKind::LeaveLobby, _) => BackendReply::LobbyLeft,
        // The two reasons that end the lobby for this player; any other heartbeat refusal is trouble reaching it.
        (RequestKind::LobbyHeartbeat { .. }, RpcOutcome::Refused(r)) if r == "host_left" || r == "removed" => {
            BackendReply::LobbyClosed { reason: r }
        }
        (_, RpcOutcome::Refused(r)) => lobby_failure(kind, &r),
        (RequestKind::ListLobbies, RpcOutcome::Reply(v)) => match protocol::parse_lobby_rows(&v) {
            Some(rows) => BackendReply::LobbyListed { rows },
            None => lobby_failure(kind, "server"),
        },
        (_, RpcOutcome::Reply(v)) => match (kind, protocol::parse_lobby_info(&v)) {
            (RequestKind::CreateLobby { .. }, Some(lobby)) => BackendReply::LobbyCreated { lobby },
            (RequestKind::JoinLobby { .. }, Some(lobby)) => BackendReply::LobbyJoined { lobby },
            (RequestKind::StartLobby { .. }, Some(lobby)) => BackendReply::LobbyStarted { lobby },
            (RequestKind::LobbyHeartbeat { .. }, Some(lobby)) => BackendReply::LobbyState { lobby },
            _ => lobby_failure(kind, "server"),
        },
    }
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
    /// A leave that arrived while a create or join was on the wire: sent when none is left (core queues one before a `SignOut`
    /// even when the engine does not know yet that the join succeeded).
    held_leave: Option<u64>,
    /// Set by `SignOut` while lobby requests were still running: the old session and what it still has to finish.
    teardown: Option<Teardown>,
    /// A `SignIn` that arrived during the teardown; it starts when the logout is sent.
    held_sign_in: Option<u64>,
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
        let device_id = ns.device_id(&options.device_secret)?;
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
            held_leave: None,
            teardown: None,
            held_sign_in: None,
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
    fn call(&self, kind: &RequestKind, legacy: bool) -> Result<ehttp::Request, BackendReply> {
        let session = if legacy { self.teardown.as_ref().map(|t| &t.session) } else { self.session.as_ref() };
        let Some(session) = session else {
            return Err(if is_lobby_kind(kind) { lobby_failure(kind, "not_signed_in") } else { kind.failure("not signed in") });
        };
        let rpc = |id: &str, body: serde_json::Value| Ok(protocol::rpc(&self.base, &session.token, id, body));
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
            // The server's lobby RPCs (`ironhold.lua`, section 2d); the reply is read in `lobby_reply`.
            RequestKind::ListLobbies => rpc("ironhold_lobby_list", json!({})),
            RequestKind::CreateLobby { name, max_players } => {
                rpc("ironhold_lobby_create", json!({ "name": name, "max_players": max_players }))
            }
            RequestKind::JoinLobby { lobby_id } => rpc("ironhold_lobby_join", json!({ "lobby_id": lobby_id })),
            RequestKind::LeaveLobby => rpc("ironhold_lobby_leave", json!({})),
            RequestKind::StartLobby { lobby_id } => rpc("ironhold_lobby_start", json!({ "lobby_id": lobby_id })),
            RequestKind::LobbyHeartbeat { lobby_id } => rpc("ironhold_lobby_heartbeat", json!({ "lobby_id": lobby_id })),
        }
    }

    fn start_call(&mut self, id: u64, kind: RequestKind, retried: bool, legacy: bool) {
        match self.call(&kind, legacy) {
            Ok(req) => {
                self.send(id, req);
                self.pending.insert(id, Pending { kind, stage: Stage::Call { retried }, legacy });
            }
            Err(reply) => self.reply(id, reply),
        }
    }

    fn authenticate(&mut self, id: u64, kind: RequestKind, stage: Stage) {
        self.send(id, protocol::authenticate_device(&self.base, &self.server_key, &self.device_id));
        self.pending.insert(id, Pending::new(kind, stage));
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
                self.pending.insert(RENEWAL_ID, Pending::new(RequestKind::SignIn, Stage::Refresh));
            }
            None => self.authenticate(RENEWAL_ID, RequestKind::SignIn, Stage::Reauth),
        }
    }

    fn resume_waiting(&mut self) {
        for (id, kind) in std::mem::take(&mut self.waiting) {
            self.start_call(id, kind, true, false);
        }
    }

    /// A lobby request of the signed-out session got a 401: park it behind one refresh of the OLD session (no re-authentication: the
    /// player signed out, so the device id must not make a new session).
    fn renew_legacy(&mut self, id: u64, kind: RequestKind) {
        let Some(t) = self.teardown.as_mut() else { return self.reply(id, fail_reply(&kind, "signed out")) };
        t.parked.push((id, kind));
        self.start_legacy_renewal();
    }

    fn start_legacy_renewal(&mut self) {
        if self.pending.contains_key(&LEGACY_RENEWAL_ID) {
            return;
        }
        let Some(refresh) = self.teardown.as_ref().map(|t| t.session.refresh_token.clone()) else { return };
        self.send(LEGACY_RENEWAL_ID, protocol::refresh_session(&self.base, &self.server_key, &refresh));
        let mut renewal = Pending::new(RequestKind::SignIn, Stage::Refresh);
        renewal.legacy = true;
        self.pending.insert(LEGACY_RENEWAL_ID, renewal);
    }

    fn handle_legacy_renewal(&mut self, result: Result<ehttp::Response, String>) {
        let parked = self.teardown.as_mut().map(|t| std::mem::take(&mut t.parked)).unwrap_or_default();
        match result.ok().and_then(|r| protocol::parse_session(&r).ok()) {
            Some(session) => {
                if let Some(t) = self.teardown.as_mut() {
                    t.session = session;
                }
                for (id, kind) in parked {
                    self.start_call(id, kind, true, true);
                }
            }
            // The old session cannot be renewed: the lobby (if any) expires on the server after a minute.
            None => {
                for (id, kind) in parked {
                    self.reply(id, fail_reply(&kind, "server unreachable"));
                }
            }
        }
    }

    /// True while a create or join is on the wire or parked (the leave waits for them).
    fn entering_in_progress(&self) -> bool {
        self.pending.values().any(|p| is_entering(&p.kind))
            || self.waiting.iter().any(|(_, k)| is_entering(k))
            || self.teardown.as_ref().is_some_and(|t| t.parked.iter().any(|(_, k)| is_entering(k)))
    }

    /// Sends the held leave once no create or join is left, as part of the teardown when the player signed out meanwhile.
    fn release_held_leave(&mut self) {
        if self.held_leave.is_none() || self.entering_in_progress() {
            return;
        }
        if let Some(id) = self.held_leave.take() {
            let legacy = self.teardown.is_some();
            self.start_call(id, RequestKind::LeaveLobby, false, legacy);
        }
    }

    /// Sends the logout of the old session once every lobby request of it has ended (answered or cancelled by the engine), then
    /// starts the `SignIn` that was held meanwhile.
    fn finish_teardown(&mut self) {
        let busy = self.held_leave.is_some()
            || self.pending.values().any(|p| p.legacy)
            || self.teardown.as_ref().is_some_and(|t| !t.parked.is_empty());
        if busy {
            return;
        }
        let Some(t) = self.teardown.take() else { return };
        ehttp::fetch(protocol::logout(&self.base, &t.session), |_| {});
        if let Some(id) = self.held_sign_in.take() {
            self.authenticate(id, RequestKind::SignIn, Stage::SignIn);
        }
    }

    fn sign_out(&mut self, id: u64) {
        let old = self.session.take();
        // The lobby requests still running are not cancelled: a create or join may already have put the player in a lobby on the
        // server, so the leave core queued before this SignOut must still reach it, with the old session, before the logout.
        let lobby_work = old.is_some()
            && (self.held_leave.is_some()
                || self.pending.values().any(|p| is_lobby_chain(&p.kind))
                || self.waiting.iter().any(|(_, k)| is_lobby_chain(k)));
        let mut parked = Vec::new();
        if lobby_work {
            for p in self.pending.values_mut().filter(|p| is_lobby_chain(&p.kind)) {
                p.legacy = true;
            }
            let (keep, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut self.waiting).into_iter().partition(|(_, k)| is_lobby_chain(k));
            parked = keep;
            self.waiting = rest;
        }
        self.cancel_all("signed out");
        match old {
            Some(session) if lobby_work => {
                let has_parked = !parked.is_empty();
                self.teardown = Some(Teardown { session, parked });
                if has_parked {
                    self.start_legacy_renewal();
                }
            }
            // Best effort on the server; locally the player is signed out right away.
            Some(session) => ehttp::fetch(protocol::logout(&self.base, &session), |_| {}),
            None => {}
        }
        self.reply(id, BackendReply::SignedOut);
        self.release_held_leave();
        self.finish_teardown();
    }

    fn fail_waiting(&mut self, reason: &str) {
        for (id, kind) in std::mem::take(&mut self.waiting) {
            self.reply(id, fail_reply(&kind, reason));
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
        self.handle_answer(answer);
        // An answer may have ended the create or join a leave was waiting for, or the last request of a teardown.
        self.release_held_leave();
        self.finish_teardown();
    }

    fn handle_answer(&mut self, answer: Answer) {
        let id = answer.id;
        let Some(Pending { kind, stage, legacy }) = self.pending.remove(&id) else { return };
        if id == RENEWAL_ID {
            return self.handle_renewal(stage, answer.result);
        }
        if id == LEGACY_RENEWAL_ID {
            return self.handle_legacy_renewal(answer.result);
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
                                        self.pending.insert(id, Pending::new(kind, Stage::Rank { listing: reply }));
                                        return;
                                    }
                                }
                                Ok(reply)
                            }
                            Err(f) => Err(f),
                        }
                    }
                    RequestKind::ListLobbies
                    | RequestKind::CreateLobby { .. }
                    | RequestKind::JoinLobby { .. }
                    | RequestKind::LeaveLobby
                    | RequestKind::StartLobby { .. }
                    | RequestKind::LobbyHeartbeat { .. } => protocol::parse_rpc(&resp).map(|outcome| lobby_reply(&kind, outcome)),
                    // `call` refuses these, so a request of this kind never reaches here.
                    RequestKind::SignIn | RequestKind::SignOut => return,
                };
                match result {
                    Ok(reply) => self.reply(id, reply),
                    Err(Failure::Unauthorized) if !retried && legacy => self.renew_legacy(id, kind),
                    Err(Failure::Unauthorized) if !retried => self.renew_for(id, kind),
                    Err(f) => self.reply(id, fail_reply(&kind, &reason(f))),
                }
            }
        }
    }

    /// Answers everything still in flight with a failure and forgets it (sign-out), except the lobby requests of the old session
    /// (`legacy`, see `sign_out`) and their renewal. The shared renewal is dropped: its late answer must not bring a session back.
    fn cancel_all(&mut self, why: &str) {
        let ids: Vec<u64> = self.pending.keys().copied().collect();
        for id in ids {
            if self.pending.get(&id).is_some_and(|p| p.legacy) {
                continue;
            }
            let Some(p) = self.pending.remove(&id) else { continue };
            if id == RENEWAL_ID {
                continue;
            }
            let reply = match p.kind {
                RequestKind::SignIn => BackendReply::SignInFailed { reason: why.to_string() },
                other => fail_reply(&other, why),
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
        other => fail_reply(other, "server unreachable"),
    }
}

impl Backend for NakamaBackend {
    fn submit(&mut self, req: BackendRequest) {
        match req.kind {
            // Held while the old session finishes its lobby requests: two sessions of one account would race each other.
            RequestKind::SignIn if self.teardown.is_some() => self.held_sign_in = Some(req.id),
            RequestKind::SignIn => self.authenticate(req.id, RequestKind::SignIn, Stage::SignIn),
            RequestKind::SignOut => self.sign_out(req.id),
            // Core queues a leave before a sign-out also while a create or join is unanswered: the leave must come after it.
            RequestKind::LeaveLobby if self.entering_in_progress() => self.held_leave = Some(req.id),
            kind => self.start_call(req.id, kind, false, false),
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
        if let Some(t) = self.teardown.as_mut() {
            t.parked.retain(|(id, _)| *id != request_id);
        }
        if self.held_leave == Some(request_id) {
            self.held_leave = None;
        }
        if self.held_sign_in == Some(request_id) {
            self.held_sign_in = None;
        }
        // A cancelled step is skipped, not waited for.
        self.release_held_leave();
        self.finish_teardown();
    }
}
