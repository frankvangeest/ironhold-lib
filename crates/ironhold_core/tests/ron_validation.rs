use ironhold_core::schema::{ProjectConfig, GameLevel};
use ron::de::from_str;

// ProjectConfig tests
#[test]
fn test_project_config_deserialization() {
    let ron_str = r#"
        (
            schema_version: 1,
            initial_scene: "scenes/main.ron"
        )
    "#;
    let config: ProjectConfig = from_str(ron_str).expect("Failed to deserialize ProjectConfig");
    assert_eq!(config.schema_version, 1);
    assert_eq!(config.initial_scene, "scenes/main.ron");
}

#[test]
fn test_project_config_missing_schema_version_is_error() {
    let ron_str = r#"
        (
            initial_scene: "scenes/main.ron"
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
fn test_project_config_wrong_schema_version_is_invalid() {
    let ron_str = r#"
        (
            schema_version: 999,
            initial_scene: "scenes/main.ron"
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
            typo_field: 123
        )
    "#;
    let result: Result<ProjectConfig, _> = ron::de::from_str(ron_str);
    assert!(result.is_err(), "unknown fields should be rejected");
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
                    action: LoadScene("scenes/game.ron")
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
                animations: (
                    idle: "Idle",
                    walk: "Walk",
                    run: "Run",
                    jump_enter: "JumpEnter",
                    jump_loop: "JumpLoop",
                    jump_exit: "JumpExit",
                    death: "Death",
                    dance: "Dance",
                    crouch_idle: "CrouchIdle",
                    crouch_forward: "CrouchForward",                    roll: "Roll"
                )
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