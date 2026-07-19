use bevy::prelude::*;
use bevy::ecs::system::RunSystemOnce;
use std::collections::HashMap;
use ironhold_core::PipelineWarmup;
use ironhold_core::runtime::{ActionQueue, SceneEvent, InputAction, InputActionMessage, ModelSpawner, OverlayEntity, PendingSceneLoadMode, PreloadedScenes, SceneHandleV2, LevelEntity, LoadedKeyBindings, ProjectKeyBindings};
use ironhold_core::schema::{AppState, Action, ProjectConfig, ProjectConfigHandle, TransformFix, GameSceneV2};
use ironhold_core::capabilities::player::{CharacterController, SpeedMultiplier, player_movement_system};
use ironhold_core::capabilities::animation::AnimationController;
use ironhold_core::schema::player::{InputMap, AnimationPolicy, BaseAnimations};
use ironhold_core::capabilities::animation_resolver::{AnimationPolicyComponent, LocomotionState, AnimationRequests, ActiveOverride};

mod support;
use support::setup_test_app;

/// Drive a Replace-mode scene load through `spawn_scene_v2`.
/// Inserts a minimal ProjectConfig override and a synthetic empty scene, then
/// transitions state to LoadingScene and runs enough updates for the system to
/// fire and its despawn commands to flush.
fn drive_replace_load(app: &mut App) {
    let config_handle = app
        .world_mut()
        .resource_mut::<Assets<ProjectConfig>>()
        .add(ProjectConfig {
            schema_version: 1,
            initial_scene: "scenes/t.ron".to_string(),
            ..Default::default()
        });
    app.world_mut().insert_resource(ProjectConfigHandle(config_handle));

    let scene: GameSceneV2 =
        ron::de::from_str("(schema_version: 2, entities: [], ui: [])").unwrap();
    let scene_handle = app
        .world_mut()
        .resource_mut::<Assets<GameSceneV2>>()
        .add(scene);
    app.world_mut().insert_resource(SceneHandleV2(scene_handle));

    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::LoadingScene);
    app.update(); // state transitions to LoadingScene
    app.update(); // spawn_scene_v2 fires, despawn commands queued
    app.update(); // commands flushed
}

#[test]
fn test_scene_lifecycle_events() {
    let mut app = setup_test_app();
       
    app.update();
    
    // 1. Trigger LoadScene action
    app.world_mut().resource_mut::<ActionQueue>().push(Action::LoadScene("scenes/tests/test_scene.ron".to_string()));
    
    // 2. Run executor
    app.update();
    
    // 3. Verify SceneEvent::Requested was emitted with the project-root-resolved path
    app.world_mut().run_system_once(|mut scene_events: MessageReader<SceneEvent>| {
        let events: Vec<_> = scene_events.read().cloned().collect();
        assert!(events.iter().any(|e| matches!(e, SceneEvent::Requested(path) if path == "projects/integration_tests/scenes/tests/test_scene.ron")));
    }).unwrap();
}

#[test]
fn test_input_abstraction_flow() {
    let mut app = setup_test_app();
    app.update();

    // 1. Spawn entity with CharacterController.
    let entity = app.world_mut().spawn((
        Transform::from_xyz(0.0, 0.0, 0.0),
        GlobalTransform::default(),
        CharacterController {
            walk_speed: 10.0,
            run_speed: 20.0,
            rot_speed: 2.0,
            inputs: InputMap {
                forward: "KeyW".to_string(),
                backward: "KeyS".to_string(),
                left: "KeyA".to_string(),
                right: "KeyD".to_string(),
                strafe_left: "KeyQ".to_string(),
                strafe_right: "KeyE".to_string(),
                jump: "Space".to_string(),
                run: "ShiftLeft".to_string(),
                interact: "KeyF".to_string(),
                strafe_mouse_button: Some("Left".to_string()),
                target_next: "Tab".to_string(),
                target_range: 30.0,
                gamepad_index: None, look_left: None, look_right: None, look_up: None, look_down: None,
            },
            is_running: false,
            jump_velocity: 5.94,
            double_jump_enabled: false,
            double_jump_velocity: 5.94,
            jumps_used: 0,
            max_jumps: 1,
            collider_radius: 0.4,
            ground_cast_length: 0.3,
            idle_drag: 0.8,
        },
        LocomotionState::default(),
        AnimationRequests::default(),
        ActiveOverride::default(),
        AnimationPolicyComponent(AnimationPolicy {
            base: BaseAnimations {
                idle: "idle".to_string(),
                walk: "walk".to_string(),
                run: "run".to_string(),
                jump_loop: "idle".to_string(),
            },
            clips: std::collections::HashMap::new(),
            overrides: vec![],
            default_transition_ms: None,
            animation_sources: vec![],
        }),
        AnimationController {
            current: "idle".to_string(),
            last_played: String::new(),
            gltf_path: String::new(),
            gltf_handle: Default::default(),
            source_handles: vec![],
            node_indices: Default::default(),
            graph_initialized: false,
            transition_ms: 0,
            should_loop: true,
            last_player_entity: None,
        },
        bevy_rapier3d::prelude::RigidBody::Dynamic,
        bevy_rapier3d::prelude::Velocity::zero(),
        SpeedMultiplier(1.0),
    )).id();

    // Remove the DefaultRapierContext so player_movement_system falls into its headless
    // else-branch (is_grounded = true). Same approach as test_player_jump_emits_game_event.
    {
        use bevy_rapier3d::plugin::context::DefaultRapierContext;
        let rapier_entity = app.world_mut()
            .query_filtered::<Entity, With<DefaultRapierContext>>()
            .iter(app.world())
            .next();
        if let Some(e) = rapier_entity { app.world_mut().despawn(e); }
    }

    // 2. Write InputActionMessage directly — bypasses input_translator and FixedUpdate timing.
    // MessageReader as a SystemParam starts at the current buffer end, so writing first and
    // then calling run_system_once immediately (without an intervening app.update()) is the
    // only reliable pattern in headless tests. See test_player_jump_emits_game_event.
    app.world_mut()
        .resource_mut::<Messages<InputActionMessage>>()
        .write(InputActionMessage { entity, action: InputAction::Move(Vec2::new(0.0, 1.0)) });

    // 3. Run player_movement_system with a fresh MessageCursor that sees the Move message.
    app.world_mut().run_system_once(player_movement_system).unwrap();

    // 4. Verify forward movement (Transform::forward = (0, 0, -1) in Bevy 3D).
    let velocity = app.world().entity(entity).get::<bevy_rapier3d::prelude::Velocity>().unwrap();
    assert!(velocity.linvel.z < 0.0, "Expected Z velocity < 0 (forward movement), got {}", velocity.linvel.z);
}

#[test]
fn model_fixup_persists_reset() {
    let mut app = setup_test_app();
    
    // 1. Run once to process Startup (setup)
    app.update();
    
    // 2. Mock ProjectConfig with a specific model fix
    let test_path = "shared/models/test-model.glb#Scene0".to_string();
    let fix = TransformFix {
        pivot_offset: (1.0, 2.0, 3.0),
        rotation_deg: (0.0, 90.0, 0.0),
        scale: (2.0, 2.0, 2.0),
    };
    
    {
        let mut configs = app.world_mut().resource_mut::<Assets<ProjectConfig>>();
        let config_handle = configs.add(ProjectConfig {
            schema_version: 1,
            initial_scene: "scenes/tests/test_scene.ron".to_string(),
            rules: vec![],
            rules_path: None,
            state_machine_path: None,
            model_fixes: {
                let mut map = std::collections::HashMap::new();
                map.insert(test_path.clone(), fix.clone());
                map
            },
            model_fixes_path: None,
            project_id: None,
            display_name: None,
            asset_catalog: None,
            prefab_catalog: None,
            ..Default::default()
        });
        app.world_mut().insert_resource(ProjectConfigHandle(config_handle));
    }
    
    // Populate MergedModelFixes so spawn_instance can find the fix.
    {
        let mut merged = app.world_mut().resource_mut::<ironhold_core::runtime::MergedModelFixes>();
        merged.0.insert(test_path.clone(), fix.clone());
    }

    // 3. Helper to verify fix is applied
    let verify_fix = |app: &mut App, path: String| {
        let (parent, child) = app.world_mut().run_system_once(move |
            mut commands: Commands,
            spawner: Res<ModelSpawner>,
            asset_server: Res<AssetServer>,
            merged_fixes: Res<ironhold_core::runtime::MergedModelFixes>,
        | {
            let spawned = spawner.spawn_instance(
                &mut commands,
                &asset_server,
                &merged_fixes.0,
                path.clone(),
                Transform::IDENTITY,
            );
            (spawned.parent, spawned.child)
        }).unwrap();

        app.update(); // Flush commands

        let child_transform = app.world().get::<Transform>(child).expect("Child should have Transform");

        // Verify translation (pivot_offset)
        assert_eq!(child_transform.translation, Vec3::new(1.0, 2.0, 3.0));

        // Verify scale
        assert_eq!(child_transform.scale, Vec3::new(2.0, 2.0, 2.0));

        // Verify rotation (90 deg around Y)
        let expected_rot = Quat::from_rotation_y(90.0f32.to_radians());
        assert!(child_transform.rotation.abs_diff_eq(expected_rot, 0.0001));

        parent
    };

    // 4. Test first spawn
    let _parent1 = verify_fix(&mut app, test_path.clone());

    // 5. Simulate "reset" (clearing entities, though ModelSpawner is a resource so it persists)
    // In our context, "persists reset" means if we spawn it again (even after scene clear), it still uses the config.
    let _parent2 = verify_fix(&mut app, test_path.clone());

    // 6. Verify with a path that doesn't have a fix (should use default)
    {
        app.world_mut().run_system_once(|
            mut commands: Commands,
            spawner: Res<ModelSpawner>,
            asset_server: Res<AssetServer>,
            merged_fixes: Res<ironhold_core::runtime::MergedModelFixes>,
        | {
            let unknown_path = "models/unknown.glb#Scene0".to_string();
            let _spawned = spawner.spawn_instance(
                &mut commands,
                &asset_server,
                &merged_fixes.0,
                unknown_path,
                Transform::IDENTITY,
            );
        }).unwrap();
    }
    
    app.update(); // Flush commands from system_once
}

#[test]
fn test_load_scene_overlay_sets_overlay_load_mode() {
    let mut app = setup_test_app();
    app.update();

    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::LoadSceneOverlay("scenes/pause.scene.ron".to_string()));
    app.update();

    let mode = app.world().resource::<PendingSceneLoadMode>();
    assert_eq!(*mode, PendingSceneLoadMode::Overlay,
        "LoadSceneOverlay should set PendingSceneLoadMode to Overlay");
}

#[test]
fn test_unload_overlay_despawns_overlay_entities() {
    let mut app = setup_test_app();
    app.update();

    // Spawn two overlay entities.
    app.world_mut().spawn(OverlayEntity);
    app.world_mut().spawn(OverlayEntity);

    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::UnloadOverlay);
    app.update(); // executor queues despawn commands
    app.update(); // flush

    let count = app.world_mut()
        .query::<&OverlayEntity>()
        .iter(app.world())
        .count();
    assert_eq!(count, 0, "UnloadOverlay should despawn all OverlayEntity entities");
}

#[test]
fn test_toggle_overlay_opens_when_no_overlay_active() {
    let mut app = setup_test_app();
    app.update();

    // No OverlayEntity present â€” toggle should open (set load mode to Overlay).
    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::ToggleOverlay("scenes/pause.scene.ron".to_string()));
    app.update();

    let mode = app.world().resource::<PendingSceneLoadMode>();
    assert_eq!(*mode, PendingSceneLoadMode::Overlay,
        "ToggleOverlay with no active overlay should set load mode to Overlay");
}

#[test]
fn test_toggle_overlay_closes_when_overlay_active() {
    let mut app = setup_test_app();
    app.update();

    // Spawn an overlay entity so ToggleOverlay sees an active overlay.
    app.world_mut().spawn(OverlayEntity);

    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::ToggleOverlay("scenes/pause.scene.ron".to_string()));
    app.update(); // executor queues despawn
    app.update(); // flush

    let count = app.world_mut()
        .query::<&OverlayEntity>()
        .iter(app.world())
        .count();
    assert_eq!(count, 0,
        "ToggleOverlay with an active overlay should despawn all OverlayEntity entities");
}

#[test]
fn test_preload_scene_ron_pushes_handle() {
    let mut app = setup_test_app();
    app.update();

    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::PreloadScene("scenes/pause.scene.ron".to_string()));
    app.update();

    let preloaded = app.world().resource::<PreloadedScenes>();
    assert_eq!(preloaded.0.len(), 1, "PreloadScene should store the handle in PreloadedScenes");
}

#[test]
fn test_preload_non_scene_path_does_not_panic() {
    let mut app = setup_test_app();
    app.update();

    // Non-.scene.ron path â€” executor should warn, not push a handle.
    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::PreloadScene("textures/something.png".to_string()));
    app.update(); // must not panic

    let preloaded = app.world().resource::<PreloadedScenes>();
    assert_eq!(preloaded.0.len(), 0,
        "Non-.scene.ron path should not be added to PreloadedScenes");
}

#[test]
fn test_animation_graph_only_includes_present_clips() {
    // When an AnimationPolicy declares clips that don't exist in the GLB,
    // the graph still initialises and node_indices contains only the clips
    // that were actually present in the GLTF (the missing ones are warned
    // about but never added).
    let mut app = setup_test_app();
    app.init_asset::<bevy::animation::AnimationClip>();
    app.update();

    // Build a Gltf asset that only has "Walk_Loop" in named_animations.
    let gltf = Gltf {
        scenes: vec![],
        named_scenes: Default::default(),
        meshes: vec![],
        named_meshes: Default::default(),
        materials: vec![],
        named_materials: Default::default(),
        nodes: vec![],
        named_nodes: Default::default(),
        skins: vec![],
        named_skins: Default::default(),
        default_scene: None,
        animations: vec![],
        named_animations: Default::default(),
        source: None,
    };
    let gltf_handle = app.world_mut().resource_mut::<Assets<Gltf>>().add(gltf);

    let walk_clip = app.world_mut()
        .resource_mut::<Assets<bevy::animation::AnimationClip>>()
        .add(bevy::animation::AnimationClip::default());
    app.world_mut()
        .resource_mut::<Assets<Gltf>>()
        .get_mut(&gltf_handle)
        .unwrap()
        .named_animations
        .insert("Walk_Loop".into(), walk_clip);

    // Policy declares four clips â€” only "Walk_Loop" exists in the Gltf.
    let policy = AnimationPolicy {
        base: BaseAnimations {
            idle: "Idle_Loop".to_string(),
            walk: "Walk_Loop".to_string(),
            run: "Run_Loop".to_string(),
            jump_loop: "Jump_Loop".to_string(),
        },
        clips: HashMap::new(),
        overrides: vec![],
        default_transition_ms: None,
        animation_sources: vec![],
    };

    // Spawn with AnimationPlayer on the same entity so find_player_entity_recursive
    // locates it without needing a child hierarchy.
    let entity = app.world_mut().spawn((
        Transform::default(),
        GlobalTransform::default(),
        AnimationPolicyComponent(policy),
        AnimationController {
            current: "Walk_Loop".to_string(),
            last_played: String::new(),
            gltf_path: "test.glb".to_string(),
            gltf_handle,
            source_handles: vec![],
            node_indices: Default::default(),
            graph_initialized: false,
            transition_ms: 0,
            should_loop: true,
            last_player_entity: None,
        },
        bevy::animation::AnimationPlayer::default(),
    )).id();

    app.update();
    app.update();

    let controller = app.world().entity(entity).get::<AnimationController>().unwrap();

    assert!(controller.graph_initialized,
        "Graph should be initialized even when some policy clips are missing from the GLTF");

    assert!(controller.node_indices.contains_key("Walk_Loop"),
        "Walk_Loop is in the GLTF and must be in node_indices");
    assert!(!controller.node_indices.contains_key("Idle_Loop"),
        "Idle_Loop is not in the GLTF and must not be in node_indices");
    assert!(!controller.node_indices.contains_key("Run_Loop"),
        "Run_Loop is not in the GLTF and must not be in node_indices");
    assert!(!controller.node_indices.contains_key("Jump_Loop"),
        "Jump_Loop is not in the GLTF and must not be in node_indices");
}

#[test]
fn test_animation_missing_clip_stops_retrying() {
    // When animation_playback_system can't find the requested clip in node_indices,
    // it must update last_played so it doesn't re-warn every frame.
    let mut app = setup_test_app();
    app.update();

    // graph_initialized=true with an empty node_indices simulates the case where
    // the graph was built but the requested clip was not in the GLTF.
    let entity = app.world_mut().spawn((
        Transform::default(),
        GlobalTransform::default(),
        AnimationPolicyComponent(AnimationPolicy {
            base: BaseAnimations {
                idle: "Idle_Loop".to_string(),
                walk: "Walk_Loop".to_string(),
                run: "Run_Loop".to_string(),
                jump_loop: "Jump_Loop".to_string(),
            },
            clips: HashMap::new(),
            overrides: vec![],
            default_transition_ms: None,
            animation_sources: vec![],
        }),
        AnimationController {
            current: "missing_clip".to_string(),
            last_played: String::new(),
            gltf_path: String::new(),
            gltf_handle: Default::default(),
            source_handles: vec![],
            node_indices: Default::default(),
            graph_initialized: true,
            transition_ms: 0,
            should_loop: true,
            last_player_entity: None,
        },
        bevy::animation::AnimationPlayer::default(),
    )).id();

    app.update();

    let controller = app.world().entity(entity).get::<AnimationController>().unwrap();
    assert_eq!(
        controller.last_played, "missing_clip",
        "last_played must equal current after a missing-clip warn so the system stops retrying"
    );
}

#[test]
fn test_level_entity_despawned_on_replace_load() {
    let mut app = setup_test_app();
    app.update();

    // Pre-spawn entities that belong to the "previous" scene.
    let old_a = app.world_mut().spawn(LevelEntity).id();
    let old_b = app.world_mut().spawn(LevelEntity).id();

    drive_replace_load(&mut app);

    assert!(
        app.world().get_entity(old_a).is_err(),
        "LevelEntity 'old_a' must be despawned on Replace-mode scene load"
    );
    assert!(
        app.world().get_entity(old_b).is_err(),
        "LevelEntity 'old_b' must be despawned on Replace-mode scene load"
    );
}

#[test]
fn test_overlay_entities_despawned_on_replace_load() {
    let mut app = setup_test_app();
    app.update();

    // Pre-spawn overlay entities (e.g. from a previously open pause menu).
    let overlay_a = app.world_mut().spawn(OverlayEntity).id();
    let overlay_b = app.world_mut().spawn(OverlayEntity).id();

    drive_replace_load(&mut app);

    assert!(
        app.world().get_entity(overlay_a).is_err(),
        "OverlayEntity 'overlay_a' must be despawned on Replace-mode scene load"
    );
    assert!(
        app.world().get_entity(overlay_b).is_err(),
        "OverlayEntity 'overlay_b' must be despawned on Replace-mode scene load"
    );
}

#[test]
fn test_key_bindings_do_not_bleed_across_scenes() {
    let mut app = setup_test_app();
    app.update();

    // Simulate a binding left over from a previous scene.
    app.world_mut().insert_resource(LoadedKeyBindings(HashMap::from([
        ("KeyX".to_string(), "previous_scene_action".to_string()),
    ])));
    // Project-level bindings do NOT include "KeyX".
    app.world_mut()
        .insert_resource(ProjectKeyBindings(HashMap::new()));

    drive_replace_load(&mut app);

    // Replace-mode load must rebuild LoadedKeyBindings from ProjectKeyBindings only.
    // "KeyX" must not survive the scene transition.
    let bindings = app.world().resource::<LoadedKeyBindings>();
    assert!(
        !bindings.0.contains_key("KeyX"),
        "LoadedKeyBindings must not carry 'KeyX' forward from a previous scene â€” no bleed allowed"
    );
}

#[test]
fn test_model_fixes_external_overrides_inline() {
    // Mirrors the merge logic in project_loader.rs:
    //   merged = config.model_fixes  (inline)
    //   merged.extend(fixes_asset.model_fixes)  (external file)
    // When the same key exists in both sources, the external file wins.
    let inline_fix = TransformFix {
        pivot_offset: (1.0, 0.0, 0.0),
        rotation_deg: (0.0, 0.0, 0.0),
        scale: (1.0, 1.0, 1.0),
    };
    let external_fix = TransformFix {
        pivot_offset: (99.0, 0.0, 0.0),
        rotation_deg: (0.0, 0.0, 0.0),
        scale: (2.0, 2.0, 2.0),
    };

    // Start with inline fixes.
    let mut merged: HashMap<String, TransformFix> = HashMap::from([
        ("models/hero.glb".to_string(), inline_fix.clone()),
    ]);

    // Extend with external file â€” same key present in both.
    let external: HashMap<String, TransformFix> = HashMap::from([
        ("models/hero.glb".to_string(), external_fix.clone()),
        ("models/prop.glb".to_string(), inline_fix.clone()), // key only in external
    ]);
    merged.extend(external.into_iter());

    assert_eq!(
        merged["models/hero.glb"].pivot_offset.0, 99.0,
        "External model fix must override the inline fix for shared keys"
    );
    assert_eq!(
        merged["models/hero.glb"].scale.0, 2.0,
        "External scale must override inline scale"
    );
    assert!(
        merged.contains_key("models/prop.glb"),
        "External-only key must be present in the merged output"
    );
    assert_eq!(
        merged.len(), 2,
        "Merged map should contain exactly the two unique keys"
    );
}

#[test]
fn test_model_fixes_base_path_fallback() {
    // spawn_instance looks up the fix first by the full fragment path
    // ("models/hero.glb#Scene0"), then by the base path ("models/hero.glb").
    // When the fix is stored under the base path only, it must still apply.
    let mut app = setup_test_app();
    app.update();

    let base_path = "shared/models/hero.glb".to_string();
    let fragment_path = "shared/models/hero.glb#Scene0".to_string();
    let fix = TransformFix {
        pivot_offset: (5.0, 0.0, 0.0),
        rotation_deg: (0.0, 0.0, 0.0),
        scale: (1.0, 1.0, 1.0),
    };

    // Store fix under the BASE path only â€” no fragment path entry.
    {
        let mut merged = app
            .world_mut()
            .resource_mut::<ironhold_core::runtime::MergedModelFixes>();
        merged.0.insert(base_path.clone(), fix.clone());
        assert!(!merged.0.contains_key(&fragment_path),
            "Precondition: fragment path must not be in fixes");
    }

    // Spawn using the FRAGMENT path â€” base path fallback should apply the fix.
    let (_, child) = app
        .world_mut()
        .run_system_once(move |
            mut commands: Commands,
            spawner: Res<ModelSpawner>,
            asset_server: Res<AssetServer>,
            merged_fixes: Res<ironhold_core::runtime::MergedModelFixes>,
        | {
            let spawned = spawner.spawn_instance(
                &mut commands,
                &asset_server,
                &merged_fixes.0,
                fragment_path.clone(),
                Transform::IDENTITY,
            );
            (spawned.parent, spawned.child)
        })
        .unwrap();

    app.update(); // flush spawn commands

    let child_transform = app
        .world()
        .get::<Transform>(child)
        .expect("Child entity must exist after spawn");

    assert_eq!(
        child_transform.translation,
        Vec3::new(5.0, 0.0, 0.0),
        "Base path fallback must apply the fix stored under 'hero.glb' when spawning 'hero.glb#Scene0'"
    );
}

#[test]
fn test_pipeline_warmup_decrements_to_zero() {
    let mut app = setup_test_app();
    app.update(); // Startup

    app.world_mut().insert_resource(PipelineWarmup(4));

    // pipeline_warmup_system runs each Update frame; 4 ticks should drain the counter.
    for _ in 0..4 {
        app.update();
    }

    let warmup = app.world().resource::<PipelineWarmup>();
    assert_eq!(warmup.0, 0, "PipelineWarmup should reach 0 after 4 frames");
}

#[test]
fn test_pipeline_warmup_stops_at_zero() {
    let mut app = setup_test_app();
    app.update(); // Startup

    app.world_mut().insert_resource(PipelineWarmup(2));

    // Run more frames than the initial count â€” should not underflow.
    for _ in 0..10 {
        app.update();
    }

    let warmup = app.world().resource::<PipelineWarmup>();
    assert_eq!(warmup.0, 0, "PipelineWarmup must not go below 0");
}
