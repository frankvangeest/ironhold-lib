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
    LobbyInfo {
        id: id.into(),
        name: "Demo lobby".into(),
        max,
        started: false,
        host: members[0].into(),
        members: members.iter().map(|m| m.to_string()).collect(),
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
    act(&mut app, Action::JoinLobby { row: 1 });
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
