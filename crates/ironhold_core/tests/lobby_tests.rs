//! Online backend v3a-1 (`planning/features/backend_abstraction.md`): lobbies through the glue system and the `FakeBackend`. Covers the
//! actions' own refusals (reason events), the list and its row events, the waiting room, the core-driven heartbeat and its diff, the
//! stale-reply drop, sign-out leaving the lobby, and the expiry window. Nothing here touches a network.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use bevy::prelude::*;

use ironhold_core::runtime::backend::{
    Backend, BackendEvent, BackendHub, BackendReply, BackendRequest, BackendSlot, FakeBackend, FakeFailures, LobbyInfo, LobbyRow,
    RequestKind,
};
use ironhold_core::runtime::{ActionQueue, GameEvent, GameplaySet};
use ironhold_core::schema::backend::BackendConfig;
use ironhold_core::schema::Action;
use ironhold_core::GameVariables;

mod support;
use support::setup_test_app;

#[derive(Resource, Default)]
struct Seen(Vec<String>);

fn record(mut r: MessageReader<GameEvent>, mut seen: ResMut<Seen>) {
    for GameEvent::Trigger(n) in r.read() {
        seen.0.push(n.clone());
    }
}

fn config(lobbies: bool) -> BackendConfig {
    let features = if lobbies { "features: [Lobbies]," } else { "" };
    ron::from_str(&format!(
        r#"BackendConfig(schema_version: 1, endpoint: Some("fake://"), {features} lobby_names: ["Demo lobby", "Quick game"])"#
    ))
    .unwrap()
}

fn app_with(backend: impl Backend + 'static, lobbies: bool) -> App {
    let mut app = setup_test_app();
    app.init_resource::<Seen>();
    app.add_systems(Update, record.in_set(GameplaySet::Interpret).after(ironhold_core::runtime::entity_fsm_interpreter_system));
    app.update();
    *app.world_mut().resource_mut::<BackendHub>() = BackendHub::ready(config(lobbies));
    app.world_mut().non_send_resource_mut::<BackendSlot>().install(Box::new(backend));
    app
}

fn fake() -> FakeBackend {
    FakeBackend::from_config(&config(true)).with_latency(1)
}

/// Pushes the action and runs two frames: the executor emits a refusal in the first, and the recorder (which reads where the
/// interpreters do, earlier in the frame) sees it in the second.
fn act(app: &mut App, a: Action) {
    app.world_mut().resource_mut::<ActionQueue>().push(a);
    app.update();
    app.update();
}

fn settle(app: &mut App) {
    for _ in 0..6 {
        app.update();
    }
}

/// Moves `Time<Real>` forward (the heartbeat and the expiry count on it) and runs one frame.
fn advance(app: &mut App, secs: f64) {
    app.world_mut().resource_mut::<Time<Real>>().advance_by(Duration::from_secs_f64(secs));
    app.update();
}

fn seen(app: &App) -> Vec<String> {
    app.world().resource::<Seen>().0.clone()
}

fn count(app: &App, name: &str) -> usize {
    seen(app).iter().filter(|n| *n == name).count()
}

fn clear_seen(app: &mut App) {
    app.world_mut().resource_mut::<Seen>().0.clear();
}

fn var(app: &App, key: &str) -> Option<String> {
    app.world().resource::<GameVariables>().0.get(key).cloned()
}

fn create(name: &str, max_players: u32) -> Action {
    Action::CreateLobby { name: name.into(), max_players }
}

fn signed_in_app() -> App {
    let mut app = app_with(fake(), true);
    act(&mut app, Action::SignIn);
    settle(&mut app);
    app
}

/// A backend that answers every request with whatever the closure says (after `delay` extra polls), and remembers what it was asked.
struct Scripted {
    pending: Vec<(u64, BackendEvent)>,
    polls: u64,
    asked: Arc<Mutex<Vec<RequestKind>>>,
    reply: Box<dyn FnMut(&RequestKind) -> BackendReply>,
    delay: Box<dyn Fn(&RequestKind) -> u64>,
}

impl Backend for Scripted {
    fn submit(&mut self, req: BackendRequest) {
        self.asked.lock().unwrap().push(req.kind.clone());
        let due = self.polls + 1 + (self.delay)(&req.kind);
        let reply = (self.reply)(&req.kind);
        self.pending.push((due, BackendEvent { request_id: req.id, reply }));
    }
    fn poll(&mut self, out: &mut Vec<BackendEvent>) {
        self.polls += 1;
        let now = self.polls;
        let (due, later): (Vec<_>, Vec<_>) = std::mem::take(&mut self.pending).into_iter().partition(|(d, _)| *d <= now);
        self.pending = later;
        out.extend(due.into_iter().map(|(_, e)| e));
    }
}

fn scripted(reply: impl FnMut(&RequestKind) -> BackendReply + 'static) -> (Scripted, Arc<Mutex<Vec<RequestKind>>>) {
    scripted_with_delay(reply, |_| 0)
}

fn scripted_with_delay(
    reply: impl FnMut(&RequestKind) -> BackendReply + 'static,
    delay: impl Fn(&RequestKind) -> u64 + 'static,
) -> (Scripted, Arc<Mutex<Vec<RequestKind>>>) {
    let asked = Arc::new(Mutex::new(Vec::new()));
    (Scripted { pending: Vec::new(), polls: 0, asked: asked.clone(), reply: Box::new(reply), delay: Box::new(delay) }, asked)
}

fn info(id: &str, members: &[&str], max: u32) -> LobbyInfo {
    info_v(id, members, max, 1, false)
}

fn info_v(id: &str, members: &[&str], max: u32, version: u64, started: bool) -> LobbyInfo {
    LobbyInfo {
        id: id.into(),
        version,
        name: "Demo lobby".into(),
        max,
        started,
        host: members[0].into(),
        members: members.iter().map(|m| m.to_string()).collect(),
    }
}

/// The fake plus a mailbox: whatever a test puts in it arrives as an unsolicited reply (request id 0), like a dropped session would.
struct Injecting {
    inner: FakeBackend,
    inbox: Arc<Mutex<Vec<BackendEvent>>>,
}

impl Backend for Injecting {
    fn submit(&mut self, req: BackendRequest) {
        self.inner.submit(req);
    }
    fn poll(&mut self, out: &mut Vec<BackendEvent>) {
        self.inner.poll(out);
        out.append(&mut self.inbox.lock().unwrap());
    }
}

#[test]
fn listing_writes_the_rows_and_fires_one_row_event_per_row_in_order() {
    let mut app = signed_in_app();
    clear_seen(&mut app);
    act(&mut app, Action::ListLobbies);
    settle(&mut app);
    let mut expected = vec!["lobby.listed".to_string()];
    expected.extend((1..=3).map(|n| format!("lobby.row_filled:{n}")));
    expected.extend((4..=10).map(|n| format!("lobby.row_empty:{n}")));
    assert_eq!(seen(&app), expected, "the list event first, then every row, and no empty-list event");
    assert_eq!(var(&app, "lobby.count").as_deref(), Some("3"));
    assert_eq!(var(&app, "lobby.1.name").as_deref(), Some("Demo lobby"));
    assert_eq!(var(&app, "lobby.2.name").as_deref(), Some("Quick game"));
    assert_eq!(var(&app, "lobby.1.players").as_deref(), Some("1/4"));
    assert_eq!(var(&app, "lobby.3.players").as_deref(), Some("2/2"));
    assert_eq!(var(&app, "lobby.2.host").as_deref(), Some("Grace"));
    assert_eq!(var(&app, "lobby.4.name"), None, "rows beyond the count are absent");
}

#[test]
fn an_empty_list_says_so_and_clears_the_old_rows() {
    let mut rows = vec![LobbyRow { id: "a".into(), name: "Demo lobby".into(), players: 1, max: 4, host: "Ada".into() }];
    let (backend, _) = scripted(move |kind| match kind {
        RequestKind::SignIn => BackendReply::SignedIn { display_name: "Player".into() },
        RequestKind::ListLobbies => BackendReply::LobbyListed { rows: std::mem::take(&mut rows) },
        _ => BackendReply::LobbyLeft,
    });
    let mut app = app_with(backend, true);
    act(&mut app, Action::SignIn);
    settle(&mut app);
    act(&mut app, Action::ListLobbies);
    settle(&mut app);
    assert_eq!(var(&app, "lobby.count").as_deref(), Some("1"));
    // Past the 5 s minimum, so the server is asked again, and it now has nothing.
    advance(&mut app, 6.0);
    clear_seen(&mut app);
    act(&mut app, Action::ListLobbies);
    settle(&mut app);
    assert_eq!(var(&app, "lobby.count").as_deref(), Some("0"));
    assert_eq!(var(&app, "lobby.1.name"), None, "the old row is gone");
    assert_eq!(count(&app, "lobby.list_empty"), 1);
    assert_eq!(count(&app, "lobby.row_empty:1"), 1);
    assert_eq!(count(&app, "lobby.row_filled:1"), 0);
}

#[test]
fn a_second_list_inside_five_seconds_is_answered_from_the_first_without_asking_the_server() {
    let (backend, asked) = scripted(|kind| match kind {
        RequestKind::SignIn => BackendReply::SignedIn { display_name: "Player".into() },
        _ => BackendReply::LobbyListed { rows: vec![] },
    });
    let mut app = app_with(backend, true);
    act(&mut app, Action::SignIn);
    settle(&mut app);
    act(&mut app, Action::ListLobbies);
    settle(&mut app);
    clear_seen(&mut app);
    act(&mut app, Action::ListLobbies);
    assert_eq!(count(&app, "lobby.listed"), 1, "answered in the same frame, from the snapshot");
    let lists = asked.lock().unwrap().iter().filter(|k| **k == RequestKind::ListLobbies).count();
    assert_eq!(lists, 1, "the server was asked once");
    advance(&mut app, 6.0);
    act(&mut app, Action::ListLobbies);
    settle(&mut app);
    let lists = asked.lock().unwrap().iter().filter(|k| **k == RequestKind::ListLobbies).count();
    assert_eq!(lists, 2, "after the minimum it asks again");
}

#[test]
fn joining_a_row_enters_the_waiting_room_with_the_host_first() {
    let mut app = signed_in_app();
    act(&mut app, Action::ListLobbies);
    settle(&mut app);
    clear_seen(&mut app);
    act(&mut app, Action::JoinLobby { row: 1 });
    settle(&mut app);
    assert_eq!(count(&app, "lobby.joined"), 1);
    assert_eq!(var(&app, "lobby.current.name").as_deref(), Some("Demo lobby"));
    assert_eq!(var(&app, "lobby.current.players").as_deref(), Some("2/4"));
    assert_eq!(var(&app, "lobby.current.is_host").as_deref(), Some("no"));
    assert_eq!(var(&app, "lobby.member.1.name").as_deref(), Some("Ada"));
    assert_eq!(var(&app, "lobby.member.2.name").as_deref(), Some("Player"));
    assert_eq!(var(&app, "lobby.member.3.name"), None);
    assert_eq!((count(&app, "lobby.member_row_filled:2"), count(&app, "lobby.member_row_empty:3")), (1, 1));
    assert_eq!(var(&app, "lobby.last_error").as_deref(), Some(""));
}

#[test]
fn failures_fire_the_generic_event_then_the_reason_and_set_last_error() {
    let mut app = signed_in_app();
    // No list yet: the row is empty.
    clear_seen(&mut app);
    act(&mut app, Action::JoinLobby { row: 1 });
    assert_eq!(seen(&app), vec!["lobby.join_failed", "lobby.join_failed:no_lobby"], "generic first, so the specific message wins");
    assert_eq!(var(&app, "lobby.last_error").as_deref(), Some("no_lobby"));
    // Out of range.
    clear_seen(&mut app);
    act(&mut app, Action::JoinLobby { row: 11 });
    assert_eq!(count(&app, "lobby.join_failed:no_lobby"), 1);
    // A full lobby (row 3 is 2/2).
    act(&mut app, Action::ListLobbies);
    settle(&mut app);
    clear_seen(&mut app);
    act(&mut app, Action::JoinLobby { row: 3 });
    settle(&mut app);
    assert_eq!(seen(&app), vec!["lobby.join_failed", "lobby.join_failed:full"]);
    assert_eq!(var(&app, "lobby.last_error").as_deref(), Some("full"));
    assert_eq!(var(&app, "lobby.current.name"), None, "a failed join leaves no lobby behind");
}

#[test]
fn a_reason_the_adapter_invents_reads_as_server() {
    let (backend, _) = scripted(|kind| match kind {
        RequestKind::SignIn => BackendReply::SignedIn { display_name: "Player".into() },
        RequestKind::ListLobbies => BackendReply::LobbyListed {
            rows: vec![LobbyRow { id: "a".into(), name: "Demo lobby".into(), players: 1, max: 4, host: "Ada".into() }],
        },
        _ => BackendReply::LobbyJoinFailed { reason: "HTTP 500 at /v2/rpc".into() },
    });
    let mut app = app_with(backend, true);
    act(&mut app, Action::SignIn);
    settle(&mut app);
    act(&mut app, Action::ListLobbies);
    settle(&mut app);
    clear_seen(&mut app);
    act(&mut app, Action::JoinLobby { row: 1 });
    settle(&mut app);
    assert_eq!(seen(&app), vec!["lobby.join_failed", "lobby.join_failed:server"]);
    assert_eq!(var(&app, "lobby.last_error").as_deref(), Some("server"));
}

#[test]
fn the_actions_refuse_before_the_server_is_asked() {
    // Not signed in.
    let mut app = app_with(fake(), true);
    act(&mut app, Action::ListLobbies);
    assert_eq!(seen(&app), vec!["lobby.list_failed", "lobby.list_failed:not_signed_in"]);
    // The feature is not declared in backend.ron.
    let mut app = app_with(fake(), false);
    act(&mut app, Action::SignIn);
    settle(&mut app);
    clear_seen(&mut app);
    act(&mut app, Action::ListLobbies);
    assert_eq!(seen(&app), vec!["lobby.list_failed", "lobby.list_failed:not_enabled"]);
    // A name that is not in `lobby_names`.
    let mut app = signed_in_app();
    clear_seen(&mut app);
    act(&mut app, create("Secret room", 4));
    assert_eq!(seen(&app), vec!["lobby.create_failed", "lobby.create_failed:server"]);
    assert_eq!(app.world().resource::<BackendHub>().pending_requests(), 0, "nothing was sent");
}

#[test]
fn creating_makes_the_player_the_host_and_a_second_create_is_refused() {
    let mut app = signed_in_app();
    clear_seen(&mut app);
    act(&mut app, create("Quick game", 3));
    settle(&mut app);
    assert_eq!(count(&app, "lobby.created"), 1);
    assert_eq!(var(&app, "lobby.current.is_host").as_deref(), Some("yes"));
    assert_eq!(var(&app, "lobby.current.players").as_deref(), Some("1/3"));
    assert_eq!(var(&app, "lobby.member.1.name").as_deref(), Some("Player"));
    clear_seen(&mut app);
    act(&mut app, create("Quick game", 3));
    assert_eq!(seen(&app), vec!["lobby.create_failed", "lobby.create_failed:already_in_lobby"]);
    act(&mut app, Action::ListLobbies);
    settle(&mut app);
    clear_seen(&mut app);
    act(&mut app, Action::JoinLobby { row: 1 });
    assert_eq!(count(&app, "lobby.join_failed:already_in_lobby"), 1, "joining while in a lobby is refused too");
}

#[test]
fn a_second_lobby_action_while_one_is_in_flight_answers_busy() {
    let mut app = signed_in_app();
    clear_seen(&mut app);
    app.world_mut().resource_mut::<ActionQueue>().push(create("Demo lobby", 4));
    app.world_mut().resource_mut::<ActionQueue>().push(create("Quick game", 4));
    app.update();
    app.update();
    assert_eq!(count(&app, "lobby.create_failed:busy"), 1);
    settle(&mut app);
    assert_eq!(count(&app, "lobby.created"), 1, "the first one still goes through");
}

#[test]
fn start_is_for_the_host_with_company_and_only_once() {
    let mut app = signed_in_app();
    clear_seen(&mut app);
    act(&mut app, Action::StartLobby);
    assert_eq!(count(&app, "lobby.start_failed:not_in_lobby"), 1);
    act(&mut app, create("Demo lobby", 4));
    settle(&mut app);
    clear_seen(&mut app);
    act(&mut app, Action::StartLobby);
    assert_eq!(count(&app, "lobby.start_failed:too_few_players"), 1, "a host alone cannot start");
    // Two heartbeats later the scripted member Ada has joined.
    advance(&mut app, 12.0);
    settle(&mut app);
    advance(&mut app, 12.0);
    settle(&mut app);
    assert_eq!(count(&app, "lobby.member_joined"), 1);
    assert_eq!(var(&app, "lobby.current.players").as_deref(), Some("2/4"));
    clear_seen(&mut app);
    act(&mut app, Action::StartLobby);
    settle(&mut app);
    assert_eq!(count(&app, "lobby.started"), 1);
    clear_seen(&mut app);
    act(&mut app, Action::StartLobby);
    assert_eq!(count(&app, "lobby.start_failed:started"), 1);
}

#[test]
fn a_joiner_cannot_start() {
    let mut app = signed_in_app();
    act(&mut app, Action::ListLobbies);
    settle(&mut app);
    act(&mut app, Action::JoinLobby { row: 1 });
    settle(&mut app);
    clear_seen(&mut app);
    act(&mut app, Action::StartLobby);
    assert_eq!(seen(&app), vec!["lobby.start_failed", "lobby.start_failed:not_host"]);
}

#[test]
fn heartbeats_wait_for_the_interval_and_the_diff_announces_other_players_only() {
    let mut app = signed_in_app();
    act(&mut app, create("Demo lobby", 4));
    settle(&mut app);
    clear_seen(&mut app);
    advance(&mut app, 5.0);
    settle(&mut app);
    assert!(seen(&app).is_empty(), "no heartbeat before the interval: {:?}", seen(&app));
    advance(&mut app, 6.0);
    settle(&mut app);
    assert!(seen(&app).is_empty(), "the first heartbeat changes nothing");
    advance(&mut app, 12.0);
    settle(&mut app);
    assert_eq!(count(&app, "lobby.member_joined"), 1, "Ada joins on the second heartbeat");
    assert_eq!(count(&app, "lobby.member_row_filled:2"), 1);
    assert_eq!(count(&app, "lobby.member_left"), 0);
    assert_eq!(var(&app, "lobby.member.2.name").as_deref(), Some("Ada"));
    assert_eq!(var(&app, "lobby.member.1.name").as_deref(), Some("Player"), "the player's own name is in the list and never announced");
}

#[test]
fn a_member_who_leaves_is_announced_and_the_rows_compact() {
    let mut app = signed_in_app();
    act(&mut app, create("Demo lobby", 4));
    settle(&mut app);
    for _ in 0..4 {
        advance(&mut app, 12.0);
        settle(&mut app);
    }
    assert_eq!(var(&app, "lobby.current.players").as_deref(), Some("3/4"), "Ada and Grace joined");
    clear_seen(&mut app);
    advance(&mut app, 12.0);
    settle(&mut app);
    assert_eq!(count(&app, "lobby.member_left"), 1, "the scripted leave on the fifth heartbeat");
    assert_eq!(var(&app, "lobby.current.players").as_deref(), Some("2/4"));
    assert_eq!(var(&app, "lobby.member.3.name"), None);
    assert_eq!(count(&app, "lobby.member_row_empty:3"), 1);
}

#[test]
fn a_host_who_walks_away_closes_the_lobby_with_its_reason() {
    let mut app = signed_in_app();
    act(&mut app, Action::ListLobbies);
    settle(&mut app);
    // Row 2 is Grace's lobby: its scripted host leaves on the sixth heartbeat (Ada, row 1, starts hers on the fourth).
    act(&mut app, Action::JoinLobby { row: 2 });
    settle(&mut app);
    for _ in 0..5 {
        advance(&mut app, 12.0);
        settle(&mut app);
    }
    clear_seen(&mut app);
    advance(&mut app, 12.0);
    settle(&mut app);
    assert_eq!(seen(&app).iter().filter(|e| e.starts_with("lobby.closed")).cloned().collect::<Vec<_>>(), vec!["lobby.closed", "lobby.closed:host_left"]);
    assert_eq!(var(&app, "lobby.current.name"), None);
    assert_eq!(var(&app, "lobby.member.1.name"), None);
}

#[test]
fn leaving_clears_the_room_and_a_client_that_lost_track_still_leaves() {
    let mut app = signed_in_app();
    act(&mut app, create("Demo lobby", 4));
    settle(&mut app);
    clear_seen(&mut app);
    act(&mut app, Action::LeaveLobby);
    settle(&mut app);
    assert_eq!(count(&app, "lobby.left"), 1);
    assert_eq!(var(&app, "lobby.current.name"), None);
    assert_eq!(var(&app, "lobby.member.1.name"), None);
    // After a page reload there is no local lobby, but the server may still hold one: the leave is still sent.
    let (backend, asked) = scripted(|kind| match kind {
        RequestKind::SignIn => BackendReply::SignedIn { display_name: "Player".into() },
        _ => BackendReply::LobbyLeft,
    });
    let mut app = app_with(backend, true);
    act(&mut app, Action::SignIn);
    settle(&mut app);
    clear_seen(&mut app);
    act(&mut app, Action::LeaveLobby);
    settle(&mut app);
    assert_eq!(count(&app, "lobby.left"), 1);
    assert!(asked.lock().unwrap().contains(&RequestKind::LeaveLobby), "the server was told");
}

#[test]
fn a_heartbeat_answer_that_arrives_after_leaving_does_not_bring_the_lobby_back() {
    // Leaving is answered at once; the heartbeat that was already on the wire takes three more polls.
    let (backend, _) = scripted_with_delay(
        |kind| match kind {
            RequestKind::SignIn => BackendReply::SignedIn { display_name: "Player".into() },
            RequestKind::CreateLobby { .. } => BackendReply::LobbyCreated { lobby: info("L1", &["Player"], 4) },
            RequestKind::LobbyHeartbeat { .. } => BackendReply::LobbyState { lobby: info("L1", &["Player", "Ada"], 4) },
            _ => BackendReply::LobbyLeft,
        },
        |kind| if matches!(kind, RequestKind::LobbyHeartbeat { .. }) { 3 } else { 0 },
    );
    let mut app = app_with(backend, true);
    act(&mut app, Action::SignIn);
    settle(&mut app);
    act(&mut app, create("Demo lobby", 4));
    settle(&mut app);
    advance(&mut app, 12.0); // a heartbeat is now on the wire
    act(&mut app, Action::LeaveLobby);
    for _ in 0..10 {
        app.update();
    }
    assert_eq!(count(&app, "lobby.left"), 1);
    assert_eq!(var(&app, "lobby.current.name"), None, "the late heartbeat reply was dropped");
    assert_eq!(var(&app, "lobby.member.2.name"), None);
    assert_eq!(count(&app, "lobby.member_joined"), 0);
}

#[test]
fn a_create_answer_that_arrives_after_signing_out_is_dropped() {
    // The create is answered three polls late, the sign-out at once: the lobby it describes belongs to a session that no longer exists.
    let (backend, _) = scripted_with_delay(
        |kind| match kind {
            RequestKind::SignIn => BackendReply::SignedIn { display_name: "Player".into() },
            RequestKind::CreateLobby { .. } => BackendReply::LobbyCreated { lobby: info("L1", &["Player"], 4) },
            RequestKind::SignOut => BackendReply::SignedOut,
            _ => BackendReply::LobbyLeft,
        },
        |kind| if matches!(kind, RequestKind::CreateLobby { .. }) { 3 } else { 0 },
    );
    let mut app = app_with(backend, true);
    act(&mut app, Action::SignIn);
    settle(&mut app);
    clear_seen(&mut app);
    act(&mut app, create("Demo lobby", 4));
    act(&mut app, Action::SignOut);
    for _ in 0..10 {
        app.update();
    }
    assert_eq!(count(&app, "backend.signed_out"), 1);
    assert_eq!(count(&app, "lobby.created"), 0, "the late create answer was dropped");
    assert!(app.world().resource::<GameVariables>().0.keys().all(|k| !k.starts_with("lobby.")), "no lobby variable after a sign-out");
}

#[test]
fn signing_out_leaves_the_lobby_and_drops_every_reply_still_on_the_way() {
    let mut app = app_with(FakeBackend::from_config(&config(true)).with_latency(3), true);
    act(&mut app, Action::SignIn);
    for _ in 0..8 {
        app.update();
    }
    act(&mut app, Action::ListLobbies);
    for _ in 0..8 {
        app.update();
    }
    act(&mut app, create("Demo lobby", 4));
    for _ in 0..8 {
        app.update();
    }
    advance(&mut app, 12.0);
    clear_seen(&mut app);
    act(&mut app, Action::SignOut);
    for _ in 0..12 {
        app.update();
    }
    assert_eq!(count(&app, "lobby.left"), 1, "SignOut leaves the lobby, once");
    assert_eq!(count(&app, "backend.signed_out"), 1);
    assert!(app.world().resource::<GameVariables>().0.keys().all(|k| !k.starts_with("lobby.")), "no lobby variable survives a sign-out");
    assert_eq!(count(&app, "lobby.member_joined"), 0);
}

#[test]
fn failing_heartbeats_raise_trouble_once_and_close_the_lobby_after_the_expiry_window() {
    let mut app = app_with(
        FakeBackend::from_config(&config(true)).with_latency(1).with_failures(FakeFailures::default()),
        true,
    );
    act(&mut app, Action::SignIn);
    settle(&mut app);
    act(&mut app, create("Demo lobby", 4));
    settle(&mut app);
    // From here on the server stops answering heartbeats.
    app.world_mut().non_send_resource_mut::<BackendSlot>().0 = Some(Box::new(
        FakeBackend::from_config(&config(true)).with_latency(1).with_failures(FakeFailures { heartbeat: true, ..Default::default() }),
    ));
    clear_seen(&mut app);
    advance(&mut app, 12.0);
    settle(&mut app);
    assert_eq!(count(&app, "lobby.connection_trouble"), 1);
    advance(&mut app, 12.0);
    settle(&mut app);
    assert_eq!(count(&app, "lobby.connection_trouble"), 1, "trouble is raised once, not on every failed heartbeat");
    assert_eq!(count(&app, "lobby.closed"), 0, "still inside the expiry window");
    assert_eq!(var(&app, "lobby.current.name").as_deref(), Some("Demo lobby"));
    advance(&mut app, 60.0);
    settle(&mut app);
    assert_eq!(count(&app, "lobby.closed"), 1);
    assert_eq!(count(&app, "lobby.closed:server"), 1);
    assert_eq!(var(&app, "lobby.current.name"), None);
}

#[test]
fn the_fake_endpoint_switches_reach_the_lobby_actions() {
    let mut cfg = config(true);
    cfg.endpoint = Some("fake://?fail=join_full".into());
    let mut app = app_with(FakeBackend::from_endpoint(&cfg, "fake://?fail=join_full").with_latency(1), true);
    act(&mut app, Action::SignIn);
    settle(&mut app);
    act(&mut app, Action::ListLobbies);
    settle(&mut app);
    clear_seen(&mut app);
    act(&mut app, Action::JoinLobby { row: 1 });
    settle(&mut app);
    assert_eq!(seen(&app), vec!["lobby.join_failed", "lobby.join_failed:full"]);
}

#[test]
fn the_lobby_actions_parse_from_ron() {
    let a: Action = ron::from_str("CreateLobby(name: \"Demo lobby\", max_players: 4)").unwrap();
    assert_eq!(a, Action::CreateLobby { name: "Demo lobby".into(), max_players: 4 });
    assert_eq!(ron::from_str::<Action>("JoinLobby(row: 3)").unwrap(), Action::JoinLobby { row: 3 });
    for (text, action) in [("ListLobbies", Action::ListLobbies), ("LeaveLobby", Action::LeaveLobby), ("StartLobby", Action::StartLobby)] {
        assert_eq!(ron::from_str::<Action>(text).unwrap(), action);
    }
    // Unlike `Some(...)` style rules, a typo in a field name is an error, not a silent default.
    assert!(ron::from_str::<Action>("JoinLobby(rows: 3)").is_err());
}

#[test]
fn every_lobby_event_and_variable_the_engine_produces_is_in_the_closed_vocabulary() {
    use ironhold_core::schema::backend::{is_lobby_event, is_lobby_variable};
    let mut app = signed_in_app();
    // A tour through everything: refusals, list, create, heartbeats (joins, a leave), start, leave, a join and the host walking away.
    let first = [Action::JoinLobby { row: 1 }, Action::ListLobbies, Action::JoinLobby { row: 3 }, create("Nope", 4), create("Demo lobby", 4), Action::StartLobby];
    for a in first {
        act(&mut app, a);
        settle(&mut app);
    }
    for _ in 0..5 {
        advance(&mut app, 12.0);
        settle(&mut app);
    }
    for a in [Action::StartLobby, Action::LeaveLobby, Action::StartLobby, Action::JoinLobby { row: 1 }] {
        act(&mut app, a);
        settle(&mut app);
    }
    for _ in 0..7 {
        advance(&mut app, 12.0);
        settle(&mut app);
    }
    act(&mut app, Action::LeaveLobby);
    settle(&mut app);
    let lobby_events: Vec<String> = seen(&app).into_iter().filter(|e| e.starts_with("lobby.")).collect();
    assert!(lobby_events.len() > 40, "the tour should produce plenty of events, got {}", lobby_events.len());
    for e in &lobby_events {
        assert!(is_lobby_event(e), "the engine emitted {e:?}, which validate would call a typo");
    }
    // Every variable of a shorter tour, looked at after each step, is also known.
    let mut app = signed_in_app();
    let known_variables = |app: &App| {
        for k in app.world().resource::<GameVariables>().0.keys().filter(|k| k.starts_with("lobby.")) {
            assert!(is_lobby_variable(k), "the engine wrote {k:?}, which validate would call a typo");
        }
    };
    for a in [Action::ListLobbies, create("Quick game", 4)] {
        act(&mut app, a);
        settle(&mut app);
        known_variables(&app);
    }
    for _ in 0..3 {
        advance(&mut app, 12.0);
        settle(&mut app);
        known_variables(&app);
    }
}

/// `ui/*.ui.ron` of the lobby demo are the copy-and-edit snippets the docs point designers at: they must parse as a UI node and appear
/// verbatim in the demo scene, so the playable demo and the snippet can never drift apart.
#[test]
fn the_lobby_demos_snippets_parse_and_are_embedded_in_its_scene() {
    use ironhold_core::schema::UiNodeDef;
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/projects/backend_demo_lobby");
    let scene = std::fs::read_to_string(format!("{root}/scenes/main.scene.ron")).unwrap().replace("\r\n", "\n");
    for name in ["signin", "lobby"] {
        let text = std::fs::read_to_string(format!("{root}/ui/{name}.ui.ron")).unwrap().replace("\r\n", "\n");
        ron::Options::default()
            .with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME)
            .from_str::<UiNodeDef>(&text)
            .unwrap_or_else(|e| panic!("ui/{name}.ui.ron must parse as a UI node: {e}"));
        for line in text.lines().filter(|l| !l.trim().is_empty() && !l.starts_with("//")) {
            assert!(scene.contains(line.trim()), "ui/{name}.ui.ron line {line:?} is missing from scenes/main.scene.ron");
        }
    }
}


fn heartbeats(asked: &Arc<Mutex<Vec<RequestKind>>>) -> usize {
    asked.lock().unwrap().iter().filter(|k| matches!(k, RequestKind::LobbyHeartbeat { .. })).count()
}

/// A scripted backend for a host with company: sign-in works, a lobby with Ada in it exists after `CreateLobby`.
fn host_script(
    heartbeat: impl Fn() -> BackendReply + 'static,
    start: impl Fn() -> BackendReply + 'static,
) -> impl FnMut(&RequestKind) -> BackendReply {
    move |kind| match kind {
        RequestKind::SignIn => BackendReply::SignedIn { display_name: "Player".into() },
        RequestKind::CreateLobby { .. } => BackendReply::LobbyCreated { lobby: info("L1", &["Player", "Ada"], 4) },
        RequestKind::LobbyHeartbeat { .. } => heartbeat(),
        RequestKind::StartLobby { .. } => start(),
        RequestKind::SignOut => BackendReply::SignedOut,
        _ => BackendReply::LobbyLeft,
    }
}

#[test]
fn a_started_lobby_does_not_expire_after_a_heartbeat_failed_before_the_start() {
    let (backend, _) = scripted(host_script(
        || BackendReply::LobbyHeartbeatFailed { reason: "server".into() },
        || BackendReply::LobbyStarted { lobby: info_v("L1", &["Player", "Ada"], 4, 2, true) },
    ));
    let mut app = app_with(backend, true);
    act(&mut app, Action::SignIn);
    settle(&mut app);
    act(&mut app, create("Demo lobby", 4));
    settle(&mut app);
    advance(&mut app, 12.0); // the heartbeat fails: trouble
    settle(&mut app);
    assert_eq!(count(&app, "lobby.connection_trouble"), 1);
    act(&mut app, Action::StartLobby);
    settle(&mut app);
    assert_eq!(count(&app, "lobby.started"), 1);
    clear_seen(&mut app);
    // Nobody checks in after a start, so nothing may close it a minute later.
    for _ in 0..3 {
        advance(&mut app, 61.0);
        settle(&mut app);
    }
    assert_eq!(count(&app, "lobby.closed"), 0, "a started lobby survives until the player leaves: {:?}", seen(&app));
    assert_eq!(var(&app, "lobby.current.name").as_deref(), Some("Demo lobby"));
}

#[test]
fn a_start_is_announced_once_when_the_heartbeat_answer_gets_there_first() {
    let (backend, _) = scripted_with_delay(
        host_script(
            || BackendReply::LobbyState { lobby: info_v("L1", &["Player", "Ada"], 4, 2, true) },
            || BackendReply::LobbyStarted { lobby: info_v("L1", &["Player", "Ada"], 4, 2, true) },
        ),
        |kind| match kind {
            RequestKind::LobbyHeartbeat { .. } => 3,
            RequestKind::StartLobby { .. } => 6,
            _ => 0,
        },
    );
    let mut app = app_with(backend, true);
    act(&mut app, Action::SignIn);
    settle(&mut app);
    act(&mut app, create("Demo lobby", 4));
    settle(&mut app);
    clear_seen(&mut app);
    advance(&mut app, 12.0); // a heartbeat is on the wire
    app.world_mut().resource_mut::<ActionQueue>().push(Action::StartLobby);
    for _ in 0..14 {
        app.update();
    }
    assert_eq!(count(&app, "lobby.started"), 1, "the heartbeat answer and the start answer must not both announce it: {:?}", seen(&app));
    assert_eq!(count(&app, "lobby.start_failed"), 0);
}

#[test]
fn an_older_lobby_answer_never_undoes_a_start() {
    // The heartbeat sent before the start is answered after it, with the lobby as it was (version 1, Ada missing).
    let (backend, asked) = scripted_with_delay(
        host_script(
            || BackendReply::LobbyState { lobby: info_v("L1", &["Player"], 4, 1, false) },
            || BackendReply::LobbyStarted { lobby: info_v("L1", &["Player", "Ada"], 4, 2, true) },
        ),
        |kind| if matches!(kind, RequestKind::LobbyHeartbeat { .. }) { 5 } else { 0 },
    );
    let mut app = app_with(backend, true);
    act(&mut app, Action::SignIn);
    settle(&mut app);
    act(&mut app, create("Demo lobby", 4));
    settle(&mut app);
    clear_seen(&mut app);
    advance(&mut app, 12.0);
    app.world_mut().resource_mut::<ActionQueue>().push(Action::StartLobby);
    for _ in 0..14 {
        app.update();
    }
    assert_eq!(count(&app, "lobby.started"), 1);
    assert_eq!(count(&app, "lobby.member_left"), 0, "the old answer was dropped, not applied: {:?}", seen(&app));
    assert_eq!(var(&app, "lobby.member.2.name").as_deref(), Some("Ada"));
    advance(&mut app, 30.0);
    settle(&mut app);
    assert_eq!(heartbeats(&asked), 1, "a started lobby does not resume heartbeats");
}

#[test]
fn signing_out_during_a_create_still_tells_the_server_to_leave() {
    let (backend, asked) = scripted_with_delay(
        host_script(|| BackendReply::LobbyLeft, || BackendReply::LobbyLeft),
        |kind| if matches!(kind, RequestKind::CreateLobby { .. }) { 3 } else { 0 },
    );
    let mut app = app_with(backend, true);
    act(&mut app, Action::SignIn);
    settle(&mut app);
    clear_seen(&mut app);
    act(&mut app, create("Demo lobby", 4));
    act(&mut app, Action::SignOut);
    for _ in 0..12 {
        app.update();
    }
    let asked = asked.lock().unwrap().clone();
    let leave = asked.iter().position(|k| *k == RequestKind::LeaveLobby).expect("the server was told to leave");
    let sign_out = asked.iter().position(|k| *k == RequestKind::SignOut).expect("and signed out");
    assert!(leave < sign_out, "the leave goes out before the logout: {asked:?}");
    assert_eq!(count(&app, "lobby.created"), 0, "the late create answer is dropped");
}

#[test]
fn a_list_answer_survives_leaving_a_lobby_that_ended_while_it_was_on_the_way() {
    let (backend, _) = scripted_with_delay(
        |kind| match kind {
            RequestKind::SignIn => BackendReply::SignedIn { display_name: "Player".into() },
            RequestKind::CreateLobby { .. } => BackendReply::LobbyCreated { lobby: info("L1", &["Player", "Ada"], 4) },
            RequestKind::ListLobbies => BackendReply::LobbyListed { rows: vec![] },
            _ => BackendReply::LobbyLeft,
        },
        |kind| if matches!(kind, RequestKind::ListLobbies) { 3 } else { 0 },
    );
    let mut app = app_with(backend, true);
    act(&mut app, Action::SignIn);
    settle(&mut app);
    act(&mut app, create("Demo lobby", 4));
    settle(&mut app);
    clear_seen(&mut app);
    act(&mut app, Action::ListLobbies); // answered three polls from now
    act(&mut app, Action::LeaveLobby); // ends the membership first
    for _ in 0..10 {
        app.update();
    }
    assert_eq!(count(&app, "lobby.left"), 1);
    assert_eq!(count(&app, "lobby.listed"), 1, "a list belongs to the session, not to the lobby that was left: {:?}", seen(&app));
}

#[test]
fn a_heartbeat_answer_that_no_longer_lists_the_player_means_they_were_removed() {
    let (backend, _) = scripted(host_script(
        || BackendReply::LobbyState { lobby: info_v("L1", &["Ada"], 4, 2, false) },
        || BackendReply::LobbyLeft,
    ));
    let mut app = app_with(backend, true);
    act(&mut app, Action::SignIn);
    settle(&mut app);
    act(&mut app, create("Demo lobby", 4));
    settle(&mut app);
    clear_seen(&mut app);
    advance(&mut app, 12.0);
    settle(&mut app);
    let closed: Vec<String> = seen(&app).into_iter().filter(|e| e.starts_with("lobby.closed")).collect();
    assert_eq!(closed, vec!["lobby.closed", "lobby.closed:removed"]);
    assert_eq!(var(&app, "lobby.current.name"), None);
}

#[test]
fn a_session_that_the_server_ends_closes_the_lobby_with_the_server_reason() {
    let inbox = Arc::new(Mutex::new(Vec::new()));
    let mut app = app_with(Injecting { inner: fake(), inbox: inbox.clone() }, true);
    act(&mut app, Action::SignIn);
    settle(&mut app);
    act(&mut app, create("Demo lobby", 4));
    settle(&mut app);
    clear_seen(&mut app);
    inbox.lock().unwrap().push(BackendEvent { request_id: 0, reply: BackendReply::SignedOut });
    settle(&mut app);
    let seen = seen(&app);
    assert_eq!(seen.iter().filter(|e| e.as_str() == "lobby.closed:server").count(), 1, "{seen:?}");
    assert_eq!(seen.iter().filter(|e| e.as_str() == "backend.signed_out").count(), 1);
    assert!(app.world().resource::<GameVariables>().0.keys().all(|k| !k.starts_with("lobby.")));
    // And an explicit sign-out (which already left the lobby) does not announce a close on top of `lobby.left`.
    let mut app = signed_in_app();
    act(&mut app, create("Demo lobby", 4));
    settle(&mut app);
    clear_seen(&mut app);
    act(&mut app, Action::SignOut);
    settle(&mut app);
    assert_eq!(count(&app, "lobby.left"), 1);
    assert_eq!(count(&app, "lobby.closed"), 0);
}

#[test]
fn a_reason_from_the_wrong_family_reads_as_server() {
    let (backend, _) = scripted(|kind| match kind {
        RequestKind::SignIn => BackendReply::SignedIn { display_name: "Player".into() },
        RequestKind::ListLobbies => BackendReply::LobbyListed {
            rows: vec![LobbyRow { id: "a".into(), name: "Demo lobby".into(), players: 1, max: 4, host: "Ada".into() }],
        },
        RequestKind::JoinLobby { .. } => BackendReply::LobbyJoinFailed { reason: "host_left".into() },
        RequestKind::CreateLobby { .. } => BackendReply::LobbyCreated { lobby: info("L1", &["Player", "Ada"], 4) },
        RequestKind::LobbyHeartbeat { .. } => BackendReply::LobbyClosed { reason: "full".into() },
        _ => BackendReply::LobbyLeft,
    });
    let mut app = app_with(backend, true);
    act(&mut app, Action::SignIn);
    settle(&mut app);
    act(&mut app, Action::ListLobbies);
    settle(&mut app);
    clear_seen(&mut app);
    act(&mut app, Action::JoinLobby { row: 1 });
    settle(&mut app);
    assert_eq!(seen(&app), vec!["lobby.join_failed", "lobby.join_failed:server"], "host_left only closes a lobby");
    act(&mut app, create("Demo lobby", 4));
    settle(&mut app);
    clear_seen(&mut app);
    advance(&mut app, 12.0);
    settle(&mut app);
    let closed: Vec<String> = seen(&app).into_iter().filter(|e| e.starts_with("lobby.closed")).collect();
    assert_eq!(closed, vec!["lobby.closed", "lobby.closed:server"], "full does not close a lobby");
}

#[test]
fn a_list_while_a_sign_out_is_in_flight_is_refused_and_the_cached_list_is_gone_after_it() {
    let (backend, asked) = scripted(|kind| match kind {
        RequestKind::SignIn => BackendReply::SignedIn { display_name: "Player".into() },
        RequestKind::SignOut => BackendReply::SignedOut,
        _ => BackendReply::LobbyListed { rows: vec![] },
    });
    let mut app = app_with(backend, true);
    act(&mut app, Action::SignIn);
    settle(&mut app);
    act(&mut app, Action::ListLobbies);
    settle(&mut app);
    clear_seen(&mut app);
    // Same frame: the sign-out is in flight when the list is asked for.
    app.world_mut().resource_mut::<ActionQueue>().push(Action::SignOut);
    app.world_mut().resource_mut::<ActionQueue>().push(Action::ListLobbies);
    app.update();
    app.update();
    assert_eq!(count(&app, "lobby.list_failed:not_signed_in"), 1, "{:?}", seen(&app));
    settle(&mut app);
    act(&mut app, Action::SignIn);
    settle(&mut app);
    let lists_before = asked.lock().unwrap().iter().filter(|k| **k == RequestKind::ListLobbies).count();
    act(&mut app, Action::ListLobbies);
    settle(&mut app);
    let lists_after = asked.lock().unwrap().iter().filter(|k| **k == RequestKind::ListLobbies).count();
    assert_eq!(lists_after, lists_before + 1, "the previous player's list must not be replayed from the cache");
}

#[test]
fn expiry_counts_from_the_last_good_answer_not_from_the_first_failure() {
    let mut app = app_with(
        FakeBackend::from_config(&config(true)).with_latency(1).with_failures(FakeFailures { heartbeat: true, ..Default::default() }),
        true,
    );
    act(&mut app, Action::SignIn);
    settle(&mut app);
    act(&mut app, create("Demo lobby", 4));
    settle(&mut app);
    for secs in [12.0, 12.0, 30.0] {
        advance(&mut app, secs);
        settle(&mut app);
    }
    assert_eq!(count(&app, "lobby.closed"), 0, "about 54 s after the last good answer: still open");
    advance(&mut app, 8.0);
    settle(&mut app);
    assert_eq!(count(&app, "lobby.closed:server"), 1, "about 62 s after it: closed, although the first failure was only about 50 s ago");
}

#[test]
fn a_long_request_timeout_does_not_delay_the_trouble_signal_beyond_one_heartbeat_interval() {
    // The heartbeat is never answered; the designer's timeout is 120 s, the heartbeat deadline is capped at 10 s.
    let (backend, _) = scripted_with_delay(
        host_script(|| BackendReply::LobbyLeft, || BackendReply::LobbyLeft),
        |kind| if matches!(kind, RequestKind::LobbyHeartbeat { .. }) { 100_000 } else { 0 },
    );
    let mut app = app_with(backend, true);
    app.world_mut().resource_mut::<BackendHub>().request_timeout_secs = 120.0;
    act(&mut app, Action::SignIn);
    settle(&mut app);
    act(&mut app, create("Demo lobby", 4));
    settle(&mut app);
    clear_seen(&mut app);
    advance(&mut app, 12.0); // the heartbeat goes out
    settle(&mut app);
    assert_eq!(count(&app, "lobby.connection_trouble"), 0);
    advance(&mut app, 11.0);
    settle(&mut app);
    assert_eq!(count(&app, "lobby.connection_trouble"), 1, "after one heartbeat interval, not after 120 s");
}

#[test]
fn a_joiner_sees_the_scripted_host_start_the_lobby_and_then_the_heartbeats_stop() {
    let mut app = signed_in_app();
    act(&mut app, Action::ListLobbies);
    settle(&mut app);
    act(&mut app, Action::JoinLobby { row: 1 }); // Ada's lobby
    settle(&mut app);
    clear_seen(&mut app);
    for _ in 0..4 {
        advance(&mut app, 12.0);
        settle(&mut app);
    }
    assert_eq!(count(&app, "lobby.started"), 1, "Ada pressed Start on the fourth heartbeat: {:?}", seen(&app));
    assert_eq!(var(&app, "lobby.current.name").as_deref(), Some("Demo lobby"), "the lobby survives its start");
    clear_seen(&mut app);
    for _ in 0..3 {
        advance(&mut app, 12.0);
        settle(&mut app);
    }
    assert!(seen(&app).is_empty(), "nothing more happens after the start: {:?}", seen(&app));
    assert_eq!(app.world().resource::<BackendHub>().pending_requests(), 0, "no heartbeat is sent for a started lobby");
}
