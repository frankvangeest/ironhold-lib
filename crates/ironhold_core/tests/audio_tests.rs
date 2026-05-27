use bevy::prelude::*;
use ironhold_core::runtime::{ActionQueue, SceneEvent, BackgroundMusic, LoadedAssetCatalog, LoadedAudioHandles};
use ironhold_core::schema::Action;
use ironhold_core::schema::catalog::{AssetCatalog, AudioEntry};

mod support;
use support::setup_test_app;

// ── PlaySound tests ───────────────────────────────────────────────────────────

#[test]
fn test_play_sound_action_spawns_audio_player() {
    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedAssetCatalog(AssetCatalog {
        audio: std::collections::HashMap::from([
            ("click".to_string(), AudioEntry { path: "shared/audio/menu-button-click.wav".to_string(), volume: 1.0 }),
        ]),
        ..Default::default()
    }));

    app.world_mut().resource_mut::<ActionQueue>().push(Action::PlaySound { key: "click".to_string(), volume: 1.0 });
    app.update();

    let count = app.world_mut()
        .query::<&bevy::audio::AudioPlayer<bevy::audio::AudioSource>>()
        .iter(app.world())
        .count();
    assert_eq!(count, 1, "Expected one AudioPlayer entity to be spawned for PlaySound");
}

#[test]
fn test_play_sound_unsupported_format_does_not_panic() {
    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedAssetCatalog(AssetCatalog {
        audio: std::collections::HashMap::from([
            ("bad".to_string(), AudioEntry { path: "shared/audio/soundtrack.aac".to_string(), volume: 1.0 }),
        ]),
        ..Default::default()
    }));

    app.world_mut().resource_mut::<ActionQueue>().push(Action::PlaySound { key: "bad".to_string(), volume: 1.0 });
    app.update();

    let count = app.world_mut()
        .query::<&bevy::audio::AudioPlayer<bevy::audio::AudioSource>>()
        .iter(app.world())
        .count();
    assert_eq!(count, 0, "Unsupported format should be rejected before spawning AudioPlayer");
}

#[test]
fn test_play_sound_missing_key_does_not_panic() {
    let mut app = setup_test_app();
    app.update();

    app.world_mut().resource_mut::<ActionQueue>().push(Action::PlaySound { key: "nonexistent".to_string(), volume: 1.0 });
    app.update();

    let count = app.world_mut()
        .query::<&bevy::audio::AudioPlayer<bevy::audio::AudioSource>>()
        .iter(app.world())
        .count();
    assert_eq!(count, 0, "No AudioPlayer should be spawned for an unknown sound key");
}

#[test]
fn test_play_sound_combined_volume_applied_to_playback_settings() {
    let mut app = setup_test_app();
    app.update();

    // catalog volume 0.5, action volume 0.5 → combined should be 0.25
    app.world_mut().insert_resource(LoadedAssetCatalog(AssetCatalog {
        audio: std::collections::HashMap::from([
            ("click".to_string(), AudioEntry { path: "shared/audio/click.wav".to_string(), volume: 0.5 }),
        ]),
        ..Default::default()
    }));

    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::PlaySound { key: "click".to_string(), volume: 0.5 });
    app.update();

    let mut q = app.world_mut()
        .query::<&bevy::audio::PlaybackSettings>();
    let settings = q.iter(app.world()).next()
        .expect("PlaybackSettings component should exist on the spawned AudioPlayer entity");
    let bevy::audio::Volume::Linear(v) = settings.volume else {
        panic!("Expected Volume::Linear");
    };
    assert!(
        (v - 0.25).abs() < 1e-5,
        "Expected combined volume 0.5 * 0.5 = 0.25, got {v}"
    );
}

#[test]
fn test_play_sound_default_volume_is_full() {
    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedAssetCatalog(AssetCatalog {
        audio: std::collections::HashMap::from([
            ("click".to_string(), AudioEntry { path: "shared/audio/click.wav".to_string(), volume: 1.0 }),
        ]),
        ..Default::default()
    }));

    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::PlaySound { key: "click".to_string(), volume: 1.0 });
    app.update();

    let mut q = app.world_mut()
        .query::<&bevy::audio::PlaybackSettings>();
    let settings = q.iter(app.world()).next()
        .expect("PlaybackSettings should exist on spawned entity");
    let bevy::audio::Volume::Linear(v) = settings.volume else {
        panic!("Expected Volume::Linear");
    };
    assert!(
        (v - 1.0).abs() < 1e-5,
        "Default volumes should produce Linear(1.0), got {v}"
    );
}

// ── Audio preload tests ───────────────────────────────────────────────────────

#[test]
fn test_preload_audio_populates_handles_on_scene_ready() {
    let mut app = setup_test_app();
    app.update();

    let mut catalog = ironhold_core::schema::catalog::AssetCatalog::default();
    catalog.audio.insert("jump".to_string(),         AudioEntry { path: "shared/audio/jump.wav".to_string(), volume: 1.0 });
    catalog.audio.insert("collect_coin".to_string(), AudioEntry { path: "shared/audio/coin.wav".to_string(), volume: 1.0 });
    app.world_mut().insert_resource(LoadedAssetCatalog(catalog));

    app.world_mut().resource_mut::<Messages<SceneEvent>>()
        .write(SceneEvent::Ready("projects/test/scenes/main.scene.ron".to_string()));
    app.update();

    let handles = app.world().resource::<LoadedAudioHandles>();
    assert_eq!(handles.0.len(), 2,
        "preload_audio_system should create one handle per catalog audio entry");
}

#[test]
fn test_preload_audio_clears_on_scene_transition() {
    let mut app = setup_test_app();
    app.update();

    let mut catalog = ironhold_core::schema::catalog::AssetCatalog::default();
    catalog.audio.insert("jump".to_string(),         AudioEntry { path: "shared/audio/jump.wav".to_string(), volume: 1.0 });
    catalog.audio.insert("collect_coin".to_string(), AudioEntry { path: "shared/audio/coin.wav".to_string(), volume: 1.0 });
    app.world_mut().insert_resource(LoadedAssetCatalog(catalog));

    app.world_mut().resource_mut::<Messages<SceneEvent>>()
        .write(SceneEvent::Ready("projects/test/scenes/scene_a.scene.ron".to_string()));
    app.update();

    app.world_mut().resource_mut::<Messages<SceneEvent>>()
        .write(SceneEvent::Ready("projects/test/scenes/scene_b.scene.ron".to_string()));
    app.update();

    let handles = app.world().resource::<LoadedAudioHandles>();
    assert_eq!(handles.0.len(), 2,
        "preload_audio_system must clear and repopulate on each Ready, not accumulate");
}

// ── PlayMusicLoop tests ───────────────────────────────────────────────────────

#[test]
fn test_play_music_loop_spawns_background_music() {
    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedAssetCatalog(AssetCatalog {
        audio: std::collections::HashMap::from([
            ("bg_music".to_string(), AudioEntry { path: "shared/audio/theme.ogg".to_string(), volume: 1.0 }),
        ]),
        ..Default::default()
    }));

    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::PlayMusicLoop { key: "bg_music".to_string(), volume: 1.0 });
    app.update();

    let count = app.world_mut()
        .query::<&BackgroundMusic>()
        .iter(app.world())
        .count();
    assert_eq!(count, 1, "PlayMusicLoop should spawn exactly one BackgroundMusic entity");
}

#[test]
fn test_play_music_loop_stops_previous_track_and_spawns_new() {
    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedAssetCatalog(AssetCatalog {
        audio: std::collections::HashMap::from([
            ("track_a".to_string(), AudioEntry { path: "shared/audio/track_a.ogg".to_string(), volume: 1.0 }),
            ("track_b".to_string(), AudioEntry { path: "shared/audio/track_b.ogg".to_string(), volume: 1.0 }),
        ]),
        ..Default::default()
    }));

    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::PlayMusicLoop { key: "track_a".to_string(), volume: 1.0 });
    app.update();
    app.update();

    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::PlayMusicLoop { key: "track_b".to_string(), volume: 1.0 });
    app.update();
    app.update();

    let count = app.world_mut()
        .query::<&BackgroundMusic>()
        .iter(app.world())
        .count();
    assert_eq!(count, 1,
        "PlayMusicLoop should replace the previous track — exactly one BackgroundMusic entity");
}

#[test]
fn test_play_music_loop_unsupported_format_does_not_panic() {
    let mut app = setup_test_app();
    app.update();

    app.world_mut().insert_resource(LoadedAssetCatalog(AssetCatalog {
        audio: std::collections::HashMap::from([
            ("bad_music".to_string(), AudioEntry { path: "shared/audio/track.aac".to_string(), volume: 1.0 }),
        ]),
        ..Default::default()
    }));

    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::PlayMusicLoop { key: "bad_music".to_string(), volume: 1.0 });
    app.update();

    let count = app.world_mut()
        .query::<&BackgroundMusic>()
        .iter(app.world())
        .count();
    assert_eq!(count, 0, "Unsupported audio format should not spawn a BackgroundMusic entity");
}

#[test]
fn test_play_music_loop_missing_key_does_not_panic() {
    let mut app = setup_test_app();
    app.update();

    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::PlayMusicLoop { key: "nonexistent_track".to_string(), volume: 1.0 });
    app.update();

    let count = app.world_mut()
        .query::<&BackgroundMusic>()
        .iter(app.world())
        .count();
    assert_eq!(count, 0, "Missing audio key should not spawn a BackgroundMusic entity");
}

#[test]
fn test_play_music_loop_combined_volume_applied_to_playback_settings() {
    let mut app = setup_test_app();
    app.update();

    // catalog 0.6 × action 0.5 = 0.3
    app.world_mut().insert_resource(LoadedAssetCatalog(AssetCatalog {
        audio: std::collections::HashMap::from([
            ("bg".to_string(), AudioEntry { path: "shared/audio/theme.ogg".to_string(), volume: 0.6 }),
        ]),
        ..Default::default()
    }));

    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::PlayMusicLoop { key: "bg".to_string(), volume: 0.5 });
    app.update();

    let mut q = app.world_mut()
        .query::<(&BackgroundMusic, &bevy::audio::PlaybackSettings)>();
    let (_, settings) = q.iter(app.world()).next()
        .expect("BackgroundMusic entity should have PlaybackSettings");
    let bevy::audio::Volume::Linear(v) = settings.volume else {
        panic!("Expected Volume::Linear");
    };
    assert!(
        (v - 0.30).abs() < 1e-5,
        "Expected combined volume 0.6 * 0.5 = 0.30, got {v}"
    );
}

#[test]
fn test_stop_music_despawns_background_music() {
    let mut app = setup_test_app();
    app.update();

    app.world_mut().spawn(BackgroundMusic);

    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::StopMusic);
    app.update();
    app.update();

    let count = app.world_mut()
        .query::<&BackgroundMusic>()
        .iter(app.world())
        .count();
    assert_eq!(count, 0, "StopMusic should despawn all BackgroundMusic entities");
}

// ── SetVolume tests ───────────────────────────────────────────────────────────

#[test]
fn test_set_volume_updates_global_volume() {
    let mut app = setup_test_app();
    app.insert_resource(bevy::audio::GlobalVolume::default());
    app.update();

    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::SetVolume(50));
    app.update();

    let gv = app.world().resource::<bevy::audio::GlobalVolume>();
    let linear = match gv.volume {
        bevy::audio::Volume::Linear(v) => v,
        _ => panic!("Expected Volume::Linear"),
    };
    assert!((linear - 0.5).abs() < 1e-5, "SetVolume(50) should set GlobalVolume to 0.5 linear");
}

#[test]
fn test_set_volume_clamped_to_100() {
    let mut app = setup_test_app();
    app.insert_resource(bevy::audio::GlobalVolume::default());
    app.update();

    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::SetVolume(150));
    app.update();

    let gv = app.world().resource::<bevy::audio::GlobalVolume>();
    let linear = match gv.volume {
        bevy::audio::Volume::Linear(v) => v,
        _ => panic!("Expected Volume::Linear"),
    };
    assert!((linear - 1.0).abs() < 1e-5, "SetVolume > 100 should clamp to 1.0 linear");
}

#[test]
fn test_set_volume_no_resource_does_not_panic() {
    let mut app = setup_test_app();
    app.update();

    app.world_mut().resource_mut::<ActionQueue>()
        .push(Action::SetVolume(80));
    app.update();
}
