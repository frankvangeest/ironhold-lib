//! The online-backend contract (`planning/features/backend_abstraction.md`, v1): requests, replies, the `Backend` poll trait,
//! the in-memory `FakeBackend`, and the one glue system that turns replies into `GameEvent`s and engine-written variables.
//!
//! No network, no async runtime, no platform code lives here: an adapter (the Nakama crate, later) implements [`Backend`] and
//! the runner installs it into [`BackendSlot`]. The online `Action`s only ever push a [`BackendRequest`] onto the
//! [`BackendHub`] outbox (a plain resource: a `Message` buffer would drop entries while a reader is gated or paused).
//!
//! # Rules
//! - Replies are **per-client external inputs**. They never touch replicated state; they arrive while paused and survive
//!   `LoadScene` (bind them in `global_on`, same trap as `EmitEventAfterDelay`).
//! - Variables are written in three places, all inside the backend slot or the executor arm that accepted the request:
//!   [`dispatch_backend_action`] (`backend.status = connecting`, `backend.last_error`), [`backend_glue_system`] (replies) and
//!   the config loader. Writing in `Emit` rather than `PostExecute` makes a reply's variables visible to the interpreter in the
//!   frame its event arrives. The glue system sorts each frame's batch by `request_id`, so the order does not depend on the adapter.
//! - Timeouts count on `Time<Real>` (virtual time is frozen while paused).
//! - Event names carry ids the designer can write (`score.submitted:best_time`), never JSON or error text; the text goes to
//!   `backend.last_error` for a Label.

use std::collections::{BTreeMap, VecDeque};

use bevy::prelude::*;

use crate::runtime::messages::GameEvent;
use crate::runtime::scene_manager::resolve_project_path;
use crate::runtime::schedule::GameStreamSet;
use crate::schema::backend::{
    is_fake_endpoint, parse_fake_endpoint, BackendConfig, Keep, LeaderboardDef, Ranking, MAX_LEADERBOARD_ROWS,
};
use crate::schema::project::{ProjectConfig, ProjectConfigHandle};
use crate::schema::{Action, ImplicitRonPlugin};
use crate::{GameVariables, ProjectRoot};

/// Seconds a request may stay unanswered before the engine fails it (counted from the frame it was handed to the adapter).
pub const DEFAULT_REQUEST_TIMEOUT_SECS: f64 = 15.0;

// ─── The contract ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum RequestKind {
    SignIn,
    SignOut,
    SubmitScore { leaderboard: String, score: i64 },
    ListLeaderboard { leaderboard: String, count: u32 },
}

impl RequestKind {
    /// Two requests with the same key must not be in flight at once (the second one answers `backend.busy`).
    fn dedupe_key(&self) -> String {
        match self {
            // One group: a sign-in and a sign-out must never be in flight together (their replies could land in either order).
            RequestKind::SignIn | RequestKind::SignOut => "session".to_string(),
            RequestKind::SubmitScore { leaderboard, .. } => format!("submit:{leaderboard}"),
            RequestKind::ListLeaderboard { leaderboard, .. } => format!("list:{leaderboard}"),
        }
    }

    /// The failure reply for this request (used for timeouts and a backend that vanished).
    fn failure(&self, reason: &str) -> BackendReply {
        match self {
            RequestKind::SignIn | RequestKind::SignOut => BackendReply::Unreachable { reason: reason.to_string() },
            RequestKind::SubmitScore { leaderboard, .. } => {
                BackendReply::ScoreFailed { leaderboard: leaderboard.clone(), reason: reason.to_string() }
            }
            RequestKind::ListLeaderboard { leaderboard, .. } => {
                BackendReply::LeaderboardFailed { leaderboard: leaderboard.clone(), reason: reason.to_string() }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BackendRequest {
    /// From a core counter, issued in executor order; the adapter echoes it in the reply.
    pub id: u64,
    pub kind: RequestKind,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LeaderRow {
    pub name: String,
    pub score: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BackendReply {
    SignedIn { display_name: String },
    SignInFailed { reason: String },
    /// The server could not be reached at all (distinct from a rejected sign-in).
    Unreachable { reason: String },
    SignedOut,
    ScoreSubmitted { leaderboard: String },
    ScoreFailed { leaderboard: String, reason: String },
    Leaderboard { leaderboard: String, rows: Vec<LeaderRow>, my_rank: Option<u32>, my_best: Option<i64> },
    LeaderboardFailed { leaderboard: String, reason: String },
    /// Unsolicited (`request_id` 0): the live connection dropped / came back.
    ConnectionLost,
    Reconnected,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BackendEvent {
    /// The request this answers; 0 for unsolicited events.
    pub request_id: u64,
    pub reply: BackendReply,
}

/// What an adapter implements. `submit` must not block; `poll` appends whatever arrived since the last call.
/// Never use Bevy's task pools for the I/O behind it (single-threaded on wasm, starved by blocking sockets on native).
pub trait Backend {
    fn submit(&mut self, req: BackendRequest);
    fn poll(&mut self, out: &mut Vec<BackendEvent>);
}

/// Holds the installed backend. A `NonSend` resource because a wasm adapter owns `!Send` JS handles; the runner installs
/// the adapter before the first frame (`BackendSlot::install`).
#[derive(Default)]
pub struct BackendSlot(pub Option<Box<dyn Backend>>);

impl BackendSlot {
    pub fn install(&mut self, backend: Box<dyn Backend>) {
        self.0 = Some(backend);
    }
}

/// What an adapter factory is told once `backend.ron` has resolved.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedBackendConfig {
    pub endpoint: String,
    /// The effective `game_id` (falls back to the project id).
    pub game_id: String,
    pub leaderboards: Vec<LeaderboardDef>,
}

/// Builds the adapter for a project's `backend.ron`; the runner puts one in `AppOptions`. Called at most once, and only for a
/// non-`fake://` endpoint with no adapter already installed in the slot.
#[derive(Clone)]
pub struct BackendFactory(pub std::sync::Arc<dyn Fn(&ResolvedBackendConfig) -> Result<Box<dyn Backend>, String> + Send + Sync>);

impl BackendFactory {
    pub fn new(f: impl Fn(&ResolvedBackendConfig) -> Result<Box<dyn Backend>, String> + Send + Sync + 'static) -> Self {
        Self(std::sync::Arc::new(f))
    }
}

impl std::fmt::Debug for BackendFactory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("BackendFactory")
    }
}

/// The factory as a resource (inserted by `start_app_with` when the runner supplied one).
#[derive(Resource, Clone, Debug)]
pub struct BackendFactoryRes(pub BackendFactory);

// ─── Hub: outbox, in-flight table, status ────────────────────────────────────

/// Where the project's backend stands. The online actions read this to decide between `backend.busy`, `backend.offline` and
/// accepting a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BackendAvailability {
    /// `backend.ron` not resolved yet.
    #[default]
    Loading,
    /// No `backend.ron`, no endpoint, a failed load or no adapter built into this runner: online actions answer `backend.offline`.
    Offline,
    Ready,
}

/// The session state mirrored to `backend.status`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BackendStatus {
    /// No usable backend at all (nothing configured, a failed `backend.ron`, or no adapter in this build).
    #[default]
    Offline,
    /// A working backend, nobody signed in.
    SignedOut,
    Connecting,
    Online,
}

impl BackendStatus {
    fn as_str(self) -> &'static str {
        match self {
            BackendStatus::Offline => "offline",
            BackendStatus::SignedOut => "signed_out",
            BackendStatus::Connecting => "connecting",
            BackendStatus::Online => "online",
        }
    }
}

struct InFlight {
    kind: RequestKind,
    /// Set when the request is handed to the adapter; the timeout counts from here.
    started: Option<f64>,
}

enum LoadPhase {
    WaitingForProject,
    Loading(Handle<BackendConfig>),
    Done,
}

#[derive(Resource)]
pub struct BackendHub {
    pub availability: BackendAvailability,
    pub status: BackendStatus,
    pub config: Option<BackendConfig>,
    pub request_timeout_secs: f64,
    /// True after an unsolicited `ConnectionLost` until `Reconnected` (or a sign-out) ends it.
    reconnecting: bool,
    next_id: u64,
    outbox: VecDeque<BackendRequest>,
    in_flight: BTreeMap<u64, InFlight>,
    load: LoadPhase,
}

impl Default for BackendHub {
    fn default() -> Self {
        Self {
            availability: BackendAvailability::Loading,
            status: BackendStatus::Offline,
            config: None,
            request_timeout_secs: DEFAULT_REQUEST_TIMEOUT_SECS,
            reconnecting: false,
            next_id: 1,
            outbox: VecDeque::new(),
            in_flight: BTreeMap::new(),
            load: LoadPhase::WaitingForProject,
        }
    }
}

impl BackendHub {
    /// A hub that already has its config and an installed backend (tests, and runners that skip `backend.ron`).
    pub fn ready(config: BackendConfig) -> Self {
        Self {
            availability: BackendAvailability::Ready,
            status: BackendStatus::SignedOut,
            config: Some(config),
            load: LoadPhase::Done,
            ..Default::default()
        }
    }

    /// Requests accepted but not yet answered (outbox included).
    pub fn pending_requests(&self) -> usize {
        self.in_flight.len()
    }

    fn is_busy(&self, kind: &RequestKind) -> bool {
        let key = kind.dedupe_key();
        self.in_flight.values().any(|f| f.kind.dedupe_key() == key)
    }

    fn enqueue(&mut self, kind: RequestKind) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.in_flight.insert(id, InFlight { kind: kind.clone(), started: None });
        self.outbox.push_back(BackendRequest { id, kind });
        id
    }
}

fn emit(events: &mut MessageWriter<GameEvent>, name: impl Into<String>) {
    events.write(GameEvent::Trigger(name.into()));
}

fn set_status(hub: &mut BackendHub, vars: &mut GameVariables, status: BackendStatus) {
    hub.status = status;
    vars.0.insert("backend.status".to_string(), status.as_str().to_string());
}

fn set_error(vars: &mut GameVariables, text: &str) {
    vars.0.insert("backend.last_error".to_string(), text.to_string());
}

// ─── Executor side: the online Actions ───────────────────────────────────────

/// Handles `SignIn`, `SignOut`, `SubmitScore` and `FetchLeaderboard` for `action_executor_system`. Anything else is ignored.
/// A request is only accepted when the backend is ready, the session allows it and no request of the same kind is in flight;
/// every refusal answers with an event the same frame, so a designer's state machine never waits on silence.
pub fn dispatch_backend_action(
    action: &Action,
    hub: &mut BackendHub,
    vars: &mut GameVariables,
    events: &mut MessageWriter<GameEvent>,
) {
    match hub.availability {
        BackendAvailability::Loading => return emit(events, "backend.busy"),
        BackendAvailability::Offline => return emit(events, "backend.offline"),
        BackendAvailability::Ready => {}
    }
    match action {
        Action::SignIn => {
            // Busy first: a sign-out in flight still reads Online, and answering "signed in" now would be a lie.
            if hub.is_busy(&RequestKind::SignIn) {
                return emit(events, "backend.busy");
            }
            if hub.status == BackendStatus::Online {
                return emit(events, "backend.signed_in");
            }
            set_error(vars, "");
            set_status(hub, vars, BackendStatus::Connecting);
            hub.enqueue(RequestKind::SignIn);
        }
        Action::SignOut => {
            if hub.is_busy(&RequestKind::SignOut) {
                return emit(events, "backend.busy");
            }
            if hub.status == BackendStatus::SignedOut {
                return emit(events, "backend.signed_out");
            }
            hub.enqueue(RequestKind::SignOut);
        }
        Action::SubmitScore { leaderboard, score_var } => {
            let fail = |vars: &mut GameVariables, events: &mut MessageWriter<GameEvent>, why: &str| {
                warn!("SubmitScore({leaderboard}): {why}");
                set_error(vars, why);
                emit(events, format!("score.submit_failed:{leaderboard}"));
            };
            if hub.config.as_ref().and_then(|c| c.leaderboard(leaderboard)).is_none() {
                return fail(vars, events, &format!("leaderboard {leaderboard:?} is not in backend.ron"));
            }
            let Some(score) = vars.0.get(score_var).and_then(|v| v.trim().parse::<i64>().ok()) else {
                return fail(vars, events, &format!("variable {score_var:?} does not hold a whole number"));
            };
            match hub.status {
                BackendStatus::Connecting => return fail(vars, events, "still signing in, try again in a moment"),
                BackendStatus::Offline | BackendStatus::SignedOut => return fail(vars, events, "not signed in"),
                BackendStatus::Online => {}
            }
            let kind = RequestKind::SubmitScore { leaderboard: leaderboard.clone(), score };
            if hub.is_busy(&kind) {
                return emit(events, "backend.busy");
            }
            hub.enqueue(kind);
        }
        Action::FetchLeaderboard { leaderboard, count } => {
            let fail = |vars: &mut GameVariables, events: &mut MessageWriter<GameEvent>, why: &str| {
                warn!("FetchLeaderboard({leaderboard}): {why}");
                set_error(vars, why);
                emit(events, format!("leaderboard.list_failed:{leaderboard}"));
            };
            if hub.config.as_ref().and_then(|c| c.leaderboard(leaderboard)).is_none() {
                return fail(vars, events, &format!("leaderboard {leaderboard:?} is not in backend.ron"));
            }
            match hub.status {
                BackendStatus::Connecting => return fail(vars, events, "still signing in, try again in a moment"),
                BackendStatus::Offline | BackendStatus::SignedOut => return fail(vars, events, "not signed in"),
                BackendStatus::Online => {}
            }
            let kind = RequestKind::ListLeaderboard {
                leaderboard: leaderboard.clone(),
                count: (*count).clamp(1, MAX_LEADERBOARD_ROWS),
            };
            if hub.is_busy(&kind) {
                return emit(events, "backend.busy");
            }
            hub.enqueue(kind);
        }
        _ => {}
    }
}

// ─── Glue: outbox -> adapter -> events and variables ─────────────────────────

/// Hands queued requests to the backend, polls it, and applies the replies. Runs in [`GameStreamSet::Backend`] (after the
/// delayed events, before the stat events) so a reply is visible to the interpreters of the same frame.
pub fn backend_glue_system(
    mut hub: ResMut<BackendHub>,
    slot: Option<NonSendMut<BackendSlot>>,
    mut vars: ResMut<GameVariables>,
    mut events: MessageWriter<GameEvent>,
    time: Res<Time<Real>>,
) {
    let now = time.elapsed_secs_f64();
    let hub = &mut *hub;
    let mut batch: Vec<BackendEvent> = Vec::new();

    match slot.and_then(|s| if s.0.is_some() { Some(s) } else { None }) {
        Some(mut slot) => {
            let backend = slot.0.as_mut().expect("checked above");
            while let Some(req) = hub.outbox.pop_front() {
                if let Some(f) = hub.in_flight.get_mut(&req.id) {
                    f.started = Some(now);
                }
                backend.submit(req);
            }
            backend.poll(&mut batch);
        }
        None => {
            // Requests were accepted but nothing can answer them: fail them instead of leaving the state machine waiting.
            while let Some(req) = hub.outbox.pop_front() {
                batch.push(BackendEvent { request_id: req.id, reply: req.kind.failure("no backend is installed") });
            }
        }
    }

    // A fixed order no matter how the adapter's threads interleaved.
    batch.sort_by_key(|e| e.request_id);

    // Requests the adapter never answered.
    let expired: Vec<u64> = hub
        .in_flight
        .iter()
        .filter(|(id, f)| f.started.is_some_and(|s| now - s >= hub.request_timeout_secs) && !batch.iter().any(|e| e.request_id == **id))
        .map(|(id, _)| *id)
        .collect();
    for id in expired {
        let kind = hub.in_flight[&id].kind.clone();
        warn!("backend request {id} ({kind:?}) timed out after {}s", hub.request_timeout_secs);
        batch.push(BackendEvent { request_id: id, reply: kind.failure("timed out") });
    }

    for ev in batch {
        if ev.request_id != 0 && hub.in_flight.remove(&ev.request_id).is_none() {
            debug!("ignoring a reply to unknown or already-failed request {}", ev.request_id);
            continue;
        }
        apply_reply(hub, &mut vars, &mut events, ev.reply);
    }
}

fn apply_reply(hub: &mut BackendHub, vars: &mut GameVariables, events: &mut MessageWriter<GameEvent>, reply: BackendReply) {
    match reply {
        BackendReply::SignedIn { display_name } => {
            set_status(hub, vars, BackendStatus::Online);
            vars.0.insert("backend.display_name".to_string(), display_name);
            set_error(vars, "");
            emit(events, "backend.signed_in");
        }
        BackendReply::SignInFailed { reason } => {
            hub.reconnecting = false;
            set_status(hub, vars, BackendStatus::SignedOut);
            set_error(vars, &reason);
            emit(events, "backend.sign_in_failed");
        }
        BackendReply::Unreachable { reason } => {
            hub.reconnecting = false;
            set_status(hub, vars, BackendStatus::SignedOut);
            set_error(vars, &reason);
            emit(events, "backend.unreachable");
        }
        BackendReply::SignedOut => {
            hub.reconnecting = false;
            set_status(hub, vars, BackendStatus::SignedOut);
            vars.0.insert("backend.display_name".to_string(), String::new());
            // The previous player's board and rank must not stay on screen for the next one.
            vars.0.retain(|k, _| !k.starts_with("leaderboard."));
            emit(events, "backend.signed_out");
        }
        BackendReply::ScoreSubmitted { leaderboard } => {
            set_error(vars, "");
            emit(events, format!("score.submitted:{leaderboard}"));
        }
        BackendReply::ScoreFailed { leaderboard, reason } => {
            set_error(vars, &reason);
            emit(events, format!("score.submit_failed:{leaderboard}"));
        }
        BackendReply::Leaderboard { leaderboard, rows, my_rank, my_best } => {
            write_leaderboard_vars(vars, &leaderboard, &rows, my_rank, my_best);
            set_error(vars, "");
            emit(events, format!("leaderboard.listed:{leaderboard}"));
        }
        BackendReply::LeaderboardFailed { leaderboard, reason } => {
            set_error(vars, &reason);
            emit(events, format!("leaderboard.list_failed:{leaderboard}"));
        }
        // Only meaningful for a live session: a socket closing after a sign-out must not make "not signed in" read "connecting".
        BackendReply::ConnectionLost => {
            if hub.status == BackendStatus::Online {
                hub.reconnecting = true;
                set_status(hub, vars, BackendStatus::Connecting);
                emit(events, "backend.connection_lost");
            }
        }
        BackendReply::Reconnected => {
            if hub.reconnecting {
                hub.reconnecting = false;
                set_status(hub, vars, BackendStatus::Online);
                emit(events, "backend.reconnected");
            }
        }
    }
}

/// `leaderboard.<name>.count`, `.<n>.name`, `.<n>.score` (n = 1..=10), `.my_rank`, `.my_best`. Rows beyond the new count are
/// removed so a shorter list never shows stale entries; an absent rank/best is an empty string.
fn write_leaderboard_vars(vars: &mut GameVariables, lb: &str, rows: &[LeaderRow], my_rank: Option<u32>, my_best: Option<i64>) {
    let p = format!("leaderboard.{lb}");
    let shown = rows.len().min(MAX_LEADERBOARD_ROWS as usize);
    vars.0.insert(format!("{p}.count"), shown.to_string());
    for (i, row) in rows.iter().take(shown).enumerate() {
        let n = i + 1;
        vars.0.insert(format!("{p}.{n}.name"), row.name.clone());
        vars.0.insert(format!("{p}.{n}.score"), row.score.to_string());
    }
    for n in shown + 1..=MAX_LEADERBOARD_ROWS as usize {
        vars.0.remove(&format!("{p}.{n}.name"));
        vars.0.remove(&format!("{p}.{n}.score"));
    }
    vars.0.insert(format!("{p}.my_rank"), my_rank.map(|r| r.to_string()).unwrap_or_default());
    vars.0.insert(format!("{p}.my_best"), my_best.map(|b| b.to_string()).unwrap_or_default());
}

// ─── FakeBackend ─────────────────────────────────────────────────────────────

/// Which operations a [`FakeBackend`] should fail (for tests and for designers trying their failure UI with `fake://`).
#[derive(Debug, Clone, Copy, Default)]
pub struct FakeFailures {
    /// Sign-in is rejected (`backend.sign_in_failed`).
    pub reject_sign_in: bool,
    /// The server is unreachable (`backend.unreachable`).
    pub unreachable: bool,
    pub submit: bool,
    pub list: bool,
    /// Never answer anything (exercises the timeout).
    pub silent: bool,
}

/// An in-memory backend. Deterministic: latency counts `poll` calls (frames), never wall-clock time.
pub struct FakeBackend {
    latency_polls: u64,
    polls: u64,
    pending: Vec<(u64, BackendEvent)>,
    signed_in: bool,
    display_name: String,
    defs: BTreeMap<String, LeaderboardDef>,
    boards: BTreeMap<String, Vec<(String, i64)>>,
    pub failures: FakeFailures,
}

impl FakeBackend {
    pub const PLAYER_NAME: &'static str = "Player";

    /// Seeds every declared leaderboard with four fixed rivals so a demo has something to show.
    pub fn from_config(config: &BackendConfig) -> Self {
        let mut fake = Self {
            latency_polls: 2,
            polls: 0,
            pending: Vec::new(),
            signed_in: false,
            display_name: Self::PLAYER_NAME.to_string(),
            defs: BTreeMap::new(),
            boards: BTreeMap::new(),
            failures: FakeFailures::default(),
        };
        for lb in &config.leaderboards {
            let seed: [(&str, i64); 4] = match lb.ranking {
                Ranking::HigherIsBetter => [("Ada", 900), ("Grace", 750), ("Linus", 600), ("Margaret", 450)],
                Ranking::LowerIsBetter => [("Ada", 30), ("Grace", 45), ("Linus", 60), ("Margaret", 90)],
            };
            fake.boards.insert(lb.name.clone(), seed.iter().map(|(n, s)| (n.to_string(), *s)).collect());
            fake.defs.insert(lb.name.clone(), lb.clone());
        }
        fake
    }

    /// The test backend a `fake://?latency=N&fail=...` endpoint asks for (`schema::backend::parse_fake_endpoint`).
    pub fn from_endpoint(config: &BackendConfig, endpoint: &str) -> Self {
        let mut fake = Self::from_config(config);
        // An invalid query was already reported by `BackendConfig::problems`; here it falls back to the defaults.
        if let Ok(o) = parse_fake_endpoint(endpoint) {
            if let Some(l) = o.latency {
                fake.latency_polls = l;
            }
            fake.failures = FakeFailures {
                reject_sign_in: o.reject_sign_in,
                unreachable: o.unreachable,
                submit: o.submit,
                list: o.list,
                silent: o.silent,
            };
        }
        fake
    }

    pub fn with_latency(mut self, polls: u64) -> Self {
        self.latency_polls = polls;
        self
    }

    pub fn with_failures(mut self, failures: FakeFailures) -> Self {
        self.failures = failures;
        self
    }

    fn reply_for(&mut self, kind: RequestKind) -> BackendReply {
        let f = self.failures;
        match kind {
            RequestKind::SignIn => {
                if f.unreachable {
                    BackendReply::Unreachable { reason: "the test backend is set to unreachable".into() }
                } else if f.reject_sign_in {
                    BackendReply::SignInFailed { reason: "the test backend rejected the sign-in".into() }
                } else {
                    self.signed_in = true;
                    BackendReply::SignedIn { display_name: self.display_name.clone() }
                }
            }
            RequestKind::SignOut => {
                self.signed_in = false;
                BackendReply::SignedOut
            }
            RequestKind::SubmitScore { leaderboard, score } => {
                if f.unreachable || f.submit || !self.signed_in {
                    return BackendReply::ScoreFailed { leaderboard, reason: "the test backend refused the score".into() };
                }
                let Some(def) = self.defs.get(&leaderboard).cloned() else {
                    return BackendReply::ScoreFailed { leaderboard, reason: "unknown leaderboard".into() };
                };
                let rows = self.boards.entry(leaderboard.clone()).or_default();
                match rows.iter_mut().find(|(n, _)| n == Self::PLAYER_NAME) {
                    None => rows.push((Self::PLAYER_NAME.to_string(), score)),
                    Some((_, old)) => match def.keep {
                        Keep::Latest => *old = score,
                        Keep::Total => *old += score,
                        Keep::Best => {
                            let better = match def.ranking {
                                Ranking::HigherIsBetter => score > *old,
                                Ranking::LowerIsBetter => score < *old,
                            };
                            if better {
                                *old = score;
                            }
                        }
                    },
                }
                BackendReply::ScoreSubmitted { leaderboard }
            }
            RequestKind::ListLeaderboard { leaderboard, count } => {
                if f.unreachable || f.list || !self.signed_in {
                    return BackendReply::LeaderboardFailed { leaderboard, reason: "the test backend refused the request".into() };
                }
                let Some(def) = self.defs.get(&leaderboard) else {
                    return BackendReply::LeaderboardFailed { leaderboard, reason: "unknown leaderboard".into() };
                };
                let mut all = self.boards.get(&leaderboard).cloned().unwrap_or_default();
                // Rank order, ties broken by name so the list is stable.
                all.sort_by(|a, b| {
                    let by_score = match def.ranking {
                        Ranking::HigherIsBetter => b.1.cmp(&a.1),
                        Ranking::LowerIsBetter => a.1.cmp(&b.1),
                    };
                    by_score.then_with(|| a.0.cmp(&b.0))
                });
                let me = all.iter().position(|(n, _)| n == Self::PLAYER_NAME);
                let my_best = me.map(|i| all[i].1);
                let rows = all.iter().take(count as usize).map(|(n, s)| LeaderRow { name: n.clone(), score: *s }).collect();
                BackendReply::Leaderboard { leaderboard, rows, my_rank: me.map(|i| i as u32 + 1), my_best }
            }
        }
    }
}

impl Backend for FakeBackend {
    fn submit(&mut self, req: BackendRequest) {
        if self.failures.silent {
            return;
        }
        let due = self.polls + self.latency_polls + 1;
        let reply = self.reply_for(req.kind);
        self.pending.push((due, BackendEvent { request_id: req.id, reply }));
    }

    fn poll(&mut self, out: &mut Vec<BackendEvent>) {
        self.polls += 1;
        let now = self.polls;
        let mut i = 0;
        while i < self.pending.len() {
            if self.pending[i].0 <= now {
                out.push(self.pending.remove(i).1);
            } else {
                i += 1;
            }
        }
    }
}

// ─── Loading backend.ron and choosing the backend ────────────────────────────

fn backend_config_pending(hub: Res<BackendHub>) -> bool {
    !matches!(hub.load, LoadPhase::Done)
}

fn backend_is_ready(hub: Res<BackendHub>) -> bool {
    hub.availability == BackendAvailability::Ready
}

/// Resolves `backend_path` after the project config is available, without blocking the scene load: until it finishes the
/// online actions answer `backend.busy`; a failed load logs an error and the project runs offline.
fn load_backend_config_system(
    mut hub: ResMut<BackendHub>,
    slot: Option<NonSendMut<BackendSlot>>,
    factory: Option<Res<BackendFactoryRes>>,
    mut vars: ResMut<GameVariables>,
    mut events: MessageWriter<GameEvent>,
    config_handle: Option<Res<ProjectConfigHandle>>,
    project_configs: Res<Assets<ProjectConfig>>,
    backend_configs: Res<Assets<BackendConfig>>,
    asset_server: Res<AssetServer>,
    project_root: Res<ProjectRoot>,
) {
    let hub = &mut *hub;
    let loading: Option<Handle<BackendConfig>> = match &hub.load {
        LoadPhase::Done => return,
        LoadPhase::WaitingForProject => None,
        LoadPhase::Loading(h) => Some(h.clone()),
    };
    let Some(handle) = loading else {
        let Some(project) = config_handle.as_deref().and_then(|h| project_configs.get(&h.0)) else { return };
        match &project.backend_path {
            None => finish_offline(hub, &mut vars, &mut events, "the project has no backend_path"),
            Some(path) => {
                let resolved = resolve_project_path(&project_root.0, path);
                info!("Loading backend config from: {resolved}");
                hub.load = LoadPhase::Loading(asset_server.load::<BackendConfig>(resolved));
            }
        }
        return;
    };
    match asset_server.load_state(&handle) {
        bevy::asset::LoadState::Loaded => {
            let Some(config) = backend_configs.get(&handle).cloned() else { return };
            let problems = config.problems();
            if !problems.is_empty() {
                error!("Invalid backend.ron: {} - running offline", problems.join("; "));
                return finish_offline(hub, &mut vars, &mut events, "backend.ron is invalid (see the log)");
            }
            let project_id = config_handle.as_deref().and_then(|h| project_configs.get(&h.0)).and_then(|p| p.project_id.clone());
            choose_backend(hub, slot, factory.as_deref(), project_id.as_deref(), &mut vars, &mut events, config);
        }
        bevy::asset::LoadState::Failed(e) => {
            error!("backend.ron failed to load: {e} - running offline");
            finish_offline(hub, &mut vars, &mut events, "backend.ron failed to load");
        }
        _ => {}
    }
}

/// The project has no usable backend: say why on screen (`backend.last_error`) and tell the rules (`backend.offline`), which is
/// also what a sign-in fired before `backend.ron` resolved should retry on.
fn finish_offline(hub: &mut BackendHub, vars: &mut GameVariables, events: &mut MessageWriter<GameEvent>, why: &str) {
    info!("Backend offline: {why}");
    hub.availability = BackendAvailability::Offline;
    hub.load = LoadPhase::Done;
    set_status(hub, vars, BackendStatus::Offline);
    set_error(vars, why);
    emit(events, "backend.offline");
}

/// Asks the runner's factory for the adapter of a non-`fake://` endpoint; the `Err` text is what `backend.last_error` shows.
fn build_adapter(
    factory: Option<&BackendFactoryRes>,
    project_id: Option<&str>,
    config: &BackendConfig,
    endpoint: &str,
) -> Result<Box<dyn Backend>, String> {
    let Some(factory) = factory else {
        return Err("no server adapter in this build; use fake://".to_string());
    };
    let resolved = ResolvedBackendConfig {
        endpoint: endpoint.to_string(),
        game_id: config.effective_game_id(project_id).unwrap_or_default(),
        leaderboards: config.leaderboards.clone(),
    };
    (factory.0 .0)(&resolved).map_err(|e| {
        error!("Backend adapter could not start: {e}");
        "the server adapter could not start (see the log)".to_string()
    })
}

/// `fake://` selects the in-memory backend; any other endpoint needs an adapter the runner already installed.
fn choose_backend(
    hub: &mut BackendHub,
    slot: Option<NonSendMut<BackendSlot>>,
    factory: Option<&BackendFactoryRes>,
    project_id: Option<&str>,
    vars: &mut GameVariables,
    events: &mut MessageWriter<GameEvent>,
    config: BackendConfig,
) {
    let endpoint = config.endpoint.clone();
    hub.config = Some(config.clone());
    let Some(endpoint) = endpoint else {
        return finish_offline(hub, vars, events, "backend.ron has no endpoint");
    };
    let Some(mut slot) = slot else {
        return finish_offline(hub, vars, events, "no backend slot");
    };
    if is_fake_endpoint(&endpoint) {
        // A `fake://` endpoint always means the in-memory backend, even if a runner installed an adapter.
        slot.install(Box::new(FakeBackend::from_endpoint(&config, &endpoint)));
    } else if slot.0.is_none() {
        match build_adapter(factory, project_id, &config, &endpoint) {
            Ok(adapter) => slot.install(adapter),
            Err(why) => {
                error!("Backend adapter unavailable: {why} - running offline");
                return finish_offline(hub, vars, events, &why);
            }
        }
    }
    if let Some(t) = config.request_timeout_secs {
        hub.request_timeout_secs = f64::from(t);
    }
    hub.availability = BackendAvailability::Ready;
    hub.load = LoadPhase::Done;
    set_status(hub, vars, BackendStatus::SignedOut);
    emit(events, "backend.ready");
    info!("Backend ready (endpoint {endpoint:?})");
}

pub struct BackendPlugin;

impl Plugin for BackendPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BackendHub>()
            .insert_non_send_resource(BackendSlot::default())
            .add_plugins(ImplicitRonPlugin::<BackendConfig>::new(&["ron"]))
            .add_systems(
                Update,
                (
                    load_backend_config_system.run_if(backend_config_pending),
                    backend_glue_system.run_if(backend_is_ready),
                )
                    .chain()
                    .in_set(GameStreamSet::Backend),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> BackendConfig {
        ron::from_str(
            r#"BackendConfig(schema_version: 1, endpoint: Some("fake://"),
                leaderboards: [Leaderboard(name: "best", ranking: HigherIsBetter, keep: Best),
                               Leaderboard(name: "fast", ranking: LowerIsBetter, keep: Latest)])"#,
        )
        .unwrap()
    }

    fn https_config() -> BackendConfig {
        ron::from_str(
            r#"BackendConfig(schema_version: 1, endpoint: Some("https://play.example.org"),
                leaderboards: [Leaderboard(name: "best", ranking: HigherIsBetter, keep: Best)])"#,
        )
        .unwrap()
    }

    #[test]
    fn the_factory_gets_the_resolved_config_with_the_project_id_as_game_id() {
        let seen = std::sync::Arc::new(std::sync::Mutex::new(None));
        let sink = seen.clone();
        let factory = BackendFactoryRes(BackendFactory::new(move |c| {
            *sink.lock().unwrap() = Some(c.clone());
            Ok(Box::new(FakeBackend::from_config(&https_config())))
        }));
        let config = https_config();
        assert!(build_adapter(Some(&factory), Some("my_game"), &config, "https://play.example.org").is_ok());
        let got = seen.lock().unwrap().clone().unwrap();
        assert_eq!(got.game_id, "my_game");
        assert_eq!(got.endpoint, "https://play.example.org");
        assert_eq!(got.leaderboards.len(), 1);
    }

    #[test]
    fn no_factory_or_a_failing_factory_means_offline_with_a_reason() {
        let config = https_config();
        let err = build_adapter(None, None, &config, "https://x").err().unwrap();
        assert!(err.contains("no server adapter"));
        let failing = BackendFactoryRes(BackendFactory::new(|_| Err("boom".to_string())));
        let err = build_adapter(Some(&failing), None, &config, "https://x").err().unwrap();
        assert!(err.contains("could not start") && !err.contains("boom"));
    }

    fn drain(fake: &mut FakeBackend, polls: u32) -> Vec<BackendEvent> {
        let mut out = Vec::new();
        for _ in 0..polls {
            fake.poll(&mut out);
        }
        out
    }

    fn sign_in(fake: &mut FakeBackend) {
        fake.submit(BackendRequest { id: 1, kind: RequestKind::SignIn });
        drain(fake, 5);
    }

    #[test]
    fn latency_counts_polls_not_time() {
        let mut fake = FakeBackend::from_config(&config()).with_latency(2);
        fake.submit(BackendRequest { id: 7, kind: RequestKind::SignIn });
        assert!(drain(&mut fake, 2).is_empty(), "two polls are not enough for latency 2");
        let out = drain(&mut fake, 1);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].request_id, 7);
    }

    #[test]
    fn zero_latency_answers_in_the_next_poll() {
        let mut fake = FakeBackend::from_config(&config()).with_latency(0);
        fake.submit(BackendRequest { id: 1, kind: RequestKind::SignIn });
        assert_eq!(drain(&mut fake, 1).len(), 1);
    }

    #[test]
    fn best_keeps_the_better_score_and_lower_is_better_ranks_ascending() {
        let mut fake = FakeBackend::from_config(&config()).with_latency(0);
        sign_in(&mut fake);
        for (id, score) in [(2, 1000), (3, 400)] {
            fake.submit(BackendRequest { id, kind: RequestKind::SubmitScore { leaderboard: "best".into(), score } });
        }
        drain(&mut fake, 1);
        fake.submit(BackendRequest { id: 4, kind: RequestKind::ListLeaderboard { leaderboard: "best".into(), count: 3 } });
        let out = drain(&mut fake, 1);
        let BackendReply::Leaderboard { rows, my_rank, my_best, .. } = &out[0].reply else { panic!("{out:?}") };
        assert_eq!(rows[0], LeaderRow { name: "Player".into(), score: 1000 }, "the 400 must not replace the best 1000");
        assert_eq!((*my_rank, *my_best), (Some(1), Some(1000)));

        fake.submit(BackendRequest { id: 5, kind: RequestKind::SubmitScore { leaderboard: "fast".into(), score: 20 } });
        drain(&mut fake, 1);
        fake.submit(BackendRequest { id: 6, kind: RequestKind::ListLeaderboard { leaderboard: "fast".into(), count: 2 } });
        let out = drain(&mut fake, 1);
        let BackendReply::Leaderboard { rows, .. } = &out[0].reply else { panic!("{out:?}") };
        assert_eq!(rows[0].name, "Player", "20 beats the seeded 30 when lower is better");
    }

    #[test]
    fn requests_before_sign_in_fail() {
        let mut fake = FakeBackend::from_config(&config()).with_latency(0);
        fake.submit(BackendRequest { id: 1, kind: RequestKind::SubmitScore { leaderboard: "best".into(), score: 5 } });
        assert!(matches!(drain(&mut fake, 1)[0].reply, BackendReply::ScoreFailed { .. }));
    }

    #[test]
    fn leaderboard_variables_clear_rows_beyond_the_new_count() {
        let mut vars = GameVariables::default();
        let rows: Vec<_> = (1..=3).map(|i| LeaderRow { name: format!("p{i}"), score: i }).collect();
        write_leaderboard_vars(&mut vars, "best", &rows, Some(2), Some(2));
        assert_eq!(vars.0["leaderboard.best.3.name"], "p3");
        write_leaderboard_vars(&mut vars, "best", &rows[..1], None, None);
        assert_eq!(vars.0["leaderboard.best.count"], "1");
        assert!(!vars.0.contains_key("leaderboard.best.3.name"), "stale row must be removed");
        assert_eq!(vars.0["leaderboard.best.my_rank"], "");
    }
}
