use ironhold_core::schema::{ProjectConfig, GameLevel, StateMachineAsset};
use ironhold_core::schema::scene_v2::GameSceneV2;
use ironhold_core::schema::catalog::{AssetCatalog, PrefabCatalog};
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

// GameLevel tests
#[test]
fn test_game_level_minimal() {
    let ron_str = r#"
        (
            schema_version: 1,
            models: [],
            ui: [],
            player: None
        )
    "#;
    let level: GameLevel = from_str(ron_str).expect("Failed to deserialize minimal GameLevel");
    assert_eq!(level.schema_version, 1);
    assert_eq!(level.models.len(), 0);
    assert!(level.player.is_none());
}

#[test]
fn test_game_level_full() {
    let ron_str = r#"
        (
            schema_version: 1,
            models: [
                (
                    path: "models/cube.glb",
                    position: (0.0, 0.0, 0.0)
                )
            ],
            ui: [
                Button(
                    text: "Play",
                    action: Trigger("play")
                )
            ],
            player: Some((
                model_path: "models/player.glb",
                initial_position: (0.0, 1.0, 0.0),
                camera: (
                    offset: (0.0, 5.0, 10.0),
                    look_at_offset: (0.0, 1.0, 0.0),
                    zoom_speed: 10.0,
                    orbit_speed: 5.0,
                    min_radius: 2.0,
                    max_radius: 20.0
                ),
                inputs: (
                    forward: "W",
                    backward: "S",
                    left: "A",
                    right: "D",
                    strafe_left: "Q",
                    strafe_right: "E",
                    jump: "Space",
                    run: "ShiftLeft"
                ),
                animation_policy: "prefabs/animation/player_policy.ron"
            ))
        )
    "#;
    let level: GameLevel = from_str(ron_str).expect("Failed to deserialize full GameLevel");
    assert_eq!(level.models.len(), 1);
    assert!(level.player.is_some());
}

#[test]
fn test_game_level_missing_schema_version_is_error() {
    let ron_str = r#"
        (
            models: [],
            ui: [],
            player: None
        )
    "#;
    let result: Result<GameLevel, _> = ron::de::from_str(ron_str);
    assert!(result.is_err(), "schema_version must be present");
}

#[test]
fn test_game_level_wrong_schema_version_is_invalid() {
    let ron_str = r#"
        (
            schema_version: 999,
            models: [],
            ui: [],
            player: None
        )
    "#;
    let level: GameLevel = ron::de::from_str(ron_str).unwrap();
    assert!(level.validate().is_err());
}

#[test]
fn test_game_level_with_terrain() {
    let ron_str = r#"
        (
            schema_version: 1,
            terrain: Some((
                heightmap_path: "terrain/heightmap.png",
                splatmap_path: "terrain/splatmap.png",
                height_scale: 10.0,
                horizontal_scale: 1.0,
                position: (0.0, -10.0, 0.0),
                chunk_size: 64,
                material_paths: [
                    "terrain/dirt.png",
                    "terrain/grass.png",
                    "terrain/rock.png",
                    "terrain/snow.png",
                ],
            )),
        )
    "#;
    let level: GameLevel = from_str(ron_str).expect("Failed to deserialize GameLevel with terrain");
    let terrain = level.terrain.unwrap();
    assert_eq!(terrain.material_paths.len(), 4);
    assert_eq!(terrain.splatmap_path, "terrain/splatmap.png");
}

#[test]
fn test_game_level_with_lighting() {
    let ron_str = r#"
        (
            schema_version: 1,
            lighting: Some((
                ambient: Some((
                    color: (0.1, 0.2, 0.3),
                    brightness: 50.0,
                )),
                directional: Some((
                    color: (1.0, 1.0, 0.9),
                    illuminance: 12000.0,
                    direction: (1.0, -1.0, 0.0),
                )),
                environment: Some((
                    intensity: 1.0,
                    fallback: Some((
                        top_color: (0.7, 0.8, 1.0),
                        bottom_color: (0.1, 0.1, 0.1),
                    )),
                )),
            )),
        )
    "#;
    let level: GameLevel = from_str(ron_str).expect("Failed to deserialize GameLevel with lighting");
    let lighting = level.lighting.unwrap();
    assert!(lighting.ambient.is_some());
    assert!(lighting.directional.is_some());
    assert!(lighting.environment.is_some());
    assert_eq!(lighting.environment.unwrap().intensity, 1.0);
}

#[test]
fn test_game_level_unknown_field_is_error() {
    let ron_str = r#"
        (
            schema_version: 1,
            models: [],
            ui: [],
            player: None,
            typo_field: 123
        )
    "#;
    let result: Result<GameLevel, _> = ron::de::from_str(ron_str);
    assert!(result.is_err(), "unknown fields should be rejected");
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