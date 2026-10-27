//! Online backend, v1 (`planning/features/backend_abstraction.md`): the online actions, the `FakeBackend`, the glue system's
//! events and engine-written variables, and the failure/offline/busy answers. Nothing here touches a network.

use bevy::prelude::*;

use ironhold_core::runtime::backend::{
    Backend, BackendAvailability, BackendEvent, BackendHub, BackendReply, BackendRequest, BackendSlot, BackendStatus,
    FakeBackend, FakeFailures, LeaderRow, RequestKind,
};
use ironhold_core::runtime::{ActionQueue, GameEvent, GameplaySet};
use ironhold_core::schema::backend::BackendConfig;
use ironhold_core::schema::Action;
use ironhold_core::GameVariables;

mod support;
use support::setup_test_app;

#[derive(Resource, Default)]
struct Seen(Vec<String>);

/// Records every `GameEvent` exactly where the interpreters read them, so a test also proves a reply is visible the frame it
/// is produced.
fn record(mut r: MessageReader<GameEvent>, mut seen: ResMut<Seen>) {
    for GameEvent::Trigger(n) in r.read() {
        seen.0.push(n.clone());
    }
}

fn config() -> BackendConfig {
    ron::from_str(
        r#"BackendConfig(schema_version: 1, endpoint: Some("fake://"),
            leaderboards: [Leaderboard(name: "best", ranking: HigherIsBetter, keep: Best),
                           Leaderboard(name: "fast", ranking: LowerIsBetter, keep: Latest)])"#,
    )
    .unwrap()
}

fn app_with(backend: impl Backend + 'static) -> App {
    let mut app = setup_test_app();
    app.init_resource::<Seen>();
    app.add_systems(Update, record.in_set(GameplaySet::Interpret).after(ironhold_core::runtime::entity_fsm_interpreter_system));
    app.update();
    *app.world_mut().resource_mut::<BackendHub>() = BackendHub::ready(config());
    app.world_mut().non_send_resource_mut::<BackendSlot>().install(Box::new(backend));
    app
}

fn fake() -> FakeBackend {
    FakeBackend::from_config(&config()).with_latency(1)
}

fn act(app: &mut App, a: Action) {
    app.world_mut().resource_mut::<ActionQueue>().push(a);
    app.update();
}

fn settle(app: &mut App) {
    for _ in 0..6 {
        app.update();
    }
}

fn seen(app: &App) -> Vec<String> {
    app.world().resource::<Seen>().0.clone()
}

fn var(app: &App, key: &str) -> Option<String> {
    app.world().resource::<GameVariables>().0.get(key).cloned()
}

fn set_var(app: &mut App, key: &str, value: &str) {
    app.world_mut().resource_mut::<GameVariables>().0.insert(key.to_string(), value.to_string());
}

fn submit(lb: &str) -> Action {
    Action::SubmitScore { leaderboard: lb.into(), score_var: "score".into() }
}

fn list(lb: &str) -> Action {
    Action::ShowLeaderboard { leaderboard: lb.into(), count: 10 }
}

fn count(app: &App, name: &str) -> usize {
    seen(app).iter().filter(|n| *n == name).count()
}

#[test]
fn sign_in_emits_the_event_and_writes_the_status_variables() {
    let mut app = app_with(fake());
    act(&mut app, Action::SignIn);
    assert_eq!(var(&app, "backend.status").as_deref(), Some("connecting"));
    settle(&mut app);
    assert_eq!(count(&app, "backend.signed_in"), 1);
    assert_eq!(var(&app, "backend.status").as_deref(), Some("online"));
    assert_eq!(var(&app, "backend.display_name").as_deref(), Some("Player"));
}

#[test]
fn a_project_without_a_backend_answers_offline() {
    let mut app = setup_test_app();
    app.init_resource::<Seen>();
    app.add_systems(Update, record.in_set(GameplaySet::Interpret).after(ironhold_core::runtime::entity_fsm_interpreter_system));
    app.update();
    app.world_mut().resource_mut::<BackendHub>().availability = BackendAvailability::Offline;
    for a in [Action::SignIn, submit("best"), list("best"), Action::SignOut] {
        act(&mut app, a);
    }
    app.update(); // the executor emits in Execute; the interpreters read it the next frame
    assert_eq!(count(&app, "backend.offline"), 4, "{:?}", seen(&app));
}

#[test]
fn while_backend_ron_is_still_loading_online_actions_answer_busy() {
    let mut app = setup_test_app();
    app.init_resource::<Seen>();
    app.add_systems(Update, record.in_set(GameplaySet::Interpret).after(ironhold_core::runtime::entity_fsm_interpreter_system));
    app.update();
    assert_eq!(app.world().resource::<BackendHub>().availability, BackendAvailability::Loading);
    act(&mut app, Action::SignIn);
    app.update();
    assert_eq!(count(&app, "backend.busy"), 1);
}

#[test]
fn a_second_sign_in_while_one_is_in_flight_is_busy_and_never_doubles_the_reply() {
    let mut app = app_with(fake());
    act(&mut app, Action::SignIn);
    act(&mut app, Action::SignIn);
    settle(&mut app);
    assert_eq!(count(&app, "backend.busy"), 1);
    assert_eq!(count(&app, "backend.signed_in"), 1);
}

#[test]
fn submit_and_list_a_leaderboard_through_the_fake() {
    let mut app = app_with(fake());
    act(&mut app, Action::SignIn);
    settle(&mut app);
    set_var(&mut app, "score", "500");
    act(&mut app, submit("best"));
    settle(&mut app);
    act(&mut app, list("best"));
    settle(&mut app);
    let s = seen(&app);
    assert!(s.contains(&"score.submitted:best".to_string()), "{s:?}");
    assert!(s.contains(&"leaderboard.listed:best".to_string()), "{s:?}");
    // Seeded rivals: 900, 750, 600, 450; the player's 500 is rank 4 of 5.
    assert_eq!(var(&app, "leaderboard.best.count").as_deref(), Some("5"));
    assert_eq!(var(&app, "leaderboard.best.1.name").as_deref(), Some("Ada"));
    assert_eq!(var(&app, "leaderboard.best.4.name").as_deref(), Some("Player"));
    assert_eq!(var(&app, "leaderboard.best.4.score").as_deref(), Some("500"));
    assert_eq!(var(&app, "leaderboard.best.my_rank").as_deref(), Some("4"));
    assert_eq!(var(&app, "leaderboard.best.my_best").as_deref(), Some("500"));
    assert_eq!(var(&app, "leaderboard.best.6.name"), None, "rows beyond the count stay unset");
}

#[test]
fn a_score_that_is_not_a_whole_number_fails_instead_of_submitting_zero() {
    let mut app = app_with(fake());
    act(&mut app, Action::SignIn);
    settle(&mut app);
    for bad in ["abc", "", "12.5"] {
        set_var(&mut app, "score", bad);
        act(&mut app, submit("best"));
    }
    app.update();
    assert_eq!(count(&app, "score.submit_failed:best"), 3);
    assert!(var(&app, "backend.last_error").unwrap().contains("whole number"));
    settle(&mut app);
    assert_eq!(count(&app, "score.submitted:best"), 0);
}

#[test]
fn an_undeclared_leaderboard_fails_with_the_operations_own_event() {
    let mut app = app_with(fake());
    act(&mut app, Action::SignIn);
    settle(&mut app);
    set_var(&mut app, "score", "1");
    act(&mut app, submit("nope"));
    act(&mut app, list("nope"));
    app.update();
    assert_eq!(count(&app, "score.submit_failed:nope"), 1);
    assert_eq!(count(&app, "leaderboard.list_failed:nope"), 1);
    assert!(var(&app, "backend.last_error").unwrap().contains("not declared"));
}

#[test]
fn online_actions_before_sign_in_fail_per_operation_and_while_connecting_say_busy() {
    let mut app = app_with(fake());
    set_var(&mut app, "score", "1");
    act(&mut app, submit("best"));
    act(&mut app, list("best"));
    app.update();
    assert_eq!(count(&app, "score.submit_failed:best"), 1);
    assert_eq!(count(&app, "leaderboard.list_failed:best"), 1);
    assert_eq!(var(&app, "backend.last_error").as_deref(), Some("not signed in"));

    act(&mut app, Action::SignIn); // now connecting
    act(&mut app, submit("best"));
    app.update();
    assert_eq!(count(&app, "backend.busy"), 1);
}

#[test]
fn a_rejected_sign_in_and_an_unreachable_server_are_different_events() {
    let mut app = app_with(fake().with_failures(FakeFailures { reject_sign_in: true, ..Default::default() }));
    act(&mut app, Action::SignIn);
    settle(&mut app);
    assert_eq!(count(&app, "backend.sign_in_failed"), 1);
    assert_eq!(var(&app, "backend.status").as_deref(), Some("offline"));
    assert!(!var(&app, "backend.last_error").unwrap().is_empty());

    let mut app = app_with(fake().with_failures(FakeFailures { unreachable: true, ..Default::default() }));
    act(&mut app, Action::SignIn);
    settle(&mut app);
    assert_eq!(count(&app, "backend.unreachable"), 1);
    assert_eq!(count(&app, "backend.sign_in_failed"), 0);
}

#[test]
fn a_failed_submit_emits_that_operations_failure_event() {
    let mut app = app_with(fake().with_failures(FakeFailures { submit: true, ..Default::default() }));
    act(&mut app, Action::SignIn);
    settle(&mut app);
    set_var(&mut app, "score", "10");
    act(&mut app, submit("best"));
    settle(&mut app);
    assert_eq!(count(&app, "score.submit_failed:best"), 1);
    assert_eq!(count(&app, "score.submitted:best"), 0);
}

#[test]
fn a_request_nobody_answers_times_out_on_real_time_and_fails() {
    let mut app = app_with(fake().with_failures(FakeFailures { silent: true, ..Default::default() }));
    app.world_mut().resource_mut::<BackendHub>().request_timeout_secs = 0.0;
    act(&mut app, Action::SignIn);
    settle(&mut app);
    assert_eq!(count(&app, "backend.unreachable"), 1);
    assert_eq!(var(&app, "backend.last_error").as_deref(), Some("timed out"));
    assert_eq!(app.world().resource::<BackendHub>().pending_requests(), 0, "the timed-out request is no longer in flight");
}

#[test]
fn replies_arrive_while_the_game_is_paused() {
    let mut app = app_with(fake());
    act(&mut app, Action::PauseGame);
    act(&mut app, Action::SignIn);
    settle(&mut app);
    assert!(app.world().resource::<Time<Virtual>>().is_paused());
    assert_eq!(count(&app, "backend.signed_in"), 1);
}

/// An adapter whose threads answer in reverse: the glue system must still present the batch in request order.
#[derive(Default)]
struct ReverseBackend {
    held: Vec<BackendEvent>,
}

impl Backend for ReverseBackend {
    fn submit(&mut self, req: BackendRequest) {
        let reply = match req.kind {
            RequestKind::SignIn => BackendReply::SignedIn { display_name: "x".into() },
            RequestKind::SignOut => BackendReply::SignedOut,
            RequestKind::SubmitScore { leaderboard, .. } => BackendReply::ScoreSubmitted { leaderboard },
            RequestKind::ListLeaderboard { leaderboard, .. } => {
                BackendReply::Leaderboard { leaderboard, rows: vec![LeaderRow { name: "a".into(), score: 1 }], my_rank: None, my_best: None }
            }
        };
        self.held.push(BackendEvent { request_id: req.id, reply });
    }

    fn poll(&mut self, out: &mut Vec<BackendEvent>) {
        out.extend(self.held.drain(..).rev());
    }
}

#[test]
fn one_frames_replies_are_presented_in_request_order_whatever_the_adapter_returns() {
    let mut app = app_with(ReverseBackend::default());
    app.world_mut().resource_mut::<BackendHub>().status = BackendStatus::Online;
    set_var(&mut app, "score", "5");
    // Three requests in one frame: ids 1, 2, 3 in executor order.
    {
        let mut q = app.world_mut().resource_mut::<ActionQueue>();
        q.push(submit("best"));
        q.push(submit("fast"));
        q.push(list("best"));
    }
    settle(&mut app);
    let s = seen(&app);
    assert_eq!(s, vec!["score.submitted:best", "score.submitted:fast", "leaderboard.listed:best"], "{s:?}");
}

#[test]
fn sign_out_resets_the_status_and_display_name() {
    let mut app = app_with(fake());
    act(&mut app, Action::SignIn);
    settle(&mut app);
    act(&mut app, Action::SignOut);
    settle(&mut app);
    assert_eq!(count(&app, "backend.signed_out"), 1);
    assert_eq!(var(&app, "backend.status").as_deref(), Some("offline"));
    assert_eq!(var(&app, "backend.display_name").as_deref(), Some(""));
}

#[test]
fn the_online_actions_parse_from_ron() {
    for (src, want) in [
        ("SignIn", Action::SignIn),
        ("SignOut", Action::SignOut),
        ("SubmitScore(leaderboard: \"best\", score_var: \"score\")", submit("best")),
        ("ShowLeaderboard(leaderboard: \"best\")", Action::ShowLeaderboard { leaderboard: "best".into(), count: 10 }),
        ("ShowLeaderboard(leaderboard: \"best\", count: 3)", Action::ShowLeaderboard { leaderboard: "best".into(), count: 3 }),
    ] {
        let got: Action = ron::de::from_str(src).unwrap_or_else(|e| panic!("{src} must parse: {e}"));
        assert_eq!(got, want);
    }
    assert!(ron::de::from_str::<Action>("SignIn(1)").is_err());
    assert!(ron::de::from_str::<Action>("SubmitScore(leaderboard: \"b\")").is_err(), "score_var is required");
}

/// The demo's `ui/*.ui.ron` files are the copy-and-edit snippets the docs point designers at: they must parse as a UI node
/// and appear verbatim in the demo scene, so the playable demo and the snippet can never drift apart.
#[test]
fn the_demo_snippets_parse_and_are_embedded_in_the_demo_scene() {
    use ironhold_core::schema::UiNodeDef;
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/projects/backend_demo");
    let scene = std::fs::read_to_string(format!("{root}/scenes/main.scene.ron")).unwrap().replace("\r\n", "\n");
    for name in ["signin", "leaderboard"] {
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
