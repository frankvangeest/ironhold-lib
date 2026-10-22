//! D3 (`planning/features/done/gameplay_pipeline_system_sets.md`): the declared order of the message -> interpreter ->
//! executor pipeline. Graph-reading tests only: they build the `Update` schedule with `initialize` and never run a frame.

use bevy::prelude::*;

mod support;
use support::{inspect_update_schedule, setup_test_app, update_schedule_conflicts, update_schedule_order, UpdateReport};

fn report() -> UpdateReport {
    let mut app = setup_test_app();
    inspect_update_schedule(&mut app)
}

/// Writes the `Update` run order to the file named by `SCHEDULE_ORDER_DUMP` (used to diff the order before/after a
/// registration change), and otherwise just checks the schedule builds and is non-empty.
#[test]
fn update_schedule_builds_and_can_be_dumped() {
    let mut app = setup_test_app();
    let order = update_schedule_order(&mut app);
    assert!(order.len() > 50, "expected the full GamePlugin Update schedule, got {} systems", order.len());
    if let Ok(path) = std::env::var("SCHEDULE_ORDER_DUMP") {
        std::fs::write(&path, order.join("\n")).expect("write the schedule dump");
    }
}

/// Writes every remaining unordered conflict to the file named by `SCHEDULE_CONFLICT_DUMP` (`a|b|components`, one per
/// line), for reviewing which unordered pairs could swap on the single-threaded (web) executor between two builds.
#[test]
fn update_schedule_conflicts_can_be_dumped() {
    let mut app = setup_test_app();
    let conflicts = update_schedule_conflicts(&mut app);
    if let Ok(path) = std::env::var("SCHEDULE_CONFLICT_DUMP") {
        let text: Vec<String> = conflicts.iter().map(|(a, b, c)| format!("{a}|{b}|{}", c.join(","))).collect();
        std::fs::write(&path, text.join("\n")).expect("write the conflict dump");
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Structure: the documented systems are in the documented sets, in the documented order.
// ---------------------------------------------------------------------------------------------------------------------

/// Every system of `a` runs before every system of `b` in the executable (single-threaded) order.
fn assert_set_before(r: &UpdateReport, a: &str, b: &str) {
    let a_members = r.members.get(a).unwrap_or_else(|| panic!("set {a} is empty or missing"));
    let b_members = r.members.get(b).unwrap_or_else(|| panic!("set {b} is empty or missing"));
    // A name registered twice (`stat_effective_value_system`, once in Stats and once in Execute) cannot be located by
    // name alone: leave it out of the position comparison (its two memberships are asserted separately).
    let a_last = a_members.iter().filter(|n| r.count(n) == 1).map(|n| r.last_pos(n)).max().unwrap();
    let b_first = b_members.iter().filter(|n| r.count(n) == 1).map(|n| r.first_pos(n)).min().unwrap();
    assert!(a_last < b_first, "{a} must run entirely before {b} (last of {a} at {a_last}, first of {b} at {b_first})");
}

fn assert_chain(r: &UpdateReport, names: &[&str]) {
    for w in names.windows(2) {
        assert!(r.last_pos(w[0]) < r.first_pos(w[1]), "`{}` must run before `{}`", w[0], w[1]);
    }
}

#[test]
fn the_documented_systems_are_in_the_documented_sets() {
    let r = report();
    let expect: &[(&str, &[&str])] = &[
        ("EmitSet::Scene", &["check_project_loaded", "spawn_scene_v2"]),
        ("EmitSet::Ui", &["global_input_system", "unclaimed_gamepad_trigger_system", "button_system", "icon_button_click_system"]),
        ("EmitSet::DirectActions", &["resolve_pending_behaviors_system", "despawn_timer_system"]),
        ("EmitSet::Dialogue", &["dialogue_tick_system"]),
        ("GameStreamSet::Targeting", &["click_select_system", "tab_targeting_system", "target_auto_clear_system", "aim_target_system"]),
        ("GameStreamSet::ActionBar", &["cooldown_tick_system", "action_bar_input_system", "action_bar_visual_system"]),
        ("GameStreamSet::Interact", &["interactable_system", "cursor_capture_system"]),
        ("GameStreamSet::DelayedEvents", &["tick_delayed_events_system"]),
        ("GameStreamSet::Stats", &["stat_modifier_system", "stat_regen_system", "stat_effective_value_system"]),
        ("GameplaySet::Interpret", &["fsm_interpreter_system", "entity_fsm_interpreter_system"]),
        (
            "GameplaySet::Execute",
            &[
                "flush_pending_intent_system", "action_executor_system", "apply_sim_clock_system", "stat_effective_value_system",
                "stat_threshold_system", "drain_spawn_queue_system", "drain_dynamic_stat_ui_system", "drain_particle_effects_system",
                "simulate_pool_system", "rebuild_pool_meshes_system", "spawn_decal_system",
            ],
        ),
        (
            "GameplaySet::PostExecute",
            &[
                "npc_hit_relay_system", "inventory_ui_system", "container_ui_system", "audio_volume_var_system",
                "preload_audio_system", "preload_decals_system", "clear_pool_on_scene_unload_system",
            ],
        ),
    ];
    for (set, systems) in expect {
        for sys in *systems {
            assert!(r.in_set(set, sys), "`{sys}` must be a member of {set}");
        }
    }
    // GameplaySet::Emit contains every Emit sub-set's systems (nested membership).
    for sub in ["EmitSet::Scene", "EmitSet::Ui", "EmitSet::DirectActions", "EmitSet::Dialogue", "GameStreamSet::Interact"] {
        for sys in r.members.get(sub).unwrap() {
            assert!(r.in_set("GameplaySet::Emit", sys), "`{sys}` ({sub}) must also be inside GameplaySet::Emit");
        }
    }
}

#[test]
fn the_pipeline_phases_and_streams_run_in_the_documented_order() {
    let r = report();
    assert_set_before(&r, "GameplaySet::Emit", "GameplaySet::Interpret");
    assert_set_before(&r, "GameplaySet::Interpret", "GameplaySet::Execute");
    assert_set_before(&r, "GameplaySet::Execute", "GameplaySet::PostExecute");
    // Inside Emit: Scene first, then Ui and Game, then DirectActions, then Dialogue.
    assert_set_before(&r, "EmitSet::Scene", "EmitSet::Ui");
    assert_set_before(&r, "EmitSet::Scene", "EmitSet::Game");
    assert_set_before(&r, "EmitSet::Ui", "EmitSet::DirectActions");
    assert_set_before(&r, "EmitSet::Game", "EmitSet::DirectActions");
    assert_set_before(&r, "EmitSet::DirectActions", "EmitSet::Dialogue");
    // The game stream: input first, then delayed events, then stat events.
    assert_set_before(&r, "GameStreamSet::Targeting", "GameStreamSet::ActionBar");
    assert_set_before(&r, "GameStreamSet::ActionBar", "GameStreamSet::Interact");
    assert_set_before(&r, "GameStreamSet::Interact", "GameStreamSet::DelayedEvents");
    assert_set_before(&r, "GameStreamSet::DelayedEvents", "GameStreamSet::Stats");
    // Chains inside the sets.
    assert_chain(&r, &["check_project_loaded", "spawn_scene_v2"]);
    assert_chain(&r, &["global_input_system", "unclaimed_gamepad_trigger_system", "button_system", "icon_button_click_system"]);
    assert_chain(&r, &["resolve_pending_behaviors_system", "despawn_timer_system"]);
    assert_chain(&r, &["fsm_interpreter_system", "entity_fsm_interpreter_system"]);
    assert_chain(&r, &["flush_pending_intent_system", "action_executor_system", "apply_sim_clock_system", "stat_threshold_system"]);
    assert_chain(&r, &["cooldown_tick_system", "action_bar_input_system", "action_bar_visual_system"]);
    // The old transitive guarantee, now explicit: targeting runs before the action bar, and both before the interpreters.
    assert_chain(&r, &["target_auto_clear_system", "cooldown_tick_system"]);
    assert!(r.last_pos("action_bar_visual_system") < r.first_pos("fsm_interpreter_system"));
    // apply_sim_clock_system runs after both writers of SimClock: spawn_scene_v2 (EmitSet::Scene) and the executor.
    assert!(r.last_pos("spawn_scene_v2") < r.first_pos("apply_sim_clock_system"));
    assert!(r.last_pos("action_executor_system") < r.first_pos("apply_sim_clock_system"));
}

#[test]
fn stat_effective_value_system_is_registered_twice_and_never_ordered_by_name() {
    // Legal as long as nothing orders against the function by name (`SystemTypeSetAmbiguity`): order against
    // `GameStreamSet::Stats` / `GameplaySet::Execute` instead. The schedule building at all proves no by-name edge exists.
    let r = report();
    assert_eq!(r.count("stat_effective_value_system"), 2, "once in GameStreamSet::Stats, once in GameplaySet::Execute");
    assert!(r.in_set("GameStreamSet::Stats", "stat_effective_value_system"));
    assert!(r.in_set("GameplaySet::Execute", "stat_effective_value_system"));
}

#[test]
fn the_schedule_has_no_redundant_membership_warnings() {
    // A system registered both in a specific set and directly in a parent set shows up as a hierarchy redundancy.
    let r = report();
    assert!(r.warnings.is_empty(), "schedule build warnings: {:#?}", r.warnings);
}

// ---------------------------------------------------------------------------------------------------------------------
// Access-based checks (they see writers hidden inside `SystemParam` bundles, which a signature grep cannot).
// ---------------------------------------------------------------------------------------------------------------------

const PIPELINE_WRITES: &[&str] = &["UiEvent", "GameEvent", "SceneEvent", "ActionQueue", "SimClock"];
const PIPELINE_READS: &[&str] = &["UiEvent", "GameEvent", "SceneEvent"];

fn in_any_gameplay_set(r: &UpdateReport, short: &str) -> bool {
    ["GameplaySet::Emit", "GameplaySet::Interpret", "GameplaySet::Execute", "GameplaySet::PostExecute"]
        .iter()
        .any(|set| r.in_set(set, short))
}

#[test]
fn every_pipeline_message_or_queue_system_is_in_a_gameplay_set() {
    // Every `Update` system that writes UiEvent/GameEvent/SceneEvent, pushes ActionQueue or writes SimClock, or reads the
    // three message streams, must be a member of a `GameplaySet`: an unlisted system is unordered against the rest of the
    // pipeline, which is exactly what D3 exists to prevent.
    let r = report();
    let mut missing: Vec<String> = r
        .systems
        .iter()
        .filter(|s| {
            s.writes.iter().any(|w| PIPELINE_WRITES.contains(w)) || s.reads.iter().any(|w| PIPELINE_READS.contains(w))
        })
        .filter(|s| !in_any_gameplay_set(&r, &s.short))
        .map(|s| format!("{} (reads {:?}, writes {:?})", s.short, s.reads, s.writes))
        .collect();
    missing.sort();
    missing.dedup();
    assert!(
        missing.is_empty(),
        "Update systems that touch the pipeline messages/queue/SimClock but are in no GameplaySet (add them to the right \
         set in lib.rs, see runtime/schedule.rs):\n  {}",
        missing.join("\n  ")
    );
}

/// The only accepted unordered conflicts on pipeline resources: first-person and flycam look read `SimClock` and run in
/// the camera chain, which is not yet ordered against the clock's writers (D-g; the deferred `CameraChainSet` follow-up
/// orders it after `GameplaySet::Execute` and removes both entries).
const ACCEPTED_CAMERA_CONFLICTS: &[(&str, &str)] = &[
    ("spawn_scene_v2", "first_person_camera_system"),
    ("spawn_scene_v2", "fly_camera_system"),
    ("action_executor_system", "first_person_camera_system"),
    ("action_executor_system", "fly_camera_system"),
];

/// Resources and components whose same-frame order is part of the declared pipeline (the message buffers, the action
/// queue, the pause clock, plus the bindings `spawn_scene_v2` rewrites before the input systems read them and the
/// targeting state the targeting chain writes before the action bar reads it). Removing the edge that orders them makes
/// the pair conflict here, which a position check in the built order cannot see.
const ORDERED_STATE: &[&str] = &[
    "UiEvent", "GameEvent", "SceneEvent", "ActionQueue", "SimClock", "LoadedKeyBindings", "LoadedGamepadBindings", "CurrentTarget",
];

fn touches_pipeline_resource(components: &[String]) -> bool {
    components.iter().any(|c| ORDERED_STATE.iter().any(|p| c.contains(p)))
}

#[test]
fn no_unordered_conflict_on_the_pipeline_resources() {
    // `conflicting_systems()` is computed on every build regardless of `ScheduleBuildSettings`. Hard-fail on a conflict
    // on the three message buffers, `ActionQueue` or `SimClock`; the assertion is "found conflicts are a SUBSET of the
    // allowlist", because shared sync nodes can order a pair incidentally.
    let r = report();
    let unexpected: Vec<String> = r
        .conflicts
        .iter()
        .filter(|(_, _, c)| touches_pipeline_resource(c))
        .filter(|(a, b, _)| {
            let (a, b) = (UpdateReport::short(a), UpdateReport::short(b));
            !ACCEPTED_CAMERA_CONFLICTS.iter().any(|(x, y)| (a == *x && b == *y) || (a == *y && b == *x))
        })
        .map(|(a, b, c)| format!("{} <-> {} on {:?}", UpdateReport::short(a), UpdateReport::short(b), c))
        .collect();
    assert!(
        unexpected.is_empty(),
        "unordered systems conflict on a pipeline resource (put them in a GameplaySet, see runtime/schedule.rs):\n  {}",
        unexpected.join("\n  ")
    );
}

#[test]
fn no_exclusive_system_conflict_involves_a_gameplay_system() {
    // A conflict caused by an exclusive system (or `AccessConflicts::All`) comes back with an EMPTY component list, so
    // the filter above silently drops it: treat any such conflict that involves a gameplay-set member as a failure.
    let r = report();
    let bad: Vec<String> = r
        .conflicts
        .iter()
        .filter(|(_, _, c)| c.is_empty())
        .filter(|(a, b, _)| in_any_gameplay_set(&r, UpdateReport::short(a)) || in_any_gameplay_set(&r, UpdateReport::short(b)))
        .map(|(a, b, _)| format!("{} <-> {}", UpdateReport::short(a), UpdateReport::short(b)))
        .collect();
    assert!(bad.is_empty(), "exclusive-system conflicts with gameplay systems: {bad:?}");
}

#[test]
fn the_fixed_update_gameplay_writers_are_ordered_against_each_other() {
    // The `FixedUpdate` GameEvent writers (collectible, trigger_zone, npc_behavior, player) are one `.chain()`; the sets
    // are re-homed into FixedUpdate by the fixed-tick item. Guard the same property there.
    let mut app = setup_test_app();
    let r = support::inspect_schedule(&mut app, FixedUpdate);
    let bad: Vec<String> = r
        .conflicts
        .iter()
        .filter(|(_, _, c)| touches_pipeline_resource(c))
        .map(|(a, b, c)| format!("{} <-> {} on {:?}", UpdateReport::short(a), UpdateReport::short(b), c))
        .collect();
    assert!(bad.is_empty(), "unordered FixedUpdate conflicts on pipeline resources:\n  {}", bad.join("\n  "));
}

#[test]
fn every_simclock_reader_is_ordered_against_the_clocks_writers() {
    // `conflicting_systems()` ignores run-condition access, so check it separately: a system whose body OR condition
    // reads `SimClock` must run after EmitSet::Scene (which may release the clock on a Replace load) and either before
    // the executor (which may take it) or after `apply_sim_clock_system`, i.e. be a member of an Emit sub-set after Scene
    // or of PostExecute. Carve-outs: the writers themselves, and the two camera systems on the accepted-conflict list.
    let r = report();
    let writers_and_apply = ["spawn_scene_v2", "action_executor_system", "apply_sim_clock_system"];
    let camera_allowlist = ["first_person_camera_system", "fly_camera_system"];
    let bad: Vec<String> = r
        .systems
        .iter()
        .filter(|s| s.reads.contains("SimClock") || s.cond_reads.contains("SimClock"))
        .filter(|s| !writers_and_apply.contains(&s.short.as_str()) && !camera_allowlist.contains(&s.short.as_str()))
        .filter(|s| {
            let after_scene = ["EmitSet::Ui", "EmitSet::Game", "EmitSet::DirectActions", "EmitSet::Dialogue"]
                .iter()
                .any(|set| r.in_set(set, &s.short));
            !(after_scene || r.in_set("GameplaySet::PostExecute", &s.short))
        })
        .map(|s| s.short.clone())
        .collect();
    assert!(bad.is_empty(), "SimClock readers not ordered after EmitSet::Scene / around the clock writers: {bad:?}");
}

#[test]
fn every_edge_triggered_input_system_reads_the_pause_clock() {
    // Real pause gates edge-triggered input per system (some in the function body on purpose, so a set-level `run_if`
    // cannot replace them). A new input system that emits pipeline messages without reading `SimClock` would keep
    // acting while the game is paused: see the pause rule in `crates/ironhold_core/src/CLAUDE.md`.
    let r = report();
    let gated = [
        "interactable_system", "click_select_system", "tab_targeting_system", "action_bar_input_system",
        "cursor_capture_system", "aim_target_system", "first_person_camera_system", "fly_camera_system", "dialogue_tick_system",
    ];
    for name in gated {
        let s = r.system(name);
        assert!(s.reads.contains("SimClock") || s.cond_reads.contains("SimClock"), "`{name}` must read SimClock (pause gate)");
    }
    // Inventory: anything reading a keyboard/mouse resource AND writing pipeline messages must be gated, unless it is a
    // UI writer that must keep working while paused (the pause/resume key itself comes through `global_input_system`).
    let must_not_be_gated = ["global_input_system", "unclaimed_gamepad_trigger_system", "button_system", "icon_button_click_system"];
    let ungated: Vec<String> = r
        .systems
        .iter()
        .filter(|s| ["KeyCode", "MouseButton", "MouseMotion"].iter().any(|i| s.reads.contains(i)))
        .filter(|s| s.writes.iter().any(|w| ["UiEvent", "GameEvent", "SceneEvent", "ActionQueue"].contains(w)))
        .filter(|s| !(s.reads.contains("SimClock") || s.cond_reads.contains("SimClock")))
        .filter(|s| !must_not_be_gated.contains(&s.short.as_str()))
        .map(|s| s.short.clone())
        .collect();
    assert!(ungated.is_empty(), "input systems that emit pipeline messages but do not read SimClock: {ungated:?}");
}

// ---------------------------------------------------------------------------------------------------------------------
// Behaviour: the documented order, observed at the point where the interpreters read it.
// ---------------------------------------------------------------------------------------------------------------------

#[derive(Resource, Default)]
struct SeenGameEvents(Vec<String>);

/// Reads `GameEvent`s at the interpreters' own position (after the entity interpreter), i.e. in the order they read them.
fn record_game_events(mut reader: MessageReader<ironhold_core::runtime::GameEvent>, mut seen: ResMut<SeenGameEvents>) {
    for ironhold_core::runtime::GameEvent::Trigger(name) in reader.read() {
        seen.0.push(name.clone());
    }
}

#[test]
fn pause_events_arrive_after_the_actions_that_ran_before_them_in_the_same_frame() {
    // `action_executor_system` runs, then `apply_sim_clock_system`: an event the executor emitted and the `game.paused` it
    // triggered are both interpreted at the head of the NEXT frame, in that order, before anything the same frame's
    // stat thresholds would add.
    use ironhold_core::runtime::{ActionQueue, GameplaySet};
    use ironhold_core::schema::Action;
    let mut app = setup_test_app();
    app.init_resource::<SeenGameEvents>();
    app.add_systems(
        Update,
        record_game_events
            .in_set(GameplaySet::Interpret)
            .after(ironhold_core::runtime::entity_fsm_interpreter_system),
    );
    app.update();

    {
        let mut queue = app.world_mut().resource_mut::<ActionQueue>();
        queue.push(Action::EmitEvent("test.before_pause".to_string()));
        queue.push(Action::PauseGame);
    }
    app.update(); // frame N: executor emits test.before_pause, apply_sim_clock_system emits game.paused
    app.update(); // frame N+1: the interpreters read both

    let seen = &app.world().resource::<SeenGameEvents>().0;
    let before = seen.iter().position(|n| n == "test.before_pause").expect("test.before_pause was read");
    let paused = seen.iter().position(|n| n == "game.paused").expect("game.paused was read");
    assert!(before < paused, "executor events come before game.paused (got {seen:?})");
}

#[test]
fn the_gameplay_sets_carry_no_run_conditions() {
    // Bevy evaluates a set condition once, at the first member's turn, and skips every member: conditions stay on the
    // systems (`sim_running`, `any_action_slots`, `resource_changed::<SimClock>`), never on a set.
    let r = report();
    assert!(r.set_conditions.is_empty(), "gameplay sets with a run condition: {:?}", r.set_conditions);
}

#[test]
fn every_game_stream_set_is_inside_emit() {
    let r = report();
    for sub in [
        "GameStreamSet::Targeting", "GameStreamSet::ActionBar", "GameStreamSet::Interact", "GameStreamSet::DelayedEvents",
        "GameStreamSet::Stats", "EmitSet::Game",
    ] {
        for sys in r.members.get(sub).unwrap_or_else(|| panic!("{sub} is empty")) {
            assert!(r.in_set("GameplaySet::Emit", sys), "`{sys}` ({sub}) must also be inside GameplaySet::Emit");
            assert!(r.in_set("EmitSet::Game", sys), "`{sys}` ({sub}) must also be inside EmitSet::Game");
        }
    }
}

#[test]
fn update_system_short_names_are_unique_except_the_documented_double() {
    // The tests match systems by the last path segment; two different functions with the same short name in different
    // modules would be conflated. `stat_effective_value_system` is registered twice on purpose (one function).
    let r = report();
    let mut seen = std::collections::HashMap::<&str, usize>::new();
    for s in &r.systems {
        *seen.entry(s.short.as_str()).or_default() += 1;
    }
    let dupes: Vec<_> = seen.into_iter().filter(|(n, c)| *c > 1 && *n != "stat_effective_value_system").collect();
    assert!(dupes.is_empty(), "duplicate Update system short names (key the tests by full name): {dupes:?}");
}
