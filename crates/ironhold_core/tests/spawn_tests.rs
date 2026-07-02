use bevy::prelude::*;
use std::collections::HashMap;
use ironhold_core::runtime::{ActionQueue, LoadedAssetCatalog, LoadedPrefabCatalog, SpawnId, SpawnRegistry, PreloadedGlbHandles, PendingEntitySpawns, SceneHandleV2, LevelEntity};
use ironhold_core::schema::{AppState, Action, ProjectConfig, ProjectConfigHandle, GameSceneV2};
use ironhold_core::capabilities::animation_resolver::LocomotionState;

mod support;
use support::setup_test_app;

fn minimal_orc_catalogs(app: &mut App) {
    use ironhold_core::schema::catalog::{AssetCatalog, PrefabCatalog, PrefabDef, ModelCatalogEntry, PrefabKind};
    app.world_mut().insert_resource(LoadedAssetCatalog(AssetCatalog {
        models: std::collections::HashMap::from([
            ("orc".to_string(), ModelCatalogEntry { path: "shared/models/creatures/orc.glb#Scene0".to_string() }),
        ]),
        ..Default::default()
    }));
    app.world_mut().insert_resource(LoadedPrefabCatalog(PrefabCatalog {
        prefabs: std::collections::HashMap::from([
            ("enemy_orc_melee".to_string(), PrefabDef {
                kind: PrefabKind::Actor,
                model: "orc".to_string(),
                ..Default::default()
            }),
        ]),
        ..Default::default()
    }));
}

#[test]
fn test_spawn_action_assigns_spawn_id_and_registers() {
    use ironhold_core::schema::catalog::{AssetCatalog, PrefabCatalog, PrefabDef, ModelCatalogEntry, PrefabKind};

    let mut app = setup_test_app();
    app.update();

    // Provide minimal catalog entries for the orc prefab
    app.world_mut().insert_resource(LoadedAssetCatalog(AssetCatalog {
        models: std::collections::HashMap::from([
            ("orc".to_string(), ModelCatalogEntry { path: "shared/models/creatures/orc.glb#Scene0".to_string() }),
        ]),
        ..Default::default()
    }));
    app.world_mut().insert_resource(LoadedPrefabCatalog(PrefabCatalog {
        prefabs: std::collections::HashMap::from([
            ("enemy_orc_melee".to_string(), PrefabDef {
                kind: PrefabKind::Actor,
                model: "orc".to_string(),
                ..Default::default()
            }),
        ]),
        ..Default::default()
    }));

    // Spawn with an explicit ID
    app.world_mut().resource_mut::<ActionQueue>().push(
        Action::Spawn { prefab: "enemy_orc_melee".to_string(), id: Some("orc_test".to_string()), position: None, spawn_point: None, yaw_deg: None }
    );
    app.update();

    // SpawnId component should exist on the spawned entity
    let ids: Vec<String> = app.world_mut()
        .query::<&SpawnId>()
        .iter(app.world())
        .map(|s| s.0.clone())
        .collect();
    assert!(ids.contains(&"orc_test".to_string()), "SpawnId 'orc_test' should be present, got: {:?}", ids);

    // Registry should track the entity
    let registry = app.world().resource::<SpawnRegistry>();
    assert!(registry.entities.contains_key("orc_test"), "Registry should contain 'orc_test'");
}

/// Regression for the spawn-site consolidation (`tag_spawned_entity`): a dynamically-spawned
/// entity must also get `PrefabKey` (drives the targeting `target_display`) and `LevelEntity`
/// (scene-unload cleanup) — not just `SpawnId`. These were the dynamic-path gaps the shared
/// helper closed; before it, runtime `Action::Spawn` entities had neither.
#[test]
fn test_spawn_action_attaches_prefab_key_and_level_entity() {
    use ironhold_core::schema::catalog::{AssetCatalog, PrefabCatalog, PrefabDef, ModelCatalogEntry, PrefabKind};
    use ironhold_core::runtime::PrefabKey;

    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedAssetCatalog(AssetCatalog {
        models: std::collections::HashMap::from([
            ("orc".to_string(), ModelCatalogEntry { path: "shared/models/creatures/orc.glb#Scene0".to_string() }),
        ]),
        ..Default::default()
    }));
    app.world_mut().insert_resource(LoadedPrefabCatalog(PrefabCatalog {
        prefabs: std::collections::HashMap::from([
            ("enemy_orc_melee".to_string(), PrefabDef {
                kind: PrefabKind::Actor,
                model: "orc".to_string(),
                ..Default::default()
            }),
        ]),
        ..Default::default()
    }));

    app.world_mut().resource_mut::<ActionQueue>().push(
        Action::Spawn { prefab: "enemy_orc_melee".to_string(), id: Some("orc_meta".to_string()), position: None, spawn_point: None, yaw_deg: None }
    );
    app.update();

    let mut q = app.world_mut().query::<(&SpawnId, Option<&PrefabKey>, Option<&LevelEntity>)>();
    let world = app.world();
    let (_, prefab_key, level) = q.iter(world)
        .find(|(id, _, _)| id.0 == "orc_meta")
        .expect("spawned entity 'orc_meta' should exist");
    assert_eq!(
        prefab_key.map(|p| p.0.as_str()), Some("enemy_orc_melee"),
        "dynamic spawn must attach PrefabKey = prefab catalog key",
    );
    assert!(level.is_some(), "dynamic spawn must attach LevelEntity for scene-unload cleanup");
}

#[test]
fn test_spawn_auto_id_increments_counter() {
    use ironhold_core::schema::catalog::{AssetCatalog, PrefabCatalog, PrefabDef, ModelCatalogEntry, PrefabKind};

    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedAssetCatalog(AssetCatalog {
        models: std::collections::HashMap::from([
            ("orc".to_string(), ModelCatalogEntry { path: "shared/models/creatures/orc.glb#Scene0".to_string() }),
        ]),
        ..Default::default()
    }));
    app.world_mut().insert_resource(LoadedPrefabCatalog(PrefabCatalog {
        prefabs: std::collections::HashMap::from([
            ("enemy_orc_melee".to_string(), PrefabDef {
                kind: PrefabKind::Actor,
                model: "orc".to_string(),
                ..Default::default()
            }),
        ]),
        ..Default::default()
    }));

    // Spawn twice without explicit IDs
    app.world_mut().resource_mut::<ActionQueue>().push(
        Action::Spawn { prefab: "enemy_orc_melee".to_string(), id: None, position: None, spawn_point: None, yaw_deg: None }
    );
    app.world_mut().resource_mut::<ActionQueue>().push(
        Action::Spawn { prefab: "enemy_orc_melee".to_string(), id: None, position: None, spawn_point: None, yaw_deg: None }
    );
    app.update();

    let ids: Vec<String> = app.world_mut()
        .query::<&SpawnId>()
        .iter(app.world())
        .map(|s| s.0.clone())
        .collect();
    assert_eq!(ids.len(), 2, "Two entities should have been spawned");
    assert!(ids.iter().any(|id| id.starts_with("enemy_orc_melee_")), "IDs should be auto-prefixed with prefab name");

    let registry = app.world().resource::<SpawnRegistry>();
    assert_eq!(registry.counter, 2, "Counter should be 2 after two auto-spawns");
    assert_eq!(registry.entities.len(), 2);
}

#[test]
fn test_despawn_removes_entity_by_spawn_id() {
    use ironhold_core::schema::catalog::{AssetCatalog, PrefabCatalog, PrefabDef, ModelCatalogEntry, PrefabKind};

    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedAssetCatalog(AssetCatalog {
        models: std::collections::HashMap::from([
            ("orc".to_string(), ModelCatalogEntry { path: "shared/models/creatures/orc.glb#Scene0".to_string() }),
        ]),
        ..Default::default()
    }));
    app.world_mut().insert_resource(LoadedPrefabCatalog(PrefabCatalog {
        prefabs: std::collections::HashMap::from([
            ("enemy_orc_melee".to_string(), PrefabDef {
                kind: PrefabKind::Actor,
                model: "orc".to_string(),
                ..Default::default()
            }),
        ]),
        ..Default::default()
    }));

    // Spawn then despawn
    app.world_mut().resource_mut::<ActionQueue>().push(
        Action::Spawn { prefab: "enemy_orc_melee".to_string(), id: Some("doomed_orc".to_string()), position: None, spawn_point: None, yaw_deg: None }
    );
    app.update();

    assert!(app.world_mut().query::<&SpawnId>().iter(app.world()).any(|s| s.0 == "doomed_orc"));

    app.world_mut().resource_mut::<ActionQueue>().push(Action::Despawn("doomed_orc".to_string()));
    app.update(); // executor queues despawn
    app.update(); // commands flush and entity is removed

    let still_exists = app.world_mut()
        .query::<&SpawnId>()
        .iter(app.world())
        .any(|s| s.0 == "doomed_orc");
    assert!(!still_exists, "Entity 'doomed_orc' should have been despawned");

    let registry = app.world().resource::<SpawnRegistry>();
    assert!(!registry.entities.contains_key("doomed_orc"), "Registry should no longer contain 'doomed_orc'");
}

#[test]
fn test_despawn_unknown_id_does_not_panic() {
    let mut app = setup_test_app();
    app.update();

    app.world_mut().resource_mut::<ActionQueue>().push(Action::Despawn("ghost".to_string()));
    app.update(); // should warn and not panic
}

#[test]
fn test_spawn_id_collision_orphans_old_entity() {
    use ironhold_core::schema::catalog::{AssetCatalog, PrefabCatalog, PrefabDef, ModelCatalogEntry, PrefabKind};

    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedAssetCatalog(AssetCatalog {
        models: std::collections::HashMap::from([
            ("box".to_string(), ModelCatalogEntry {
                path: "shared/models/box.glb#Scene0".to_string(),
            }),
        ]),
        ..Default::default()
    }));
    app.world_mut().insert_resource(LoadedPrefabCatalog(PrefabCatalog {
        prefabs: std::collections::HashMap::from([
            ("crate".to_string(), PrefabDef {
                kind: PrefabKind::Prop,
                model: "box".to_string(),
                ..Default::default()
            }),
        ]),
        ..Default::default()
    }));

    // First spawn with explicit ID "crate_1".
    app.world_mut().resource_mut::<ActionQueue>().push(
        Action::Spawn { prefab: "crate".to_string(), id: Some("crate_1".to_string()), position: None, spawn_point: None, yaw_deg: None },
    );
    app.update();

    // Second spawn with the same ID â€” silently overwrites the registry entry.
    app.world_mut().resource_mut::<ActionQueue>().push(
        Action::Spawn { prefab: "crate".to_string(), id: Some("crate_1".to_string()), position: None, spawn_point: None, yaw_deg: None },
    );
    app.update();

    // Both entities carry SpawnId("crate_1"); registry tracks only the latest one.
    let id_count = app
        .world_mut()
        .query::<&SpawnId>()
        .iter(app.world())
        .filter(|s| s.0 == "crate_1")
        .count();
    assert_eq!(id_count, 2,
        "Both spawns should produce a SpawnId('crate_1') â€” the first entity is now orphaned");

    let registry = app.world().resource::<SpawnRegistry>();
    assert_eq!(registry.entities.len(), 1,
        "Registry must track only one entity under 'crate_1' after the collision");

    // Despawn by ID removes the registry entry but leaves the orphaned entity alive.
    app.world_mut()
        .resource_mut::<ActionQueue>()
        .push(Action::Despawn("crate_1".to_string()));
    app.update(); // executor issues despawn command
    app.update(); // command flushed

    let remaining = app
        .world_mut()
        .query::<&SpawnId>()
        .iter(app.world())
        .filter(|s| s.0 == "crate_1")
        .count();
    assert_eq!(remaining, 1,
        "After despawn the orphaned entity (not tracked by registry) must still exist");

    let registry = app.world().resource::<SpawnRegistry>();
    assert!(
        !registry.entities.contains_key("crate_1"),
        "Registry must be empty for 'crate_1' after the despawn"
    );
}

#[test]
fn test_spawn_yaw_deg_sets_transform_rotation() {
    use ironhold_core::schema::catalog::{AssetCatalog, PrefabCatalog, PrefabDef, ModelCatalogEntry, PrefabKind};

    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedAssetCatalog(AssetCatalog {
        models: HashMap::from([
            ("box".to_string(), ModelCatalogEntry { path: "shared/models/box.glb#Scene0".to_string() }),
        ]),
        ..Default::default()
    }));
    app.world_mut().insert_resource(LoadedPrefabCatalog(PrefabCatalog {
        prefabs: HashMap::from([
            ("crate".to_string(), PrefabDef {
                kind: PrefabKind::Prop,
                model: "box".to_string(),
                ..Default::default()
            }),
        ]),
        ..Default::default()
    }));

    app.world_mut().resource_mut::<ActionQueue>().push(
        Action::Spawn {
            prefab: "crate".to_string(),
            id: Some("rotated_crate".to_string()),
            position: Some((0.0, 0.0, 0.0)),
            spawn_point: None,
            yaw_deg: Some(90.0),
        }
    );
    app.update();

    let mut q = app.world_mut().query::<(&SpawnId, &Transform)>();
    let transform = q
        .iter(app.world())
        .find(|(sid, _)| sid.0 == "rotated_crate")
        .map(|(_, t)| *t)
        .expect("rotated_crate should have been spawned");

    let expected = Quat::from_rotation_y(90f32.to_radians());
    assert!(
        transform.rotation.abs_diff_eq(expected, 1e-5),
        "yaw_deg: 90 should produce a 90Â° Y-axis rotation, got {:?}",
        transform.rotation,
    );
}

#[test]
fn test_preload_prefab_stores_glb_handle() {
    let mut app = setup_test_app();
    app.update();
    minimal_orc_catalogs(&mut app);

    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::PreloadPrefab("enemy_orc_melee".to_string()));
    app.update();

    let handles = app.world().resource::<PreloadedGlbHandles>();
    assert_eq!(handles.0.len(), 1, "PreloadedGlbHandles should hold one handle after PreloadPrefab");
}

#[test]
fn test_preload_prefab_unknown_key_does_not_panic() {
    let mut app = setup_test_app();
    app.update();

    // No catalogs inserted â€” should log a warning and keep going, not panic.
    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::PreloadPrefab("nonexistent_prefab".to_string()));
    app.update();

    let handles = app.world().resource::<PreloadedGlbHandles>();
    assert_eq!(handles.0.len(), 0, "No handle should be stored for an unknown prefab key");
}

#[test]
fn test_spawn_queue_rate_limits_to_two_per_frame() {
    let mut app = setup_test_app();
    app.update();
    minimal_orc_catalogs(&mut app);

    // Queue 3 spawns â€” only 2 should be processed in the first update.
    for i in 0..3 {
        app.world_mut().resource_mut::<ActionQueue>().push(
            Action::Spawn {
                prefab: "enemy_orc_melee".to_string(),
                id: Some(format!("orc_{}", i)),
                position: None, spawn_point: None, yaw_deg: None,
            }
        );
    }
    app.update();

    let ids_after_first: Vec<String> = app.world_mut()
        .query::<&SpawnId>()
        .iter(app.world())
        .map(|s| s.0.clone())
        .collect();
    assert_eq!(ids_after_first.len(), 2, "Only 2 of 3 queued spawns should process in the first frame");

    let queue_len = app.world().resource::<PendingEntitySpawns>().0.len();
    assert_eq!(queue_len, 1, "One spawn should remain in the queue after the first frame");

    // Second update drains the remaining spawn.
    app.update();

    let ids_after_second: Vec<String> = app.world_mut()
        .query::<&SpawnId>()
        .iter(app.world())
        .map(|s| s.0.clone())
        .collect();
    assert_eq!(ids_after_second.len(), 3, "All 3 spawns should be present after the second frame");

    let queue_len = app.world().resource::<PendingEntitySpawns>().0.len();
    assert_eq!(queue_len, 0, "Queue should be empty after all spawns are drained");
}

#[test]
fn test_pending_spawns_cleared_on_load_scene() {
    let mut app = setup_test_app();
    app.update();
    minimal_orc_catalogs(&mut app);

    // Pre-populate the queue directly (simulates spawns queued in a prior frame).
    {
        use ironhold_core::runtime::QueuedSpawn;
        use ironhold_core::schema::catalog::PrefabKind;
        let mut pending = app.world_mut().resource_mut::<PendingEntitySpawns>();
        pending.0.push_back(QueuedSpawn {
            prefab_def: ironhold_core::schema::catalog::PrefabDef {
                kind: PrefabKind::Actor,
                model: "orc".to_string(),
                ..Default::default()
            },
            model_path: "shared/models/creatures/orc.glb#Scene0".to_string(),
            transform: Transform::default(),
            spawn_id: "should_be_cancelled".to_string(),
            prefab_key: "enemy_orc_melee".to_string(),
            project_root: String::new(),
            player_config: None,
        });
    }

    // LoadScene should clear the queue before the drain system runs.
    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::LoadScene("scenes/dummy.scene.ron".to_string()));
    app.update();

    let queue_len = app.world().resource::<PendingEntitySpawns>().0.len();
    assert_eq!(queue_len, 0, "LoadScene should clear PendingEntitySpawns");

    let ids: Vec<String> = app.world_mut()
        .query::<&SpawnId>()
        .iter(app.world())
        .map(|s| s.0.clone())
        .collect();
    assert!(!ids.contains(&"should_be_cancelled".to_string()), "Queued spawn should not have been executed after LoadScene");
}

#[test]
fn test_composite_prefab_with_trigger_zone_spawns_trigger_zone_component() {
    // Regression test for the composite-primitive branch of spawn_scene_v2.
    // Before the fix, trigger_zone was only inserted on the single-mesh path;
    // a composite prefab (model: "", non-empty children) silently dropped it.
    use ironhold_core::schema::catalog::{AssetCatalog, PrefabCatalog};
    use ironhold_core::capabilities::trigger_zone::TriggerZone;

    let mut app = setup_test_app();
    app.update();

    // Build a minimal catalog with a composite prefab that has trigger_zone set.
    // Deserialise the prefab catalog from RON to avoid constructing the structs by hand
    // (ChildPrimitiveDef does not derive Default).
    let prefab_ron = r#"
        (
            schema_version: 2,
            prefabs: {
                "danger_zone": (
                    kind: Primitive,
                    model: "",
                    trigger_zone: (radius: 2.0),
                    children: [
                        (
                            shape: Cuboid,
                            primitive: (size: (4.0, 0.1, 4.0)),
                        ),
                    ],
                ),
            },
        )
    "#;
    let catalog: PrefabCatalog = ron::Options::default()
        .with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME)
        .from_str(prefab_ron)
        .expect("inline prefab catalog must parse");
    app.world_mut().insert_resource(LoadedAssetCatalog(AssetCatalog::default()));
    app.world_mut().insert_resource(LoadedPrefabCatalog(catalog));

    // Insert a ProjectConfig and a scene that uses the composite prefab.
    let config_handle = app
        .world_mut()
        .resource_mut::<Assets<ProjectConfig>>()
        .add(ProjectConfig {
            schema_version: 1,
            initial_scene: "scenes/t.ron".to_string(),
            ..Default::default()
        });
    app.world_mut().insert_resource(ProjectConfigHandle(config_handle));

    let scene: GameSceneV2 = ron::Options::default()
        .with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME)
        .from_str(r#"
            (
                schema_version: 2,
                entities: [
                    ( id: "pad_01", prefab: "danger_zone", transform: () ),
                ],
                ui: [],
            )
        "#)
        .expect("test scene must parse");
    let scene_handle = app
        .world_mut()
        .resource_mut::<Assets<GameSceneV2>>()
        .add(scene);
    app.world_mut().insert_resource(SceneHandleV2(scene_handle));

    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::LoadingScene);
    app.update(); // state transitions to LoadingScene
    app.update(); // spawn_scene_v2 fires, commands queued
    app.update(); // commands flushed

    // The spawned entity must carry a TriggerZone component.
    let count = app
        .world_mut()
        .query::<&TriggerZone>()
        .iter(app.world())
        .count();
    assert_eq!(count, 1,
        "composite prefab with trigger_zone must produce exactly one entity with a TriggerZone component");
}

/// Regression guard for the GLB Actor NPC spawn path added in df8c94b.
/// A prefab with `kind: Actor` + `components.npc` must receive `NpcAgent` on
/// the spawned entity. With `animation_policy` set, `LocomotionState` must also
/// be inserted so `npc_behavior_system` can update locomotion without a panic.
#[test]
fn test_glb_actor_npc_attaches_npc_agent_and_locomotion_state() {
    use ironhold_core::schema::catalog::{
        AssetCatalog, PrefabCatalog, PrefabDef, ModelCatalogEntry, PrefabKind,
        PrefabComponents, NpcDef, NpcFaction, NpcOnPlayerNear,
    };
    use ironhold_core::capabilities::npc::NpcAgent;

    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedAssetCatalog(AssetCatalog {
        models: HashMap::from([
            ("snake".to_string(), ModelCatalogEntry { path: "shared/models/creatures/snake01.glb#Scene0".to_string() }),
        ]),
        ..Default::default()
    }));
    app.world_mut().insert_resource(LoadedPrefabCatalog(PrefabCatalog {
        prefabs: HashMap::from([
            ("enemy_snake".to_string(), PrefabDef {
                kind: PrefabKind::Actor,
                model: "snake".to_string(),
                animation_policy: Some("fake_policy.ron".to_string()),
                components: PrefabComponents {
                    npc: Some(NpcDef {
                        faction: NpcFaction::Hostile,
                        on_player_near: NpcOnPlayerNear::Chase,
                        detection_radius: 7.0,
                        chase_radius: 14.0,
                        fov_degrees: None,
                        requires_los: false,
                        approach_distance: 1.5,
                        patrol_speed: 1.5,
                        chase_speed: 3.5,
                        patrol_waypoints: vec![],
                        eye_height: 0.3,
                        alerted_duration: 0.3,
                        drag: 0.8,
                        waypoint_reach_radius: 0.5,
                        interact_leave_factor: 1.5,
                        home_arrival_radius: 0.5,
                        linear_damping: 0.5,
                        angular_damping: 0.5,
                        collider_radius: None,
                        collider_height: None,
                        investigate_timeout_secs: 5.0,
                        waypoint_wait_secs: 0.0,
                    }),
                    ..Default::default()
                },
                ..Default::default()
            }),
        ]),
        ..Default::default()
    }));

    app.world_mut().resource_mut::<ActionQueue>().push(
        Action::Spawn { prefab: "enemy_snake".to_string(), id: Some("snake_01".to_string()), position: None, spawn_point: None, yaw_deg: None }
    );
    app.update();

    let has_npc_agent = app.world_mut()
        .query::<(&SpawnId, &NpcAgent)>()
        .iter(app.world())
        .any(|(id, _)| id.0 == "snake_01");
    assert!(has_npc_agent, "GLB Actor with components.npc must attach NpcAgent");

    let has_locomotion = app.world_mut()
        .query::<(&SpawnId, &LocomotionState)>()
        .iter(app.world())
        .any(|(id, _)| id.0 == "snake_01");
    assert!(has_locomotion, "GLB Actor with animation_policy must attach LocomotionState");
}

#[test]
fn test_reset_to_spawn_teleports_npc_to_origin_and_zeros_velocity() {
    use ironhold_core::capabilities::npc::{NpcAgent, NpcState};
    use ironhold_core::schema::catalog::{NpcFaction, NpcOnPlayerNear};
    use bevy_rapier3d::prelude::Velocity;

    let mut app = setup_test_app();
    app.update();

    let origin = Vec3::new(5.0, 0.0, 3.0);

    let entity = app.world_mut().spawn((
        SpawnId("npc_01".to_string()),
        Transform::from_translation(Vec3::new(20.0, 0.0, 15.0)),
        Velocity { linvel: Vec3::new(3.0, 0.0, 2.0), angvel: Vec3::ZERO },
        NpcAgent {
            npc_id: "npc_01".to_string(),
            faction: NpcFaction::Hostile,
            on_player_near: NpcOnPlayerNear::Chase,
            detection_radius: 8.0,
            chase_radius: 16.0,
            fov_cos: -1.0,
            requires_los: false,
            approach_distance: 2.0,
            patrol_speed: 2.0,
            chase_speed: 4.0,
            waypoints: vec![],
            current_waypoint: 0,
            state: NpcState::Idle,
            target: None,
            state_timer: 0.0,
            origin,
            eye_height: 1.0,
            alerted_duration: 0.3,
            drag: 0.8,
            waypoint_reach_radius: 0.5,
            interact_leave_factor: 1.5,
            home_arrival_radius: 0.5,
            investigate_timeout_secs: 5.0,
            waypoint_wait_secs: 0.0,
            waypoint_wait_timer: 0.0,
            last_known_attacker_pos: None,
            investigate_timer: 0.0,
        },
    )).id();
    app.world_mut()
        .resource_mut::<SpawnRegistry>()
        .entities
        .insert("npc_01".to_string(), entity);

    app.world_mut()
        .resource_mut::<ActionQueue>()
        .push(Action::ResetToSpawn("npc_01".to_string()));
    app.update();

    let tf = app.world().entity(entity).get::<Transform>().unwrap();
    assert_eq!(tf.translation, origin, "entity must be teleported to NpcAgent.origin");

    let vel = app.world().entity(entity).get::<Velocity>().unwrap();
    assert_eq!(vel.linvel, Vec3::ZERO, "entity velocity must be zeroed after ResetToSpawn");
}

#[test]
fn test_set_entity_visible_hides_then_shows_spawned_entity() {
    let mut app = setup_test_app();
    app.update();

    // Manually register an entity in the SpawnRegistry and give it a Visibility component.
    let entity = app.world_mut().spawn((
        SpawnId("test_obj".to_string()),
        Visibility::Visible,
    )).id();
    app.world_mut()
        .resource_mut::<SpawnRegistry>()
        .entities
        .insert("test_obj".to_string(), entity);

    // Hide it.
    app.world_mut()
        .resource_mut::<ActionQueue>()
        .push(Action::SetEntityVisible { entity: "test_obj".to_string(), visible: false });
    app.update();

    let vis = *app.world().entity(entity).get::<Visibility>().unwrap();
    assert_eq!(vis, Visibility::Hidden, "entity must be hidden after SetEntityVisible(false)");

    // Show it again.
    app.world_mut()
        .resource_mut::<ActionQueue>()
        .push(Action::SetEntityVisible { entity: "test_obj".to_string(), visible: true });
    app.update();

    let vis = *app.world().entity(entity).get::<Visibility>().unwrap();
    assert_eq!(vis, Visibility::Visible, "entity must be visible after SetEntityVisible(true)");
}
