#![allow(dead_code, unused_imports)]

use bevy::prelude::*;
use bevy::input::gamepad::{
    Gamepad, GamepadConnection, GamepadConnectionEvent, GamepadButton, GamepadAxis,
    RawGamepadEvent, RawGamepadButtonChangedEvent, RawGamepadAxisChangedEvent,
    gamepad_connection_system, gamepad_event_processing_system,
};
use bevy::input::ButtonState;
use ironhold_core::{GamePlugin, ProjectConfigPath, ProjectRoot};
use ironhold_core::runtime::{UiEvent, GameEvent, SceneEvent, InputActionMessage};

pub fn setup_test_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
       .add_plugins(bevy::state::app::StatesPlugin)
       .add_plugins(bevy::transform::TransformPlugin)
       .add_plugins(AssetPlugin::default())
       .add_plugins(bevy::scene::ScenePlugin)
       .add_message::<bevy::input::mouse::MouseMotion>()
       .add_message::<bevy::input::mouse::MouseWheel>()
       .init_resource::<ButtonInput<KeyCode>>()
       .init_resource::<ButtonInput<MouseButton>>()
       .init_resource::<Messages<UiEvent>>()
       .init_resource::<Messages<GameEvent>>()
       .init_resource::<Messages<SceneEvent>>()
       .init_resource::<Messages<InputActionMessage>>()
       .init_resource::<Messages<AppExit>>()
       .init_resource::<Messages<bevy::input::mouse::MouseMotion>>()
       .init_resource::<Messages<bevy::input::mouse::MouseWheel>>()
       .init_asset::<Mesh>()
       .init_asset::<bevy::shader::Shader>()
       .init_asset::<ironhold_core::capabilities::terrain_material::TerrainMaterial>()
       .init_asset::<StandardMaterial>()
       .init_asset::<Image>()
       .init_asset::<Scene>()
       .init_asset::<Gltf>()
       .init_asset::<AnimationGraph>()
       // `animation_playback_system` reads `Res<Assets<AnimationClip>>` to resolve a clip's
       // duration for PlayAnimationOn's start_at_fraction/freeze seek (dynamic_animation_
       // control.md) — every test that spawns any AnimationController entity now needs this
       // resource registered, not just tests that construct clips themselves (which previously
       // called `app.init_asset::<AnimationClip>()` individually; centralized here instead so
       // no test file has to remember it).
        .init_asset::<bevy::animation::AnimationClip>()
        .init_asset::<ironhold_core::schema::player::AnimationPolicy>()
        .init_asset::<ironhold_core::schema::project::StateMachineAsset>()
        .init_asset::<bevy::audio::AudioSource>()
       .insert_resource(ProjectConfigPath("projects/integration_tests/integration_tests.project.ron".to_string()))
       .insert_resource(ProjectRoot("projects/integration_tests".to_string()))
       // Gamepad connection/event processing — turns simulated RawGamepadEvent/
       // GamepadConnectionEvent messages (see connect_test_gamepad/press_gamepad_button/
       // set_gamepad_axis below) into a real `Gamepad` component's digital/analog state, the
       // same systems Bevy's own GilrsPlugin would normally drive from real hardware. Harmless
       // to register unconditionally — tests that don't touch gamepads never send these events.
       .add_message::<bevy::input::gamepad::GamepadEvent>()
       .add_message::<GamepadConnectionEvent>()
       .add_message::<RawGamepadButtonChangedEvent>()
       .add_message::<bevy::input::gamepad::GamepadButtonChangedEvent>()
       .add_message::<bevy::input::gamepad::GamepadButtonStateChangedEvent>()
       .add_message::<bevy::input::gamepad::GamepadAxisChangedEvent>()
       .add_message::<RawGamepadAxisChangedEvent>()
       .add_message::<RawGamepadEvent>()
       .add_systems(
           PreUpdate,
           (gamepad_connection_system, gamepad_event_processing_system.after(gamepad_connection_system)),
       )
       .add_plugins(GamePlugin)
       // Pin the virtual clock to exactly one `FixedUpdate` tick's worth of time per
       // `app.update()` call — not real wall-clock time. As of `planning/features/
       // deterministic_fixed_timestep.md` (v1), Rapier physics runs in `FixedUpdate` alongside
       // the gameplay chain; without this, every test in this suite that constructs a physics
       // world (`RigidBody`/`Collider`/`Velocity`) would step Rapier a wall-clock-dependent,
       // nondeterministic number of times per `app.update()` call, since headless tests run far
       // faster than real time and the elapsed-time accumulator would rarely cross a full tick.
       // A test needing a different cadence (e.g. a settle/advance loop simulating a longer
       // real-time gap) overrides this afterward with its own `insert_resource` call, same as
       // before this existed.
       .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
           std::time::Duration::from_secs_f32(1.0 / ironhold_core::capabilities::physics::FIXED_TICK_RATE),
       ));
    app
}

/// Simulates a freshly-connected gamepad, returning its `Entity` (the value to use as a
/// player's `InputMap.gamepad_index` *seed* — index into connection order, sorted by
/// `Entity::index()`, same sort `gamepad_bind_system` uses to resolve a pending player's seed
/// into a `BoundGamepad`; see `gamepad_player_binding_hardening.md`. The old crate-shared
/// `resolve_gamepad` helper this comment used to reference was deleted along with that refactor
/// — every consumer now either reads `BoundGamepad` directly or re-sorts inline). Call
/// `app.update()` once after this (and before pressing any button/setting any axis) so
/// `gamepad_connection_system` has spawned the real `Gamepad` component before the input systems
/// under test read it.
pub fn connect_test_gamepad(app: &mut App) -> Entity {
    let gamepad = app.world_mut().spawn_empty().id();
    app.world_mut()
        .resource_mut::<Messages<GamepadConnectionEvent>>()
        .write(GamepadConnectionEvent::new(
            gamepad,
            GamepadConnection::Connected {
                name: "Test Gamepad".to_string(),
                vendor_id: None,
                product_id: None,
            },
        ));
    gamepad
}

/// Simulates a button press on a connected gamepad (see `connect_test_gamepad`). Takes effect
/// on the next `app.update()` — `just_pressed` is true for exactly that one frame, mirroring
/// real hardware input.
pub fn press_gamepad_button(app: &mut App, gamepad: Entity, button: GamepadButton) {
    app.world_mut()
        .resource_mut::<Messages<RawGamepadEvent>>()
        .write(RawGamepadEvent::Button(RawGamepadButtonChangedEvent::new(
            gamepad, button, 1.0,
        )));
}

/// Simulates an analog stick/axis value on a connected gamepad (see `connect_test_gamepad`).
/// Takes effect on the next `app.update()`.
pub fn set_gamepad_axis(app: &mut App, gamepad: Entity, axis: GamepadAxis, value: f32) {
    app.world_mut()
        .resource_mut::<Messages<RawGamepadEvent>>()
        .write(RawGamepadEvent::Axis(RawGamepadAxisChangedEvent::new(gamepad, axis, value)));
}

/// The `Update` schedule's executable system order (what the single-threaded executor, i.e. the web build,
/// runs, `ApplyDeferred` sync nodes included), read without running a frame. Builds the schedule by
/// `initialize`, so call it on a fresh `setup_test_app()`; it never calls `app.update()`.
///
/// Note: the test app is `MinimalPlugins` + `GamePlugin` (no UI/picking/audio/render systems, no `inspector`/`webgpu`),
/// so this is a proxy for the web graph, not the web graph itself.
pub fn update_schedule_order(app: &mut App) -> Vec<String> {
    app.finish();
    app.cleanup();
    app.world_mut().schedule_scope(Update, |world, schedule| {
        schedule.initialize(world).expect("the Update schedule must build");
        schedule
            .systems()
            .expect("the schedule was just initialized")
            .map(|(_, system)| system.name().to_string())
            .collect()
    })
}

/// Every pair of `Update` systems that still conflict on data without being ordered (what Bevy would call an
/// ambiguity), as `(system a, system b, conflicting component/resource names)`. An empty list means the two conflict on
/// whole-`World` access (an exclusive system). `Schedule::initialize` computes it regardless of `ScheduleBuildSettings`.
pub fn update_schedule_conflicts(app: &mut App) -> Vec<(String, String, Vec<String>)> {
    app.finish();
    app.cleanup();
    app.world_mut().schedule_scope(Update, |world, schedule| {
        schedule.initialize(world).expect("the Update schedule must build");
        // `Schedule::initialize` moves the systems out of the graph, so resolve names through `schedule.systems()`
        // (same `SystemKey`s) instead of `graph().get_node_name`.
        let names: std::collections::HashMap<_, _> = schedule
            .systems()
            .expect("the schedule was just initialized")
            .map(|(key, system)| (key, system.name().to_string()))
            .collect();
        let components = world.components();
        schedule
            .graph()
            .conflicting_systems()
            .iter()
            .map(|(a, b, ids)| {
                (
                    names[a].clone(),
                    names[b].clone(),
                    ids.iter().map(|id| components.get_name(*id).map(|n| n.to_string()).unwrap_or_default()).collect(),
                )
            })
            .collect()
    })
}

/// What one `Update` system touches, read from its real access (so writers hidden inside `SystemParam` bundles,
/// e.g. `SimClock` inside `SceneV2Params`, are seen). Resource labels: `UiEvent`, `GameEvent`, `SceneEvent`
/// (the `Messages<T>` buffers), `ActionQueue`, `SimClock`, `KeyCode`, `MouseButton` (`ButtonInput<T>`), `MouseMotion`.
pub struct SystemReport {
    pub name: String,
    pub short: String,
    pub reads: std::collections::BTreeSet<&'static str>,
    pub writes: std::collections::BTreeSet<&'static str>,
    /// Resources read by the system's own run conditions (`conflicting_systems()` ignores condition access).
    pub cond_reads: std::collections::BTreeSet<&'static str>,
}

/// A read-only picture of the built `Update` schedule (D3 tests). Nothing here runs a frame.
pub struct UpdateReport {
    /// Executable order, `ApplyDeferred` sync nodes included (what the single-threaded executor runs).
    pub order: Vec<String>,
    pub systems: Vec<SystemReport>,
    /// `"GameplaySet::Emit"`-style label -> short names of the systems in that set (nested sets included).
    pub members: std::collections::HashMap<String, std::collections::BTreeSet<String>>,
    pub warnings: Vec<String>,
    /// Still-unordered conflicts: `(system a, system b, conflicting component/resource names)`.
    pub conflicts: Vec<(String, String, Vec<String>)>,
    /// Gameplay sets that carry a run condition (there must be none: Bevy evaluates a set condition once, at the first
    /// member's turn, and skips every member).
    pub set_conditions: Vec<String>,
}

impl UpdateReport {
    pub fn short(name: &str) -> &str {
        name.rsplit("::").next().unwrap_or(name)
    }
    pub fn system(&self, short: &str) -> &SystemReport {
        let hits: Vec<_> = self.systems.iter().filter(|s| s.short == short).collect();
        assert!(!hits.is_empty(), "no Update system named `{short}` (renamed or unregistered?)");
        hits[0]
    }
    pub fn count(&self, short: &str) -> usize {
        self.systems.iter().filter(|s| s.short == short).count()
    }
    /// Index of the first/last executable-order entry whose short name is `short`.
    pub fn first_pos(&self, short: &str) -> usize {
        self.order.iter().position(|n| Self::short(n) == short).unwrap_or_else(|| panic!("`{short}` is not in the Update order"))
    }
    pub fn last_pos(&self, short: &str) -> usize {
        self.order.iter().rposition(|n| Self::short(n) == short).unwrap_or_else(|| panic!("`{short}` is not in the Update order"))
    }
    pub fn in_set(&self, set: &str, short: &str) -> bool {
        self.members.get(set).is_some_and(|m| m.contains(short))
    }
}

pub fn inspect_update_schedule(app: &mut App) -> UpdateReport {
    inspect_schedule(app, Update)
}

/// Same as [`inspect_update_schedule`] for any schedule (e.g. `FixedUpdate`).
pub fn inspect_schedule<L: bevy::ecs::schedule::ScheduleLabel + Clone>(app: &mut App, label: L) -> UpdateReport {
    use bevy::ecs::schedule::SystemSet;
    use ironhold_core::runtime::{ActionQueue, EmitSet, GameStreamSet, GameplaySet, SimClock};
    use std::collections::{BTreeSet, HashMap};

    app.finish();
    app.cleanup();
    app.world_mut().schedule_scope(label, |world, schedule| {
        let tracked: Vec<(&'static str, Option<bevy::ecs::component::ComponentId>)> = vec![
            ("UiEvent", world.resource_id::<Messages<UiEvent>>()),
            ("GameEvent", world.resource_id::<Messages<GameEvent>>()),
            ("SceneEvent", world.resource_id::<Messages<SceneEvent>>()),
            ("ActionQueue", world.resource_id::<ActionQueue>()),
            ("SimClock", world.resource_id::<SimClock>()),
            ("KeyCode", world.resource_id::<ButtonInput<KeyCode>>()),
            ("MouseButton", world.resource_id::<ButtonInput<MouseButton>>()),
            ("MouseMotion", world.resource_id::<Messages<bevy::input::mouse::MouseMotion>>()),
            ("LoadedKeyBindings", world.resource_id::<ironhold_core::runtime::LoadedKeyBindings>()),
            ("LoadedGamepadBindings", world.resource_id::<ironhold_core::runtime::LoadedGamepadBindings>()),
            ("CurrentTarget", world.resource_id::<ironhold_core::capabilities::action_bar::CurrentTarget>()),
        ];
        let label = |access: &bevy::ecs::query::FilteredAccessSet, write: bool| -> BTreeSet<&'static str> {
            let combined = access.combined_access();
            tracked
                .iter()
                .filter_map(|(name, id)| {
                    // A missing resource would silently turn a check vacuous: fail loudly instead.
                    let id = id.unwrap_or_else(|| panic!("tracked resource `{name}` is not registered in the test app (renamed or removed?)"));
                    let hit = if write { combined.has_resource_write(id) } else { combined.has_resource_read(id) };
                    hit.then_some(*name)
                })
                .collect()
        };

        // Step 1: `Schedule::initialize` moves systems and conditions out of the graph, so collect access first.
        schedule.graph_mut().initialize(world);
        let mut systems = Vec::new();
        for (key, system, conditions) in schedule.graph().systems.iter() {
            let name = system.name().to_string();
            let access = &schedule.graph().systems.get(key).expect("system exists").access;
            let mut cond_reads = BTreeSet::new();
            for c in conditions {
                cond_reads.extend(label(&c.access, false));
            }
            systems.push(SystemReport {
                short: UpdateReport::short(&name).to_string(),
                name,
                reads: label(access, false),
                writes: label(access, true),
                cond_reads,
            });
        }

        // Set-level run conditions (checked before the build moves them): the gameplay sets must carry none.
        let mut set_conditions: Vec<String> = Vec::new();
        {
            let sets = &schedule.graph().system_sets;
            let mut check = |label: String, set: bevy::ecs::intern::Interned<dyn SystemSet>| {
                if let Some(key) = sets.get_key(set) {
                    if sets.get_conditions(key).is_some_and(|c| !c.is_empty()) {
                        set_conditions.push(label);
                    }
                }
            };
            for set in [GameplaySet::Emit, GameplaySet::Interpret, GameplaySet::Execute, GameplaySet::PostExecute] {
                check(format!("GameplaySet::{set:?}"), set.intern());
            }
            for set in [EmitSet::Scene, EmitSet::Ui, EmitSet::Game, EmitSet::DirectActions, EmitSet::Dialogue] {
                check(format!("EmitSet::{set:?}"), set.intern());
            }
            for set in [GameStreamSet::Targeting, GameStreamSet::ActionBar, GameStreamSet::Interact, GameStreamSet::DelayedEvents, GameStreamSet::Stats] {
                check(format!("GameStreamSet::{set:?}"), set.intern());
            }
        }

        // Step 2: build the schedule and read what only exists after it.
        schedule.initialize(world).expect("the Update schedule must build");
        let order: Vec<String> = schedule
            .systems()
            .expect("the schedule was just initialized")
            .map(|(_, s)| s.name().to_string())
            .collect();
        let exec_names: HashMap<_, _> = schedule.systems().expect("initialized").map(|(k, s)| (k, s.name().to_string())).collect();

        let mut members: HashMap<String, BTreeSet<String>> = HashMap::new();
        let mut record = |label: &str, set: bevy::ecs::intern::Interned<dyn SystemSet>| {
            if let Ok(keys) = schedule.graph().systems_in_set(set) {
                members.insert(
                    label.to_string(),
                    keys.iter().filter_map(|k| exec_names.get(k)).map(|n| UpdateReport::short(n).to_string()).collect(),
                );
            }
        };
        for s in [GameplaySet::Emit, GameplaySet::Interpret, GameplaySet::Execute, GameplaySet::PostExecute] {
            record(&format!("GameplaySet::{s:?}"), s.intern());
        }
        for s in [EmitSet::Scene, EmitSet::Ui, EmitSet::Game, EmitSet::DirectActions, EmitSet::Dialogue] {
            record(&format!("EmitSet::{s:?}"), s.intern());
        }
        for s in [GameStreamSet::Targeting, GameStreamSet::ActionBar, GameStreamSet::Interact, GameStreamSet::DelayedEvents, GameStreamSet::Stats] {
            record(&format!("GameStreamSet::{s:?}"), s.intern());
        }

        let components = world.components();
        let conflicts = schedule
            .graph()
            .conflicting_systems()
            .iter()
            .map(|(a, b, ids)| {
                (
                    exec_names[a].clone(),
                    exec_names[b].clone(),
                    ids.iter().map(|id| components.get_name(*id).map(|n| n.to_string()).unwrap_or_default()).collect(),
                )
            })
            .collect();
        let warnings = schedule.warnings().iter().map(|w| format!("{w:?}")).collect();

        UpdateReport { order, systems, members, warnings, conflicts, set_conditions }
    })
}
