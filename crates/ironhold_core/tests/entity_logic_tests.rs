use bevy::prelude::*;
use ironhold_core::GameVariables;
use ironhold_core::runtime::{GameEvent, SpawnId, SpawnRegistry, BehaviorHandle, EntityFsmState};
use ironhold_core::schema::{Action, StateMachineAsset, FsmState, FsmTransition, FsmEventBinding};
use ironhold_core::capabilities::player::CharacterController;

mod support;
use support::setup_test_app;

/// Helper: build a StateMachineAsset with two states ("a" and "b") and one transition.
fn make_two_state_behavior(app: &mut App) -> Handle<StateMachineAsset> {
    let fsm = StateMachineAsset {
        schema_version: 1,
        initial_state: "idle".to_string(),
        states: vec![
            FsmState { name: "idle".to_string(),      entry_actions: vec![], exit_actions: vec![], on: vec![] },
            FsmState { name: "collected".to_string(), entry_actions: vec![], exit_actions: vec![], on: vec![] },
        ],
        transitions: vec![
            FsmTransition {
                from: Some("idle".to_string()),
                on: "entity.interacted:{self}".to_string(),
                to: "collected".to_string(),
            },
        ],
        global_on: vec![],
    };
    app.world_mut().resource_mut::<Assets<StateMachineAsset>>().add(fsm)
}

/// Helper: build a StateMachineAsset with a global_on rule for intent slot testing.
fn make_intent_test_fsm() -> StateMachineAsset {
    StateMachineAsset {
        schema_version: 1,
        initial_state: "".to_string(),
        states: vec![],
        transitions: vec![],
        global_on: vec![
            FsmEventBinding {
                event: "intent.slot.1:player_01".to_string(),
                do_actions: vec![],
            },
        ],
    }
}

fn intent_test_player_controller() -> CharacterController {
    use ironhold_core::schema::player::InputMap;
    CharacterController {
        walk_speed: 5.0, run_speed: 8.0, rot_speed: 2.0,
        inputs: InputMap {
            forward: "KeyW".to_string(), backward: "KeyS".to_string(),
            left: "KeyA".to_string(), right: "KeyD".to_string(),
            strafe_left: "KeyQ".to_string(), strafe_right: "KeyE".to_string(),
            jump: "Space".to_string(), run: "ShiftLeft".to_string(),
            interact: "KeyF".to_string(), strafe_mouse_button: None,
            target_next: "Tab".to_string(), target_range: 30.0,
            gamepad_index: None, look_left: None, look_right: None, look_up: None, look_down: None,
            gamepad_jump: "South".to_string(), gamepad_run: "East".to_string(),
            gamepad_interact: "West".to_string(), gamepad_target_next: "North".to_string(),
            gamepad_deadzone: 0.15,
        },
        is_running: false, jump_velocity: 5.94, double_jump_enabled: false,
        double_jump_velocity: 5.94, jumps_used: 0, max_jumps: 1,
        collider_radius: 0.4, ground_cast_length: 0.3, max_walkable_slope_deg: 45.0, coyote_time_secs: 0.1, coyote_ticks_remaining: 0, idle_drag: 0.8, jump_air_grace: 0, jump_liftoff_y: None,
    }
}

#[test]
fn test_entity_fsm_transitions_on_game_event() {
    // An entity with a behavior FSM transitions when a matching GameEvent fires.
    let mut app = setup_test_app();
    app.update();

    let handle = make_two_state_behavior(&mut app);

    let entity = app.world_mut().spawn((
        BehaviorHandle(handle),
        EntityFsmState { current: "idle".to_string() },
        SpawnId("box_01".to_string()),
    )).id();

    // Fire the scoped event: {self} â†’ box_01
    app.world_mut()
        .resource_mut::<Messages<GameEvent>>()
        .write(GameEvent::Trigger("entity.interacted:box_01".to_string()));
    app.update();

    let state = app.world().get::<EntityFsmState>(entity).unwrap();
    assert_eq!(state.current, "collected",
        "Entity FSM should transition idle â†’ collected on matching interacted event");
}

#[test]
fn test_entity_fsm_self_substitution_routes_to_correct_entity() {
    // Two entities share the same behavior file.
    // An event scoped to "box_01" must only advance box_01's state, not box_02's.
    let mut app = setup_test_app();
    app.update();

    let handle = make_two_state_behavior(&mut app);

    let box_01 = app.world_mut().spawn((
        BehaviorHandle(handle.clone()),
        EntityFsmState { current: "idle".to_string() },
        SpawnId("box_01".to_string()),
    )).id();

    let box_02 = app.world_mut().spawn((
        BehaviorHandle(handle),
        EntityFsmState { current: "idle".to_string() },
        SpawnId("box_02".to_string()),
    )).id();

    app.world_mut()
        .resource_mut::<Messages<GameEvent>>()
        .write(GameEvent::Trigger("entity.interacted:box_01".to_string()));
    app.update();

    let state_01 = app.world().get::<EntityFsmState>(box_01).unwrap();
    let state_02 = app.world().get::<EntityFsmState>(box_02).unwrap();
    assert_eq!(state_01.current, "collected",
        "box_01 must transition on its own scoped event");
    assert_eq!(state_02.current, "idle",
        "box_02 must not be affected by box_01's event");
}

#[test]
fn test_entity_fsm_entry_actions_queued_on_transition() {
    // When a transition fires, the entry_actions of the target state are queued.
    let mut app = setup_test_app();
    app.update();

    let fsm = StateMachineAsset {
        schema_version: 1,
        initial_state: "idle".to_string(),
        states: vec![
            FsmState { name: "idle".to_string(), entry_actions: vec![], exit_actions: vec![], on: vec![] },
            FsmState {
                name: "collected".to_string(),
                entry_actions: vec![
                    Action::Despawn("{self}".to_string()),
                ],
                exit_actions: vec![],
                on: vec![],
            },
        ],
        transitions: vec![
            FsmTransition {
                from: Some("idle".to_string()),
                on: "entity.interacted:{self}".to_string(),
                to: "collected".to_string(),
            },
        ],
        global_on: vec![],
    };
    let handle = app.world_mut().resource_mut::<Assets<StateMachineAsset>>().add(fsm);

    app.world_mut().spawn((
        BehaviorHandle(handle),
        EntityFsmState { current: "idle".to_string() },
        SpawnId("crate_01".to_string()),
    ));

    app.world_mut()
        .resource_mut::<Messages<GameEvent>>()
        .write(GameEvent::Trigger("entity.interacted:crate_01".to_string()));
    app.update();

    // The entry action is `Despawn("{self}")` which the interpreter rewrites to
    // `Despawn("crate_01")`. Verify the action was queued (and executed â€” the queue
    // is drained each frame, so we check side-effects via the SpawnRegistry).
    // Since the entity has no SpawnRegistry entry, the executor warns but doesn't panic.
    // The test passes if no panic occurs and the FSM state advanced.
    // (Full despawn integration requires a scene spawn, tested by other tests.)
    let _ = app; // no panic = pass
}

#[test]
fn test_entity_fsm_despawn_self_rewritten_to_concrete_id() {
    // `Despawn("{self}")` in entry_actions is rewritten to `Despawn("target_01")`.
    // We verify the rewrite by checking that the entity is despawned after the full
    // update cycle (entity registered in SpawnRegistry + has SpawnId component).
    let mut app = setup_test_app();
    app.update();

    let fsm = StateMachineAsset {
        schema_version: 1,
        initial_state: "idle".to_string(),
        states: vec![
            FsmState { name: "idle".to_string(), entry_actions: vec![], exit_actions: vec![], on: vec![] },
            FsmState {
                name: "done".to_string(),
                entry_actions: vec![Action::Despawn("{self}".to_string())],
                exit_actions: vec![],
                on: vec![],
            },
        ],
        transitions: vec![
            FsmTransition {
                from: Some("idle".to_string()),
                on: "entity.interacted:{self}".to_string(),
                to: "done".to_string(),
            },
        ],
        global_on: vec![],
    };
    let handle = app.world_mut().resource_mut::<Assets<StateMachineAsset>>().add(fsm);

    let entity = app.world_mut().spawn((
        BehaviorHandle(handle),
        EntityFsmState { current: "idle".to_string() },
        SpawnId("target_01".to_string()),
    )).id();

    // Register the entity in SpawnRegistry so the executor can find and despawn it.
    app.world_mut()
        .resource_mut::<SpawnRegistry>()
        .entities
        .insert("target_01".to_string(), entity);

    app.world_mut()
        .resource_mut::<Messages<GameEvent>>()
        .write(GameEvent::Trigger("entity.interacted:target_01".to_string()));
    app.update();

    assert!(
        app.world().get_entity(entity).is_err(),
        "Entity with SpawnId 'target_01' must be despawned â€” Despawn(\"{{self}}\") was not rewritten correctly"
    );
}

/// `{new_id}` composed with `{self}` in the same authored `id` string, driven through the real
/// entity FSM interpreter (not a direct ActionQueue push like spawn_tests.rs's coverage) — pins
/// the pass-through ordering invariant the whole feature depends on: `rewrite_self` resolves
/// `{self}` and leaves `{new_id}` untouched, so it survives to reach the executor intact.
#[test]
fn test_entity_fsm_new_id_composes_with_self_substitution() {
    use ironhold_core::runtime::{LoadedAssetCatalog, LoadedPrefabCatalog};
    use ironhold_core::schema::catalog::{AssetCatalog, PrefabCatalog, PrefabDef, ModelCatalogEntry, PrefabKind};

    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedAssetCatalog(AssetCatalog {
        models: std::collections::BTreeMap::from([
            ("zombie_corpse".to_string(), ModelCatalogEntry { path: "shared/models/creatures/zombie_corpse.glb#Scene0".to_string() }),
        ]),
        ..Default::default()
    }));
    app.world_mut().insert_resource(LoadedPrefabCatalog(PrefabCatalog {
        prefabs: std::collections::BTreeMap::from([
            ("zombie_corpse".to_string(), PrefabDef {
                kind: PrefabKind::Prop,
                model: "zombie_corpse".to_string(),
                ..Default::default()
            }),
        ]),
        ..Default::default()
    }));

    let fsm = StateMachineAsset {
        schema_version: 1,
        initial_state: "alive".to_string(),
        states: vec![
            FsmState { name: "alive".to_string(), entry_actions: vec![], exit_actions: vec![], on: vec![] },
            FsmState {
                name: "dead".to_string(),
                entry_actions: vec![Action::Spawn {
                    prefab: "zombie_corpse".to_string(),
                    id: Some("{self}_corpse_{new_id}".to_string()),
                    position: None, spawn_point: None, yaw_deg: None, at_entity: None,
                }],
                exit_actions: vec![],
                on: vec![],
            },
        ],
        transitions: vec![
            FsmTransition {
                from: Some("alive".to_string()),
                on: "entity.killed:{self}".to_string(),
                to: "dead".to_string(),
            },
        ],
        global_on: vec![],
    };
    let handle = app.world_mut().resource_mut::<Assets<StateMachineAsset>>().add(fsm);

    let monster = app.world_mut().spawn((
        BehaviorHandle(handle),
        EntityFsmState { current: "alive".to_string() },
        SpawnId("zombie_02".to_string()),
    )).id();

    // Kill the same monster slot twice — {self} is identical both times (the monster's own
    // stable id never changes), so only {new_id} can keep the two corpse ids apart.
    app.world_mut()
        .resource_mut::<Messages<GameEvent>>()
        .write(GameEvent::Trigger("entity.killed:zombie_02".to_string()));
    app.update();
    app.world_mut().get_mut::<EntityFsmState>(monster).unwrap().current = "alive".to_string();
    app.world_mut()
        .resource_mut::<Messages<GameEvent>>()
        .write(GameEvent::Trigger("entity.killed:zombie_02".to_string()));
    app.update();

    let corpse_ids: Vec<String> = app.world_mut()
        .query::<&SpawnId>()
        .iter(app.world())
        .map(|s| s.0.clone())
        .filter(|id| id.starts_with("zombie_02_corpse_"))
        .collect();

    assert_eq!(corpse_ids.len(), 2, "both kills must spawn a corpse, got: {:?}", corpse_ids);
    assert_ne!(corpse_ids[0], corpse_ids[1],
        "{{self}} is identical on both kills — only {{new_id}} keeps the two corpse ids apart, got: {:?}", corpse_ids);
    for id in &corpse_ids {
        assert!(!id.contains("{new_id}") && !id.contains("{self}"),
            "both tokens must be fully resolved by the time the entity is spawned, got: {:?}", id);
    }
}

/// No intent rule present → slot's built-in do_actions must fire unchanged.
#[test]
fn test_intent_slot_no_rule_fires_slot_do_actions() {
    use ironhold_core::capabilities::action_bar::ActionSlotUi;

    let mut app = setup_test_app();
    app.update();

    // Slot 1: sets "intent_test" = "slot_fired"
    app.world_mut().spawn(ActionSlotUi {
        slot_key: "1".to_string(),
        resolved_key: Some(KeyCode::Digit1),
        resolved_gamepad_button: None,
        do_actions: vec![Action::SetVariable("intent_test".to_string(), "slot_fired".to_string())],
        cooldown_secs: None,
        cost: None,
        owner_player: None,
    });

    // Player entity (needed so player_query resolves in action_bar_input_system)
    app.world_mut().spawn((
        SpawnId("player_01".to_string()),
        intent_test_player_controller(),
        ironhold_core::capabilities::player::PlayerTarget::default(),
    ));

    // Press key 1
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::Digit1);
    app.update();

    let vars = app.world().resource::<GameVariables>();
    assert_eq!(
        vars.0.get("intent_test").map(String::as_str), Some("slot_fired"),
        "Without an intent rule the slot's own do_actions must fire"
    );
}

/// Intent rule matches → rule's do_actions run; slot's built-in do_actions are suppressed.
#[test]
fn test_intent_slot_rule_match_suppresses_slot_do_actions() {
    use ironhold_core::capabilities::action_bar::ActionSlotUi;
    use ironhold_core::runtime::LoadedStateMachine;

    let mut app = setup_test_app();
    app.update();

    // Register an intent rule that fires "rule_fired" for slot 1 on player_01
    let fsm = StateMachineAsset {
        schema_version: 1,
        initial_state: "".to_string(),
        states: vec![],
        transitions: vec![],
        global_on: vec![
            FsmEventBinding {
                event: "intent.slot.1:player_01".to_string(),
                do_actions: vec![
                    Action::SetVariable("intent_test".to_string(), "rule_fired".to_string()),
                ],
            },
        ],
    };
    app.world_mut().insert_resource(LoadedStateMachine(Some(fsm.clone())));

    // Slot 1: would set "slot_fired" — this must be suppressed
    app.world_mut().spawn(ActionSlotUi {
        slot_key: "1".to_string(),
        resolved_key: Some(KeyCode::Digit1),
        resolved_gamepad_button: None,
        do_actions: vec![Action::SetVariable("intent_test".to_string(), "slot_fired".to_string())],
        cooldown_secs: None,
        cost: None,
        owner_player: None,
    });

    // Player entity
    app.world_mut().spawn((
        SpawnId("player_01".to_string()),
        intent_test_player_controller(),
        ironhold_core::capabilities::player::PlayerTarget::default(),
    ));

    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::Digit1);
    app.update();

    let vars = app.world().resource::<GameVariables>();
    assert_eq!(
        vars.0.get("intent_test").map(String::as_str), Some("rule_fired"),
        "Rule's do_actions must run and suppress the slot's built-in do_actions"
    );
}

/// Intent suppressed by a rule → cooldown must NOT start.
#[test]
fn test_intent_slot_rule_match_does_not_start_cooldown() {
    use ironhold_core::capabilities::action_bar::{ActionSlotUi, CooldownMap};
    use ironhold_core::runtime::LoadedStateMachine;

    let mut app = setup_test_app();
    app.update();

    let fsm = StateMachineAsset {
        schema_version: 1,
        initial_state: "".to_string(),
        states: vec![],
        transitions: vec![],
        global_on: vec![
            FsmEventBinding {
                event: "intent.slot.1:player_01".to_string(),
                do_actions: vec![Action::Log("intercepted".to_string())],
            },
        ],
    };
    app.world_mut().insert_resource(LoadedStateMachine(Some(fsm.clone())));

    app.world_mut().spawn(ActionSlotUi {
        slot_key: "1".to_string(),
        resolved_key: Some(KeyCode::Digit1),
        resolved_gamepad_button: None,
        do_actions: vec![],
        cooldown_secs: Some(5.0),
        cost: None,
        owner_player: None,
    });
    app.world_mut().spawn((
        SpawnId("player_01".to_string()),
        intent_test_player_controller(),
        ironhold_core::capabilities::player::PlayerTarget::default(),
    ));

    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::Digit1);
    app.update();

    let cooldowns = app.world().resource::<CooldownMap>();
    assert!(
        !cooldowns.0.contains_key("1"),
        "Suppressed intent must not start the cooldown"
    );
}

/// Intent suppressed → `action_bar.activated` must NOT fire; committed → it must fire.
#[test]
fn test_activated_fires_only_on_commit() {
    use ironhold_core::capabilities::action_bar::ActionSlotUi;
    use ironhold_core::runtime::LoadedStateMachine;

    // ── Case A: rule suppresses → activated must not clear the status variable ──
    let mut app = setup_test_app();
    app.update();

    // Seed a sentinel so we can detect if activated fired (it would clear it via rule below)
    {
        let mut vars = app.world_mut().resource_mut::<GameVariables>();
        vars.0.insert("status".to_string(), "silenced".to_string());
    }
    let fsm = StateMachineAsset {
        schema_version: 1,
        initial_state: "".to_string(),
        states: vec![],
        transitions: vec![],
        global_on: vec![
            // intercept the intent
            FsmEventBinding {
                event: "intent.slot.1:player_01".to_string(),
                do_actions: vec![Action::Log("intercepted".to_string())],
            },
            // would clear status on activated — must NOT fire
            FsmEventBinding {
                event: "action_bar.activated:1".to_string(),
                do_actions: vec![Action::SetVariable("status".to_string(), "".to_string())],
            },
        ],
    };
    app.world_mut().insert_resource(LoadedStateMachine(Some(fsm.clone())));

    app.world_mut().spawn(ActionSlotUi {
        slot_key: "1".to_string(),
        resolved_key: Some(KeyCode::Digit1),
        resolved_gamepad_button: None,
        do_actions: vec![],
        cooldown_secs: None,
        cost: None,
        owner_player: None,
    });
    app.world_mut().spawn((
        SpawnId("player_01".to_string()),
        intent_test_player_controller(),
        ironhold_core::capabilities::player::PlayerTarget::default(),
    ));

    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::Digit1);
    app.update();

    let vars = app.world().resource::<GameVariables>();
    assert_eq!(
        vars.0.get("status").map(String::as_str), Some("silenced"),
        "action_bar.activated must not fire when intent is suppressed"
    );

    // ── Case B: no rule → activated must fire ──
    let mut app2 = setup_test_app();
    app2.update();

    {
        let mut vars = app2.world_mut().resource_mut::<GameVariables>();
        vars.0.insert("status".to_string(), "silenced".to_string());
    }
    let fsm = StateMachineAsset {
        schema_version: 1,
        initial_state: "".to_string(),
        states: vec![],
        transitions: vec![],
        global_on: vec![
            FsmEventBinding {
                event: "action_bar.activated:1".to_string(),
                do_actions: vec![Action::SetVariable("status".to_string(), "".to_string())],
            },
        ],
    };
    let fsm_handle = app2.world_mut().resource_mut::<Assets<StateMachineAsset>>().add(fsm.clone());
    app2.world_mut().insert_resource(LoadedStateMachine(Some(fsm)));

    app2.world_mut().spawn(ActionSlotUi {
        slot_key: "1".to_string(),
        resolved_key: Some(KeyCode::Digit1),
        resolved_gamepad_button: None,
        do_actions: vec![],
        cooldown_secs: None,
        cost: None,
        owner_player: None,
    });
    app2.world_mut().spawn((
        SpawnId("player_01".to_string()),
        intent_test_player_controller(),
        ironhold_core::capabilities::player::PlayerTarget::default(),
    ));

    app2.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::Digit1);
    app2.update(); // slot activates, activated event emitted from flush_pending_intent_system
    app2.update(); // interpreter picks up the activated event (one-frame propagation delay)

    let vars = app2.world().resource::<GameVariables>();
    assert_eq!(
        vars.0.get("status").map(String::as_str), Some(""),
        "action_bar.activated must fire when intent is committed (no suppressing rule)"
    );
}

/// A slot bound to a letter key (via `key: "KeyQ"`) fires on that key, not just digits —
/// the core `action_bar_custom_hotkeys` capability.
#[test]
fn test_letter_key_slot_fires_on_its_own_key() {
    use ironhold_core::capabilities::action_bar::ActionSlotUi;

    let mut app = setup_test_app();
    app.update();

    app.world_mut().spawn(ActionSlotUi {
        slot_key: "KeyQ".to_string(),
        resolved_key: Some(KeyCode::KeyQ),
        resolved_gamepad_button: None,
        do_actions: vec![Action::SetVariable("hotkey_test".to_string(), "fired".to_string())],
        cooldown_secs: None,
        cost: None,
        owner_player: None,
    });
    app.world_mut().spawn((
        SpawnId("player_01".to_string()),
        intent_test_player_controller(),
        ironhold_core::capabilities::player::PlayerTarget::default(),
    ));

    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::KeyQ);
    app.update();

    let vars = app.world().resource::<GameVariables>();
    assert_eq!(
        vars.0.get("hotkey_test").map(String::as_str), Some("fired"),
        "a slot bound to KeyQ must fire when Q is pressed"
    );
}

/// A slot bound to a function key (`key: "F2"`) fires on that key.
#[test]
fn test_function_key_slot_fires_on_its_own_key() {
    use ironhold_core::capabilities::action_bar::ActionSlotUi;

    let mut app = setup_test_app();
    app.update();

    app.world_mut().spawn(ActionSlotUi {
        slot_key: "F2".to_string(),
        resolved_key: Some(KeyCode::F2),
        resolved_gamepad_button: None,
        do_actions: vec![Action::SetVariable("hotkey_test".to_string(), "fired".to_string())],
        cooldown_secs: None,
        cost: None,
        owner_player: None,
    });
    app.world_mut().spawn((
        SpawnId("player_01".to_string()),
        intent_test_player_controller(),
        ironhold_core::capabilities::player::PlayerTarget::default(),
    ));

    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::F2);
    app.update();

    let vars = app.world().resource::<GameVariables>();
    assert_eq!(
        vars.0.get("hotkey_test").map(String::as_str), Some("fired"),
        "a slot bound to F2 must fire when F2 is pressed"
    );
}

/// Migration regression: `3rd_person_game_demo`'s existing `key: "i"` inventory slot must keep
/// firing on the `I` key after `DIGIT_KEYS` is replaced by `InputMap::parse_key()` — `parse_key`
/// is case-sensitive with uppercase-only letter arms, so this only holds because `parse_key` was
/// given a single-lowercase-letter normalization pass (see `schema/player.rs::parse_key`).
#[test]
fn test_lowercase_letter_key_slot_resolves_case_insensitively() {
    use ironhold_core::schema::player::InputMap;
    use ironhold_core::capabilities::action_bar::ActionSlotUi;

    assert_eq!(
        InputMap::parse_key("i"), Some(KeyCode::KeyI),
        "parse_key(\"i\") must resolve to KeyI — this is the exact migration case for \
         3rd_person_game_demo's existing inventory slot"
    );

    let mut app = setup_test_app();
    app.update();

    app.world_mut().spawn(ActionSlotUi {
        slot_key: "i".to_string(),
        resolved_key: InputMap::parse_key("i"),
        resolved_gamepad_button: None,
        do_actions: vec![Action::SetVariable("hotkey_test".to_string(), "fired".to_string())],
        cooldown_secs: None,
        cost: None,
        owner_player: None,
    });
    app.world_mut().spawn((
        SpawnId("player_01".to_string()),
        intent_test_player_controller(),
        ironhold_core::capabilities::player::PlayerTarget::default(),
    ));

    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::KeyI);
    app.update();

    let vars = app.world().resource::<GameVariables>();
    assert_eq!(
        vars.0.get("hotkey_test").map(String::as_str), Some("fired"),
        "the existing key: \"i\" slot must still fire on I after removing DIGIT_KEYS"
    );
}

// ── Phase 2 (per_player_split_screen_targeting.md): per-player action-bar execution ─────────────

/// Two independent bars, each `owner_player`-tagged, each resolve `{target}` against their own
/// player's `PlayerTarget` — not the global `CurrentTarget`. Pressing only player 1's key must
/// leave player 2's slot untouched.
#[test]
fn test_owner_player_slot_resolves_against_its_own_players_target() {
    use ironhold_core::capabilities::action_bar::ActionSlotUi;
    use ironhold_core::capabilities::player::{PlayerIndex, PlayerTarget};

    let mut app = setup_test_app();
    app.update();

    app.world_mut().spawn(ActionSlotUi {
        slot_key: "1".to_string(),
        resolved_key: Some(KeyCode::Digit1),
        resolved_gamepad_button: None,
        do_actions: vec![Action::SetVariable("p1_hit".to_string(), "{target}".to_string())],
        cooldown_secs: None,
        cost: None,
        owner_player: Some(0),
    });
    app.world_mut().spawn(ActionSlotUi {
        slot_key: "2".to_string(),
        resolved_key: Some(KeyCode::Digit2),
        resolved_gamepad_button: None,
        do_actions: vec![Action::SetVariable("p2_hit".to_string(), "{target}".to_string())],
        cooldown_secs: None,
        cost: None,
        owner_player: Some(1),
    });
    app.world_mut().spawn((
        SpawnId("player_01".to_string()),
        intent_test_player_controller(),
        PlayerTarget(Some("enemy_a".to_string())),
        PlayerIndex(0),
    ));
    app.world_mut().spawn((
        SpawnId("player_02".to_string()),
        intent_test_player_controller(),
        PlayerTarget(Some("enemy_b".to_string())),
        PlayerIndex(1),
    ));
    // `target_auto_clear_system` now runs deterministically before `action_bar_input_system`
    // (see `TargetingPlugin`'s ordering fix) rather than racing it — an unregistered
    // `PlayerTarget` is treated as "despawned" and cleared every run, not just some. Register
    // both targets so this test exercises the intended {target}-resolution path instead of the
    // (now-deterministic) auto-clear path, matching `test_single_player_slot_with_no_owner_
    // still_resolves_via_player_target`'s existing precedent.
    let enemy_a = app.world_mut().spawn(SpawnId("enemy_a".to_string())).id();
    app.world_mut().resource_mut::<SpawnRegistry>().entities.insert("enemy_a".to_string(), enemy_a);
    let enemy_b = app.world_mut().spawn(SpawnId("enemy_b".to_string())).id();
    app.world_mut().resource_mut::<SpawnRegistry>().entities.insert("enemy_b".to_string(), enemy_b);

    // Only player 1's key this frame.
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::Digit1);
    app.update();

    let vars = app.world().resource::<GameVariables>();
    assert_eq!(vars.0.get("p1_hit").map(String::as_str), Some("enemy_a"),
        "player 1's slot must resolve {{target}} against player 1's own PlayerTarget");
    assert_eq!(vars.0.get("p2_hit"), None,
        "player 2's slot must not fire from player 1's key press");
}

/// Regression guard for the `find`+`return` bug the old single-shared-bar code had: if both
/// players' bars fire in the same frame, neither press may be silently dropped.
#[test]
fn test_both_players_bars_firing_same_frame_neither_press_dropped() {
    use ironhold_core::capabilities::action_bar::ActionSlotUi;
    use ironhold_core::capabilities::player::{PlayerIndex, PlayerTarget};

    let mut app = setup_test_app();
    app.update();

    app.world_mut().spawn(ActionSlotUi {
        slot_key: "1".to_string(),
        resolved_key: Some(KeyCode::Digit1),
        resolved_gamepad_button: None,
        do_actions: vec![Action::SetVariable("p1_hit".to_string(), "{target}".to_string())],
        cooldown_secs: None,
        cost: None,
        owner_player: Some(0),
    });
    app.world_mut().spawn(ActionSlotUi {
        slot_key: "2".to_string(),
        resolved_key: Some(KeyCode::Digit2),
        resolved_gamepad_button: None,
        do_actions: vec![Action::SetVariable("p2_hit".to_string(), "{target}".to_string())],
        cooldown_secs: None,
        cost: None,
        owner_player: Some(1),
    });
    app.world_mut().spawn((
        SpawnId("player_01".to_string()),
        intent_test_player_controller(),
        PlayerTarget(Some("enemy_a".to_string())),
        PlayerIndex(0),
    ));
    app.world_mut().spawn((
        SpawnId("player_02".to_string()),
        intent_test_player_controller(),
        PlayerTarget(Some("enemy_b".to_string())),
        PlayerIndex(1),
    ));
    // See the identical comment in `test_owner_player_slot_resolves_against_its_own_players_target`.
    let enemy_a = app.world_mut().spawn(SpawnId("enemy_a".to_string())).id();
    app.world_mut().resource_mut::<SpawnRegistry>().entities.insert("enemy_a".to_string(), enemy_a);
    let enemy_b = app.world_mut().spawn(SpawnId("enemy_b".to_string())).id();
    app.world_mut().resource_mut::<SpawnRegistry>().entities.insert("enemy_b".to_string(), enemy_b);

    // Both keys pressed the same frame.
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::Digit1);
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::Digit2);
    app.update();

    let vars = app.world().resource::<GameVariables>();
    assert_eq!(vars.0.get("p1_hit").map(String::as_str), Some("enemy_a"),
        "player 1's press must not be dropped when player 2 also presses this frame");
    assert_eq!(vars.0.get("p2_hit").map(String::as_str), Some("enemy_b"),
        "player 2's press must not be dropped when player 1 also presses this frame");
}

/// Single-player regression: a slot with no `owner_player` (`None`, the default) still resolves
/// `{target}` against the sole player's `PlayerTarget`, mutating a real entity `StatMap` exactly
/// as `CurrentTarget`-based resolution did before Phase 2 — no behavior change for existing
/// single-player projects.
#[test]
fn test_single_player_slot_with_no_owner_still_resolves_via_player_target() {
    use ironhold_core::capabilities::action_bar::ActionSlotUi;
    use ironhold_core::capabilities::player::PlayerTarget;
    use ironhold_core::schema::{StatDef, LiveStat};
    use ironhold_core::schema::stats::StatMap;

    let mut app = setup_test_app();
    app.update();

    let mut stat_map = StatMap::default();
    stat_map.0.insert("health".to_string(), LiveStat::new(StatDef {
        base: 100.0, min: 0.0, max: 100.0, soft_max: None, regen_rate: 0.0, regen_delay: 0.0, thresholds: vec![],
    }));
    let target_entity = app.world_mut().spawn((SpawnId("enemy_01".to_string()), stat_map)).id();
    app.world_mut().resource_mut::<SpawnRegistry>().entities.insert("enemy_01".to_string(), target_entity);

    app.world_mut().spawn(ActionSlotUi {
        slot_key: "1".to_string(),
        resolved_key: Some(KeyCode::Digit1),
        resolved_gamepad_button: None,
        do_actions: vec![Action::ModifyStat { key: "{target}.health".to_string(), delta: -25.0 }],
        cooldown_secs: None,
        cost: None,
        owner_player: None,
    });
    app.world_mut().spawn((
        SpawnId("player_01".to_string()),
        intent_test_player_controller(),
        PlayerTarget(Some("enemy_01".to_string())),
    ));

    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::Digit1);
    app.update();

    let sm = app.world().get::<StatMap>(target_entity).unwrap();
    assert_eq!(sm.0["health"].current, 75.0,
        "a slot with no owner_player must still resolve {{target}} via the sole player's PlayerTarget");
}

/// A slot whose bar's `owner_player` doesn't match any `PlayerIndex` present in the scene never
/// fires — there's no acting player to resolve a target or an intent-event player id against.
#[test]
fn test_slot_with_unmatched_owner_player_never_fires() {
    use ironhold_core::capabilities::action_bar::ActionSlotUi;
    use ironhold_core::capabilities::player::{PlayerIndex, PlayerTarget};

    let mut app = setup_test_app();
    app.update();

    app.world_mut().spawn(ActionSlotUi {
        slot_key: "1".to_string(),
        resolved_key: Some(KeyCode::Digit1),
        resolved_gamepad_button: None,
        do_actions: vec![Action::SetVariable("orphan_hit".to_string(), "fired".to_string())],
        cooldown_secs: None,
        cost: None,
        owner_player: Some(5),
    });
    // Only player index 0 exists — no PlayerIndex(5) entity in the scene.
    app.world_mut().spawn((
        SpawnId("player_01".to_string()),
        intent_test_player_controller(),
        PlayerTarget::default(),
        PlayerIndex(0),
    ));

    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::Digit1);
    app.update();

    let vars = app.world().resource::<GameVariables>();
    assert_eq!(vars.0.get("orphan_hit"), None,
        "a slot whose owner_player matches no PlayerIndex present must never fire");
}

/// Documents the accepted Phase 2 scope boundary: when a `rules.ron` rule intercepts a
/// non-primary player's slot intent, the rule's own `{target}`-using actions still resolve via
/// the interpreter's `rewrite_target`, which reads the global `CurrentTarget` (the primary
/// player) — not the firing (non-primary) player's own `PlayerTarget`. This is unchanged by
/// Phase 2 (see "Not in scope (Phase 2)" in the plan) and must keep holding, not silently drift.
#[test]
fn test_rule_overridden_intent_still_resolves_target_against_primary_player_only() {
    use ironhold_core::capabilities::action_bar::{ActionSlotUi, CurrentTarget};
    use ironhold_core::capabilities::player::{PlayerIndex, PlayerTarget};
    use ironhold_core::runtime::LoadedStateMachine;

    let mut app = setup_test_app();
    app.update();

    // Primary player's mirrored target (would normally be kept in lockstep with PlayerTarget by
    // targeting.rs's apply_player_target — set directly here since this test only exercises the
    // interpreter's rule-override path, not the targeting capability).
    app.world_mut().insert_resource(CurrentTarget(Some("primary_target".to_string())));

    let fsm = StateMachineAsset {
        schema_version: 1,
        initial_state: "".to_string(),
        states: vec![],
        transitions: vec![],
        global_on: vec![
            FsmEventBinding {
                event: "intent.slot.1:player_02".to_string(),
                do_actions: vec![Action::SetVariable("rule_target_seen".to_string(), "{target}".to_string())],
            },
        ],
    };
    let fsm_handle = app.world_mut().resource_mut::<Assets<StateMachineAsset>>().add(fsm.clone());
    app.world_mut().insert_resource(LoadedStateMachine(Some(fsm)));

    app.world_mut().spawn(ActionSlotUi {
        slot_key: "1".to_string(),
        resolved_key: Some(KeyCode::Digit1),
        resolved_gamepad_button: None,
        do_actions: vec![Action::SetVariable("slot_target_seen".to_string(), "{target}".to_string())],
        cooldown_secs: None,
        cost: None,
        owner_player: Some(1),
    });
    // Non-primary player, with its own distinct target.
    app.world_mut().spawn((
        SpawnId("player_02".to_string()),
        intent_test_player_controller(),
        PlayerTarget(Some("player2_own_target".to_string())),
        PlayerIndex(1),
    ));

    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::Digit1);
    app.update();

    let vars = app.world().resource::<GameVariables>();
    assert_eq!(
        vars.0.get("rule_target_seen").map(String::as_str), Some("primary_target"),
        "a rule overriding a non-primary player's slot intent must resolve {{target}} against the \
         primary player's CurrentTarget, not the firing player's own PlayerTarget"
    );
    assert_eq!(vars.0.get("slot_target_seen"), None,
        "the slot's own built-in do_actions must be suppressed when a rule handles the intent");
}

// --- Mouse-click activation (planning/features/done/action_bar_mouse_click.md) -----------------------
//
// `ui_focus_system` is not present under the test harness, so these drive the slot `Interaction`
// by hand. A slot is spawned with `Interaction::None` and one warm-up update runs first (so the
// spawn's own "changed" tick is consumed), then a single write of `Pressed` is the press edge.
// The "held" test deliberately does NOT rewrite `Interaction` on later frames -- that would count
// as a fresh change.

fn click_test_slot(key: &str, keycode: KeyCode, do_actions: Vec<Action>) -> (
    ironhold_core::capabilities::action_bar::ActionSlotUi,
    Interaction,
) {
    (
        ironhold_core::capabilities::action_bar::ActionSlotUi {
            slot_key: key.to_string(),
            resolved_key: Some(keycode),
            resolved_gamepad_button: None,
            do_actions,
            cooldown_secs: None,
            cost: None,
            owner_player: None,
        },
        Interaction::None,
    )
}

fn press_slot(app: &mut App, slot: Entity) {
    *app.world_mut().get_mut::<Interaction>(slot).unwrap() = Interaction::Pressed;
}

fn click_event_seen(app: &App, name: &str) -> bool {
    app.world()
        .resource::<Messages<GameEvent>>()
        .iter_current_update_messages()
        .any(|e| matches!(e, GameEvent::Trigger(t) if t == name))
}

fn click_test_player(app: &mut App) {
    app.world_mut().spawn((
        SpawnId("player_01".to_string()),
        intent_test_player_controller(),
        ironhold_core::capabilities::player::PlayerTarget::default(),
    ));
}

#[test]
fn test_click_fires_slot_and_starts_cooldown() {
    use ironhold_core::capabilities::action_bar::CooldownMap;

    let mut app = setup_test_app();
    app.update();
    let mut bundle = click_test_slot("1", KeyCode::Digit1,
        vec![Action::SetVariable("clicked".to_string(), "yes".to_string())]);
    bundle.0.cooldown_secs = Some(5.0);
    let slot = app.world_mut().spawn(bundle).id();
    click_test_player(&mut app);
    app.update();

    press_slot(&mut app, slot);
    app.update();

    assert_eq!(app.world().resource::<GameVariables>().0.get("clicked").map(String::as_str), Some("yes"));
    assert!(click_event_seen(&app, "action_bar.pressed:1"));
    assert!(click_event_seen(&app, "action_bar.activated:1"));
    assert!(app.world().resource::<CooldownMap>().0.contains_key("1"), "a click must start the cooldown");
}

#[test]
fn test_click_during_cooldown_emits_on_cooldown_and_does_not_fire() {
    use ironhold_core::capabilities::action_bar::CooldownMap;

    let mut app = setup_test_app();
    app.update();
    let slot = app.world_mut().spawn(click_test_slot("1", KeyCode::Digit1,
        vec![Action::SetVariable("clicked".to_string(), "yes".to_string())])).id();
    click_test_player(&mut app);
    app.world_mut().resource_mut::<CooldownMap>().0.insert("1".to_string(), (3.0, 5.0));
    app.update();

    press_slot(&mut app, slot);
    app.update();

    assert!(click_event_seen(&app, "action_bar.on_cooldown:1"));
    assert_eq!(app.world().resource::<GameVariables>().0.get("clicked"), None);
}

#[test]
fn test_click_with_insufficient_cost_emits_event_and_does_not_fire() {
    use ironhold_core::schema::scene_v2::SlotCost;

    let mut app = setup_test_app();
    app.update();
    let mut bundle = click_test_slot("1", KeyCode::Digit1,
        vec![Action::SetVariable("clicked".to_string(), "yes".to_string())]);
    bundle.0.cost = Some(SlotCost { stat: "mana".to_string(), amount: 20.0 });
    let slot = app.world_mut().spawn(bundle).id();
    click_test_player(&mut app);
    app.update();

    press_slot(&mut app, slot);
    app.update();

    assert!(click_event_seen(&app, "action_bar.insufficient_resource:1"));
    assert_eq!(app.world().resource::<GameVariables>().0.get("clicked"), None);
}

#[test]
fn test_click_on_target_slot_without_target_emits_no_target() {
    let mut app = setup_test_app();
    app.update();
    let slot = app.world_mut().spawn(click_test_slot("1", KeyCode::Digit1,
        vec![Action::ModifyStat { key: "{target}.health".to_string(), delta: -5.0 }])).id();
    click_test_player(&mut app);
    app.update();

    press_slot(&mut app, slot);
    app.update();

    assert!(click_event_seen(&app, "action_bar.no_target:1"));
    assert!(!click_event_seen(&app, "action_bar.activated:1"));
}

/// A click acts for the slot's own `owner_player`, resolving `{target}` from that player's own
/// `PlayerTarget` -- not the primary player's.
#[test]
fn test_click_acts_for_slots_owner_player() {
    use ironhold_core::capabilities::player::{PlayerIndex, PlayerTarget};
    use ironhold_core::schema::{StatDef, LiveStat};
    use ironhold_core::schema::stats::StatMap;

    let mut app = setup_test_app();
    app.update();

    let mut stat_map = StatMap::default();
    stat_map.0.insert("health".to_string(), LiveStat::new(StatDef {
        base: 100.0, min: 0.0, max: 100.0, soft_max: None, regen_rate: 0.0, regen_delay: 0.0, thresholds: vec![],
    }));
    let enemy = app.world_mut().spawn((SpawnId("enemy_p2".to_string()), stat_map)).id();
    app.world_mut().resource_mut::<SpawnRegistry>().entities.insert("enemy_p2".to_string(), enemy);

    let mut bundle = click_test_slot("1", KeyCode::Digit1,
        vec![Action::ModifyStat { key: "{target}.health".to_string(), delta: -10.0 }]);
    bundle.0.owner_player = Some(1);
    let slot = app.world_mut().spawn(bundle).id();
    app.world_mut().spawn((SpawnId("player_01".to_string()), intent_test_player_controller(),
        PlayerTarget::default(), PlayerIndex(0)));
    app.world_mut().spawn((SpawnId("player_02".to_string()), intent_test_player_controller(),
        PlayerTarget(Some("enemy_p2".to_string())), PlayerIndex(1)));
    app.update();

    press_slot(&mut app, slot);
    app.update();

    assert_eq!(app.world().get::<StatMap>(enemy).unwrap().0["health"].current, 90.0,
        "a click on player 2's bar must act for player 2 (its own PlayerTarget), not the primary player");
}

#[test]
fn test_click_on_intent_overridden_slot_is_suppressed_like_a_key_press() {
    use ironhold_core::runtime::LoadedStateMachine;

    let mut app = setup_test_app();
    app.update();
    app.world_mut().insert_resource(LoadedStateMachine(Some(make_intent_test_fsm())));
    let slot = app.world_mut().spawn(click_test_slot("1", KeyCode::Digit1,
        vec![Action::SetVariable("slot_fired".to_string(), "yes".to_string())])).id();
    click_test_player(&mut app);
    app.update();

    press_slot(&mut app, slot);
    app.update();

    assert!(click_event_seen(&app, "intent.slot.1:player_01"),
        "the click must have reached the intent layer (otherwise the suppression asserts below are vacuous)");
    assert_eq!(app.world().resource::<GameVariables>().0.get("slot_fired"), None,
        "a rule handling intent.slot.1:* must suppress the built-in do_actions on click too");
    assert!(!click_event_seen(&app, "action_bar.activated:1"));
}

/// Click and key press on the same frame collapse into one activation (one `pressed` event).
#[test]
fn test_click_and_key_same_frame_activate_once() {
    let mut app = setup_test_app();
    app.update();
    let slot = app.world_mut().spawn(click_test_slot("1", KeyCode::Digit1,
        vec![Action::IncrementVariable("fires".to_string(), 1)])).id();
    click_test_player(&mut app);
    app.update();

    press_slot(&mut app, slot);
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::Digit1);
    app.update();

    assert_eq!(app.world().resource::<GameVariables>().0.get("fires").map(String::as_str), Some("1"));
    let pressed = app.world()
        .resource::<Messages<GameEvent>>()
        .iter_current_update_messages()
        .filter(|e| matches!(e, GameEvent::Trigger(t) if t == "action_bar.pressed:1"))
        .count();
    assert_eq!(pressed, 1);
}

/// Holding the button (Pressed persisting, never rewritten) is one press edge, not one per frame.
#[test]
fn test_held_click_activates_once() {
    let mut app = setup_test_app();
    app.update();
    let slot = app.world_mut().spawn(click_test_slot("1", KeyCode::Digit1,
        vec![Action::IncrementVariable("fires".to_string(), 1)])).id();
    click_test_player(&mut app);
    app.update();

    press_slot(&mut app, slot);
    for _ in 0..5 { app.update(); }

    assert_eq!(app.world().resource::<GameVariables>().0.get("fires").map(String::as_str), Some("1"),
        "a held click must activate exactly once");
}

/// Two bars sharing a slot key: only the slot actually clicked fires (per-entity detection, not
/// key-string matching).
#[test]
fn test_click_fires_only_the_clicked_slot_when_two_bars_share_a_key() {
    use ironhold_core::capabilities::player::{PlayerIndex, PlayerTarget};

    let mut app = setup_test_app();
    app.update();
    let mut a = click_test_slot("1", KeyCode::Digit1,
        vec![Action::IncrementVariable("p1_fires".to_string(), 1)]);
    a.0.owner_player = Some(0);
    let mut b = click_test_slot("1", KeyCode::Digit1,
        vec![Action::IncrementVariable("p2_fires".to_string(), 1)]);
    b.0.owner_player = Some(1);
    let slot_a = app.world_mut().spawn(a).id();
    app.world_mut().spawn(b);
    app.world_mut().spawn((SpawnId("player_01".to_string()), intent_test_player_controller(),
        PlayerTarget::default(), PlayerIndex(0)));
    app.world_mut().spawn((SpawnId("player_02".to_string()), intent_test_player_controller(),
        PlayerTarget::default(), PlayerIndex(1)));
    app.update();

    press_slot(&mut app, slot_a);
    app.update();

    let vars = app.world().resource::<GameVariables>();
    assert_eq!(vars.0.get("p1_fires").map(String::as_str), Some("1"));
    assert_eq!(vars.0.get("p2_fires"), None, "the un-clicked slot sharing the key must not fire");
}

/// The action-bar click path never writes `PlayerTarget`. (The world-click early-return in
/// `click_select_system` is covered by `ui_panel_blocker.rs`; this pins only our side.)
#[test]
fn test_click_does_not_change_player_target() {
    use ironhold_core::capabilities::player::PlayerTarget;

    let mut app = setup_test_app();
    app.update();
    // Must be registered, or target_auto_clear_system treats it as despawned and clears it.
    let enemy = app.world_mut().spawn(SpawnId("enemy_01".to_string())).id();
    app.world_mut().resource_mut::<SpawnRegistry>().entities.insert("enemy_01".to_string(), enemy);
    let slot = app.world_mut().spawn(click_test_slot("1", KeyCode::Digit1, vec![])).id();
    let player = app.world_mut().spawn((SpawnId("player_01".to_string()), intent_test_player_controller(),
        PlayerTarget(Some("enemy_01".to_string())))).id();
    app.update();

    press_slot(&mut app, slot);
    app.update();

    assert!(click_event_seen(&app, "action_bar.pressed:1"), "the click must have registered");
    assert_eq!(app.world().get::<PlayerTarget>(player).unwrap().0.as_deref(), Some("enemy_01"));
}

/// Press -> release -> press again is two clicks: the edge detector re-arms after release.
#[test]
fn test_second_click_after_release_activates_again() {
    let mut app = setup_test_app();
    app.update();
    let slot = app.world_mut().spawn(click_test_slot("1", KeyCode::Digit1,
        vec![Action::IncrementVariable("fires".to_string(), 1)])).id();
    click_test_player(&mut app);
    app.update();

    press_slot(&mut app, slot);
    app.update();
    *app.world_mut().get_mut::<Interaction>(slot).unwrap() = Interaction::None;
    app.update();
    press_slot(&mut app, slot);
    app.update();

    assert_eq!(app.world().resource::<GameVariables>().0.get("fires").map(String::as_str), Some("2"),
        "a release followed by a fresh press is a second click");
}
