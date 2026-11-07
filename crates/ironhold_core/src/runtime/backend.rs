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
//! - Lobbies (v3a): core drives the heartbeat (`LobbyHeartbeat` every [`LOBBY_HEARTBEAT_SECS`] on `Time<Real>`, a normal request), diffs
//!   each returned lobby into `lobby.member_joined`/`member_left`/`started`/`closed`, and drops a reply that belongs to a membership
//!   that no longer exists (`LobbyTracker::gen`). The failure reasons are the fixed set `LOBBY_REASONS`; an adapter's other text is
//!   coerced to `server`.

use std::collections::{BTreeMap, VecDeque};

use bevy::prelude::*;

use crate::runtime::messages::GameEvent;
use crate::runtime::scene_manager::resolve_project_path;
use crate::runtime::schedule::GameStreamSet;
use crate::schema::backend::{
    is_fake_endpoint, parse_fake_endpoint, BackendConfig, BackendFeature, Keep, LeaderboardDef, Ranking, LOBBY_EXPIRY_SECS,
    LOBBY_HEARTBEAT_SECS, LOBBY_LIST_MIN_SECS, LOBBY_REASONS, MAX_LEADERBOARD_ROWS, MAX_LOBBY_PLAYERS, MAX_LOBBY_ROWS,
    MIN_LOBBY_PLAYERS,
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
    ListLobbies,
    CreateLobby { name: String, max_players: u32 },
    /// `lobby_id` comes from the row snapshot of the last list (core resolves `JoinLobby(row:)`).
    JoinLobby { lobby_id: String },
    /// Leaves whatever lobby the server has for this user, also when this client lost track of it (a page reload).
    LeaveLobby,
    StartLobby,
    /// Sent by core on a timer while in a lobby; the reply is the whole lobby.
    LobbyHeartbeat { lobby_id: String },
}

impl RequestKind {
    /// Two requests with the same key must not be in flight at once (the second one answers `backend.busy`).
    fn dedupe_key(&self) -> String {
        match self {
            // One group: a sign-in and a sign-out must never be in flight together (their replies could land in either order).
            RequestKind::SignIn | RequestKind::SignOut => "session".to_string(),
            RequestKind::SubmitScore { leaderboard, .. } => format!("submit:{leaderboard}"),
            RequestKind::ListLeaderboard { leaderboard, .. } => format!("list:{leaderboard}"),
            RequestKind::ListLobbies => "lobby_list".to_string(),
            // One group: two of these in flight together could answer in either order. The heartbeat is its own key, so a
            // `LeaveLobby` never answers `busy` just because a heartbeat is on the wire.
            RequestKind::CreateLobby { .. }
            | RequestKind::JoinLobby { .. }
            | RequestKind::LeaveLobby
            | RequestKind::StartLobby => "lobby".to_string(),
            RequestKind::LobbyHeartbeat { .. } => "lobby_heartbeat".to_string(),
        }
    }

    /// The failure reply for this request (used for timeouts and a backend that vanished).
    pub fn failure(&self, reason: &str) -> BackendReply {
        match self {
            RequestKind::SignIn | RequestKind::SignOut => BackendReply::Unreachable { reason: reason.to_string() },
            RequestKind::SubmitScore { leaderboard, .. } => {
                BackendReply::ScoreFailed { leaderboard: leaderboard.clone(), reason: reason.to_string() }
            }
            RequestKind::ListLeaderboard { leaderboard, .. } => {
                BackendReply::LeaderboardFailed { leaderboard: leaderboard.clone(), reason: reason.to_string() }
            }
            // Lobby failures carry a fixed reason token, never free text (the text was already logged by the caller).
            RequestKind::ListLobbies => BackendReply::LobbyListFailed { reason: "server".to_string() },
            RequestKind::CreateLobby { .. } => BackendReply::LobbyCreateFailed { reason: "server".to_string() },
            RequestKind::JoinLobby { .. } => BackendReply::LobbyJoinFailed { reason: "server".to_string() },
            // A leave that could not be confirmed still leaves: the engine must not stay in a lobby it asked to quit.
            RequestKind::LeaveLobby => BackendReply::LobbyLeft,
            RequestKind::StartLobby => BackendReply::LobbyStartFailed { reason: "server".to_string() },
            RequestKind::LobbyHeartbeat { .. } => BackendReply::LobbyHeartbeatFailed { reason: "server".to_string() },
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

/// One row of the lobby list.
#[derive(Debug, Clone, PartialEq)]
pub struct LobbyRow {
    pub id: String,
    pub name: String,
    pub players: u32,
    pub max: u32,
    pub host: String,
}

/// A lobby as the server describes it. `members` are in the server's join order, host first, and include this player.
#[derive(Debug, Clone, PartialEq)]
pub struct LobbyInfo {
    pub id: String,
    pub name: String,
    pub max: u32,
    pub started: bool,
    pub host: String,
    pub members: Vec<String>,
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
    LobbyListed { rows: Vec<LobbyRow> },
    /// `reason` is one of `LOBBY_REASONS`; anything else is read as `server`.
    LobbyListFailed { reason: String },
    LobbyCreated { lobby: LobbyInfo },
    LobbyCreateFailed { reason: String },
    LobbyJoined { lobby: LobbyInfo },
    LobbyJoinFailed { reason: String },
    LobbyLeft,
    LobbyStarted { lobby: LobbyInfo },
    LobbyStartFailed { reason: String },
    /// The answer to a heartbeat: the whole lobby as it is now.
    LobbyState { lobby: LobbyInfo },
    /// The lobby is gone for this player (`host_left`, `removed`, `server`).
    LobbyClosed { reason: String },
    LobbyHeartbeatFailed { reason: String },
}

impl BackendReply {
    /// Replies that belong to a membership (or a list) and must be dropped when the engine has moved on since the request was made.
    fn is_lobby(&self) -> bool {
        matches!(
            self,
            BackendReply::LobbyListed { .. }
                | BackendReply::LobbyListFailed { .. }
                | BackendReply::LobbyCreated { .. }
                | BackendReply::LobbyCreateFailed { .. }
                | BackendReply::LobbyJoined { .. }
                | BackendReply::LobbyJoinFailed { .. }
                | BackendReply::LobbyLeft
                | BackendReply::LobbyStarted { .. }
                | BackendReply::LobbyStartFailed { .. }
                | BackendReply::LobbyState { .. }
                | BackendReply::LobbyClosed { .. }
                | BackendReply::LobbyHeartbeatFailed { .. }
        )
    }
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
    /// The engine gave up on this request (it timed out): stop any follow-up work for it (a retry, a refresh chain) and drop
    /// the answer when it arrives. Work already on the wire cannot be recalled. Default: nothing to cancel.
    fn cancel(&mut self, _request_id: u64) {}
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
    /// `backend.ron`'s `server_key`, only when the endpoint is the file's own: it is `None` when a runner override replaced the
    /// endpoint, so the file's key is never sent to a server the file did not name. Public, so `Debug` may print it.
    pub server_key: Option<String>,
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

/// Replaces the `endpoint` of `backend.ron` (a runner's `--backend` / `?backend=`): the same project runs against the fake
/// on a developer machine and a real server elsewhere without editing its data. Validated like the file's own endpoint.
#[derive(Resource, Clone, Debug)]
pub struct BackendEndpointOverride(pub String);

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
    /// `LobbyTracker::gen` when the request was made: a lobby reply for an older membership is dropped.
    lobby_gen: u64,
}

/// What the engine knows about lobbies (`planning/features/backend_abstraction.md`, v3a).
#[derive(Default)]
struct LobbyTracker {
    /// Bumped whenever the membership ends or the session does; a lobby reply made under an older value is dropped.
    gen: u64,
    current: Option<LobbyInfo>,
    /// The rows of the last list; `JoinLobby(row: n)` resolves against `rows[n - 1]`, never against the variables (a designer can overwrite those).
    rows: Vec<LobbyRow>,
    listed_at: Option<f64>,
    next_heartbeat: f64,
    /// `Time<Real>` of the last answer that proved the server still has this lobby.
    last_ok: f64,
    trouble: bool,
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
    /// `Time<Real>` as of the last glue frame (the executor has no clock of its own).
    clock: f64,
    /// The player's own name from the last sign-in (to tell the host and the other members apart).
    display_name: String,
    lobby: LobbyTracker,
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
            clock: 0.0,
            display_name: String::new(),
            lobby: LobbyTracker::default(),
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
        self.in_flight.insert(id, InFlight { kind: kind.clone(), started: None, lobby_gen: self.lobby.gen });
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
            // Signing out leaves the lobby: the server is told first (requests go out in order), the engine drops its state now, and
            // every lobby reply still on its way belongs to the old session and is dropped.
            if hub.lobby.current.is_some() {
                hub.enqueue(RequestKind::LeaveLobby);
                drop_lobby(hub, vars, events, None);
            }
            hub.lobby.gen += 1;
            hub.enqueue(RequestKind::SignOut);
        }
        Action::ListLobbies
        | Action::CreateLobby { .. }
        | Action::JoinLobby { .. }
        | Action::LeaveLobby
        | Action::StartLobby => dispatch_lobby_action(action, hub, vars, events),
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

/// The lobby half of [`dispatch_backend_action`]. Every refusal answers the same frame with the operation's failure event and then the
/// reason form (`lobby.join_failed`, `lobby.join_failed:full`), so a designer can word a specific message and a generic one.
fn dispatch_lobby_action(action: &Action, hub: &mut BackendHub, vars: &mut GameVariables, events: &mut MessageWriter<GameEvent>) {
    let op = match action {
        Action::ListLobbies => "list",
        Action::CreateLobby { .. } => "create",
        Action::JoinLobby { .. } => "join",
        Action::StartLobby => "start",
        Action::LeaveLobby => "leave",
        _ => return,
    };
    let enabled = hub.config.as_ref().is_some_and(|c| c.features.contains(&BackendFeature::Lobbies));
    if op == "leave" {
        // Nothing to leave without the feature or a session: the engine is simply not in a lobby, which `lobby.left` says.
        if !enabled || hub.status != BackendStatus::Online {
            return drop_lobby(hub, vars, events, None);
        }
        if hub.is_busy(&RequestKind::LeaveLobby) {
            return emit(events, "backend.busy");
        }
        hub.enqueue(RequestKind::LeaveLobby);
        return;
    }
    if !enabled {
        warn!("{action:?}: add Lobbies to `features` in backend.ron");
        return lobby_fail(vars, events, op, "not_enabled");
    }
    if hub.status != BackendStatus::Online {
        return lobby_fail(vars, events, op, "not_signed_in");
    }
    let kind = match action {
        Action::ListLobbies => {
            if hub.is_busy(&RequestKind::ListLobbies) {
                return lobby_fail(vars, events, op, "busy");
            }
            // Refresh faster than the minimum and the last list is shown again: a designer's 1 s refresh loop must not turn into 1 request/s
            // per player (the shared Cloudflare limit would then block everybody behind that address).
            if hub.lobby.listed_at.is_some_and(|at| hub.clock - at < LOBBY_LIST_MIN_SECS) {
                let rows = hub.lobby.rows.clone();
                return write_lobby_list(vars, events, &rows);
            }
            RequestKind::ListLobbies
        }
        Action::CreateLobby { name, max_players } => {
            if !hub.config.as_ref().is_some_and(|c| c.lobby_names.contains(name)) {
                warn!("CreateLobby: {name:?} is not in `lobby_names` of backend.ron");
                return lobby_fail(vars, events, op, "server");
            }
            if hub.lobby.current.is_some() {
                return lobby_fail(vars, events, op, "already_in_lobby");
            }
            RequestKind::CreateLobby { name: name.clone(), max_players: (*max_players).clamp(MIN_LOBBY_PLAYERS, MAX_LOBBY_PLAYERS) }
        }
        Action::JoinLobby { row } => {
            let Some(target) = (*row as usize).checked_sub(1).filter(|_| (1..=MAX_LOBBY_ROWS).contains(row)).and_then(|i| hub.lobby.rows.get(i))
            else {
                warn!("JoinLobby(row: {row}): that row is empty or outside 1-{MAX_LOBBY_ROWS} (call ListLobbies first)");
                return lobby_fail(vars, events, op, "no_lobby");
            };
            if hub.lobby.current.is_some() {
                return lobby_fail(vars, events, op, "already_in_lobby");
            }
            RequestKind::JoinLobby { lobby_id: target.id.clone() }
        }
        Action::StartLobby => {
            let Some(cur) = hub.lobby.current.as_ref() else { return lobby_fail(vars, events, op, "not_in_lobby") };
            if cur.host != hub.display_name {
                return lobby_fail(vars, events, op, "not_host");
            }
            if cur.started {
                return lobby_fail(vars, events, op, "started");
            }
            if (cur.members.len() as u32) < MIN_LOBBY_PLAYERS {
                return lobby_fail(vars, events, op, "too_few_players");
            }
            RequestKind::StartLobby
        }
        _ => return,
    };
    if hub.is_busy(&kind) {
        return lobby_fail(vars, events, op, "busy");
    }
    hub.enqueue(kind);
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
    hub.clock = now;
    let mut batch: Vec<BackendEvent> = Vec::new();
    // Only touch the variables when a lobby really expires: a `&mut vars` every frame would mark them changed every frame.
    if lobby_tick(hub, now) {
        drop_lobby(hub, &mut vars, &mut events, Some("server"));
    }

    let mut slot = slot.filter(|s| s.0.is_some());
    match slot.as_mut() {
        Some(slot) => {
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
    if let Some(backend) = slot.as_mut().and_then(|s| s.0.as_mut()) {
        for id in &expired {
            backend.cancel(*id);
        }
    }
    for id in expired {
        let kind = hub.in_flight[&id].kind.clone();
        warn!("backend request {id} ({kind:?}) timed out after {}s", hub.request_timeout_secs);
        batch.push(BackendEvent { request_id: id, reply: kind.failure("timed out") });
    }

    for ev in batch {
        let flight = if ev.request_id != 0 {
            match hub.in_flight.remove(&ev.request_id) {
                Some(f) => Some(f),
                None => {
                    debug!("ignoring a reply to unknown or already-failed request {}", ev.request_id);
                    continue;
                }
            }
        } else {
            None
        };
        // A reply made under an older membership (the player left, the lobby closed, they signed out) must not bring it back.
        if flight.as_ref().is_some_and(|f| f.lobby_gen != hub.lobby.gen) && ev.reply.is_lobby() {
            debug!("dropping a stale lobby reply to request {}", ev.request_id);
            continue;
        }
        apply_reply(hub, &mut vars, &mut events, ev.reply, now);
    }
}

fn apply_reply(hub: &mut BackendHub, vars: &mut GameVariables, events: &mut MessageWriter<GameEvent>, reply: BackendReply, now: f64) {
    match reply {
        BackendReply::SignedIn { display_name } => {
            set_status(hub, vars, BackendStatus::Online);
            hub.display_name = display_name.clone();
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
            hub.display_name.clear();
            // The previous player's board, rank and lobby must not stay on screen for the next one.
            vars.0.retain(|k, _| !k.starts_with("leaderboard.") && !k.starts_with("lobby."));
            hub.lobby = LobbyTracker { gen: hub.lobby.gen + 1, ..Default::default() };
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
        BackendReply::LobbyListed { rows } => {
            let rows: Vec<LobbyRow> = rows.into_iter().take(MAX_LOBBY_ROWS as usize).collect();
            hub.lobby.rows = rows.clone();
            hub.lobby.listed_at = Some(now);
            write_lobby_list(vars, events, &rows);
        }
        BackendReply::LobbyListFailed { reason } => lobby_fail(vars, events, "list", &reason),
        BackendReply::LobbyCreateFailed { reason } => lobby_fail(vars, events, "create", &reason),
        BackendReply::LobbyJoinFailed { reason } => lobby_fail(vars, events, "join", &reason),
        BackendReply::LobbyStartFailed { reason } => lobby_fail(vars, events, "start", &reason),
        BackendReply::LobbyCreated { lobby } => enter_lobby(hub, vars, events, lobby, "lobby.created", now),
        BackendReply::LobbyJoined { lobby } => enter_lobby(hub, vars, events, lobby, "lobby.joined", now),
        BackendReply::LobbyLeft => drop_lobby(hub, vars, events, None),
        BackendReply::LobbyStarted { lobby } => {
            if hub.lobby.current.is_none() {
                return;
            }
            put(vars, "lobby.last_error", String::new());
            write_current_lobby(&hub.display_name, vars, &lobby);
            hub.lobby.current = Some(lobby);
            hub.lobby.last_ok = now;
            emit(events, "lobby.started");
        }
        BackendReply::LobbyState { lobby } => apply_lobby_state(hub, vars, events, lobby, now),
        BackendReply::LobbyClosed { reason } => {
            if hub.lobby.current.is_some() {
                drop_lobby(hub, vars, events, Some(fixed_reason(&reason)));
            }
        }
        BackendReply::LobbyHeartbeatFailed { reason } => {
            debug!("lobby heartbeat failed: {reason}");
            if hub.lobby.current.is_some() && !hub.lobby.trouble {
                hub.lobby.trouble = true;
                emit(events, "lobby.connection_trouble");
            }
        }
    }
}

// ─── Lobbies ─────────────────────────────────────────────────────────────────

/// A reason an adapter reported, as one of the fixed tokens a designer can write (`full`, `gone`, ...); anything else is `server`.
fn fixed_reason(reason: &str) -> &'static str {
    LOBBY_REASONS.iter().copied().find(|r| *r == reason).unwrap_or("server")
}

/// Writes a variable only when its value differs, so a heartbeat that changes nothing leaves the variables alone.
fn put(vars: &mut GameVariables, key: &str, value: String) {
    if vars.0.get(key) != Some(&value) {
        vars.0.insert(key.to_string(), value);
    }
}

/// `lobby.last_error` first, then the generic failure event, then the reason form.
fn lobby_fail(vars: &mut GameVariables, events: &mut MessageWriter<GameEvent>, op: &str, reason: &str) {
    let reason = fixed_reason(reason);
    put(vars, "lobby.last_error", reason.to_string());
    emit(events, format!("lobby.{op}_failed"));
    emit(events, format!("lobby.{op}_failed:{reason}"));
}

/// `lobby.count`, `lobby.<n>.name`/`.players`/`.host`/`.id` (n = 1..=10), then `lobby.listed`, one row event per row (a state machine
/// cannot test a variable, so these are how a designer shows or hides a row) and `lobby.list_empty`.
fn write_lobby_list(vars: &mut GameVariables, events: &mut MessageWriter<GameEvent>, rows: &[LobbyRow]) {
    let shown = rows.len().min(MAX_LOBBY_ROWS as usize);
    put(vars, "lobby.count", shown.to_string());
    for (i, row) in rows.iter().take(shown).enumerate() {
        let n = i + 1;
        put(vars, &format!("lobby.{n}.name"), row.name.clone());
        put(vars, &format!("lobby.{n}.players"), format!("{}/{}", row.players, row.max));
        put(vars, &format!("lobby.{n}.host"), row.host.clone());
        put(vars, &format!("lobby.{n}.id"), row.id.clone());
    }
    for n in shown + 1..=MAX_LOBBY_ROWS as usize {
        for field in ["name", "players", "host", "id"] {
            vars.0.remove(&format!("lobby.{n}.{field}"));
        }
    }
    put(vars, "lobby.last_error", String::new());
    emit(events, "lobby.listed");
    for n in 1..=MAX_LOBBY_ROWS as usize {
        emit(events, if n <= shown { format!("lobby.row_filled:{n}") } else { format!("lobby.row_empty:{n}") });
    }
    if shown == 0 {
        emit(events, "lobby.list_empty");
    }
}

/// `lobby.current.name`/`.players`/`.is_host`/`.id` and `lobby.member.<n>.name` (n = 1..=8, the host first).
fn write_current_lobby(own_name: &str, vars: &mut GameVariables, lobby: &LobbyInfo) {
    put(vars, "lobby.current.name", lobby.name.clone());
    put(vars, "lobby.current.players", format!("{}/{}", lobby.members.len(), lobby.max));
    put(vars, "lobby.current.is_host", if lobby.host == own_name { "yes" } else { "no" }.to_string());
    put(vars, "lobby.current.id", lobby.id.clone());
    let shown = lobby.members.len().min(MAX_LOBBY_PLAYERS as usize);
    for (i, name) in lobby.members.iter().take(shown).enumerate() {
        put(vars, &format!("lobby.member.{}.name", i + 1), name.clone());
    }
    for n in shown + 1..=MAX_LOBBY_PLAYERS as usize {
        vars.0.remove(&format!("lobby.member.{n}.name"));
    }
}

fn emit_member_rows(events: &mut MessageWriter<GameEvent>, members: usize) {
    for n in 1..=MAX_LOBBY_PLAYERS as usize {
        emit(events, if n <= members { format!("lobby.member_row_filled:{n}") } else { format!("lobby.member_row_empty:{n}") });
    }
}

fn enter_lobby(hub: &mut BackendHub, vars: &mut GameVariables, events: &mut MessageWriter<GameEvent>, lobby: LobbyInfo, event: &str, now: f64) {
    put(vars, "lobby.last_error", String::new());
    write_current_lobby(&hub.display_name, vars, &lobby);
    let members = lobby.members.len();
    hub.lobby.current = Some(lobby);
    hub.lobby.last_ok = now;
    hub.lobby.next_heartbeat = now + LOBBY_HEARTBEAT_SECS;
    hub.lobby.trouble = false;
    emit(events, event);
    emit_member_rows(events, members);
}

/// Ends the membership: the generation moves on (so late replies are dropped), `lobby.current.*` and `lobby.member.*` go together,
/// and the designer hears `lobby.left` or `lobby.closed` plus `lobby.closed:<reason>`.
fn drop_lobby(hub: &mut BackendHub, vars: &mut GameVariables, events: &mut MessageWriter<GameEvent>, closed_reason: Option<&str>) {
    hub.lobby.gen += 1;
    hub.lobby.current = None;
    hub.lobby.trouble = false;
    vars.0.retain(|k, _| !k.starts_with("lobby.current.") && !k.starts_with("lobby.member."));
    match closed_reason {
        None => emit(events, "lobby.left"),
        Some(reason) => {
            emit(events, "lobby.closed");
            emit(events, format!("lobby.closed:{reason}"));
        }
    }
}

/// A heartbeat answer: diff against the lobby we had, so `lobby.member_joined`, `lobby.member_left` and `lobby.started` come from one place
/// (and the fake gets them for free). The player's own join and leave are not announced; `lobby.created`/`joined`/`left` cover those.
fn apply_lobby_state(hub: &mut BackendHub, vars: &mut GameVariables, events: &mut MessageWriter<GameEvent>, lobby: LobbyInfo, now: f64) {
    let Some(prev) = hub.lobby.current.clone() else { return };
    if prev.id != lobby.id {
        debug!("ignoring a lobby update for {} while in {}", lobby.id, prev.id);
        return;
    }
    hub.lobby.last_ok = now;
    hub.lobby.trouble = false;
    write_current_lobby(&hub.display_name, vars, &lobby);
    let own = hub.display_name.as_str();
    for m in lobby.members.iter().filter(|m| !prev.members.contains(m) && m.as_str() != own) {
        debug!("{m} joined the lobby");
        emit(events, "lobby.member_joined");
    }
    for m in prev.members.iter().filter(|m| !lobby.members.contains(m) && m.as_str() != own) {
        debug!("{m} left the lobby");
        emit(events, "lobby.member_left");
    }
    if lobby.members != prev.members {
        emit_member_rows(events, lobby.members.len());
    }
    if lobby.started && !prev.started {
        emit(events, "lobby.started");
    }
    hub.lobby.current = Some(lobby);
}

/// Per glue frame: send the next heartbeat when it is due. Returns true when the heartbeats have failed for the whole expiry window (the
/// server has dropped us by then) and the caller must close the lobby. Members stop heartbeating once the lobby has started.
fn lobby_tick(hub: &mut BackendHub, now: f64) -> bool {
    let Some(cur) = hub.lobby.current.as_ref() else { return false };
    if hub.lobby.trouble && now - hub.lobby.last_ok >= LOBBY_EXPIRY_SECS {
        warn!("lobby heartbeats failed for {LOBBY_EXPIRY_SECS}s: the lobby is closed for this player");
        return true;
    }
    if cur.started || now < hub.lobby.next_heartbeat {
        return false;
    }
    let kind = RequestKind::LobbyHeartbeat { lobby_id: cur.id.clone() };
    if hub.is_busy(&kind) {
        return false;
    }
    // Deterministic jitter (no RNG in core): the request counter spreads players who joined in the same frame by up to a second.
    hub.lobby.next_heartbeat = now + LOBBY_HEARTBEAT_SECS + (hub.next_id % 5) as f64 * 0.25;
    hub.enqueue(kind);
    false
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
    /// Every `JoinLobby` answers `full`.
    pub join_full: bool,
    /// Every `JoinLobby` answers `gone`.
    pub gone: bool,
    /// `StartLobby` fails (`server`).
    pub start: bool,
    /// Heartbeats fail: `lobby.connection_trouble`, and after the expiry window `lobby.closed:server`.
    pub heartbeat: bool,
}

/// A lobby inside the fake. The other members are scripted rivals, so a demo has someone to wait for.
struct FakeLobby {
    id: String,
    name: String,
    max: u32,
    started: bool,
    host: String,
    members: Vec<String>,
}

impl FakeLobby {
    fn info(&self) -> LobbyInfo {
        LobbyInfo {
            id: self.id.clone(),
            name: self.name.clone(),
            max: self.max,
            started: self.started,
            host: self.host.clone(),
            members: self.members.clone(),
        }
    }
}

/// The names the scripted lobby members come from, in the order they join.
const FAKE_RIVALS: [&str; 4] = ["Ada", "Grace", "Linus", "Margaret"];

/// An in-memory backend. Deterministic: latency counts `poll` calls (frames), never wall-clock time.
pub struct FakeBackend {
    latency_polls: u64,
    polls: u64,
    pending: Vec<(u64, BackendEvent)>,
    signed_in: bool,
    display_name: String,
    defs: BTreeMap<String, LeaderboardDef>,
    boards: BTreeMap<String, Vec<(String, i64)>>,
    lobbies: Vec<FakeLobby>,
    next_lobby: u32,
    /// The id of the lobby this player is in.
    mine: Option<String>,
    /// Heartbeats answered for the current lobby (drives the script of the other members).
    heartbeats: u32,
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
            lobbies: Vec::new(),
            next_lobby: 4,
            mine: None,
            heartbeats: 0,
            failures: FakeFailures::default(),
        };
        // Three fixed lobbies: one with room, one half full, one full. Named from the project's `lobby_names` (cycled).
        let name = |i: usize| config.lobby_names.get(i % config.lobby_names.len().max(1)).cloned().unwrap_or_else(|| "Lobby".to_string());
        for (i, (host, members, max)) in
            [("Ada", vec!["Ada"], 4u32), ("Grace", vec!["Grace", "Linus", "Margaret"], 4), ("Linus", vec!["Linus", "Margaret"], 2)]
                .into_iter()
                .enumerate()
        {
            fake.lobbies.push(FakeLobby {
                id: format!("fake-{}", i + 1),
                name: name(i),
                max,
                started: false,
                host: host.to_string(),
                members: members.into_iter().map(str::to_string).collect(),
            });
        }
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
                join_full: o.join_full,
                gone: o.gone,
                start: o.start,
                heartbeat: o.heartbeat,
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
            RequestKind::ListLobbies => {
                if f.unreachable {
                    return BackendReply::LobbyListFailed { reason: "server".into() };
                }
                if !self.signed_in {
                    return BackendReply::LobbyListFailed { reason: "not_signed_in".into() };
                }
                let rows = self
                    .lobbies
                    .iter()
                    .filter(|l| !l.started)
                    .map(|l| LobbyRow { id: l.id.clone(), name: l.name.clone(), players: l.members.len() as u32, max: l.max, host: l.host.clone() })
                    .collect();
                BackendReply::LobbyListed { rows }
            }
            RequestKind::CreateLobby { name, max_players } => {
                if f.unreachable {
                    return BackendReply::LobbyCreateFailed { reason: "server".into() };
                }
                if !self.signed_in {
                    return BackendReply::LobbyCreateFailed { reason: "not_signed_in".into() };
                }
                if self.mine.is_some() {
                    return BackendReply::LobbyCreateFailed { reason: "already_in_lobby".into() };
                }
                let lobby = FakeLobby {
                    id: format!("fake-{}", self.next_lobby),
                    name,
                    max: max_players.clamp(MIN_LOBBY_PLAYERS, MAX_LOBBY_PLAYERS),
                    started: false,
                    host: Self::PLAYER_NAME.to_string(),
                    members: vec![Self::PLAYER_NAME.to_string()],
                };
                self.next_lobby += 1;
                self.mine = Some(lobby.id.clone());
                self.heartbeats = 0;
                let info = lobby.info();
                self.lobbies.push(lobby);
                BackendReply::LobbyCreated { lobby: info }
            }
            RequestKind::JoinLobby { lobby_id } => {
                if f.unreachable {
                    return BackendReply::LobbyJoinFailed { reason: "server".into() };
                }
                if !self.signed_in {
                    return BackendReply::LobbyJoinFailed { reason: "not_signed_in".into() };
                }
                if self.mine.is_some() {
                    return BackendReply::LobbyJoinFailed { reason: "already_in_lobby".into() };
                }
                if f.gone {
                    return BackendReply::LobbyJoinFailed { reason: "gone".into() };
                }
                let Some(lobby) = self.lobbies.iter_mut().find(|l| l.id == lobby_id) else {
                    return BackendReply::LobbyJoinFailed { reason: "gone".into() };
                };
                if lobby.started {
                    return BackendReply::LobbyJoinFailed { reason: "started".into() };
                }
                if f.join_full || lobby.members.len() as u32 >= lobby.max {
                    return BackendReply::LobbyJoinFailed { reason: "full".into() };
                }
                lobby.members.push(Self::PLAYER_NAME.to_string());
                let info = lobby.info();
                self.mine = Some(info.id.clone());
                self.heartbeats = 0;
                BackendReply::LobbyJoined { lobby: info }
            }
            RequestKind::LeaveLobby => {
                if let Some(id) = self.mine.take() {
                    if let Some(i) = self.lobbies.iter().position(|l| l.id == id) {
                        self.lobbies[i].members.retain(|m| m != Self::PLAYER_NAME);
                        // A host who leaves closes the lobby; an emptied one goes away.
                        if self.lobbies[i].host == Self::PLAYER_NAME || self.lobbies[i].members.is_empty() {
                            self.lobbies.remove(i);
                        }
                    }
                }
                BackendReply::LobbyLeft
            }
            RequestKind::StartLobby => {
                let Some(id) = self.mine.clone() else { return BackendReply::LobbyStartFailed { reason: "not_in_lobby".into() } };
                if f.unreachable || f.start {
                    return BackendReply::LobbyStartFailed { reason: "server".into() };
                }
                let Some(lobby) = self.lobbies.iter_mut().find(|l| l.id == id) else {
                    return BackendReply::LobbyStartFailed { reason: "gone".into() };
                };
                if lobby.host != Self::PLAYER_NAME {
                    return BackendReply::LobbyStartFailed { reason: "not_host".into() };
                }
                if lobby.started {
                    return BackendReply::LobbyStartFailed { reason: "started".into() };
                }
                if (lobby.members.len() as u32) < MIN_LOBBY_PLAYERS {
                    return BackendReply::LobbyStartFailed { reason: "too_few_players".into() };
                }
                lobby.started = true;
                BackendReply::LobbyStarted { lobby: lobby.info() }
            }
            RequestKind::LobbyHeartbeat { lobby_id } => {
                if f.unreachable || f.heartbeat {
                    return BackendReply::LobbyHeartbeatFailed { reason: "server".into() };
                }
                if self.mine.as_deref() != Some(lobby_id.as_str()) {
                    return BackendReply::LobbyClosed { reason: "removed".into() };
                }
                self.heartbeats += 1;
                let beat = self.heartbeats;
                let Some(i) = self.lobbies.iter().position(|l| l.id == lobby_id) else {
                    self.mine = None;
                    return BackendReply::LobbyClosed { reason: "host_left".into() };
                };
                // The script of the other members, so a demo shows joins, a leave and a host who walks away. Nothing random.
                if self.lobbies[i].host != Self::PLAYER_NAME && beat == 6 {
                    self.lobbies.remove(i);
                    self.mine = None;
                    return BackendReply::LobbyClosed { reason: "host_left".into() };
                }
                let lobby = &mut self.lobbies[i];
                if !lobby.started {
                    if beat % 2 == 0 && (lobby.members.len() as u32) < lobby.max {
                        if let Some(rival) = FAKE_RIVALS.iter().find(|r| !lobby.members.iter().any(|m| m == *r)) {
                            lobby.members.push((*rival).to_string());
                        }
                    }
                    if beat == 5 && lobby.members.len() > 2 {
                        lobby.members.pop();
                    }
                }
                BackendReply::LobbyState { lobby: lobby.info() }
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
    endpoint_override: Option<Res<BackendEndpointOverride>>,
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
            let Some(mut config) = backend_configs.get(&handle).cloned() else { return };
            if let Some(o) = endpoint_override.as_deref() {
                info!("backend endpoint overridden by the runner: {:?}", o.0);
                config.endpoint = Some(o.0.clone());
            }
            let problems = config.problems();
            if !problems.is_empty() {
                error!("Invalid backend.ron: {} - running offline", problems.join("; "));
                return finish_offline(hub, &mut vars, &mut events, "backend.ron is invalid (see the log)");
            }
            let project_id = config_handle.as_deref().and_then(|h| project_configs.get(&h.0)).and_then(|p| p.project_id.clone());
            choose_backend(hub, slot, factory.as_deref(), project_id.as_deref(), &mut vars, &mut events, config, endpoint_override.is_some());
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
    endpoint_overridden: bool,
) -> Result<Box<dyn Backend>, String> {
    let Some(factory) = factory else {
        return Err("no server adapter in this build; use fake://".to_string());
    };
    let resolved = ResolvedBackendConfig {
        endpoint: endpoint.to_string(),
        game_id: config.effective_game_id(project_id).unwrap_or_default(),
        server_key: if endpoint_overridden { None } else { config.server_key.clone() },
        leaderboards: config.leaderboards.clone(),
    };
    // The adapter's reason is written for the screen (it never contains the server key), so it goes to `backend.last_error` as is.
    (factory.0 .0)(&resolved).map_err(|e| {
        error!("Backend adapter could not start: {e}");
        e.chars().take(120).collect()
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
    endpoint_overridden: bool,
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
        match build_adapter(factory, project_id, &config, &endpoint, endpoint_overridden) {
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
        assert!(build_adapter(Some(&factory), Some("my_game"), &config, "https://play.example.org", false).is_ok());
        let got = seen.lock().unwrap().clone().unwrap();
        assert_eq!(got.game_id, "my_game");
        assert_eq!(got.endpoint, "https://play.example.org");
        assert_eq!(got.leaderboards.len(), 1);
    }

    #[test]
    fn the_files_server_key_reaches_the_factory_unless_the_endpoint_was_overridden() {
        let seen = std::sync::Arc::new(std::sync::Mutex::new(None));
        let sink = seen.clone();
        let factory = BackendFactoryRes(BackendFactory::new(move |c| {
            *sink.lock().unwrap() = Some(c.server_key.clone());
            Ok(Box::new(FakeBackend::from_config(&https_config())))
        }));
        let mut config = https_config();
        config.server_key = Some("pub-key".to_string());
        assert!(build_adapter(Some(&factory), None, &config, "https://play.example.org", false).is_ok());
        assert_eq!(seen.lock().unwrap().clone().unwrap().as_deref(), Some("pub-key"));
        assert!(build_adapter(Some(&factory), None, &config, "http://other:7350", true).is_ok());
        assert_eq!(seen.lock().unwrap().clone().unwrap(), None, "the file's key must not follow an overridden endpoint");
    }

    #[test]
    fn no_factory_or_a_failing_factory_means_offline_with_a_reason() {
        let config = https_config();
        let err = build_adapter(None, None, &config, "https://x", false).err().unwrap();
        assert!(err.contains("no server adapter"));
        let failing = BackendFactoryRes(BackendFactory::new(|_| Err("boom".to_string())));
        let err = build_adapter(Some(&failing), None, &config, "https://x", false).err().unwrap();
        assert_eq!(err, "boom", "the adapter's reason reaches the screen");
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

    fn lobby_config() -> BackendConfig {
        ron::from_str(
            r#"BackendConfig(schema_version: 1, endpoint: Some("fake://"), features: [Lobbies], lobby_names: ["Demo lobby", "Quick game"])"#,
        )
        .unwrap()
    }

    fn ask(fake: &mut FakeBackend, id: u64, kind: RequestKind) -> BackendReply {
        fake.submit(BackendRequest { id, kind });
        drain(fake, 1).remove(0).reply
    }

    fn lobby_fake() -> FakeBackend {
        let mut fake = FakeBackend::from_config(&lobby_config()).with_latency(0);
        sign_in(&mut fake);
        fake
    }

    #[test]
    fn the_fake_lists_three_seeded_lobbies_named_from_the_config() {
        let mut fake = lobby_fake();
        let BackendReply::LobbyListed { rows } = ask(&mut fake, 2, RequestKind::ListLobbies) else { panic!() };
        assert_eq!(rows.len(), 3);
        assert_eq!((rows[0].name.as_str(), rows[1].name.as_str(), rows[2].name.as_str()), ("Demo lobby", "Quick game", "Demo lobby"));
        assert_eq!((rows[2].players, rows[2].max), (2, 2), "the third lobby is full");
    }

    #[test]
    fn create_join_leave_and_start_follow_the_rules() {
        let mut fake = lobby_fake();
        let BackendReply::LobbyCreated { lobby } = ask(&mut fake, 2, RequestKind::CreateLobby { name: "Demo lobby".into(), max_players: 3 }) else {
            panic!()
        };
        assert_eq!((lobby.host.as_str(), lobby.members.clone(), lobby.max), ("Player", vec!["Player".to_string()], 3));
        assert!(matches!(
            ask(&mut fake, 3, RequestKind::CreateLobby { name: "Demo lobby".into(), max_players: 3 }),
            BackendReply::LobbyCreateFailed { reason } if reason == "already_in_lobby"
        ));
        assert!(matches!(ask(&mut fake, 4, RequestKind::StartLobby), BackendReply::LobbyStartFailed { reason } if reason == "too_few_players"));
        assert!(matches!(ask(&mut fake, 5, RequestKind::LeaveLobby), BackendReply::LobbyLeft));
        // Joining: the full lobby refuses, a lobby with room takes the player, a joiner is not the host.
        assert!(matches!(ask(&mut fake, 6, RequestKind::JoinLobby { lobby_id: "fake-3".into() }), BackendReply::LobbyJoinFailed { reason } if reason == "full"));
        let BackendReply::LobbyJoined { lobby } = ask(&mut fake, 7, RequestKind::JoinLobby { lobby_id: "fake-1".into() }) else { panic!() };
        assert_eq!(lobby.members, vec!["Ada".to_string(), "Player".to_string()]);
        assert!(matches!(ask(&mut fake, 8, RequestKind::StartLobby), BackendReply::LobbyStartFailed { reason } if reason == "not_host"));
        assert!(matches!(ask(&mut fake, 9, RequestKind::JoinLobby { lobby_id: "fake-2".into() }), BackendReply::LobbyJoinFailed { reason } if reason == "already_in_lobby"));
    }

    #[test]
    fn a_hosted_lobby_fills_up_on_heartbeats_and_can_then_be_started() {
        let mut fake = lobby_fake();
        let BackendReply::LobbyCreated { lobby } = ask(&mut fake, 2, RequestKind::CreateLobby { name: "Quick game".into(), max_players: 4 }) else {
            panic!()
        };
        let beat = |fake: &mut FakeBackend, id: u64| ask(fake, id, RequestKind::LobbyHeartbeat { lobby_id: lobby.id.clone() });
        let BackendReply::LobbyState { lobby: l1 } = beat(&mut fake, 3) else { panic!() };
        assert_eq!(l1.members.len(), 1, "nobody joins on the first heartbeat");
        let BackendReply::LobbyState { lobby: l2 } = beat(&mut fake, 4) else { panic!() };
        assert_eq!(l2.members, vec!["Player".to_string(), "Ada".to_string()]);
        let BackendReply::LobbyStarted { lobby: started } = ask(&mut fake, 5, RequestKind::StartLobby) else { panic!() };
        assert!(started.started);
        assert!(matches!(ask(&mut fake, 6, RequestKind::StartLobby), BackendReply::LobbyStartFailed { reason } if reason == "started"));
        let BackendReply::LobbyListed { rows } = ask(&mut fake, 7, RequestKind::ListLobbies) else { panic!() };
        assert!(rows.iter().all(|r| r.id != started.id), "a started lobby is not listed");
    }

    #[test]
    fn a_joined_lobbys_host_walks_away_on_the_sixth_heartbeat() {
        let mut fake = lobby_fake();
        let _ = ask(&mut fake, 2, RequestKind::JoinLobby { lobby_id: "fake-1".into() });
        let hb = RequestKind::LobbyHeartbeat { lobby_id: "fake-1".into() };
        for id in 3..8 {
            assert!(matches!(ask(&mut fake, id, hb.clone()), BackendReply::LobbyState { .. }), "beat {}", id - 2);
        }
        assert!(matches!(ask(&mut fake, 8, hb.clone()), BackendReply::LobbyClosed { reason } if reason == "host_left"));
        assert!(matches!(ask(&mut fake, 9, hb), BackendReply::LobbyClosed { reason } if reason == "removed"), "no longer in any lobby");
    }

    #[test]
    fn the_lobby_failure_switches_do_what_they_say() {
        let failing = |f: FakeFailures| FakeBackend::from_config(&lobby_config()).with_latency(0).with_failures(f);
        let mut full = failing(FakeFailures { join_full: true, ..Default::default() });
        sign_in(&mut full);
        assert!(matches!(ask(&mut full, 2, RequestKind::JoinLobby { lobby_id: "fake-1".into() }), BackendReply::LobbyJoinFailed { reason } if reason == "full"));
        let mut gone = failing(FakeFailures { gone: true, ..Default::default() });
        sign_in(&mut gone);
        assert!(matches!(ask(&mut gone, 2, RequestKind::JoinLobby { lobby_id: "fake-1".into() }), BackendReply::LobbyJoinFailed { reason } if reason == "gone"));
        let mut start = failing(FakeFailures { start: true, ..Default::default() });
        sign_in(&mut start);
        let _ = ask(&mut start, 2, RequestKind::CreateLobby { name: "Demo lobby".into(), max_players: 4 });
        assert!(matches!(ask(&mut start, 3, RequestKind::StartLobby), BackendReply::LobbyStartFailed { reason } if reason == "server"));
        let mut beat = failing(FakeFailures { heartbeat: true, ..Default::default() });
        sign_in(&mut beat);
        assert!(matches!(
            ask(&mut beat, 2, RequestKind::LobbyHeartbeat { lobby_id: "fake-1".into() }),
            BackendReply::LobbyHeartbeatFailed { .. }
        ));
    }

    #[test]
    fn lobby_requests_before_sign_in_fail_and_reasons_outside_the_fixed_set_become_server() {
        let mut fake = FakeBackend::from_config(&lobby_config()).with_latency(0);
        assert!(matches!(ask(&mut fake, 1, RequestKind::ListLobbies), BackendReply::LobbyListFailed { reason } if reason == "not_signed_in"));
        assert_eq!(fixed_reason("full"), "full");
        assert_eq!(fixed_reason("host_left"), "host_left");
        assert_eq!(fixed_reason("HTTP 500 at /x"), "server");
    }

    #[test]
    fn failure_replies_for_lobby_requests_use_fixed_reasons_and_a_leave_still_leaves() {
        assert!(matches!(RequestKind::ListLobbies.failure("timed out"), BackendReply::LobbyListFailed { reason } if reason == "server"));
        assert!(matches!(RequestKind::LeaveLobby.failure("timed out"), BackendReply::LobbyLeft));
        let hb = RequestKind::LobbyHeartbeat { lobby_id: "x".into() };
        assert!(matches!(hb.failure("timed out"), BackendReply::LobbyHeartbeatFailed { .. }));
        // The heartbeat has its own dedupe key; leave, join, create and start share one.
        assert_ne!(hb.dedupe_key(), RequestKind::LeaveLobby.dedupe_key());
        assert_eq!(RequestKind::StartLobby.dedupe_key(), RequestKind::LeaveLobby.dedupe_key());
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
