use bevy::prelude::*;
use bevy::ecs::system::RunSystemOnce;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use ironhold_core::runtime::{ActionQueue, GameEvent, SpawnId, SpawnRegistry, SceneHandleV2};
use ironhold_core::schema::{
    Action, AppState, ProjectConfig, ProjectConfigHandle, GameSceneV2,
    StatDef, StatThreshold, ThresholdCondition, LiveStat, LoadedStats,
    ModifierDef, ModifierKind, StackRule, ActiveModifier, LoadedModifiers,
};
use ironhold_core::schema::stats::StatMap;
use ironhold_core::capabilities::stat_radar::StatRadarNode;
use ironhold_core::capabilities::stat_display::resolve_stat;

mod support;
use support::setup_test_app;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn make_stat_def(base: f32, max: f32) -> StatDef {
    StatDef { base, min: 0.0, max, soft_max: None, regen_rate: 0.0, regen_delay: 0.0, thresholds: vec![] }
}

fn make_additive_modifier(stat: &str, amount: f32, stack_rule: StackRule) -> ModifierDef {
    ModifierDef { stat: stat.to_string(), kind: ModifierKind::Additive(amount), duration_secs: None, stack_rule }
}

#[allow(dead_code)]
fn make_timed_additive_modifier(stat: &str, amount: f32, duration: f32) -> ModifierDef {
    ModifierDef { stat: stat.to_string(), kind: ModifierKind::Additive(amount), duration_secs: Some(duration), stack_rule: StackRule::Add }
}

fn make_multiplicative_modifier(stat: &str, factor: f32) -> ModifierDef {
    ModifierDef { stat: stat.to_string(), kind: ModifierKind::Multiplicative(factor), duration_secs: None, stack_rule: StackRule::Add }
}

// ── StatMap / ModifyStat / SetStat tests ──────────────────────────────────────

#[test]
fn test_stat_map_component_holds_correct_initial_values() {
    let mut app = setup_test_app();
    app.update();

    let mut stat_map = StatMap::default();
    stat_map.0.insert("health".to_string(), LiveStat::new(make_stat_def(80.0, 100.0)));
    let entity = app.world_mut().spawn(stat_map).id();

    let sm = app.world().get::<StatMap>(entity).unwrap();
    assert!(sm.0.contains_key("health"), "StatMap must contain the inserted stat key");
    assert_eq!(sm.0["health"].current, 80.0, "LiveStat must initialise to the declared base value");
    assert_eq!(sm.0["health"].def.max, 100.0);
}

#[test]
fn test_modify_stat_with_dot_key_routes_to_entity_stat_map() {
    let mut app = setup_test_app();
    app.update();

    let mut stat_map = StatMap::default();
    stat_map.0.insert("health".to_string(), LiveStat::new(make_stat_def(100.0, 100.0)));

    let entity = app.world_mut().spawn((
        SpawnId("goblin_01".to_string()),
        stat_map,
    )).id();
    app.world_mut()
        .resource_mut::<SpawnRegistry>()
        .entities
        .insert("goblin_01".to_string(), entity);

    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::ModifyStat { key: "goblin_01.health".to_string(), delta: -25.0 });
    app.update();

    let sm = app.world().get::<StatMap>(entity).unwrap();
    assert_eq!(sm.0["health"].current, 75.0,
        "ModifyStat with dot key must mutate the entity's StatMap, not LoadedStats");
}

#[test]
fn test_modify_stat_without_dot_key_routes_to_loaded_stats() {
    let mut app = setup_test_app();
    app.update();

    let mut loaded = LoadedStats::default();
    loaded.0.insert("player_health".to_string(), LiveStat::new(make_stat_def(100.0, 100.0)));
    app.world_mut().insert_resource(loaded);

    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::ModifyStat { key: "player_health".to_string(), delta: -30.0 });
    app.update();

    let loaded = app.world().resource::<LoadedStats>();
    assert_eq!(loaded.0["player_health"].current, 70.0,
        "ModifyStat without dot key must mutate LoadedStats, not any entity StatMap");
}

#[test]
fn test_stat_map_threshold_crossing_emits_game_event() {
    let mut app = setup_test_app();
    app.update();

    let def = StatDef {
        base: 50.0, min: 0.0, max: 50.0, soft_max: None,
        regen_rate: 0.0, regen_delay: 0.0,
        thresholds: vec![
            StatThreshold {
                when: ThresholdCondition::BelowOrEqual(0.0),
                emit: "stat.enemy_01.health.depleted".to_string(),
            },
        ],
    };
    let mut stat_map = StatMap::default();
    stat_map.0.insert("health".to_string(), LiveStat::new(def));

    let entity = app.world_mut().spawn((
        SpawnId("enemy_01".to_string()),
        stat_map,
    )).id();
    app.world_mut()
        .resource_mut::<SpawnRegistry>()
        .entities
        .insert("enemy_01".to_string(), entity);

    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::ModifyStat { key: "enemy_01.health".to_string(), delta: -50.0 });
    app.update();

    app.world_mut().run_system_once(|mut events: MessageReader<GameEvent>| {
        let names: Vec<String> = events.read()
            .map(|e| { let GameEvent::Trigger(n) = e; n.clone() })
            .collect();
        assert!(
            names.contains(&"stat.enemy_01.health.depleted".to_string()),
            "stat_threshold_system must emit the configured event on false→true crossing; got: {:?}", names
        );
    }).unwrap();
}

#[test]
fn test_despawn_action_removes_entity_and_stat_map() {
    let mut app = setup_test_app();
    app.update();

    let mut stat_map = StatMap::default();
    stat_map.0.insert("health".to_string(), LiveStat::new(make_stat_def(40.0, 100.0)));

    let entity = app.world_mut().spawn((
        SpawnId("dying_01".to_string()),
        stat_map,
    )).id();
    app.world_mut()
        .resource_mut::<SpawnRegistry>()
        .entities
        .insert("dying_01".to_string(), entity);

    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::Despawn("dying_01".to_string()));
    app.update();
    app.update();

    assert!(
        app.world().get_entity(entity).is_err(),
        "Despawned entity must no longer exist — StatMap is removed with the entity"
    );
}

#[test]
fn test_stat_radar_scene_load_spawns_node_with_correct_stat_keys() {
    let mut app = setup_test_app();
    app.update();

    let config_handle = app
        .world_mut()
        .resource_mut::<Assets<ProjectConfig>>()
        .add(ProjectConfig {
            schema_version: 1,
            initial_scene: "scenes/t.ron".to_string(),
            ..Default::default()
        });
    app.world_mut().insert_resource(ProjectConfigHandle(config_handle));

    let scene: GameSceneV2 = ron::de::from_str(r#"
        (
            schema_version: 2,
            entities: [],
            ui: [
                StatRadar((
                    id: "test_radar",
                    stats: ["player_health", "player_mana", "player_stamina"],
                )),
            ],
        )
    "#).expect("test scene RON must parse");

    let scene_handle = app
        .world_mut()
        .resource_mut::<Assets<GameSceneV2>>()
        .add(scene);
    app.world_mut().insert_resource(SceneHandleV2(scene_handle));

    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::LoadingScene);
    app.update();
    app.update();
    app.update();

    let mut found = false;
    let world = app.world_mut();
    let mut q = world.query::<&StatRadarNode>();
    for node in q.iter(&world) {
        if node.stat_keys == vec!["player_health", "player_mana", "player_stamina"] {
            found = true;
        }
    }
    assert!(found, "scene loader must spawn an entity with StatRadarNode carrying the RON-defined stat keys");
}

// ── Modifier computation tests (pure logic, no App) ───────────────────────────

#[test]
fn test_additive_modifier_raises_effective_value() {
    let def = make_stat_def(50.0, 100.0);
    let mut stat = LiveStat::new(def);
    let mut modifier_defs = HashMap::new();
    modifier_defs.insert("flat_boost".to_string(), make_additive_modifier("health", 20.0, StackRule::Add));

    stat.active_modifiers.push(ActiveModifier { key: "flat_boost".to_string(), remaining_secs: None });
    let eff = stat.compute_effective(&modifier_defs);
    assert_eq!(eff, 70.0, "additive +20 on current=50 should give effective=70");
}

#[test]
fn test_additive_modifiers_stack_with_add_rule() {
    let def = make_stat_def(50.0, 100.0);
    let mut stat = LiveStat::new(def);
    let mut modifier_defs = HashMap::new();
    modifier_defs.insert("flat_boost".to_string(), make_additive_modifier("health", 10.0, StackRule::Add));

    stat.active_modifiers.push(ActiveModifier { key: "flat_boost".to_string(), remaining_secs: None });
    stat.active_modifiers.push(ActiveModifier { key: "flat_boost".to_string(), remaining_secs: None });
    let eff = stat.compute_effective(&modifier_defs);
    assert_eq!(eff, 70.0, "two Add-rule +10 modifiers should accumulate to +20");
}

#[test]
fn test_max_stack_rule_ignores_weaker_instance() {
    let def = make_stat_def(40.0, 100.0);
    let mut stat = LiveStat::new(def);
    let mut modifier_defs = HashMap::new();
    modifier_defs.insert("poison".to_string(), ModifierDef {
        stat: "health".to_string(),
        kind: ModifierKind::Additive(-5.0),
        duration_secs: None,
        stack_rule: StackRule::Max,
    });

    stat.active_modifiers.push(ActiveModifier { key: "poison".to_string(), remaining_secs: None });
    stat.active_modifiers.push(ActiveModifier { key: "poison".to_string(), remaining_secs: None });
    let eff = stat.compute_effective(&modifier_defs);
    assert_eq!(eff, 35.0, "Max rule: two instances of -5 should still only apply -5 once (not -10)");
}

#[test]
fn test_multiplicative_modifier_scales_current() {
    let def = make_stat_def(10.0, 20.0);
    let mut stat = LiveStat::new(def);
    let mut modifier_defs = HashMap::new();
    modifier_defs.insert("speed_boost".to_string(), make_multiplicative_modifier("speed", 1.5));

    stat.active_modifiers.push(ActiveModifier { key: "speed_boost".to_string(), remaining_secs: None });
    let eff = stat.compute_effective(&modifier_defs);
    assert_eq!(eff, 15.0, "multiplicative 1.5× on current=10 should give effective=15");
}

#[test]
fn test_soft_max_allows_overheal() {
    let mut def = make_stat_def(100.0, 100.0);
    def.soft_max = Some(125.0);
    let mut stat = LiveStat::new(def);
    let mut modifier_defs = HashMap::new();
    modifier_defs.insert("overheal".to_string(), make_additive_modifier("health", 25.0, StackRule::Add));

    stat.active_modifiers.push(ActiveModifier { key: "overheal".to_string(), remaining_secs: None });
    let eff = stat.compute_effective(&modifier_defs);
    assert_eq!(eff, 125.0, "additive +25 with soft_max=125 should reach 125");
}

#[test]
fn test_soft_max_caps_overheal() {
    let mut def = make_stat_def(100.0, 100.0);
    def.soft_max = Some(125.0);
    let mut stat = LiveStat::new(def);
    let mut modifier_defs = HashMap::new();
    modifier_defs.insert("big_overheal".to_string(), make_additive_modifier("health", 999.0, StackRule::Add));

    stat.active_modifiers.push(ActiveModifier { key: "big_overheal".to_string(), remaining_secs: None });
    let eff = stat.compute_effective(&modifier_defs);
    assert_eq!(eff, 125.0, "effective value must be clamped to soft_max");
}

#[test]
fn test_no_modifiers_effective_equals_current() {
    let def = make_stat_def(75.0, 100.0);
    let stat = LiveStat::new(def);
    let modifier_defs = HashMap::new();
    let eff = stat.compute_effective(&modifier_defs);
    assert_eq!(eff, 75.0, "with no active modifiers effective must equal current");
}

// ── ApplyModifier / RemoveModifier action tests ───────────────────────────────

#[test]
fn test_apply_modifier_action_adds_to_loaded_stats() {
    let mut app = setup_test_app();
    app.update();

    let mut loaded_stats = LoadedStats::default();
    loaded_stats.0.insert("speed".to_string(), LiveStat::new(make_stat_def(10.0, 20.0)));
    app.world_mut().insert_resource(loaded_stats);

    let mut modifier_defs = HashMap::new();
    modifier_defs.insert("speed_boost".to_string(), make_multiplicative_modifier("speed", 1.5));
    app.world_mut().insert_resource(LoadedModifiers(modifier_defs));

    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::ApplyModifier { modifier_key: "speed_boost".to_string() });
    app.update();

    let stats = app.world().resource::<LoadedStats>();
    assert_eq!(stats.0["speed"].active_modifiers.len(), 1,
        "ApplyModifier must push one ActiveModifier onto the stat");
    assert_eq!(stats.0["speed"].active_modifiers[0].key, "speed_boost");
}

#[test]
fn test_remove_modifier_action_clears_active_modifier() {
    let mut app = setup_test_app();
    app.update();

    let mut loaded_stats = LoadedStats::default();
    let mut stat = LiveStat::new(make_stat_def(10.0, 20.0));
    stat.active_modifiers.push(ActiveModifier { key: "speed_boost".to_string(), remaining_secs: None });
    loaded_stats.0.insert("speed".to_string(), stat);
    app.world_mut().insert_resource(loaded_stats);

    let mut modifier_defs = HashMap::new();
    modifier_defs.insert("speed_boost".to_string(), make_multiplicative_modifier("speed", 1.5));
    app.world_mut().insert_resource(LoadedModifiers(modifier_defs));

    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::RemoveModifier { modifier_key: "speed_boost".to_string() });
    app.update();

    let stats = app.world().resource::<LoadedStats>();
    assert!(stats.0["speed"].active_modifiers.is_empty(),
        "RemoveModifier must remove all instances of the modifier from the stat");
}

#[test]
fn test_threshold_uses_effective_value_not_current() {
    let mut def = make_stat_def(80.0, 100.0);
    def.thresholds = vec![StatThreshold {
        when: ThresholdCondition::BelowPercent(0.25),
        emit: "stat.health.low".to_string(),
    }];
    let mut stat = LiveStat::new(def);
    let mut modifier_defs = HashMap::new();
    modifier_defs.insert("heavy_curse".to_string(), make_additive_modifier("health", -65.0, StackRule::Add));

    stat.active_modifiers.push(ActiveModifier { key: "heavy_curse".to_string(), remaining_secs: None });
    let eff = stat.compute_effective(&modifier_defs);
    assert!(eff < 25.0, "effective should be below 25 after debuff: got {}", eff);
    assert!(stat.current >= 25.0);
    let is_met = ThresholdCondition::BelowPercent(0.25).is_met(eff, stat.def.max);
    assert!(is_met, "threshold must be met based on effective value");
    let raw_is_met = ThresholdCondition::BelowPercent(0.25).is_met(stat.current, stat.def.max);
    assert!(!raw_is_met, "threshold must NOT be met based on raw current");
}

// ── resolve_stat routing tests ─────────────────────────────────────────────────

#[test]
fn test_resolve_stat_routes_entity_local_key_through_stat_map() {
    let mut app = setup_test_app();
    app.update();

    let mut loaded_stats = LoadedStats::default();
    loaded_stats.0.insert("dummy_01.health".to_string(), LiveStat::new(make_stat_def(999.0, 999.0)));
    app.world_mut().insert_resource(loaded_stats);

    let mut stat_map = StatMap(indexmap::IndexMap::new());
    stat_map.0.insert("health".to_string(), LiveStat::new(make_stat_def(75.0, 100.0)));
    app.world_mut().spawn((SpawnId("dummy_01".to_string()), stat_map));

    let result: Arc<Mutex<Option<Option<(f32, f32, f32)>>>> = Arc::new(Mutex::new(None));
    let result_clone = result.clone();

    let _ = app.world_mut().run_system_once(move |
        loaded_stats: Res<LoadedStats>,
        stat_map_query: Query<(&SpawnId, &StatMap)>,
    | {
        let val = resolve_stat("dummy_01.health", &loaded_stats, &stat_map_query);
        *result_clone.lock().unwrap() = Some(val);
    });

    let val = result.lock().unwrap().unwrap();
    assert!(val.is_some(), "resolve_stat must find 'dummy_01.health' in entity StatMap");
    let (effective, min, max) = val.unwrap();
    assert_eq!(effective, 75.0, "effective must come from StatMap, not the global LoadedStats sentinel");
    assert_eq!(min, 0.0);
    assert_eq!(max, 100.0);
}

#[test]
fn test_resolve_stat_routes_global_key_through_loaded_stats() {
    let mut app = setup_test_app();
    app.update();

    let mut loaded_stats = LoadedStats::default();
    loaded_stats.0.insert("player_health".to_string(), LiveStat::new(make_stat_def(60.0, 100.0)));
    app.world_mut().insert_resource(loaded_stats);

    let result: Arc<Mutex<Option<Option<(f32, f32, f32)>>>> = Arc::new(Mutex::new(None));
    let result_clone = result.clone();

    let _ = app.world_mut().run_system_once(move |
        loaded_stats: Res<LoadedStats>,
        stat_map_query: Query<(&SpawnId, &StatMap)>,
    | {
        let val = resolve_stat("player_health", &loaded_stats, &stat_map_query);
        *result_clone.lock().unwrap() = Some(val);
    });

    let val = result.lock().unwrap().unwrap();
    assert!(val.is_some(), "resolve_stat must find 'player_health' in LoadedStats");
    let (effective, _, max) = val.unwrap();
    assert_eq!(effective, 60.0);
    assert_eq!(max, 100.0);
}

#[test]
fn test_resolve_stat_returns_none_for_missing_entity_key() {
    let mut app = setup_test_app();
    app.update();

    let result: Arc<Mutex<Option<Option<(f32, f32, f32)>>>> = Arc::new(Mutex::new(None));
    let result_clone = result.clone();

    let _ = app.world_mut().run_system_once(move |
        loaded_stats: Res<LoadedStats>,
        stat_map_query: Query<(&SpawnId, &StatMap)>,
    | {
        let val = resolve_stat("ghost_entity.health", &loaded_stats, &stat_map_query);
        *result_clone.lock().unwrap() = Some(val);
    });

    assert!(
        result.lock().unwrap().unwrap().is_none(),
        "resolve_stat must return None when entity does not exist"
    );
}
