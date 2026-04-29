use ironhold_core::schema::{ProjectConfig, StateMachineAsset};
use ironhold_core::schema::scene_v2::GameSceneV2;
use ironhold_core::schema::catalog::{AssetCatalog, PrefabCatalog, MovementConfig, JumpConfig};
use ironhold_core::schema::project::LogicRulesAsset;
use ron::de::from_str;

// ProjectConfig tests
#[test]
fn test_project_config_deserialization() {
    let ron_str = r#"
        (
            schema_version: 1,
            initial_scene: "scenes/main.ron",
            rules: []
        )
    "#;
    let config: ProjectConfig = from_str(ron_str).expect("Failed to deserialize ProjectConfig");
    assert_eq!(config.schema_version, 1);
    assert_eq!(config.initial_scene, "scenes/main.ron");
}

#[test]
fn test_project_config_v2_deserialization() {
    let ron_str = r#"
        (
            schema_version: 2,
            initial_scene: "scenes/main.ron",
            project_id: Some("my_project"),
            display_name: Some("My Project"),
            asset_catalog: Some("assets.ron"),
            prefab_catalog: Some("prefabs/prefabs.ron"),
            rules_path: Some("logic/rules.ron"),
            model_fixes_path: Some("overrides/model_fixes.ron"),
        )
    "#;
    let config: ProjectConfig = from_str(ron_str).expect("Failed to deserialize v2 ProjectConfig");
    assert_eq!(config.schema_version, 2);
    assert_eq!(config.project_id.as_deref(), Some("my_project"));
    assert!(config.validate().is_ok());
}

#[test]
fn test_project_config_missing_schema_version_is_error() {
    let ron_str = r#"
        (
            initial_scene: "scenes/main.ron",
            rules: []
        )
    "#;
    let result: Result<ProjectConfig, _> = ron::de::from_str(ron_str);
    assert!(result.is_err(), "schema_version must be present");
}

#[test]
fn test_invalid_project_config() {
    let ron_str = r#"
        (
            missing_field: "oops"
        )
    "#;
    let result: Result<ProjectConfig, _> = from_str(ron_str);
    assert!(result.is_err(), "Should have failed due to missing initial_scene");
}

#[test]
fn test_project_config_v3_deserialization() {
    let ron_str = r#"
        (
            schema_version: 3,
            initial_scene: "scenes/main.ron",
            project_id: Some("fsm_project"),
            state_machine_path: Some("logic/state_machine.ron"),
        )
    "#;
    let config: ProjectConfig = from_str(ron_str).expect("Failed to deserialize v3 ProjectConfig");
    assert_eq!(config.schema_version, 3);
    assert_eq!(config.state_machine_path.as_deref(), Some("logic/state_machine.ron"));
    assert!(config.rules_path.is_none());
    assert!(config.validate().is_ok());
}

#[test]
fn test_project_config_wrong_schema_version_is_invalid() {
    let ron_str = r#"
        (
            schema_version: 999,
            initial_scene: "scenes/main.ron",
            rules: []
        )
    "#;
    let config: ProjectConfig = ron::de::from_str(ron_str).unwrap();
    assert!(config.validate().is_err());
}

#[test]
fn test_project_config_unknown_field_is_error() {
    let ron_str = r#"
        (
            schema_version: 1,
            initial_scene: "scenes/main.ron",
            rules: [],
            typo_field: 123
        )
    "#;
    let result: Result<ProjectConfig, _> = ron::de::from_str(ron_str);
    assert!(result.is_err(), "unknown fields should be rejected");
}

// StateMachineAsset tests

#[test]
fn test_state_machine_asset_deserialization() {
    // Covers: explicit from: Some(...), omitted from (any-state), global_on, in-state on.
    let ron_str = r#"
        (
            schema_version: 1,
            initial_state: "menu",
            global_on: [
                ( event: "ui.button_pressed:debug", do_actions: [ Log("debug") ] ),
            ],
            states: [
                (
                    name: "menu",
                    entry_actions: [],
                    exit_actions: [],
                    on: [
                        ( event: "ui.button_pressed:start", do_actions: [ LoadScene("scenes/main.scene.ron") ] ),
                    ],
                ),
                (
                    name: "playing",
                    entry_actions: [ PlayMusicLoop("bg") ],
                    exit_actions:  [ StopMusic ],
                    on: [],
                ),
            ],
            transitions: [
                // Omitted `from` = any state.
                ( on: "scene.ready:main", to: "playing" ),
                // Explicit from: Some(...).
                ( from: Some("playing"), on: "ui.button_pressed:quit", to: "menu" ),
            ],
        )
    "#;
    let fsm: StateMachineAsset = from_str(ron_str).expect("StateMachineAsset failed to parse");
    assert_eq!(fsm.schema_version, 1);
    assert_eq!(fsm.initial_state, "menu");
    assert_eq!(fsm.states.len(), 2);
    assert_eq!(fsm.transitions.len(), 2);
    assert_eq!(fsm.global_on.len(), 1);
    assert!(fsm.transitions[0].from.is_none(), "omitted from must deserialise as None");
    assert_eq!(fsm.transitions[1].from.as_deref(), Some("playing"));
}

#[test]
fn test_state_machine_asset_bare_string_from_is_error() {
    // Regression: `from: "playing"` (no Some wrapper) must fail — it burned us once.
    let ron_str = r#"
        (
            schema_version: 1,
            initial_state: "a",
            states: [],
            transitions: [
                ( from: "playing", on: "ui.button_pressed:x", to: "b" ),
            ],
        )
    "#;
    let result: Result<StateMachineAsset, _> = from_str(ron_str);
    assert!(result.is_err(), "bare string for Option<String> must be rejected by RON");
}

// ── StateMachineAsset validation ─────────────────────────────────────────────

#[test]
fn test_state_machine_validates_ok() {
    let ron_str = r#"
        (
            schema_version: 1,
            initial_state: "menu",
            states: [
                ( name: "menu", entry_actions: [], exit_actions: [], on: [] ),
                ( name: "playing", entry_actions: [], exit_actions: [], on: [] ),
            ],
            transitions: [
                ( on: "scene.ready:main", to: "playing" ),
                ( from: Some("playing"), on: "ui.button_pressed:quit", to: "menu" ),
            ],
        )
    "#;
    let fsm: StateMachineAsset = from_str(ron_str).unwrap();
    assert!(fsm.validate().is_ok());
}

#[test]
fn test_state_machine_wrong_version_is_invalid() {
    let ron_str = r#"
        (
            schema_version: 99,
            initial_state: "menu",
            states: [],
            transitions: [],
        )
    "#;
    let fsm: StateMachineAsset = from_str(ron_str).unwrap();
    assert!(fsm.validate().is_err());
}

#[test]
fn test_state_machine_unknown_initial_state_is_invalid() {
    let ron_str = r#"
        (
            schema_version: 1,
            initial_state: "nonexistent",
            states: [
                ( name: "menu", entry_actions: [], exit_actions: [], on: [] ),
            ],
            transitions: [],
        )
    "#;
    let fsm: StateMachineAsset = from_str(ron_str).unwrap();
    assert!(fsm.validate().is_err());
}

#[test]
fn test_state_machine_transition_to_unknown_state_is_invalid() {
    let ron_str = r#"
        (
            schema_version: 1,
            initial_state: "menu",
            states: [
                ( name: "menu", entry_actions: [], exit_actions: [], on: [] ),
            ],
            transitions: [
                ( on: "some_event", to: "ghost_state" ),
            ],
        )
    "#;
    let fsm: StateMachineAsset = from_str(ron_str).unwrap();
    assert!(fsm.validate().is_err());
}

#[test]
fn test_state_machine_duplicate_state_names_is_invalid() {
    let ron_str = r#"
        (
            schema_version: 1,
            initial_state: "menu",
            states: [
                ( name: "menu", entry_actions: [], exit_actions: [], on: [] ),
                ( name: "menu", entry_actions: [], exit_actions: [], on: [] ),
            ],
            transitions: [],
        )
    "#;
    let fsm: StateMachineAsset = from_str(ron_str).unwrap();
    assert!(fsm.validate().is_err());
}

// ── GameSceneV2 validation ────────────────────────────────────────────────────

#[test]
fn test_game_scene_v2_validates_ok() {
    let ron_str = r#"
        (
            schema_version: 2,
            entities: [
                ( id: "player", prefab: "player_warrior", transform: () ),
            ],
            ui: [
                ( kind: "button", id: "quit_btn", text: "Quit", action: "quit", size: (120.0, 40.0) ),
            ],
        )
    "#;
    let scene: GameSceneV2 = from_str(ron_str).unwrap();
    assert!(scene.validate().is_ok());
}

#[test]
fn test_game_scene_v2_wrong_version_is_invalid() {
    let ron_str = r#"
        (
            schema_version: 1,
            entities: [],
            ui: [],
        )
    "#;
    let scene: GameSceneV2 = from_str(ron_str).unwrap();
    assert!(scene.validate().is_err());
}

#[test]
fn test_game_scene_v2_duplicate_entity_ids_is_invalid() {
    let ron_str = r#"
        (
            schema_version: 2,
            entities: [
                ( id: "player", prefab: "hero", transform: () ),
                ( id: "player", prefab: "hero", transform: () ),
            ],
            ui: [],
        )
    "#;
    let scene: GameSceneV2 = from_str(ron_str).unwrap();
    assert!(scene.validate().is_err());
}

#[test]
fn test_game_scene_v2_duplicate_ui_ids_is_invalid() {
    let ron_str = r#"
        (
            schema_version: 2,
            entities: [],
            ui: [
                ( kind: "button", id: "btn", text: "A", size: (100.0, 40.0) ),
                ( kind: "button", id: "btn", text: "B", size: (100.0, 40.0) ),
            ],
        )
    "#;
    let scene: GameSceneV2 = from_str(ron_str).unwrap();
    assert!(scene.validate().is_err());
}

#[test]
fn test_game_scene_v2_unknown_ui_kind_is_invalid() {
    let ron_str = r#"
        (
            schema_version: 2,
            entities: [],
            ui: [
                ( kind: "checkbox", id: "opt", text: "Enable", size: (100.0, 40.0) ),
            ],
        )
    "#;
    let scene: GameSceneV2 = from_str(ron_str).unwrap();
    assert!(scene.validate().is_err());
}

#[test]
fn test_game_scene_v2_unknown_field_is_error() {
    let ron_str = r#"
        (
            schema_version: 2,
            entities: [],
            ui: [],
            typo_field: 123,
        )
    "#;
    let result: Result<GameSceneV2, _> = from_str(ron_str);
    assert!(result.is_err(), "unknown fields should be rejected");
}

#[test]
fn test_game_scene_v2_tonemapping_defaults_to_aces_fitted() {
    let ron_str = r#"(schema_version: 2, entities: [], ui: [])"#;
    let scene: GameSceneV2 = from_str(ron_str).unwrap();
    assert_eq!(
        scene.tonemapping,
        ironhold_core::schema::scene_v2::TonemappingOption::AcesFitted,
        "omitting tonemapping should default to AcesFitted",
    );
}

#[test]
fn test_game_scene_v2_excluded_tonemapping_variants_are_rejected() {
    // TonyMcMapface and BlenderFilmic require a LUT and are intentionally excluded.
    for variant in &["TonyMcMapface", "BlenderFilmic"] {
        let ron_str = format!(r#"(schema_version: 2, entities: [], ui: [], tonemapping: {})"#, variant);
        let result: Result<GameSceneV2, _> = from_str(&ron_str);
        assert!(result.is_err(), "{} should be rejected as an unsupported tonemapping option", variant);
    }
}

#[test]
fn test_game_scene_v2_label_depth_scale_full() {
    let ron_str = r#"
        (
            schema_version: 2,
            entities: [],
            ui: [],
            label_depth_scale: Some((
                reference_distance: 80.0,
                min_scale: Some(0.25),
            )),
        )
    "#;
    let scene: GameSceneV2 = from_str(ron_str).expect("label_depth_scale should parse");
    let cfg = scene.label_depth_scale.expect("label_depth_scale should be Some");
    assert_eq!(cfg.reference_distance, 80.0);
    assert_eq!(cfg.min_scale, Some(0.25));
}

#[test]
fn test_game_scene_v2_label_depth_scale_defaults() {
    // Only reference_distance is required; min_scale defaults to None.
    let ron_str = r#"
        (
            schema_version: 2,
            entities: [],
            ui: [],
            label_depth_scale: Some(()),
        )
    "#;
    let scene: GameSceneV2 = from_str(ron_str).expect("label_depth_scale with defaults should parse");
    let cfg = scene.label_depth_scale.expect("label_depth_scale should be Some");
    assert_eq!(cfg.reference_distance, 50.0, "reference_distance should default to 50.0");
    assert_eq!(cfg.min_scale, None, "min_scale should default to None");
}

#[test]
fn test_game_scene_v2_label_depth_scale_omitted() {
    // Existing scenes without the field must still deserialize cleanly.
    let ron_str = r#"(schema_version: 2, entities: [], ui: [])"#;
    let scene: GameSceneV2 = from_str(ron_str).expect("scene without label_depth_scale should parse");
    assert!(scene.label_depth_scale.is_none());
}

#[test]
fn test_game_scene_v2_entity_label_depth_scale_override() {
    let ron_str = r#"
        (
            schema_version: 2,
            entities: [
                (
                    id: "obj",
                    prefab: "some_prefab",
                    transform: (),
                    label: Some((
                        text: "Header",
                        depth_scale: Some(false),
                    )),
                ),
            ],
            ui: [],
        )
    "#;
    let scene: GameSceneV2 = from_str(ron_str).expect("entity label with depth_scale override should parse");
    let label = scene.entities[0].label.as_ref().expect("label should be Some");
    assert_eq!(label.depth_scale, Some(false));
}

#[test]
fn test_game_scene_v2_directional_light_cascade_options() {
    let ron_str = r#"
        (
            schema_version: 2,
            entities: [],
            ui: [],
            lighting: Some((
                directional: Some((
                    color: (1.0, 1.0, 1.0),
                    intensity: 10000.0,
                    rotation_euler_deg: (-45.0, 0.0, 0.0),
                    shadow_distance: Some(400.0),
                    cascade_overlap: Some(0.5),
                )),
            )),
        )
    "#;
    let scene: GameSceneV2 = from_str(ron_str).expect("directional light with cascade options should parse");
    let dl = scene.lighting.unwrap().directional.unwrap();
    assert_eq!(dl.shadow_distance, Some(400.0));
    assert_eq!(dl.cascade_overlap, Some(0.5));
}

#[test]
fn test_game_scene_v2_shadow_map_sizes_explicit() {
    let ron_str = r#"
        (
            schema_version: 2,
            entities: [],
            ui: [],
            lighting: Some((
                shadow_map_size: Some(1024),
                point_shadow_map_size: Some(512),
            )),
        )
    "#;
    let scene: GameSceneV2 = from_str(ron_str).expect("shadow_map_size and point_shadow_map_size should parse");
    let lighting = scene.lighting.unwrap();
    assert_eq!(lighting.shadow_map_size, Some(1024));
    assert_eq!(lighting.point_shadow_map_size, Some(512));
}

#[test]
fn test_game_scene_v2_shadow_map_sizes_default_to_none() {
    let ron_str = r#"(schema_version: 2, entities: [], ui: [], lighting: Some(()))"#;
    let scene: GameSceneV2 = from_str(ron_str).expect("lighting with all defaults should parse");
    let lighting = scene.lighting.unwrap();
    assert_eq!(lighting.shadow_map_size, None, "shadow_map_size should default to None");
    assert_eq!(lighting.point_shadow_map_size, None, "point_shadow_map_size should default to None");
}

#[test]
fn test_game_scene_v2_directional_light_num_cascades_explicit() {
    let ron_str = r#"
        (
            schema_version: 2,
            entities: [],
            ui: [],
            lighting: Some((
                directional: Some((
                    color: (1.0, 1.0, 1.0),
                    intensity: 5000.0,
                    rotation_euler_deg: (-45.0, 0.0, 0.0),
                    num_cascades: Some(2),
                )),
            )),
        )
    "#;
    let scene: GameSceneV2 = from_str(ron_str).expect("num_cascades should parse");
    let dl = scene.lighting.unwrap().directional.unwrap();
    assert_eq!(dl.num_cascades, Some(2));
}

#[test]
fn test_game_scene_v2_directional_light_num_cascades_defaults_to_none() {
    let ron_str = r#"
        (
            schema_version: 2,
            entities: [],
            ui: [],
            lighting: Some((
                directional: Some((
                    color: (1.0, 1.0, 1.0),
                    intensity: 5000.0,
                    rotation_euler_deg: (-45.0, 0.0, 0.0),
                )),
            )),
        )
    "#;
    let scene: GameSceneV2 = from_str(ron_str).expect("directional light without num_cascades should parse");
    let dl = scene.lighting.unwrap().directional.unwrap();
    assert_eq!(dl.num_cascades, None, "num_cascades should default to None");
}

// ── AssetCatalog validation ───────────────────────────────────────────────────

#[test]
fn test_asset_catalog_validates_ok() {
    let ron_str = r#"
        (
            schema_version: 1,
            models: {
                "hero": ( path: "models/hero.glb#Scene0" ),
            },
        )
    "#;
    let catalog: AssetCatalog = from_str(ron_str).unwrap();
    assert!(catalog.validate().is_ok());
}

#[test]
fn test_asset_catalog_wrong_version_is_invalid() {
    let ron_str = r#"
        (
            schema_version: 99,
            models: {},
        )
    "#;
    let catalog: AssetCatalog = from_str(ron_str).unwrap();
    assert!(catalog.validate().is_err());
}

#[test]
fn test_asset_catalog_empty_model_path_is_invalid() {
    let ron_str = r#"
        (
            schema_version: 1,
            models: {
                "hero": ( path: "" ),
            },
        )
    "#;
    let catalog: AssetCatalog = from_str(ron_str).unwrap();
    assert!(catalog.validate().is_err());
}

#[test]
fn test_asset_catalog_missing_schema_version_is_error() {
    let ron_str = r#"
        (
            models: {},
        )
    "#;
    let result: Result<AssetCatalog, _> = from_str(ron_str);
    assert!(result.is_err(), "schema_version must be present");
}

// ── PrefabCatalog validation ──────────────────────────────────────────────────

#[test]
fn test_prefab_catalog_validates_ok() {
    let ron_str = r#"
        (
            schema_version: 1,
            prefabs: {
                "hero": ( kind: "actor", model: "hero", components: () ),
                "crate": ( kind: "prop", model: "crate", components: () ),
                "cube": ( kind: "primitive", model: "Cuboid", components: () ),
            },
        )
    "#;
    let catalog: PrefabCatalog = from_str(ron_str).unwrap();
    assert!(catalog.validate().is_ok());
}

#[test]
fn test_prefab_catalog_wrong_version_is_invalid() {
    let ron_str = r#"
        (
            schema_version: 99,
            prefabs: {},
        )
    "#;
    let catalog: PrefabCatalog = from_str(ron_str).unwrap();
    assert!(catalog.validate().is_err());
}

#[test]
fn test_prefab_catalog_unknown_kind_is_invalid() {
    let ron_str = r#"
        (
            schema_version: 1,
            prefabs: {
                "hero": ( kind: "npc", model: "hero", components: () ),
            },
        )
    "#;
    let catalog: PrefabCatalog = from_str(ron_str).unwrap();
    assert!(catalog.validate().is_err());
}

#[test]
fn test_prefab_catalog_missing_schema_version_is_error() {
    let ron_str = r#"
        (
            prefabs: {},
        )
    "#;
    let result: Result<PrefabCatalog, _> = from_str(ron_str);
    assert!(result.is_err(), "schema_version must be present");
}

// ── LogicRulesAsset validation ────────────────────────────────────────────────

#[test]
fn test_logic_rules_asset_validates_ok() {
    let ron_str = r#"
        (
            schema_version: 2,
            rules: [
                ( on: "ui.button_pressed:start", do_actions: [ Quit ] ),
            ],
        )
    "#;
    let rules: LogicRulesAsset = from_str(ron_str).unwrap();
    assert!(rules.validate().is_ok());
}

#[test]
fn test_logic_rules_asset_wrong_version_is_invalid() {
    let ron_str = r#"
        (
            schema_version: 99,
            rules: [],
        )
    "#;
    let rules: LogicRulesAsset = from_str(ron_str).unwrap();
    assert!(rules.validate().is_err());
}

#[test]
fn test_logic_rules_asset_missing_schema_version_is_error() {
    let ron_str = r#"
        (
            rules: [],
        )
    "#;
    let result: Result<LogicRulesAsset, _> = from_str(ron_str);
    assert!(result.is_err(), "schema_version must be present");
}

// ── MovementConfig / JumpConfig deserialization ───────────────────────────────

#[test]
fn test_movement_config_all_defaults() {
    let config: MovementConfig = from_str("()").unwrap();
    assert_eq!(config.walk_speed, MovementConfig::default().walk_speed);
    assert_eq!(config.run_speed, MovementConfig::default().run_speed);
    assert!(config.jump.is_none());
    assert!(!config.double_jump);
}

#[test]
fn test_movement_config_speeds() {
    let config: MovementConfig = from_str("(walk_speed: 5.5, run_speed: 10.0)").unwrap();
    assert_eq!(config.walk_speed, 5.5);
    assert_eq!(config.run_speed, 10.0);
}

#[test]
fn test_jump_config_fixed() {
    // RON 0.11 requires explicit Some(...) for Option<T> — no implicit Some for enums.
    let config: MovementConfig = from_str("(jump: Some(Fixed(height: 2.5)))").unwrap();
    assert!(matches!(config.jump, Some(JumpConfig::Fixed { height }) if (height - 2.5).abs() < 0.001));
}

#[test]
fn test_jump_config_relative_to_height() {
    let config: MovementConfig = from_str("(jump: Some(RelativeToHeight(percent: 120.0)))").unwrap();
    assert!(matches!(config.jump, Some(JumpConfig::RelativeToHeight { percent }) if (percent - 120.0).abs() < 0.001));
}

#[test]
fn test_movement_config_double_jump() {
    let config: MovementConfig =
        from_str("(double_jump: true, double_jump_height: Some(Fixed(height: 3.0)))").unwrap();
    assert!(config.double_jump);
    assert!(matches!(config.double_jump_height, Some(JumpConfig::Fixed { height }) if (height - 3.0).abs() < 0.001));
}

#[test]
fn test_movement_config_unknown_field_is_error() {
    let result: Result<MovementConfig, _> = from_str("(wlak_speed: 3.5)");
    assert!(result.is_err(), "typos in MovementConfig should be rejected (deny_unknown_fields)");
}

#[test]
fn test_prefab_catalog_with_player_movement_parses() {
    let ron_str = r#"
        (
            schema_version: 1,
            prefabs: {
                "player": (
                    kind: "primitive",
                    model: "Capsule3d",
                    components: (
                        tags: ["player"],
                        movement: (
                            walk_speed: 5.5,
                            run_speed: 10.0,
                            jump: Some(Fixed(height: 2.5)),
                            double_jump: true,
                        ),
                    ),
                ),
            },
        )
    "#;
    let catalog: PrefabCatalog = from_str(ron_str).expect("PrefabCatalog with movement should parse");
    let player = &catalog.prefabs["player"];
    assert_eq!(player.components.movement.walk_speed, 5.5);
    assert!(player.components.movement.double_jump);
    assert!(matches!(player.components.movement.jump, Some(JumpConfig::Fixed { .. })));
}