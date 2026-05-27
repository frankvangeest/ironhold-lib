use bevy::prelude::*;
use ironhold_core::runtime::{ActionQueue, LoadedAssetCatalog, SpawnId, SpawnRegistry, SceneHandleV2};
use ironhold_core::schema::{Action, AppState, ProjectConfig, ProjectConfigHandle, GameSceneV2};
use ironhold_core::schema::catalog::{
    AssetCatalog, EffectDef, EffectLightDef, EmitterShape, LayerDef, VelocityCurve,
};
use ironhold_core::capabilities::particle_renderer::ParticlePool;
use ironhold_core::capabilities::particle::PendingParticleEffects;
use ironhold_core::capabilities::fading_light::FadingLight;
use ironhold_core::capabilities::decal::PendingDecalSpawns;

mod support;
use support::setup_test_app;

// ── SpawnEffect tests ─────────────────────────────────────────────────────────

fn minimal_effect_def(particle_count: u32, lifetime_secs: f32) -> EffectDef {
    EffectDef {
        particle_count,
        lifetime_secs,
        speed: 2.0,
        speed_jitter: 0.0,
        spread_deg: 180.0,
        offset: (0.0, 0.0, 0.0),
        emit_radius: 0.0,
        size: 0.05,
        size_end: None,
        size_jitter: 0.0,
        color_start: (1.0, 1.0, 0.0, 1.0),
        color_mid: None,
        color_end: (1.0, 0.0, 0.0, 0.0),
        gravity: 0.0,
        turbulence: 0.0,
        sprite: None,
        sprites: vec![],
        additive: false,
        uv_distort: 0.0,
        uv_scroll_speed: 0.0,
        rotation_start_deg: 0.0, rotation_end_deg: 0.0, rotation_speed_deg: 0.0,
        size_x: None, size_y: None, size_x_end: None, size_y_end: None,
        emitter: Default::default(), velocity_curve: Default::default(),
        layers: vec![],
        light: None,
    }
}

#[test]
fn test_spawn_effect_with_position_spawns_particle_entities() {
    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedAssetCatalog(AssetCatalog {
        effects: std::collections::HashMap::from([("sparks".to_string(), minimal_effect_def(8, 0.5))]),
        ..Default::default()
    }));

    app.world_mut().resource_mut::<ActionQueue>().push(Action::SpawnEffect {
        key: "sparks".to_string(),
        position: Some((3.0, 1.0, -2.0)),
        entity: None,
    });
    app.update();

    let pool = app.world().resource::<ParticlePool>();
    let count = pool.particles.iter().filter(|p| p.is_alive()).count();
    assert_eq!(count, 8, "drain_particle_effects_system must add particle_count entries to the pool");
}

#[test]
fn test_spawn_effect_with_entity_resolves_global_transform() {
    let mut app = setup_test_app();
    app.update();

    let mut def = minimal_effect_def(4, 0.3);
    def.speed = 1.0;
    def.spread_deg = 90.0;
    def.offset = (0.0, 0.5, 0.0);
    def.color_start = (0.0, 1.0, 0.0, 1.0);
    def.color_end = (0.0, 0.0, 0.0, 0.0);
    app.world_mut().insert_resource(LoadedAssetCatalog(AssetCatalog {
        effects: std::collections::HashMap::from([("heal".to_string(), def)]),
        ..Default::default()
    }));

    let entity = app.world_mut().spawn((
        SpawnId("npc_01".to_string()),
        GlobalTransform::from_translation(Vec3::new(5.0, 0.0, 3.0)),
    )).id();
    app.world_mut().resource_mut::<SpawnRegistry>().entities.insert("npc_01".to_string(), entity);

    app.world_mut().resource_mut::<ActionQueue>().push(Action::SpawnEffect {
        key: "heal".to_string(),
        position: None,
        entity: Some("npc_01".to_string()),
    });
    app.update();

    let pool = app.world().resource::<ParticlePool>();
    let positions: Vec<Vec3> = pool.particles.iter()
        .filter(|p| p.is_alive())
        .map(|p| p.position)
        .collect();
    assert_eq!(positions.len(), 4, "must add particle_count pool entries for entity-based effect");
    for pos in &positions {
        assert!((pos.x - 5.0).abs() < 0.1, "particle x must be near entity x (got {})", pos.x);
        assert!((pos.y - 0.5).abs() < 0.1, "particle y must be near entity y + offset (got {})", pos.y);
        assert!((pos.z - 3.0).abs() < 0.1, "particle z must be near entity z (got {})", pos.z);
    }
}

#[test]
fn test_spawn_effect_unknown_key_does_not_push() {
    let mut app = setup_test_app();
    app.update();

    app.world_mut().resource_mut::<ActionQueue>().push(Action::SpawnEffect {
        key: "nonexistent_effect".to_string(),
        position: Some((0.0, 0.0, 0.0)),
        entity: None,
    });
    app.update();

    let pending = app.world().resource::<PendingParticleEffects>();
    assert!(pending.0.is_empty(), "unknown effect key must not push to PendingParticleEffects");
}

#[test]
fn test_spawn_effect_entity_missing_does_not_push() {
    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedAssetCatalog(AssetCatalog {
        effects: std::collections::HashMap::from([("sparks".to_string(), minimal_effect_def(4, 0.3))]),
        ..Default::default()
    }));

    app.world_mut().resource_mut::<ActionQueue>().push(Action::SpawnEffect {
        key: "sparks".to_string(),
        position: None,
        entity: Some("ghost_entity".to_string()),
    });
    app.update();

    let pending = app.world().resource::<PendingParticleEffects>();
    assert!(pending.0.is_empty(), "unresolvable entity must not push to PendingParticleEffects");
}

#[test]
fn test_spawn_effect_multi_layer_spawns_all_layer_particles() {
    let mut app = setup_test_app();
    app.update();

    let layer0 = LayerDef {
        particle_count: 4, lifetime_secs: 1.0,
        speed: 0.0, speed_jitter: 0.0, spread_deg: 180.0,
        offset: (0.0, 0.0, 0.0), emit_radius: 0.0,
        size: 0.1, size_end: None, size_jitter: 0.0,
        color_start: (1.0, 0.5, 0.0, 1.0), color_mid: None,
        color_end: (0.0, 0.0, 0.0, 0.0),
        gravity: 0.0, turbulence: 0.0,
        sprite: None, sprites: vec![], additive: true,
        uv_distort: 0.0, uv_scroll_speed: 0.0,
        rotation_start_deg: 0.0, rotation_end_deg: 0.0, rotation_speed_deg: 0.0,
        size_x: None, size_y: None, size_x_end: None, size_y_end: None,
        emitter: EmitterShape::Point, velocity_curve: VelocityCurve::Linear,
    };
    let layer1 = LayerDef {
        particle_count: 2, lifetime_secs: 0.8,
        speed: 0.0, speed_jitter: 0.0, spread_deg: 0.0,
        offset: (0.0, 0.1, 0.0), emit_radius: 0.0,
        size: 0.05, size_end: None, size_jitter: 0.0,
        color_start: (1.0, 1.0, 0.9, 1.0), color_mid: None,
        color_end: (1.0, 0.3, 0.0, 0.0),
        gravity: 0.0, turbulence: 0.0,
        sprite: None, sprites: vec![], additive: true,
        uv_distort: 0.0, uv_scroll_speed: 0.0,
        rotation_start_deg: 0.0, rotation_end_deg: 0.0, rotation_speed_deg: 0.0,
        size_x: None, size_y: None, size_x_end: None, size_y_end: None,
        emitter: EmitterShape::Point, velocity_curve: VelocityCurve::Linear,
    };
    let mut effect_def = minimal_effect_def(12, 1.0);
    effect_def.layers = vec![layer0, layer1];
    app.world_mut().insert_resource(LoadedAssetCatalog(AssetCatalog {
        effects: std::collections::HashMap::from([("campfire_fire".to_string(), effect_def)]),
        ..Default::default()
    }));

    app.world_mut().resource_mut::<ActionQueue>().push(Action::SpawnEffect {
        key: "campfire_fire".to_string(),
        position: Some((0.0, 0.0, 0.0)),
        entity: None,
    });
    app.update();

    let pool = app.world().resource::<ParticlePool>();
    let alive = pool.particles.iter().filter(|p| p.is_alive()).count();
    assert_eq!(alive, 6, "multi-layer effect must spawn layer[0].particle_count + layer[1].particle_count particles (4 + 2 = 6)");
}

#[test]
fn test_spawn_effect_with_light_spawns_point_light_entity() {
    let mut app = setup_test_app();
    app.update();

    let mut effect_def = minimal_effect_def(4, 1.0);
    effect_def.color_start = (1.0, 0.5, 0.0, 1.0);
    effect_def.light = Some(EffectLightDef {
        color: (1.0, 0.55, 0.15),
        intensity: 8000.0,
        range: 6.0,
        fade_in_secs: 0.05,
        fade_out_secs: 0.4,
        duration_secs: None,
    });
    app.world_mut().insert_resource(LoadedAssetCatalog(AssetCatalog {
        effects: std::collections::HashMap::from([("campfire".to_string(), effect_def)]),
        ..Default::default()
    }));

    app.world_mut().resource_mut::<ActionQueue>().push(Action::SpawnEffect {
        key: "campfire".to_string(),
        position: Some((0.0, 0.0, 0.0)),
        entity: None,
    });
    app.update();

    let light_count = app.world_mut().query::<&FadingLight>().iter(app.world()).count();
    assert_eq!(light_count, 1, "SpawnEffect with light block must spawn exactly one FadingLight entity");

    let point_light_count = app.world_mut().query::<&PointLight>().iter(app.world()).count();
    assert_eq!(point_light_count, 1, "FadingLight entity must have a PointLight component");
}

#[test]
fn test_spawn_effect_without_light_spawns_no_point_light() {
    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedAssetCatalog(AssetCatalog {
        effects: std::collections::HashMap::from([("no_light".to_string(), minimal_effect_def(4, 0.5))]),
        ..Default::default()
    }));

    app.world_mut().resource_mut::<ActionQueue>().push(Action::SpawnEffect {
        key: "no_light".to_string(),
        position: Some((1.0, 0.0, 1.0)),
        entity: None,
    });
    app.update();

    let light_count = app.world_mut().query::<&FadingLight>().iter(app.world()).count();
    assert_eq!(light_count, 0, "SpawnEffect without light block must not spawn any FadingLight entity");
}

// ── particles_demo smoke test ─────────────────────────────────────────────────

#[test]
fn test_particles_demo_project_config_loads() {
    let mut app = setup_test_app();
    app.update();

    let config_handle = {
        let ron_str = std::fs::read_to_string(
            "../../assets/projects/particles_demo/particles_demo.project.ron"
        ).expect("particles_demo.project.ron must be readable");
        let config: ProjectConfig = ron::Options::default()
            .with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME)
            .from_str(&ron_str)
            .expect("particles_demo project config must parse");
        app.world_mut()
            .resource_mut::<Assets<ProjectConfig>>()
            .add(config)
    };
    app.world_mut().insert_resource(ProjectConfigHandle(config_handle));

    let scene: GameSceneV2 = ron::de::from_str("(schema_version: 2, entities: [], ui: [])").unwrap();
    let scene_handle = app.world_mut().resource_mut::<Assets<GameSceneV2>>().add(scene);
    app.world_mut().insert_resource(SceneHandleV2(scene_handle));

    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::LoadingScene);
    app.update();
    app.update();
    app.update();

    let state = app.world().resource::<State<AppState>>();
    assert_ne!(*state.get(), AppState::Bootstrap,
        "particles_demo project must advance past the Bootstrap state");
}

// ── Extended particle behaviour tests ─────────────────────────────────────────

#[test]
fn test_rotation_speed_produces_nonzero_rotation_rad() {
    let mut app = setup_test_app();
    app.update();

    let layer = LayerDef {
        particle_count: 4, lifetime_secs: 1.0,
        speed: 0.0, speed_jitter: 0.0, spread_deg: 0.0,
        offset: (0.0, 0.0, 0.0), emit_radius: 0.0,
        size: 0.1, size_end: None, size_jitter: 0.0,
        color_start: (1.0, 1.0, 1.0, 1.0), color_mid: None,
        color_end: (1.0, 1.0, 1.0, 0.0),
        gravity: 0.0, turbulence: 0.0,
        sprite: None, sprites: vec![], additive: false,
        uv_distort: 0.0, uv_scroll_speed: 0.0,
        rotation_start_deg: 0.0, rotation_end_deg: 0.0,
        rotation_speed_deg: 360.0,
        size_x: None, size_y: None, size_x_end: None, size_y_end: None,
        emitter: EmitterShape::Point, velocity_curve: VelocityCurve::Linear,
    };
    let mut effect_def = minimal_effect_def(4, 1.0);
    effect_def.layers = vec![layer];
    app.world_mut().insert_resource(LoadedAssetCatalog(AssetCatalog {
        effects: std::collections::HashMap::from([("spin".to_string(), effect_def)]),
        ..Default::default()
    }));
    app.world_mut().resource_mut::<ActionQueue>().push(Action::SpawnEffect {
        key: "spin".to_string(),
        position: Some((0.0, 0.0, 0.0)),
        entity: None,
    });
    app.update();

    let pool = app.world().resource::<ParticlePool>();
    let alive: Vec<_> = pool.particles.iter().filter(|p| p.is_alive()).collect();
    assert_eq!(alive.len(), 4, "must have spawned 4 particles");

    for p in &alive {
        assert!(p.rotation_start_rad.abs() < 0.001,
            "rotation_start_rad must be 0 (rotation_start_deg=0), got {}", p.rotation_start_rad);
    }
    for p in &alive {
        let expected_end = std::f32::consts::TAU;
        assert!((p.rotation_end_rad - expected_end).abs() < 0.001,
            "rotation_end_rad must be 2π for 360 deg/s over 1 s, got {}", p.rotation_end_rad);
    }
}

#[test]
fn test_non_uniform_scale_stored_in_particle() {
    let mut app = setup_test_app();
    app.update();

    let layer = LayerDef {
        particle_count: 2, lifetime_secs: 1.0,
        speed: 0.0, speed_jitter: 0.0, spread_deg: 0.0,
        offset: (0.0, 0.0, 0.0), emit_radius: 0.0,
        size: 0.1, size_end: None, size_jitter: 0.0,
        color_start: (1.0, 1.0, 1.0, 1.0), color_mid: None,
        color_end: (1.0, 1.0, 1.0, 0.0),
        gravity: 0.0, turbulence: 0.0,
        sprite: None, sprites: vec![], additive: false,
        uv_distort: 0.0, uv_scroll_speed: 0.0,
        rotation_start_deg: 0.0, rotation_end_deg: 0.0, rotation_speed_deg: 0.0,
        size_x: Some(0.10), size_y: Some(0.50),
        size_x_end: Some(0.05), size_y_end: None,
        emitter: EmitterShape::Point, velocity_curve: VelocityCurve::Linear,
    };
    let mut effect_def = minimal_effect_def(2, 1.0);
    effect_def.layers = vec![layer];
    app.world_mut().insert_resource(LoadedAssetCatalog(AssetCatalog {
        effects: std::collections::HashMap::from([("shard".to_string(), effect_def)]),
        ..Default::default()
    }));
    app.world_mut().resource_mut::<ActionQueue>().push(Action::SpawnEffect {
        key: "shard".to_string(),
        position: Some((0.0, 0.0, 0.0)),
        entity: None,
    });
    app.update();

    let pool = app.world().resource::<ParticlePool>();
    for p in pool.particles.iter().filter(|p| p.is_alive()) {
        assert!((p.start_size_x - 0.10).abs() < 0.001, "start_size_x must be 0.10, got {}", p.start_size_x);
        assert!((p.start_size_y - 0.50).abs() < 0.001, "start_size_y must be 0.50, got {}", p.start_size_y);
        assert_eq!(p.end_size_x, Some(0.05), "end_size_x must be Some(0.05)");
        assert_eq!(p.end_size_y, None, "end_size_y must be None (falls back to layer.size_end)");
    }
}

#[test]
fn test_ring_emitter_places_particles_on_circumference() {
    let mut app = setup_test_app();
    app.update();

    let layer = LayerDef {
        particle_count: 4, lifetime_secs: 1.0,
        speed: 0.0, speed_jitter: 0.0, spread_deg: 0.0,
        offset: (0.0, 0.0, 0.0), emit_radius: 0.0,
        size: 0.1, size_end: None, size_jitter: 0.0,
        color_start: (1.0, 1.0, 1.0, 1.0), color_mid: None,
        color_end: (1.0, 1.0, 1.0, 0.0),
        gravity: 0.0, turbulence: 0.0,
        sprite: None, sprites: vec![], additive: false,
        uv_distort: 0.0, uv_scroll_speed: 0.0,
        rotation_start_deg: 0.0, rotation_end_deg: 0.0, rotation_speed_deg: 0.0,
        size_x: None, size_y: None, size_x_end: None, size_y_end: None,
        emitter: EmitterShape::Ring { radius: 2.0 }, velocity_curve: VelocityCurve::Linear,
    };
    let mut effect_def = minimal_effect_def(4, 1.0);
    effect_def.layers = vec![layer];
    app.world_mut().insert_resource(LoadedAssetCatalog(AssetCatalog {
        effects: std::collections::HashMap::from([("ring".to_string(), effect_def)]),
        ..Default::default()
    }));
    app.world_mut().resource_mut::<ActionQueue>().push(Action::SpawnEffect {
        key: "ring".to_string(),
        position: Some((0.0, 0.0, 0.0)),
        entity: None,
    });
    app.update();

    let pool = app.world().resource::<ParticlePool>();
    let alive: Vec<_> = pool.particles.iter().filter(|p| p.is_alive()).collect();
    assert_eq!(alive.len(), 4, "ring emitter with 4 particles must spawn 4 pool entries");
    for p in &alive {
        let xz_dist = (p.position.x * p.position.x + p.position.z * p.position.z).sqrt();
        assert!((xz_dist - 2.0).abs() < 0.01,
            "Ring emitter particle must be ~2.0 m from origin on XZ plane, got {:.4}", xz_dist);
    }
}

#[test]
fn test_velocity_curve_stored_in_particle() {
    let mut app = setup_test_app();
    app.update();

    let layer = LayerDef {
        particle_count: 3, lifetime_secs: 2.0,
        speed: 1.0, speed_jitter: 0.0, spread_deg: 0.0,
        offset: (0.0, 0.0, 0.0), emit_radius: 0.0,
        size: 0.1, size_end: None, size_jitter: 0.0,
        color_start: (1.0, 1.0, 1.0, 1.0), color_mid: None,
        color_end: (1.0, 1.0, 1.0, 0.0),
        gravity: 0.0, turbulence: 0.0,
        sprite: None, sprites: vec![], additive: false,
        uv_distort: 0.0, uv_scroll_speed: 0.0,
        rotation_start_deg: 0.0, rotation_end_deg: 0.0, rotation_speed_deg: 0.0,
        size_x: None, size_y: None, size_x_end: None, size_y_end: None,
        emitter: EmitterShape::Point, velocity_curve: VelocityCurve::EaseOut,
    };
    let mut effect_def = minimal_effect_def(3, 2.0);
    effect_def.speed = 1.0;
    effect_def.layers = vec![layer];
    app.world_mut().insert_resource(LoadedAssetCatalog(AssetCatalog {
        effects: std::collections::HashMap::from([("ease_test".to_string(), effect_def)]),
        ..Default::default()
    }));
    app.world_mut().resource_mut::<ActionQueue>().push(Action::SpawnEffect {
        key: "ease_test".to_string(), position: Some((0.0, 0.0, 0.0)), entity: None,
    });
    app.update();

    let pool = app.world().resource::<ParticlePool>();
    let alive: Vec<_> = pool.particles.iter().filter(|p| p.is_alive()).collect();
    assert_eq!(alive.len(), 3, "must spawn 3 particles");
    for p in &alive {
        assert_eq!(p.velocity_curve, VelocityCurve::EaseOut,
            "velocity_curve must be EaseOut as authored in LayerDef");
    }
}

// ── ProjectDecal tests ────────────────────────────────────────────────────────

#[test]
fn test_project_decal_with_position_queues_pending_spawn() {
    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedAssetCatalog(AssetCatalog {
        decals: std::collections::HashMap::from([
            ("test_ring".to_string(), "shared/textures/decals/ring_thick.png".to_string()),
        ]),
        ..Default::default()
    }));

    app.world_mut().resource_mut::<ActionQueue>().push(Action::ProjectDecal {
        key: "test_ring".to_string(),
        entity: None,
        position: Some((5.0, 0.0, -3.0)),
        radius: 2.0,
        duration_secs: 3.0,
        color: (1.0, 0.5, 0.0, 0.8),
        pulse_speed: 0.0,
    });
    app.update();

    let pending = app.world().resource::<PendingDecalSpawns>();
    assert!(pending.0.is_empty(), "spawn_decal_system must drain PendingDecalSpawns");
}

#[test]
fn test_project_decal_unknown_key_does_not_queue() {
    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedAssetCatalog(AssetCatalog {
        ..Default::default()
    }));

    app.world_mut().resource_mut::<ActionQueue>().push(Action::ProjectDecal {
        key: "nonexistent".to_string(),
        entity: None,
        position: Some((0.0, 0.0, 0.0)),
        radius: 1.0,
        duration_secs: 1.0,
        color: (1.0, 1.0, 1.0, 1.0),
        pulse_speed: 0.0,
    });
    app.update();

    let pending = app.world().resource::<PendingDecalSpawns>();
    assert!(pending.0.is_empty(), "unknown decal key must not push anything to PendingDecalSpawns");
}

#[test]
fn test_project_decal_no_position_no_entity_skips() {
    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedAssetCatalog(AssetCatalog {
        decals: std::collections::HashMap::from([
            ("test_ring".to_string(), "shared/textures/decals/ring_thick.png".to_string()),
        ]),
        ..Default::default()
    }));

    app.world_mut().resource_mut::<ActionQueue>().push(Action::ProjectDecal {
        key: "test_ring".to_string(),
        entity: None,
        position: None,
        radius: 2.0,
        duration_secs: 3.0,
        color: (1.0, 1.0, 1.0, 1.0),
        pulse_speed: 0.0,
    });
    app.update();

    let pending = app.world().resource::<PendingDecalSpawns>();
    assert!(pending.0.is_empty(), "no entity and no position must skip without queueing");
}
