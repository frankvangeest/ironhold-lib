use ironhold_core::schema::{ProjectConfig, StateMachineAsset, MaterialDef};
use ironhold_core::schema::scene_v2::GameSceneV2;
use ironhold_core::schema::catalog::{AssetCatalog, PrefabCatalog, MovementConfig, JumpConfig, NpcFaction, NpcOnPlayerNear, FlyCamDef};
use ironhold_core::schema::project::LogicRulesAsset;
use ron::extensions::Extensions;

/// Deserialize a RON string with `implicit_some` enabled — matches runtime loader behaviour.
fn from_str<'de, T: serde::Deserialize<'de>>(s: &'de str) -> Result<T, ron::error::SpannedError> {
    ron::Options::default()
        .with_default_extension(Extensions::IMPLICIT_SOME)
        .from_str(s)
}

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
fn test_state_machine_asset_bare_string_from_is_valid() {
    // With implicit_some enabled, `from: "playing"` is equivalent to `from: Some("playing")`.
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
    let sm = result.expect("implicit_some: bare string for Option<String> must be accepted");
    assert_eq!(sm.transitions[0].from, Some("playing".to_string()));
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

// ── Terrain uv_scale ──────────────────────────────────────────────────────────

#[test]
fn test_terrain_config_v2_uv_scale_defaults_to_ten() {
    let ron_str = r#"
        (
            schema_version: 2,
            entities: [],
            ui: [],
            terrain: Some((
                heightmap: "projects/terrain_demo/terrain/heightmap.png",
                splatmap: "shared/terrain/splatmap.png",
                scale: (0.5, 30.0, 0.5),
                material_paths: ["shared/terrain/grass.png"],
            )),
        )
    "#;
    let scene: GameSceneV2 = from_str(ron_str).expect("terrain block should parse");
    let terrain = scene.terrain.expect("terrain should be Some");
    assert_eq!(terrain.uv_scale, 10.0, "uv_scale should default to 10.0");
}

#[test]
fn test_terrain_config_v2_uv_scale_explicit() {
    let ron_str = r#"
        (
            schema_version: 2,
            entities: [],
            ui: [],
            terrain: Some((
                heightmap: "projects/terrain_demo/terrain/heightmap.png",
                splatmap: "shared/terrain/splatmap.png",
                scale: (0.5, 30.0, 0.5),
                material_paths: ["shared/terrain/grass.png"],
                uv_scale: 25.0,
            )),
        )
    "#;
    let scene: GameSceneV2 = from_str(ron_str).expect("terrain block with uv_scale should parse");
    let terrain = scene.terrain.expect("terrain should be Some");
    assert_eq!(terrain.uv_scale, 25.0, "uv_scale should be 25.0 as authored");
}

#[test]
fn test_terrain_material_def_uv_scale_defaults_to_ten() {
    let ron_str = r#"
        (
            kind: Terrain((
                splatmap: "shared/terrain/splatmap.png",
                layers: ["shared/terrain/grass.png"],
            )),
        )
    "#;
    let mat: MaterialDef = from_str(ron_str).expect("terrain MaterialDef should parse");
    if let ironhold_core::schema::MaterialKind::Terrain(terrain_def) = mat.kind {
        assert_eq!(terrain_def.uv_scale, 10.0, "uv_scale should default to 10.0");
    } else {
        panic!("expected MaterialKind::Terrain");
    }
}

#[test]
fn test_terrain_material_def_uv_scale_explicit() {
    let ron_str = r#"
        (
            kind: Terrain((
                splatmap: "shared/terrain/splatmap.png",
                layers: ["shared/terrain/grass.png"],
                uv_scale: 30.0,
            )),
        )
    "#;
    let mat: MaterialDef = from_str(ron_str).expect("terrain MaterialDef with uv_scale should parse");
    if let ironhold_core::schema::MaterialKind::Terrain(terrain_def) = mat.kind {
        assert_eq!(terrain_def.uv_scale, 30.0, "uv_scale should be 30.0 as authored");
    } else {
        panic!("expected MaterialKind::Terrain");
    }
}

#[test]
fn test_terrain_material_def_layers_empty_parses_ok() {
    // Designers may omit layers entirely — schema accepts it; runtime warns at load time.
    let ron_str = r#"
        (
            kind: Terrain((
                splatmap: "shared/terrain/splatmap.png",
            )),
        )
    "#;
    let mat: MaterialDef = from_str(ron_str).expect("terrain MaterialDef with no layers should parse");
    if let ironhold_core::schema::MaterialKind::Terrain(terrain_def) = mat.kind {
        assert!(terrain_def.layers.is_empty(), "layers should default to empty");
    } else {
        panic!("expected MaterialKind::Terrain");
    }
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

// ── UiTextAlign ──────────────────────────────────────────────────────────────

#[test]
fn test_ui_label_align_defaults_to_center() {
    let ron_str = r#"(schema_version: 2, entities: [], ui: [
        ( kind: "label", id: "lbl", text: "hi", size: (100.0, 30.0) )
    ])"#;
    let scene: GameSceneV2 = from_str(ron_str).expect("label without align should parse");
    assert_eq!(
        scene.ui[0].align,
        ironhold_core::schema::scene_v2::UiTextAlign::Center,
        "omitting align should default to Center",
    );
}

#[test]
fn test_ui_label_align_explicit_variants() {
    for (variant, expected) in &[
        ("Left",   ironhold_core::schema::scene_v2::UiTextAlign::Left),
        ("Center", ironhold_core::schema::scene_v2::UiTextAlign::Center),
        ("Right",  ironhold_core::schema::scene_v2::UiTextAlign::Right),
    ] {
        let ron_str = format!(
            r#"(schema_version: 2, entities: [], ui: [
                ( kind: "label", id: "lbl", text: "hi", size: (100.0, 30.0), align: {variant} )
            ])"#,
        );
        let scene: GameSceneV2 = from_str(&ron_str)
            .unwrap_or_else(|e| panic!("align: {variant} failed to parse: {e}"));
        assert_eq!(&scene.ui[0].align, expected, "align: {variant} should deserialize correctly");
    }
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
fn test_logic_rules_set_variable_and_increment_variable_parse() {
    let ron_str = r#"
        (
            schema_version: 2,
            rules: [
                ( on: "scene.ready:main", do_actions: [ SetVariable("level", "1") ] ),
                ( on: "entity.collected:coin_01", do_actions: [ IncrementVariable("score", 10) ] ),
                ( on: "npc.player_reached:goblin_01", do_actions: [ IncrementVariable("score", -5) ] ),
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
    assert!(config.rot_speed.is_none());
    assert!(config.collider_radius.is_none());
    assert!(config.collider_height.is_none());
}

#[test]
fn test_movement_config_speeds() {
    let config: MovementConfig = from_str("(walk_speed: 5.5, run_speed: 10.0)").unwrap();
    assert_eq!(config.walk_speed, 5.5);
    assert_eq!(config.run_speed, 10.0);
}

#[test]
fn test_movement_config_glb_player_fields() {
    let config: MovementConfig =
        from_str("(rot_speed: 2.5, collider_radius: 0.35, collider_height: 1.75)").unwrap();
    assert!((config.rot_speed.unwrap() - 2.5).abs() < 0.001);
    assert!((config.collider_radius.unwrap() - 0.35).abs() < 0.001);
    assert!((config.collider_height.unwrap() - 1.75).abs() < 0.001);
}

#[test]
fn test_jump_config_fixed() {
    // explicit Some(...) always accepted; implicit_some also allows bare enum variant
    let config: MovementConfig = from_str("(jump: Some(Fixed(height: 2.5)))").unwrap();
    assert!(matches!(config.jump, Some(JumpConfig::Fixed { height }) if (height - 2.5).abs() < 0.001));
}

#[test]
fn test_jump_config_relative_to_height() {
    let config: MovementConfig = from_str("(jump: Some(RelativeToHeight(percent: 120.0)))").unwrap();
    assert!(matches!(config.jump, Some(JumpConfig::RelativeToHeight { percent }) if (percent - 120.0).abs() < 0.001));
}

#[test]
fn test_jump_config_bare_variant_implicit_some() {
    // implicit_some: bare enum variant accepted for Option<JumpConfig>
    let config: MovementConfig = from_str("(jump: RelativeToHeight(percent: 80.0))").unwrap();
    assert!(matches!(config.jump, Some(JumpConfig::RelativeToHeight { percent }) if (percent - 80.0).abs() < 0.001));
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
    // Primitive player — collider dims come from primitive.radius/height, not movement
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

#[test]
fn test_glb_player_prefab_with_movement_parses() {
    // GLB player (kind: "actor") — collider_radius/collider_height override capsule shape.
    // sounds map is still valid on PrefabComponents; designers use it in state_machine.ron
    // to wire player.jumped → PlaySound rather than hardcoding it in Rust.
    let ron_str = r#"
        (
            schema_version: 1,
            prefabs: {
                "player_warrior": (
                    kind: "actor",
                    model: "hero",
                    animation_policy: "prefabs/animation/player_policy.ron",
                    components: (
                        tags: ["player"],
                        movement: (
                            walk_speed: 4.0,
                            run_speed: 8.0,
                            rot_speed: 2.5,
                            double_jump: true,
                            collider_radius: 0.35,
                            collider_height: 1.75,
                        ),
                    ),
                ),
            },
        )
    "#;
    let catalog: PrefabCatalog = from_str(ron_str).expect("GLB player with movement should parse");
    let mv = &catalog.prefabs["player_warrior"].components.movement;
    assert_eq!(mv.walk_speed, 4.0);
    assert_eq!(mv.run_speed, 8.0);
    assert!((mv.rot_speed.unwrap() - 2.5).abs() < 0.001);
    assert!(mv.double_jump);
    assert!((mv.collider_radius.unwrap() - 0.35).abs() < 0.001);
    assert!((mv.collider_height.unwrap() - 1.75).abs() < 0.001);
}

// ── Nested-prefab children ────────────────────────────────────────────────────

#[test]
fn test_nested_prefab_child_validates_ok() {
    // Two-level nesting: "village" → "well". Both keys exist; no cycle.
    let ron_str = r#"
        (
            schema_version: 1,
            prefabs: {
                "well": (
                    kind: "primitive",
                    model: "",
                    components: (),
                    children: [
                        ( shape: "Cylinder", primitive: (radius: Some(0.7), height: Some(0.8)) ),
                    ],
                ),
                "village": (
                    kind: "primitive",
                    model: "",
                    components: (),
                    children: [
                        ( prefab: Some("well"), offset: (5.0, 0.0, 0.0) ),
                    ],
                ),
            },
        )
    "#;
    let catalog: PrefabCatalog = from_str(ron_str).expect("nested prefab catalog should parse");
    assert!(catalog.validate().is_ok(), "two-level nesting with known keys must validate OK");
}

#[test]
fn test_nested_prefab_both_shape_and_prefab_is_invalid() {
    let ron_str = r#"
        (
            schema_version: 1,
            prefabs: {
                "well": ( kind: "primitive", model: "", components: () ),
                "village": (
                    kind: "primitive",
                    model: "",
                    components: (),
                    children: [
                        ( shape: "Cuboid", prefab: Some("well") ),
                    ],
                ),
            },
        )
    "#;
    let catalog: PrefabCatalog = from_str(ron_str).expect("should parse but must fail validation");
    assert!(
        catalog.validate().is_err(),
        "setting both shape and prefab on the same child must be rejected"
    );
}

#[test]
fn test_nested_prefab_neither_shape_nor_prefab_is_invalid() {
    let ron_str = r#"
        (
            schema_version: 1,
            prefabs: {
                "village": (
                    kind: "primitive",
                    model: "",
                    components: (),
                    children: [
                        ( offset: (0.0, 0.0, 0.0) ),
                    ],
                ),
            },
        )
    "#;
    let catalog: PrefabCatalog = from_str(ron_str).expect("should parse but must fail validation");
    assert!(
        catalog.validate().is_err(),
        "a child with neither shape nor prefab must be rejected"
    );
}

#[test]
fn test_nested_prefab_unknown_key_is_invalid() {
    let ron_str = r#"
        (
            schema_version: 1,
            prefabs: {
                "village": (
                    kind: "primitive",
                    model: "",
                    components: (),
                    children: [
                        ( prefab: Some("ghost_prefab") ),
                    ],
                ),
            },
        )
    "#;
    let catalog: PrefabCatalog = from_str(ron_str).expect("should parse but must fail validation");
    assert!(
        catalog.validate().is_err(),
        "referencing a non-existent prefab key must be rejected"
    );
}

#[test]
fn test_nested_prefab_cycle_is_invalid() {
    // a → b → a: circular reference must be caught at validation time.
    let ron_str = r#"
        (
            schema_version: 1,
            prefabs: {
                "a": (
                    kind: "primitive",
                    model: "",
                    components: (),
                    children: [ ( prefab: Some("b") ) ],
                ),
                "b": (
                    kind: "primitive",
                    model: "",
                    components: (),
                    children: [ ( prefab: Some("a") ) ],
                ),
            },
        )
    "#;
    let catalog: PrefabCatalog = from_str(ron_str).expect("cyclic catalog should parse (RON is just data)");
    assert!(
        catalog.validate().is_err(),
        "a → b → a cycle must be detected by validate()"
    );
}

#[test]
fn test_nested_prop_in_composite_validates_ok() {
    // A kind:"prop" (GLB) prefab referenced as a nested child of a kind:"primitive" composite
    // must pass validation — the schema allows all kinds as nested children.
    let ron_str = r#"
        (
            schema_version: 1,
            prefabs: {
                "oak_tree": (
                    kind: "prop",
                    model: "some_glb_key",
                    components: (),
                ),
                "clearing": (
                    kind: "primitive",
                    model: "",
                    components: (),
                    children: [
                        (
                            shape: "Cuboid",
                            primitive: (size: Some((10.0, 0.1, 10.0))),
                        ),
                        ( prefab: Some("oak_tree"), offset: (3.0, 0.0, 0.0) ),
                    ],
                ),
            },
        )
    "#;
    let catalog: PrefabCatalog = from_str(ron_str).expect("should parse");
    assert!(
        catalog.validate().is_ok(),
        "kind:\"prop\" nested inside kind:\"primitive\" must validate OK"
    );
}

#[test]
fn test_colliders_field_parses_on_prefab() {
    // A prefab with colliders: [...] must deserialise cleanly, including multiple entries.
    let ron_str = r#"
        (
            schema_version: 1,
            prefabs: {
                "chest": (
                    kind: "prop",
                    model: "chest_key",
                    components: (),
                    colliders: [
                        (shape: "Cuboid", size: Some((0.70, 0.55, 1.00)), offset: (0.0, -0.125, 0.0)),
                        (shape: "Cuboid", size: Some((0.68, 0.28, 0.98)), offset: (0.0,  0.275, 0.0)),
                    ],
                ),
            },
        )
    "#;
    let catalog: PrefabCatalog = from_str(ron_str).expect("should parse");
    assert!(catalog.validate().is_ok());
    let chest = &catalog.prefabs["chest"];
    assert_eq!(chest.colliders.len(), 2, "two collider shapes");
    assert_eq!(chest.colliders[0].shape, "Cuboid");
    assert_eq!(chest.colliders[0].size, Some((0.70, 0.55, 1.00)));
    assert_eq!(chest.colliders[1].offset, (0.0, 0.275, 0.0));
}

#[test]
fn test_nested_actor_in_composite_validates_ok() {
    // kind:"actor" (e.g. an NPC) nested inside a composite primitive must pass validation.
    let ron_str = r#"
        (
            schema_version: 1,
            prefabs: {
                "npc_guard": (
                    kind: "actor",
                    model: "guard_glb_key",
                    components: (),
                ),
                "guard_post": (
                    kind: "primitive",
                    model: "",
                    components: (),
                    children: [
                        ( shape: "Cuboid", primitive: (size: Some((2.0, 0.1, 2.0))) ),
                        ( prefab: Some("npc_guard"), offset: (0.0, 0.1, 0.0) ),
                    ],
                ),
            },
        )
    "#;
    let catalog: PrefabCatalog = from_str(ron_str).expect("should parse");
    assert!(
        catalog.validate().is_ok(),
        "kind:\"actor\" nested inside kind:\"primitive\" must validate OK"
    );
}

#[test]
fn test_nested_single_shape_primitive_validates_ok() {
    // A kind:"primitive" with a top-level model (no children) referenced as a nested child
    // must pass validation — the spawner will build a single mesh for it.
    let ron_str = r#"
        (
            schema_version: 1,
            prefabs: {
                "beacon": (
                    kind: "primitive",
                    model: "Sphere",
                    components: (),
                    primitive: Some((radius: Some(0.3))),
                ),
                "outpost": (
                    kind: "primitive",
                    model: "",
                    components: (),
                    children: [
                        ( shape: "Cuboid", primitive: (size: Some((4.0, 0.1, 4.0))) ),
                        ( prefab: Some("beacon"), offset: (0.0, 1.5, 0.0) ),
                    ],
                ),
            },
        )
    "#;
    let catalog: PrefabCatalog = from_str(ron_str).expect("should parse");
    assert!(
        catalog.validate().is_ok(),
        "single-shape primitive nested inside composite must validate OK"
    );
}

#[test]
fn test_colliders_empty_list_is_valid() {
    // colliders: [] (empty) must parse and validate — it means "no physics collider".
    let ron_str = r#"
        (
            schema_version: 1,
            prefabs: {
                "ghost_prop": (
                    kind: "prop",
                    model: "some_key",
                    components: (),
                    colliders: [],
                ),
            },
        )
    "#;
    let catalog: PrefabCatalog = from_str(ron_str).expect("should parse");
    assert!(catalog.validate().is_ok());
    assert_eq!(catalog.prefabs["ghost_prop"].colliders.len(), 0);
}

#[test]
fn test_composite_child_physics_true_validates_ok() {
    // An inline primitive child with physics: true inside a composite must validate.
    let ron_str = r#"
        (
            schema_version: 1,
            prefabs: {
                "platform": (
                    kind: "primitive",
                    model: "",
                    components: (),
                    children: [
                        (
                            shape: "Cuboid",
                            primitive: (
                                size: Some((4.0, 0.2, 4.0)),
                                physics: true,
                            ),
                        ),
                    ],
                ),
            },
        )
    "#;
    let catalog: PrefabCatalog = from_str(ron_str).expect("should parse");
    assert!(catalog.validate().is_ok());
    assert!(catalog.prefabs["platform"].children[0].primitive.physics);
}

#[test]
fn test_nested_glb_with_colliders_inside_composite_validates_ok() {
    // A kind:"prop" with colliders: [...] referenced as a nested child must validate.
    // This is the intersection of GLB nesting (Feature 1) and colliders (Feature 2).
    let ron_str = r#"
        (
            schema_version: 1,
            prefabs: {
                "crate": (
                    kind: "prop",
                    model: "crate_glb",
                    components: (),
                    colliders: [
                        (shape: "Cuboid", size: Some((0.8, 0.8, 0.8))),
                    ],
                ),
                "storage_room": (
                    kind: "primitive",
                    model: "",
                    components: (),
                    children: [
                        ( shape: "Cuboid", primitive: (size: Some((5.0, 0.1, 5.0))), ),
                        ( prefab: Some("crate"), offset: (1.0, 0.0, 0.0) ),
                        ( prefab: Some("crate"), offset: (-1.0, 0.0, 0.0) ),
                    ],
                ),
            },
        )
    "#;
    let catalog: PrefabCatalog = from_str(ron_str).expect("should parse");
    assert!(
        catalog.validate().is_ok(),
        "GLB prop with colliders nested inside composite must validate OK"
    );
    assert_eq!(catalog.prefabs["crate"].colliders.len(), 1);
}

// ── PrefabComponents ──────────────────────────────────────────────────────────

#[test]
fn test_sounds_map_parses() {
    // sounds: { "event": "catalog_key" } must round-trip cleanly.
    let ron_str = r#"
        (
            schema_version: 1,
            prefabs: {
                "hero": (
                    kind: "actor",
                    model: "hero_glb",
                    components: (
                        sounds: {
                            "jump":    "jump_sfx",
                            "collect": "coin_sfx",
                            "death":   "death_sfx",
                        },
                    ),
                ),
            },
        )
    "#;
    let catalog: PrefabCatalog = from_str(ron_str).expect("should parse");
    assert!(catalog.validate().is_ok());
    let sounds = &catalog.prefabs["hero"].components.sounds;
    assert_eq!(sounds.len(), 3);
    assert_eq!(sounds["jump"], "jump_sfx");
    assert_eq!(sounds["collect"], "coin_sfx");
    assert_eq!(sounds["death"], "death_sfx");
}

#[test]
fn test_jump_relative_to_height_parses() {
    // RelativeToHeight is the variant used by most prefabs; only Fixed was previously covered.
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
                            jump: Some(RelativeToHeight(percent: 100.0)),
                            double_jump: true,
                            double_jump_height: Some(RelativeToHeight(percent: 60.0)),
                        ),
                    ),
                ),
            },
        )
    "#;
    let catalog: PrefabCatalog = from_str(ron_str).expect("should parse");
    assert!(catalog.validate().is_ok());
    let mv = &catalog.prefabs["player"].components.movement;
    assert!(matches!(mv.jump, Some(JumpConfig::RelativeToHeight { percent }) if percent == 100.0));
    assert!(matches!(mv.double_jump_height, Some(JumpConfig::RelativeToHeight { percent }) if percent == 60.0));
}

#[test]
fn test_npc_def_full_parses() {
    // Full NpcDef with all fields — round-trip every required and optional field.
    let ron_str = r#"
        (
            schema_version: 1,
            prefabs: {
                "orc_patrol": (
                    kind: "actor",
                    model: "orc_glb",
                    components: (
                        npc: Some((
                            faction: Hostile,
                            on_player_near: Chase,
                            detection_radius: 8.0,
                            chase_radius: 20.0,
                            fov_degrees: Some(110.0),
                            requires_los: true,
                            approach_distance: 1.5,
                            patrol_speed: 2.5,
                            chase_speed: 5.0,
                            patrol_waypoints: [
                                (5.0, 0.0, 0.0),
                                (5.0, 0.0, 10.0),
                            ],
                            eye_height: 1.2,
                            alerted_duration: 0.5,
                            drag: 0.6,
                            waypoint_reach_radius: 1.0,
                        )),
                    ),
                ),
            },
        )
    "#;
    let catalog: PrefabCatalog = from_str(ron_str).expect("should parse");
    assert!(catalog.validate().is_ok());
    let npc = catalog.prefabs["orc_patrol"].components.npc.as_ref().expect("npc should be Some");
    assert_eq!(npc.faction, NpcFaction::Hostile);
    assert_eq!(npc.on_player_near, NpcOnPlayerNear::Chase);
    assert_eq!(npc.detection_radius, 8.0);
    assert_eq!(npc.chase_radius, 20.0);
    assert_eq!(npc.fov_degrees, Some(110.0));
    assert!(npc.requires_los);
    assert_eq!(npc.approach_distance, 1.5);
    assert_eq!(npc.patrol_speed, 2.5);
    assert_eq!(npc.chase_speed, 5.0);
    assert_eq!(npc.patrol_waypoints.len(), 2);
    assert_eq!(npc.patrol_waypoints[0], (5.0, 0.0, 0.0));
    assert_eq!(npc.eye_height, 1.2);
    assert_eq!(npc.alerted_duration, 0.5);
    assert_eq!(npc.drag, 0.6);
    assert_eq!(npc.waypoint_reach_radius, 1.0);
}

#[test]
fn test_npc_def_minimal_uses_defaults() {
    // Only required fields set — defaulted fields must resolve to their documented values.
    let ron_str = r#"
        (
            schema_version: 1,
            prefabs: {
                "friendly_npc": (
                    kind: "actor",
                    model: "villager_glb",
                    components: (
                        npc: Some((
                            faction: Friendly,
                            on_player_near: Interact,
                            detection_radius: 5.0,
                            chase_radius: 10.0,
                        )),
                    ),
                ),
            },
        )
    "#;
    let catalog: PrefabCatalog = from_str(ron_str).expect("should parse");
    assert!(catalog.validate().is_ok());
    let npc = catalog.prefabs["friendly_npc"].components.npc.as_ref().unwrap();
    assert_eq!(npc.faction, NpcFaction::Friendly);
    assert_eq!(npc.on_player_near, NpcOnPlayerNear::Interact);
    assert_eq!(npc.fov_degrees, None);       // default: 360° awareness
    assert!(!npc.requires_los);              // default: no LOS check
    assert_eq!(npc.approach_distance, 2.0); // default_approach_distance()
    assert_eq!(npc.patrol_speed, 2.0);      // default_patrol_speed()
    assert_eq!(npc.chase_speed, 4.5);       // default_chase_speed()
    assert!(npc.patrol_waypoints.is_empty()); // default: idle
    assert_eq!(npc.eye_height, 0.9);           // default_npc_eye_height()
    assert_eq!(npc.alerted_duration, 0.3);     // default_npc_alerted_duration()
    assert_eq!(npc.drag, 0.8);                 // default_npc_drag()
    assert_eq!(npc.waypoint_reach_radius, 0.5); // default_npc_waypoint_reach_radius()
}

#[test]
fn test_npc_all_faction_and_behavior_variants_parse() {
    // Verify every NpcFaction and NpcOnPlayerNear variant deserialises without error.
    let cases = [
        ("Friendly", "Interact"),
        ("Hostile",  "Chase"),
        ("Neutral",  "Flee"),
        ("Neutral",  "Alert"),
    ];
    for (faction, behavior) in cases {
        let ron_str = format!(r#"
            (
                schema_version: 1,
                prefabs: {{
                    "npc": (
                        kind: "actor",
                        model: "m",
                        components: (
                            npc: Some((
                                faction: {faction},
                                on_player_near: {behavior},
                                detection_radius: 5.0,
                                chase_radius: 10.0,
                            )),
                        ),
                    ),
                }},
            )
        "#);
        let catalog: PrefabCatalog = from_str(&ron_str)
            .unwrap_or_else(|e| panic!("faction={faction} behavior={behavior} failed to parse: {e}"));
        assert!(catalog.validate().is_ok(),
            "faction={faction} behavior={behavior} failed validate()");
    }
}

// ── PrefabComponents.inputs (M-2) ─────────────────────────────────────────────

#[test]
fn test_player_prefab_inputs_all_keys_parses() {
    let ron_str = r#"
        (
            schema_version: 1,
            prefabs: {
                "player": (
                    kind: "actor",
                    model: "hero",
                    components: (
                        tags: ["player"],
                        inputs: (
                            forward:      "KeyW",
                            backward:     "KeyS",
                            left:         "KeyA",
                            right:        "KeyD",
                            strafe_left:  "KeyQ",
                            strafe_right: "KeyE",
                            jump:         "Space",
                            run:          "ShiftLeft",
                            interact:     "KeyF",
                        ),
                    ),
                ),
            },
        )
    "#;
    let catalog: PrefabCatalog = from_str(ron_str).expect("player with full inputs block should parse");
    assert!(catalog.validate().is_ok());
    let inputs = catalog.prefabs["player"].components.inputs.as_ref()
        .expect("inputs should be Some after explicit RON block");
    assert_eq!(inputs.forward,      "KeyW");
    assert_eq!(inputs.backward,     "KeyS");
    assert_eq!(inputs.left,         "KeyA");
    assert_eq!(inputs.right,        "KeyD");
    assert_eq!(inputs.strafe_left,  "KeyQ");
    assert_eq!(inputs.strafe_right, "KeyE");
    assert_eq!(inputs.jump,         "Space");
    assert_eq!(inputs.run,          "ShiftLeft");
    assert_eq!(inputs.interact,     "KeyF");
}

#[test]
fn test_player_prefab_inputs_optional_keys_default() {
    // `run` and `interact` have serde defaults; omitting them must not fail.
    let ron_str = r#"
        (
            schema_version: 1,
            prefabs: {
                "player": (
                    kind: "actor",
                    model: "hero",
                    components: (
                        tags: ["player"],
                        inputs: (
                            forward:      "ArrowUp",
                            backward:     "ArrowDown",
                            left:         "ArrowLeft",
                            right:        "ArrowRight",
                            strafe_left:  "KeyQ",
                            strafe_right: "KeyE",
                            jump:         "Space",
                        ),
                    ),
                ),
            },
        )
    "#;
    let catalog: PrefabCatalog = from_str(ron_str).expect("inputs missing run/interact should parse");
    let inputs = catalog.prefabs["player"].components.inputs.as_ref().unwrap();
    assert_eq!(inputs.run,      "ShiftLeft", "run should default to ShiftLeft");
    assert_eq!(inputs.interact, "KeyF",      "interact should default to KeyF");
    assert_eq!(inputs.forward,  "ArrowUp");
}

#[test]
fn test_player_prefab_inputs_omitted_backward_compat() {
    // Existing prefabs without an inputs block must still parse cleanly.
    let ron_str = r#"
        (
            schema_version: 1,
            prefabs: {
                "player": (
                    kind: "actor",
                    model: "hero",
                    components: ( tags: ["player"] ),
                ),
            },
        )
    "#;
    let catalog: PrefabCatalog = from_str(ron_str).expect("player without inputs block should parse");
    assert!(catalog.prefabs["player"].components.inputs.is_none(),
        "inputs should be None when the field is absent");
}

// ── PrefabComponents.flycam (M-3) ─────────────────────────────────────────────

#[test]
fn test_flycam_prefab_full_config_parses() {
    let ron_str = r#"
        (
            schema_version: 1,
            prefabs: {
                "cam": (
                    kind: "prop",
                    model: "",
                    components: (
                        tags: ["flycam"],
                        flycam: (
                            speed:       50.0,
                            fast_speed:  150.0,
                            sensitivity: 0.001,
                        ),
                    ),
                ),
            },
        )
    "#;
    let catalog: PrefabCatalog = from_str(ron_str).expect("flycam with full config should parse");
    assert!(catalog.validate().is_ok());
    let fc = catalog.prefabs["cam"].components.flycam.as_ref()
        .expect("flycam should be Some after explicit RON block");
    assert_eq!(fc.speed,       50.0);
    assert_eq!(fc.fast_speed,  150.0);
    assert_eq!(fc.sensitivity, 0.001);
}

#[test]
fn test_flycam_prefab_partial_config_uses_defaults() {
    // Any subset of fields may be provided; omitted fields fall back to compiled defaults.
    let ron_str = r#"
        (
            schema_version: 1,
            prefabs: {
                "cam": (
                    kind: "prop",
                    model: "",
                    components: (
                        tags: ["flycam"],
                        flycam: ( speed: 40.0 ),
                    ),
                ),
            },
        )
    "#;
    let catalog: PrefabCatalog = from_str(ron_str).expect("flycam with partial config should parse");
    let fc = catalog.prefabs["cam"].components.flycam.as_ref().unwrap();
    assert_eq!(fc.speed,       40.0,  "overridden speed should be 40.0");
    assert_eq!(fc.fast_speed,  200.0, "fast_speed should default to 200.0");
    assert_eq!(fc.sensitivity, 0.002, "sensitivity should default to 0.002");
}

#[test]
fn test_flycam_prefab_config_omitted_backward_compat() {
    // Existing flycam prefabs without a flycam block must still parse cleanly.
    let ron_str = r#"
        (
            schema_version: 1,
            prefabs: {
                "cam": (
                    kind: "prop",
                    model: "",
                    components: ( tags: ["flycam"] ),
                ),
            },
        )
    "#;
    let catalog: PrefabCatalog = from_str(ron_str).expect("flycam without config block should parse");
    assert!(catalog.prefabs["cam"].components.flycam.is_none(),
        "flycam field should be None when omitted");
}

#[test]
fn test_flycam_def_unknown_field_is_error() {
    // FlyCamDef uses deny_unknown_fields — typos must not silently vanish.
    let fc: Result<FlyCamDef, _> = from_str("(speeed: 50.0)");
    assert!(fc.is_err(), "typos in FlyCamDef must be rejected");
}

#[test]
fn test_flycam_def_all_defaults() {
    let fc: FlyCamDef = from_str("()").expect("empty FlyCamDef should use all defaults");
    assert_eq!(fc.speed,       100.0);
    assert_eq!(fc.fast_speed,  200.0);
    assert_eq!(fc.sensitivity, 0.002);
}