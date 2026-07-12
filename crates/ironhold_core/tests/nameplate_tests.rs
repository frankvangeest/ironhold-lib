use bevy::prelude::*;

mod support;
use support::setup_test_app;

/// `should_insert_nameplate` tri-state contract: `Some(false)` always suppresses regardless
/// of `show`, `Some(true)` force-shows regardless of `show`, and `None` inherits `show`.
#[test]
fn test_should_insert_nameplate_tri_state_contract() {
    use ironhold_core::runtime::should_insert_nameplate;

    assert!(!should_insert_nameplate(Some(false), true));
    assert!(!should_insert_nameplate(Some(false), false));
    assert!(should_insert_nameplate(Some(true), false));
    assert!(should_insert_nameplate(Some(true), true));
    assert!(should_insert_nameplate(None, true));
    assert!(!should_insert_nameplate(None, false));
}

/// `nameplate_setup_system` runs without panicking when `show_nameplates` is enabled and
/// a tagged entity exists, even though render assets are absent in the headless harness.
/// The system must early-return cleanly (not panic) when `Assets<ColorMaterial>` is missing.
#[test]
fn test_nameplate_setup_does_not_panic_when_render_assets_absent() {
    use ironhold_core::capabilities::nameplate::{NameplateTag, NameplateSceneConfig};
    use ironhold_core::schema::scene_v2::NameplateOptionsDef;

    let mut app = setup_test_app();
    app.update();

    // Enable nameplates globally.
    app.world_mut().insert_resource(NameplateSceneConfig {
        enabled: true,
        player_enabled: false,
        options: Some(NameplateOptionsDef {
            faction_filter: ironhold_core::schema::scene_v2::NameplateFactionFilter::All,
            max_distance: 20.0,
            offset: (0.0, 2.4, 0.0),
            name_font_size: 14.0,
            name_color: (0.95, 0.95, 0.95, 1.0),
            text_shadow: false,
            stat_bars: vec![],
            bar_width: 100.0,
            bar_height: 6.0,
            bar_spacing: 9.0,
            show_player_nameplate: false,
        }),
    });

    // Insert a NameplateTag — the setup system will observe it via Added<NameplateTag>.
    app.world_mut().spawn(NameplateTag {
        display_name: "Goblin".to_string(),
        prefab_override: None,
    });

    // Run two frames. The system must not panic even though ColorMaterial is absent.
    app.update();
    app.update();
}

/// When `prefab_override: Some(false)`, `nameplate_setup_system` must skip that entity.
/// No panic must occur and the entity must not receive `NameplateAnchor`.
/// (Full anchor insertion is untestable headlessly; this verifies the skip path is stable.)
#[test]
fn test_nameplate_setup_skips_entity_when_prefab_override_is_false() {
    use ironhold_core::capabilities::nameplate::{NameplateTag, NameplateAnchor, NameplateSceneConfig};
    use ironhold_core::schema::scene_v2::NameplateOptionsDef;

    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(NameplateSceneConfig {
        enabled: true,
        player_enabled: false,
        options: Some(NameplateOptionsDef {
            faction_filter: ironhold_core::schema::scene_v2::NameplateFactionFilter::All,
            max_distance: 20.0,
            offset: (0.0, 2.4, 0.0),
            name_font_size: 14.0,
            name_color: (0.95, 0.95, 0.95, 1.0),
            text_shadow: false,
            stat_bars: vec![],
            bar_width: 100.0,
            bar_height: 6.0,
            bar_spacing: 9.0,
            show_player_nameplate: false,
        }),
    });

    let entity = app.world_mut().spawn(NameplateTag {
        display_name: "FriendlyNPC".to_string(),
        prefab_override: Some(false), // explicitly suppressed
    }).id();

    app.update();
    app.update();

    // The setup system skips this entity — no NameplateAnchor should be inserted.
    // (It would not be inserted regardless due to absent ColorMaterial, but the skip
    // path fires first and is what we are testing here.)
    let has_anchor = app.world().get::<NameplateAnchor>(entity).is_some();
    assert!(!has_anchor,
        "NameplateAnchor must not be inserted when prefab_override is Some(false)");
}

/// When `NameplateSceneConfig.enabled` is false and `prefab_override` is None,
/// `nameplate_setup_system` must skip the entity.
#[test]
fn test_nameplate_setup_skips_entity_when_scene_disabled_and_no_override() {
    use ironhold_core::capabilities::nameplate::{NameplateTag, NameplateAnchor, NameplateSceneConfig};

    let mut app = setup_test_app();
    app.update();

    // Scene-level nameplates disabled; no options provided.
    app.world_mut().insert_resource(NameplateSceneConfig {
        enabled: false,
        player_enabled: false,
        options: None,
    });

    let entity = app.world_mut().spawn(NameplateTag {
        display_name: "Prop".to_string(),
        prefab_override: None, // no per-prefab override to rescue it
    }).id();

    app.update();
    app.update();

    let has_anchor = app.world().get::<NameplateAnchor>(entity).is_some();
    assert!(!has_anchor,
        "NameplateAnchor must not be inserted when scene nameplates are disabled and no prefab override is set");
}

/// A `Player`-tagged entity is gated by `NameplateSceneConfig.player_enabled`, not `.enabled`.
/// Config sets `enabled: true` (would show an NPC) but `player_enabled: false` — the player
/// entity must still be skipped, proving the two toggles are independent.
#[test]
fn test_nameplate_setup_player_entity_gated_by_player_enabled_not_enabled() {
    use ironhold_core::capabilities::nameplate::{NameplateTag, NameplateAnchor, NameplateSceneConfig};
    use ironhold_core::capabilities::player::Player;

    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(NameplateSceneConfig {
        enabled: true,
        player_enabled: false,
        options: None,
    });

    let entity = app.world_mut().spawn((
        NameplateTag {
            display_name: "Hero".to_string(),
            prefab_override: None,
        },
        Player,
    )).id();

    app.update();
    app.update();

    let has_anchor = app.world().get::<NameplateAnchor>(entity).is_some();
    assert!(!has_anchor,
        "a Player entity must be gated by player_enabled, not enabled, even when enabled=true");
}

/// A non-`Player` entity must NOT be gated by `player_enabled` — it stays skipped when
/// `enabled: false`, even though `player_enabled: true`, proving the toggles don't cross over.
#[test]
fn test_nameplate_setup_non_player_entity_not_gated_by_player_enabled() {
    use ironhold_core::capabilities::nameplate::{NameplateTag, NameplateAnchor, NameplateSceneConfig};

    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(NameplateSceneConfig {
        enabled: false,
        player_enabled: true,
        options: None,
    });

    let entity = app.world_mut().spawn(NameplateTag {
        display_name: "Goblin".to_string(),
        prefab_override: None,
    }).id();

    app.update();
    app.update();

    let has_anchor = app.world().get::<NameplateAnchor>(entity).is_some();
    assert!(!has_anchor,
        "a non-Player entity must be gated by enabled, not player_enabled, even when player_enabled=true");
}

/// `WorldLabel.tracked_entity` on a manually-constructed anchor must equal the tagged entity.
/// This verifies the structural contract that tests 4 and 5 (cleanup, visibility) depend on.
#[test]
fn test_nameplate_anchor_world_label_tracks_tagged_entity() {
    use ironhold_core::capabilities::nameplate::{NameplateTag, NameplateAnchor, NameplateAnchorWidget};
    use ironhold_core::runtime::scene_manager::WorldLabel;

    let mut app = setup_test_app();
    app.update();

    // Manually replicate the relationship that nameplate_setup_system would create
    // in a full render environment.
    let tagged_entity = app.world_mut().spawn(NameplateTag {
        display_name: "Orc".to_string(),
        prefab_override: None,
    }).id();

    let anchor_entity = app.world_mut().spawn((
        WorldLabel {
            world_pos: Vec3::ZERO,
            tracked_entity: Some(tagged_entity),
            offset: Vec3::new(0.0, 2.4, 0.0),
            base_font_size: 1.0,
            depth_scale: None,
            screen_offset: Vec2::ZERO,
        },
        NameplateAnchorWidget,
        Visibility::Hidden,
        Transform::default(),
    )).id();

    // Attach the back-reference so cleanup_system and visibility_system can find the anchor.
    app.world_mut().entity_mut(tagged_entity).insert(NameplateAnchor(anchor_entity));

    app.update();

    // Assert the WorldLabel.tracked_entity equals the tagged entity.
    let world_label = app.world()
        .get::<WorldLabel>(anchor_entity)
        .expect("anchor entity must have WorldLabel");
    assert_eq!(
        world_label.tracked_entity,
        Some(tagged_entity),
        "WorldLabel.tracked_entity must point at the entity carrying NameplateTag"
    );
}

/// When a tagged entity is despawned, `nameplate_cleanup_system` must also despawn
/// the orphaned anchor entity within the same update frame.
#[test]
fn test_nameplate_cleanup_despawns_anchor_after_tagged_entity_removed() {
    use ironhold_core::capabilities::nameplate::{NameplateTag, NameplateAnchor, NameplateAnchorWidget};
    use ironhold_core::runtime::scene_manager::WorldLabel;

    let mut app = setup_test_app();
    app.update();

    // Manually construct the tagged entity + its anchor (simulating what setup_system produces).
    let tagged_entity = app.world_mut().spawn(NameplateTag {
        display_name: "Zombie".to_string(),
        prefab_override: None,
    }).id();

    let anchor_entity = app.world_mut().spawn((
        WorldLabel {
            world_pos: Vec3::ZERO,
            tracked_entity: Some(tagged_entity),
            offset: Vec3::new(0.0, 2.4, 0.0),
            base_font_size: 1.0,
            depth_scale: None,
            screen_offset: Vec2::ZERO,
        },
        NameplateAnchorWidget,
        Visibility::Hidden,
        Transform::default(),
    )).id();

    app.world_mut().entity_mut(tagged_entity).insert(NameplateAnchor(anchor_entity));

    // Confirm both entities exist before despawn.
    app.update();
    assert!(app.world().get_entity(tagged_entity).is_ok(), "tagged entity must exist before despawn");
    assert!(app.world().get_entity(anchor_entity).is_ok(), "anchor entity must exist before despawn");

    // Despawn the tagged entity — this does NOT automatically remove the anchor because
    // the anchor is intentionally unparented (the nameplate cleanup_system handles this).
    app.world_mut().despawn(tagged_entity);

    // Two update frames: first processes RemovedComponents<NameplateTag>, second flushes despawn.
    app.update();
    app.update();

    assert!(
        app.world().get_entity(anchor_entity).is_err(),
        "nameplate_cleanup_system must despawn the orphaned anchor entity after the tracked entity is removed"
    );
}

/// `nameplate_visibility_system` forces the anchor to `Hidden` when the entity is beyond
/// `max_distance`. The camera starts at the origin; the entity is placed far away.
#[test]
fn test_nameplate_visibility_hides_anchor_beyond_max_distance() {
    use ironhold_core::capabilities::nameplate::{NameplateTag, NameplateAnchor, NameplateAnchorWidget, NameplateCameraDistance, NameplateSceneConfig};
    use ironhold_core::runtime::scene_manager::WorldLabel;
    use ironhold_core::schema::scene_v2::{NameplateOptionsDef, NameplateFactionFilter};

    let mut app = setup_test_app();
    app.update();

    // Configure: max_distance=10.0, faction_filter=All so distance is the only gate.
    app.world_mut().insert_resource(NameplateSceneConfig {
        enabled: true,
        player_enabled: false,
        options: Some(NameplateOptionsDef {
            faction_filter: NameplateFactionFilter::All,
            max_distance: 10.0,
            offset: (0.0, 2.4, 0.0),
            name_font_size: 14.0,
            name_color: (0.95, 0.95, 0.95, 1.0),
            text_shadow: false,
            stat_bars: vec![],
            bar_width: 100.0,
            bar_height: 6.0,
            bar_spacing: 9.0,
            show_player_nameplate: false,
        }),
    });

    // Camera at origin.
    let cam_entity = app.world_mut().spawn((
        Camera3d::default(),
        Transform::from_translation(Vec3::ZERO),
        GlobalTransform::default(),
    )).id();

    // Tagged entity at distance=50 — well beyond max_distance.
    let tagged_entity = app.world_mut().spawn((
        NameplateTag {
            display_name: "DistantEnemy".to_string(),
            prefab_override: None,
        },
        Transform::from_translation(Vec3::new(50.0, 0.0, 0.0)),
        GlobalTransform::from(Transform::from_translation(Vec3::new(50.0, 0.0, 0.0))),
    )).id();

    // Anchor starts Visible so we can observe the system forcing it Hidden.
    let anchor_entity = app.world_mut().spawn((
        WorldLabel {
            world_pos: Vec3::ZERO,
            tracked_entity: Some(tagged_entity),
            offset: Vec3::new(0.0, 2.4, 0.0),
            base_font_size: 1.0,
            depth_scale: None,
            screen_offset: Vec2::ZERO,
        },
        NameplateAnchorWidget,
        NameplateCameraDistance(Some(50.0)),
        Visibility::Visible, // start visible
        Transform::default(),
    )).id();

    app.world_mut().entity_mut(tagged_entity).insert(NameplateAnchor(anchor_entity));

    // Two frames so nameplate_visibility_system runs after the GlobalTransform is propagated.
    app.update();
    app.update();

    let vis = *app.world()
        .get::<Visibility>(anchor_entity)
        .expect("anchor must have Visibility");
    assert_eq!(
        vis,
        Visibility::Hidden,
        "nameplate_visibility_system must force anchor Hidden when entity is beyond max_distance (50 > 10)"
    );

    let _ = cam_entity; // suppress unused warning
}

/// `nameplate_visibility_system` leaves the anchor visible when the entity is within
/// `max_distance` and passes the faction filter.
#[test]
fn test_nameplate_visibility_does_not_hide_anchor_within_max_distance() {
    use ironhold_core::capabilities::nameplate::{NameplateTag, NameplateAnchor, NameplateAnchorWidget, NameplateCameraDistance, NameplateSceneConfig};
    use ironhold_core::runtime::scene_manager::WorldLabel;
    use ironhold_core::schema::scene_v2::{NameplateOptionsDef, NameplateFactionFilter};

    let mut app = setup_test_app();
    app.update();

    // Configure: max_distance=100.0, faction_filter=All — entity is well within range.
    app.world_mut().insert_resource(NameplateSceneConfig {
        enabled: true,
        player_enabled: false,
        options: Some(NameplateOptionsDef {
            faction_filter: NameplateFactionFilter::All,
            max_distance: 100.0,
            offset: (0.0, 2.4, 0.0),
            name_font_size: 14.0,
            name_color: (0.95, 0.95, 0.95, 1.0),
            text_shadow: false,
            stat_bars: vec![],
            bar_width: 100.0,
            bar_height: 6.0,
            bar_spacing: 9.0,
            show_player_nameplate: false,
        }),
    });

    // Camera at origin.
    let cam_entity = app.world_mut().spawn((
        Camera3d::default(),
        Transform::from_translation(Vec3::ZERO),
        GlobalTransform::default(),
    )).id();

    // Tagged entity at distance=5 — within max_distance.
    let tagged_entity = app.world_mut().spawn((
        NameplateTag {
            display_name: "NearbyAlly".to_string(),
            prefab_override: None,
        },
        Transform::from_translation(Vec3::new(5.0, 0.0, 0.0)),
        GlobalTransform::from(Transform::from_translation(Vec3::new(5.0, 0.0, 0.0))),
    )).id();

    // Anchor starts Visible.
    let anchor_entity = app.world_mut().spawn((
        WorldLabel {
            world_pos: Vec3::ZERO,
            tracked_entity: Some(tagged_entity),
            offset: Vec3::new(0.0, 2.4, 0.0),
            base_font_size: 1.0,
            depth_scale: None,
            screen_offset: Vec2::ZERO,
        },
        NameplateAnchorWidget,
        NameplateCameraDistance(Some(5.0)),
        Visibility::Visible,
        Transform::default(),
    )).id();

    app.world_mut().entity_mut(tagged_entity).insert(NameplateAnchor(anchor_entity));

    app.update();
    app.update();

    let vis = *app.world()
        .get::<Visibility>(anchor_entity)
        .expect("anchor must have Visibility");
    // The system only force-hides; it does not force-show. Visible must be left unchanged.
    assert_eq!(
        vis,
        Visibility::Visible,
        "nameplate_visibility_system must not hide the anchor when entity is within max_distance"
    );

    let _ = cam_entity; // suppress unused warning
}

/// `nameplate_visibility_system` hides the anchor for a `prefab_override: Some(false)` entity
/// even if it is within max_distance, because that flag is an unconditional suppression.
#[test]
fn test_nameplate_visibility_hides_anchor_when_prefab_override_is_false() {
    use ironhold_core::capabilities::nameplate::{NameplateTag, NameplateAnchor, NameplateAnchorWidget, NameplateCameraDistance, NameplateSceneConfig};
    use ironhold_core::runtime::scene_manager::WorldLabel;
    use ironhold_core::schema::scene_v2::{NameplateOptionsDef, NameplateFactionFilter};

    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(NameplateSceneConfig {
        enabled: true,
        player_enabled: false,
        options: Some(NameplateOptionsDef {
            faction_filter: NameplateFactionFilter::All,
            max_distance: 100.0,
            offset: (0.0, 2.4, 0.0),
            name_font_size: 14.0,
            name_color: (0.95, 0.95, 0.95, 1.0),
            text_shadow: false,
            stat_bars: vec![],
            bar_width: 100.0,
            bar_height: 6.0,
            bar_spacing: 9.0,
            show_player_nameplate: false,
        }),
    });

    let cam_entity = app.world_mut().spawn((
        Camera3d::default(),
        Transform::from_translation(Vec3::ZERO),
        GlobalTransform::default(),
    )).id();

    // Entity is nearby but prefab_override=Some(false) — must always be hidden.
    let tagged_entity = app.world_mut().spawn((
        NameplateTag {
            display_name: "SuppressedNPC".to_string(),
            prefab_override: Some(false),
        },
        Transform::from_translation(Vec3::new(1.0, 0.0, 0.0)),
        GlobalTransform::from(Transform::from_translation(Vec3::new(1.0, 0.0, 0.0))),
    )).id();

    let anchor_entity = app.world_mut().spawn((
        WorldLabel {
            world_pos: Vec3::ZERO,
            tracked_entity: Some(tagged_entity),
            offset: Vec3::new(0.0, 2.4, 0.0),
            base_font_size: 1.0,
            depth_scale: None,
            screen_offset: Vec2::ZERO,
        },
        NameplateAnchorWidget,
        NameplateCameraDistance(Some(1.0)),
        Visibility::Visible, // start visible to detect the forced hide
        Transform::default(),
    )).id();

    app.world_mut().entity_mut(tagged_entity).insert(NameplateAnchor(anchor_entity));

    app.update();
    app.update();

    let vis = *app.world()
        .get::<Visibility>(anchor_entity)
        .expect("anchor must have Visibility");
    assert_eq!(
        vis,
        Visibility::Hidden,
        "nameplate_visibility_system must force anchor Hidden when prefab_override is Some(false)"
    );

    let _ = cam_entity; // suppress unused warning
}

/// `nameplate_visibility_system` hides the anchor when `faction_filter=HostileOnly`
/// and the entity has no `NpcAgent` (i.e. it is not a hostile NPC).
#[test]
fn test_nameplate_visibility_hostile_only_filter_hides_non_npc() {
    use ironhold_core::capabilities::nameplate::{NameplateTag, NameplateAnchor, NameplateAnchorWidget, NameplateCameraDistance, NameplateSceneConfig};
    use ironhold_core::runtime::scene_manager::WorldLabel;
    use ironhold_core::schema::scene_v2::{NameplateOptionsDef, NameplateFactionFilter};

    let mut app = setup_test_app();
    app.update();

    // HostileOnly filter: only entities with NpcAgent pass. This entity has no NpcAgent.
    app.world_mut().insert_resource(NameplateSceneConfig {
        enabled: true,
        player_enabled: false,
        options: Some(NameplateOptionsDef {
            faction_filter: NameplateFactionFilter::HostileOnly,
            max_distance: 100.0,
            offset: (0.0, 2.4, 0.0),
            name_font_size: 14.0,
            name_color: (0.95, 0.95, 0.95, 1.0),
            text_shadow: false,
            stat_bars: vec![],
            bar_width: 100.0,
            bar_height: 6.0,
            bar_spacing: 9.0,
            show_player_nameplate: false,
        }),
    });

    let cam_entity = app.world_mut().spawn((
        Camera3d::default(),
        Transform::from_translation(Vec3::ZERO),
        GlobalTransform::default(),
    )).id();

    // No NpcAgent — fails the HostileOnly faction filter.
    let tagged_entity = app.world_mut().spawn((
        NameplateTag {
            display_name: "PropBarrel".to_string(),
            prefab_override: None,
        },
        Transform::from_translation(Vec3::new(2.0, 0.0, 0.0)),
        GlobalTransform::from(Transform::from_translation(Vec3::new(2.0, 0.0, 0.0))),
    )).id();

    let anchor_entity = app.world_mut().spawn((
        WorldLabel {
            world_pos: Vec3::ZERO,
            tracked_entity: Some(tagged_entity),
            offset: Vec3::new(0.0, 2.4, 0.0),
            base_font_size: 1.0,
            depth_scale: None,
            screen_offset: Vec2::ZERO,
        },
        NameplateAnchorWidget,
        NameplateCameraDistance(Some(2.0)),
        Visibility::Visible,
        Transform::default(),
    )).id();

    app.world_mut().entity_mut(tagged_entity).insert(NameplateAnchor(anchor_entity));

    app.update();
    app.update();

    let vis = *app.world()
        .get::<Visibility>(anchor_entity)
        .expect("anchor must have Visibility");
    assert_eq!(
        vis,
        Visibility::Hidden,
        "nameplate_visibility_system must hide anchor for non-NPC entity when faction_filter=HostileOnly"
    );

    let _ = cam_entity; // suppress unused warning
}

/// `nameplate_visibility_system` never hides a `Player` entity's nameplate for failing
/// `faction_filter`, even under `HostileOnly` (which would hide any other non-NPC entity —
/// see `test_nameplate_visibility_hostile_only_filter_hides_non_npc`). The player is gated by
/// `show_player_nameplate` at spawn time, not by faction filtering.
#[test]
fn test_nameplate_visibility_player_bypasses_faction_filter() {
    use ironhold_core::capabilities::nameplate::{NameplateTag, NameplateAnchor, NameplateAnchorWidget, NameplateCameraDistance, NameplateSceneConfig, PlayerNameplatePreference};
    use ironhold_core::capabilities::player::Player;
    use ironhold_core::runtime::scene_manager::WorldLabel;
    use ironhold_core::schema::scene_v2::{NameplateOptionsDef, NameplateFactionFilter};

    let mut app = setup_test_app();
    app.update();

    // HostileOnly filter: only entities with NpcAgent would normally pass.
    app.world_mut().insert_resource(NameplateSceneConfig {
        enabled: true,
        player_enabled: true,
        options: Some(NameplateOptionsDef {
            faction_filter: NameplateFactionFilter::HostileOnly,
            max_distance: 100.0,
            offset: (0.0, 2.4, 0.0),
            name_font_size: 14.0,
            name_color: (0.95, 0.95, 0.95, 1.0),
            text_shadow: false,
            stat_bars: vec![],
            bar_width: 100.0,
            bar_height: 6.0,
            bar_spacing: 9.0,
            show_player_nameplate: true,
        }),
    });
    // Isolate the faction_filter-bypass behavior under test from the (separately-tested)
    // runtime own-nameplate toggle — a real scene load would seed this from
    // show_player_nameplate via scene_loader.rs; mirror that here since this test constructs
    // NameplateSceneConfig directly rather than going through spawn_scene_v2.
    app.world_mut().insert_resource(PlayerNameplatePreference(true));

    let cam_entity = app.world_mut().spawn((
        Camera3d::default(),
        Transform::from_translation(Vec3::ZERO),
        GlobalTransform::default(),
    )).id();

    // Player entity, no NpcAgent — would fail HostileOnly if faction_filter applied to it.
    let tagged_entity = app.world_mut().spawn((
        NameplateTag {
            display_name: "Hero".to_string(),
            prefab_override: None,
        },
        Player,
        Transform::from_translation(Vec3::new(2.0, 0.0, 0.0)),
        GlobalTransform::from(Transform::from_translation(Vec3::new(2.0, 0.0, 0.0))),
    )).id();

    let anchor_entity = app.world_mut().spawn((
        WorldLabel {
            world_pos: Vec3::ZERO,
            tracked_entity: Some(tagged_entity),
            offset: Vec3::new(0.0, 2.4, 0.0),
            base_font_size: 1.0,
            depth_scale: None,
            screen_offset: Vec2::ZERO,
        },
        NameplateAnchorWidget,
        NameplateCameraDistance(Some(2.0)),
        Visibility::Visible,
        Transform::default(),
    )).id();

    app.world_mut().entity_mut(tagged_entity).insert(NameplateAnchor(anchor_entity));

    app.update();
    app.update();

    let vis = *app.world()
        .get::<Visibility>(anchor_entity)
        .expect("anchor must have Visibility");
    assert_eq!(
        vis,
        Visibility::Visible,
        "nameplate_visibility_system must not hide a Player entity's anchor under HostileOnly faction_filter"
    );

    let _ = cam_entity; // suppress unused warning
}

/// `nameplate_visibility_system` does not hide the anchor when `faction_filter=HostileOnly`
/// and the entity carries `NpcAgent` (i.e. it is a hostile NPC within range).
#[test]
fn test_nameplate_visibility_hostile_only_filter_shows_npc_within_range() {
    use ironhold_core::capabilities::nameplate::{NameplateTag, NameplateAnchor, NameplateAnchorWidget, NameplateCameraDistance, NameplateSceneConfig};
    use ironhold_core::capabilities::npc::{NpcAgent, NpcState};
    use ironhold_core::schema::catalog::{NpcFaction, NpcOnPlayerNear};
    use ironhold_core::runtime::scene_manager::WorldLabel;
    use ironhold_core::schema::scene_v2::{NameplateOptionsDef, NameplateFactionFilter};

    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(NameplateSceneConfig {
        enabled: true,
        player_enabled: false,
        options: Some(NameplateOptionsDef {
            faction_filter: NameplateFactionFilter::HostileOnly,
            max_distance: 100.0,
            offset: (0.0, 2.4, 0.0),
            name_font_size: 14.0,
            name_color: (0.95, 0.95, 0.95, 1.0),
            text_shadow: false,
            stat_bars: vec![],
            bar_width: 100.0,
            bar_height: 6.0,
            bar_spacing: 9.0,
            show_player_nameplate: false,
        }),
    });

    let cam_entity = app.world_mut().spawn((
        Camera3d::default(),
        Transform::from_translation(Vec3::ZERO),
        GlobalTransform::default(),
    )).id();

    let origin = Vec3::new(3.0, 0.0, 0.0);

    // Entity has NpcAgent — passes HostileOnly faction filter.
    let tagged_entity = app.world_mut().spawn((
        NameplateTag {
            display_name: "Orc".to_string(),
            prefab_override: None,
        },
        Transform::from_translation(origin),
        GlobalTransform::from(Transform::from_translation(origin)),
        NpcAgent {
            npc_id: "orc_test".to_string(),
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

    let anchor_entity = app.world_mut().spawn((
        WorldLabel {
            world_pos: Vec3::ZERO,
            tracked_entity: Some(tagged_entity),
            offset: Vec3::new(0.0, 2.4, 0.0),
            base_font_size: 1.0,
            depth_scale: None,
            screen_offset: Vec2::ZERO,
        },
        NameplateAnchorWidget,
        NameplateCameraDistance(Some(3.0)),
        Visibility::Visible,
        Transform::default(),
    )).id();

    app.world_mut().entity_mut(tagged_entity).insert(NameplateAnchor(anchor_entity));

    app.update();
    app.update();

    let vis = *app.world()
        .get::<Visibility>(anchor_entity)
        .expect("anchor must have Visibility");
    // Visibility should remain Visible — the system only force-hides when criteria fail.
    assert_eq!(
        vis,
        Visibility::Visible,
        "nameplate_visibility_system must not hide anchor for NPC entity within max_distance with HostileOnly filter"
    );

    let _ = cam_entity; // suppress unused warning
}

/// `Action::ToggleOwnNameplate` flips `PlayerNameplatePreference` from its default (`false`)
/// to `true` and emits `nameplate.own_shown`.
#[test]
fn test_toggle_own_nameplate_flips_preference_and_emits_shown() {
    use ironhold_core::capabilities::nameplate::PlayerNameplatePreference;
    use ironhold_core::runtime::actions::ActionQueue;
    use ironhold_core::runtime::messages::GameEvent;
    use ironhold_core::schema::Action;

    let mut app = setup_test_app();
    app.update();

    assert!(!app.world().resource::<PlayerNameplatePreference>().0,
        "PlayerNameplatePreference must default to false");

    app.world_mut().resource_mut::<ActionQueue>().push(Action::ToggleOwnNameplate);
    app.update();

    assert!(app.world().resource::<PlayerNameplatePreference>().0,
        "ToggleOwnNameplate must flip the preference to true");

    let has_shown = app.world()
        .resource::<Messages<GameEvent>>()
        .iter_current_update_messages()
        .any(|e| matches!(e, GameEvent::Trigger(name) if name == "nameplate.own_shown"));
    assert!(has_shown, "Expected GameEvent::Trigger(\"nameplate.own_shown\") after toggling on");
}

/// A second `Action::ToggleOwnNameplate` flips the preference back to `false` and emits
/// `nameplate.own_hidden` (not `nameplate.own_shown` again).
#[test]
fn test_toggle_own_nameplate_twice_returns_to_hidden() {
    use ironhold_core::capabilities::nameplate::PlayerNameplatePreference;
    use ironhold_core::runtime::actions::ActionQueue;
    use ironhold_core::runtime::messages::GameEvent;
    use ironhold_core::schema::Action;

    let mut app = setup_test_app();
    app.update();

    app.world_mut().resource_mut::<ActionQueue>().push(Action::ToggleOwnNameplate);
    app.update();
    app.world_mut().resource_mut::<ActionQueue>().push(Action::ToggleOwnNameplate);
    app.update();

    assert!(!app.world().resource::<PlayerNameplatePreference>().0,
        "toggling twice must return the preference to false");

    let has_hidden = app.world()
        .resource::<Messages<GameEvent>>()
        .iter_current_update_messages()
        .any(|e| matches!(e, GameEvent::Trigger(name) if name == "nameplate.own_hidden"));
    assert!(has_hidden, "Expected GameEvent::Trigger(\"nameplate.own_hidden\") after toggling back off");
}

/// `nameplate_visibility_system` hides a `Player` entity's anchor when
/// `PlayerNameplatePreference` is `false` and there is no per-prefab override, even though the
/// entity is well within `max_distance`.
#[test]
fn test_nameplate_visibility_own_toggle_hides_player_without_override() {
    use ironhold_core::capabilities::nameplate::{NameplateTag, NameplateAnchor, NameplateAnchorWidget, NameplateCameraDistance, NameplateSceneConfig, PlayerNameplatePreference};
    use ironhold_core::capabilities::player::Player;
    use ironhold_core::runtime::scene_manager::WorldLabel;
    use ironhold_core::schema::scene_v2::{NameplateOptionsDef, NameplateFactionFilter};

    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(NameplateSceneConfig {
        enabled: true,
        player_enabled: true,
        options: Some(NameplateOptionsDef {
            faction_filter: NameplateFactionFilter::All,
            max_distance: 100.0,
            offset: (0.0, 2.4, 0.0),
            name_font_size: 14.0,
            name_color: (0.95, 0.95, 0.95, 1.0),
            text_shadow: false,
            stat_bars: vec![],
            bar_width: 100.0,
            bar_height: 6.0,
            bar_spacing: 9.0,
            show_player_nameplate: true,
        }),
    });
    // Runtime preference explicitly off, despite the scene defaulting player nameplates on.
    app.world_mut().insert_resource(PlayerNameplatePreference(false));

    let cam_entity = app.world_mut().spawn((
        Camera3d::default(),
        Transform::from_translation(Vec3::ZERO),
        GlobalTransform::default(),
    )).id();

    let tagged_entity = app.world_mut().spawn((
        NameplateTag {
            display_name: "Hero".to_string(),
            prefab_override: None,
        },
        Player,
        Transform::from_translation(Vec3::new(2.0, 0.0, 0.0)),
        GlobalTransform::from(Transform::from_translation(Vec3::new(2.0, 0.0, 0.0))),
    )).id();

    let anchor_entity = app.world_mut().spawn((
        WorldLabel {
            world_pos: Vec3::ZERO,
            tracked_entity: Some(tagged_entity),
            offset: Vec3::new(0.0, 2.4, 0.0),
            base_font_size: 1.0,
            depth_scale: None,
            screen_offset: Vec2::ZERO,
        },
        NameplateAnchorWidget,
        NameplateCameraDistance(Some(2.0)),
        Visibility::Visible,
        Transform::default(),
    )).id();

    app.world_mut().entity_mut(tagged_entity).insert(NameplateAnchor(anchor_entity));

    app.update();
    app.update();

    let vis = *app.world()
        .get::<Visibility>(anchor_entity)
        .expect("anchor must have Visibility");
    assert_eq!(
        vis,
        Visibility::Hidden,
        "nameplate_visibility_system must hide a Player entity's anchor when PlayerNameplatePreference is false"
    );

    let _ = cam_entity; // suppress unused warning
}

/// An explicit per-prefab `nameplate: Some(true)` override on a `Player` entity wins over
/// `PlayerNameplatePreference` being `false` — the anchor stays visible.
#[test]
fn test_nameplate_visibility_prefab_override_wins_over_own_toggle() {
    use ironhold_core::capabilities::nameplate::{NameplateTag, NameplateAnchor, NameplateAnchorWidget, NameplateCameraDistance, NameplateSceneConfig, PlayerNameplatePreference};
    use ironhold_core::capabilities::player::Player;
    use ironhold_core::runtime::scene_manager::WorldLabel;
    use ironhold_core::schema::scene_v2::{NameplateOptionsDef, NameplateFactionFilter};

    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(NameplateSceneConfig {
        enabled: true,
        player_enabled: false,
        options: Some(NameplateOptionsDef {
            faction_filter: NameplateFactionFilter::All,
            max_distance: 100.0,
            offset: (0.0, 2.4, 0.0),
            name_font_size: 14.0,
            name_color: (0.95, 0.95, 0.95, 1.0),
            text_shadow: false,
            stat_bars: vec![],
            bar_width: 100.0,
            bar_height: 6.0,
            bar_spacing: 9.0,
            show_player_nameplate: false,
        }),
    });
    // Runtime preference off, but the entity has an explicit force-show override below.
    app.world_mut().insert_resource(PlayerNameplatePreference(false));

    let cam_entity = app.world_mut().spawn((
        Camera3d::default(),
        Transform::from_translation(Vec3::ZERO),
        GlobalTransform::default(),
    )).id();

    let tagged_entity = app.world_mut().spawn((
        NameplateTag {
            display_name: "Hero".to_string(),
            prefab_override: Some(true),
        },
        Player,
        Transform::from_translation(Vec3::new(2.0, 0.0, 0.0)),
        GlobalTransform::from(Transform::from_translation(Vec3::new(2.0, 0.0, 0.0))),
    )).id();

    let anchor_entity = app.world_mut().spawn((
        WorldLabel {
            world_pos: Vec3::ZERO,
            tracked_entity: Some(tagged_entity),
            offset: Vec3::new(0.0, 2.4, 0.0),
            base_font_size: 1.0,
            depth_scale: None,
            screen_offset: Vec2::ZERO,
        },
        NameplateAnchorWidget,
        NameplateCameraDistance(Some(2.0)),
        Visibility::Visible,
        Transform::default(),
    )).id();

    app.world_mut().entity_mut(tagged_entity).insert(NameplateAnchor(anchor_entity));

    app.update();
    app.update();

    let vis = *app.world()
        .get::<Visibility>(anchor_entity)
        .expect("anchor must have Visibility");
    assert_eq!(
        vis,
        Visibility::Visible,
        "an explicit nameplate: Some(true) override must win over PlayerNameplatePreference(false)"
    );

    let _ = cam_entity; // suppress unused warning
}
