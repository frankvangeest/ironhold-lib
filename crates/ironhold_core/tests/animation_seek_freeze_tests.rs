//! `planning/features/dynamic_animation_control.md` — `PlayAnimationOn.start_at_fraction`/
//! `freeze` (`ActiveOverride.seek_fraction`/`.frozen`, `AnimationController.pending_seek`).
//!
//! Uses a synthetic `Gltf`/`AnimationClip`/`AnimationPolicy` (no real asset files, mirroring
//! `scene_lifecycle_tests.rs::test_animation_graph_only_includes_present_clips`) so the real
//! graph-init → `node_indices` → seek/duration-lookup path runs against a known, controllable
//! clip duration, rather than only exercising the resolver's pre-playback bookkeeping in
//! isolation. `corpse_loot_interact_tests.rs` covers the RON-authoring side of this feature
//! (the real `corpse_policy_zombie.ron` + `lootable_corpse.behavior.ron` wiring) — this file
//! covers the four correctness issues found in this feature's own pre-implementation design
//! review (see the plan file) that are specific to Bevy's animation playback mechanics.

use bevy::prelude::*;
use bevy::gltf::Gltf;
use std::collections::HashMap;

use ironhold_core::capabilities::animation::AnimationController;
use ironhold_core::capabilities::animation_resolver::{
    ActiveOverride, AnimationPolicyComponent, AnimationRequest, AnimationRequests, LocomotionState,
    resolve_initial_override,
};
use ironhold_core::schema::player::{AnimationOverrideDef, AnimationPolicy, BaseAnimations};

mod support;
use support::setup_test_app;

const CLIP_A_DURATION: f32 = 4.0;
const CLIP_B_DURATION: f32 = 2.0;

/// Builds a Gltf with two named clips ("ClipA"/"ClipB", known durations) and a policy exposing
/// each as an override id ("pose_a"/"pose_b") — mirrors the corpse's real resolution path, where
/// `PlayAnimationOn(clip: "death", ...)` matches an override *id*, not a raw glTF clip name.
fn spawn_test_entity(app: &mut App) -> Entity {
    let mut clip_a = bevy::animation::AnimationClip::default();
    clip_a.set_duration(CLIP_A_DURATION);
    let clip_a_handle = app.world_mut().resource_mut::<Assets<bevy::animation::AnimationClip>>().add(clip_a);

    let mut clip_b = bevy::animation::AnimationClip::default();
    clip_b.set_duration(CLIP_B_DURATION);
    let clip_b_handle = app.world_mut().resource_mut::<Assets<bevy::animation::AnimationClip>>().add(clip_b);

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
    {
        let mut gltfs = app.world_mut().resource_mut::<Assets<Gltf>>();
        let g = gltfs.get_mut(&gltf_handle).unwrap();
        g.named_animations.insert("ClipA".into(), clip_a_handle);
        g.named_animations.insert("ClipB".into(), clip_b_handle);
    }

    let policy = AnimationPolicy {
        base: BaseAnimations {
            idle: "ClipA".to_string(),
            walk: "ClipA".to_string(),
            run: "ClipA".to_string(),
            jump_loop: "ClipA".to_string(),
        },
        clips: HashMap::new(),
        overrides: vec![
            AnimationOverrideDef {
                id: "pose_a".to_string(),
                clip: "ClipA".to_string(),
                priority: 100,
                cancel_on_move: false,
                stop_action: None,
                looping: false,
                duration: None,
                transition_ms: None,
                start_at_fraction: None,
                freeze: false,
            },
            AnimationOverrideDef {
                id: "pose_b".to_string(),
                clip: "ClipB".to_string(),
                priority: 100,
                cancel_on_move: false,
                stop_action: None,
                looping: false,
                duration: None,
                transition_ms: None,
                start_at_fraction: None,
                freeze: false,
            },
        ],
        default_transition_ms: Some(0),
        animation_sources: vec![],
        initial_override: None,
    };

    app.world_mut()
        .spawn((
            Transform::default(),
            GlobalTransform::default(),
            AnimationPolicyComponent(policy),
            LocomotionState::default(),
            ActiveOverride::default(),
            AnimationRequests::default(),
            AnimationController {
                current: String::new(),
                last_played: String::new(),
                gltf_path: "test.glb".to_string(),
                gltf_handle,
                source_handles: Vec::new(),
                node_indices: Default::default(),
                graph_initialized: false,
                transition_ms: 0,
                should_loop: true,
                last_player_entity: None,
                pending_seek: false,
                graph_handle: None,
                awaiting_reveal: false,
                awaiting_reveal_since: None,
            },
            bevy::animation::AnimationPlayer::default(),
        ))
        .id()
}

fn push_request(app: &mut App, entity: Entity, req: AnimationRequest) {
    app.world_mut().get_mut::<AnimationRequests>(entity).unwrap().queue.push_back(req);
}

/// Resolves the entity's live `ActiveAnimation` for a given clip name, once the graph has
/// initialized (panics otherwise — every test here drives enough `app.update()`s first).
fn active_animation<'a>(
    app: &'a mut App,
    entity: Entity,
    clip_name: &str,
) -> Option<bevy::animation::ActiveAnimation> {
    let index = *app.world().get::<AnimationController>(entity).unwrap().node_indices.get(clip_name)?;
    app.world().get::<bevy::animation::AnimationPlayer>(entity).unwrap().animation(index).copied()
}

#[test]
fn override_id_branch_honors_start_at_fraction_and_freeze() {
    // The corpse's real resolution path: `PlayAnimationOn(clip: "death", ...)` matches an
    // AnimationOverrideDef.id, not a raw clip name — the branch most likely to be missed if
    // start_at_fraction/freeze were only wired into the raw-clip-name branch.
    let mut app = setup_test_app();
    app.init_asset::<bevy::animation::AnimationClip>();
    app.update();

    let entity = spawn_test_entity(&mut app);
    push_request(&mut app, entity, AnimationRequest {
        clip_or_id: "pose_a".to_string(),
        start_at_fraction: Some(0.5),
        freeze: true,
    });

    for _ in 0..4 {
        app.update();
    }

    let active = app.world().get::<ActiveOverride>(entity).unwrap();
    assert_eq!(active.clip.as_deref(), Some("ClipA"));
    assert_eq!(active.seek_fraction, Some(0.5));
    assert!(active.frozen);
    assert!(!active.looping, "freeze must force looping off");

    let anim = active_animation(&mut app, entity, "ClipA").expect("ClipA must be playing");
    assert!(anim.is_paused(), "must be paused at the requested fraction");
    assert!(
        (anim.seek_time() - 0.5 * CLIP_A_DURATION).abs() < 0.01,
        "seek_time {} should be ~{}", anim.seek_time(), 0.5 * CLIP_A_DURATION
    );
}

#[test]
fn freeze_without_start_at_fraction_pauses_at_frame_zero() {
    // Regression: `pause()` used to live inside the `if let Some(fraction) = seek_fraction`
    // block, so `freeze: true` with no `start_at_fraction` silently did nothing (found
    // independently by three review passes) — the clip played once and held its LAST frame
    // (via Bevy's own non-looping-completion behavior) rather than being frozen at frame 0 as
    // documented.
    let mut app = setup_test_app();
    app.init_asset::<bevy::animation::AnimationClip>();
    app.update();

    let entity = spawn_test_entity(&mut app);
    push_request(&mut app, entity, AnimationRequest {
        clip_or_id: "pose_a".to_string(),
        start_at_fraction: None,
        freeze: true,
    });

    for _ in 0..4 {
        app.update();
    }

    let active = app.world().get::<ActiveOverride>(entity).unwrap();
    assert_eq!(active.seek_fraction, None);
    assert!(active.frozen);

    let anim = active_animation(&mut app, entity, "ClipA").expect("ClipA must be playing");
    assert!(anim.is_paused(), "freeze: true with no start_at_fraction must still pause");
    assert!(
        anim.seek_time() < 0.01,
        "must be paused at frame 0 (no seek requested), was {}", anim.seek_time()
    );
}

#[test]
fn should_loop_false_on_a_same_node_reseek_does_not_stay_stuck_looping_forever() {
    // Regression: `AnimationPlayer::start()` (called by `transitions.play()`) reuses the
    // existing `ActiveAnimation` entry when replaying the SAME node index, and its `.replay()`
    // does not reset `repeat`. The old code only called `.repeat()` conditionally
    // (`if controller.should_loop`), with no `else` — so a node previously set to
    // `RepeatAnimation::Forever` stayed stuck looping forever even after a later play set
    // `should_loop: false`. `pending_seek` makes a same-node replay reachable for the first
    // time (previously `current != last_played` was required to replay at all), so this bug
    // was newly reachable by this feature, not merely pre-existing and untouched.
    //
    // Reproduces the exact shape: ClipA first plays via the resolver's `base.idle` fallback
    // (always `should_loop: true`, no override), THEN a `pose_a` override request for the SAME
    // clip ("ClipA") with `looping: false` and a seek fraction forces a same-node replay via
    // `pending_seek`.
    let mut app = setup_test_app();
    app.init_asset::<bevy::animation::AnimationClip>();
    app.update();

    let entity = spawn_test_entity(&mut app);
    // No request queued yet — resolver falls back to base.idle = "ClipA", should_loop: true.
    for _ in 0..4 {
        app.update();
    }
    let looping_anim = active_animation(&mut app, entity, "ClipA").expect("ClipA must be playing via base.idle fallback");
    assert_eq!(looping_anim.repeat_mode(), bevy::animation::RepeatAnimation::Forever);

    // Same clip ("ClipA"), but via the "pose_a" override (looping: false), with a seek fraction
    // so pending_seek forces the same-node replay.
    push_request(&mut app, entity, AnimationRequest {
        clip_or_id: "pose_a".to_string(),
        start_at_fraction: Some(0.5),
        freeze: false,
    });
    for _ in 0..4 {
        app.update();
    }

    let anim = active_animation(&mut app, entity, "ClipA").expect("ClipA must still be playing");
    assert_eq!(
        anim.repeat_mode(), bevy::animation::RepeatAnimation::Never,
        "must not stay stuck at Forever from the earlier looping play"
    );
    assert!(!anim.is_paused(), "freeze: false must leave it playing");
}

// ── awaiting_reveal (hide until the initial_override pose is confirmed applied) ────────────
//
// Real-playtest regression, round 2: fixing the looping bug above (via initial_override)
// stopped the corpse falling/standing/falling loop, but exposed a SEPARATE, smaller issue —
// the entity is visible in bind pose for however many frames it takes the GLTF mesh to render
// and the animation graph to initialize, before the correct pose is ever applied. Since an
// initial_override user has a KNOWN correct pose (unlike an ordinary base.idle fallback, which
// has no "wrong" state to hide), animation_policy_loader_system hides it (Visibility::Hidden,
// AnimationController.awaiting_reveal = true) the moment initial_override resolves, and
// animation_playback_system reveals it (Visibility::Inherited) the moment that pose is
// confirmed applied — not before.

#[test]
fn awaiting_reveal_entity_stays_hidden_until_the_pose_is_confirmed_applied() {
    let mut app = setup_test_app();
    app.init_asset::<bevy::animation::AnimationClip>();
    app.update();

    let entity = spawn_test_entity(&mut app);
    // Simulates exactly what animation_policy_loader_system does the moment
    // resolve_initial_override succeeds — this test bypasses that async system the same way
    // spawn_test_entity already bypasses PendingAnimationPolicy's own async resolution.
    app.world_mut().entity_mut(entity).insert(Visibility::Hidden);
    app.world_mut().get_mut::<AnimationController>(entity).unwrap().awaiting_reveal = true;
    push_request(&mut app, entity, AnimationRequest {
        clip_or_id: "pose_a".to_string(),
        start_at_fraction: Some(1.0),
        freeze: true,
    });

    // Still hidden while the graph initializes — proves the reveal isn't accidentally
    // unconditional/immediate.
    app.update();
    assert_eq!(*app.world().get::<Visibility>(entity).unwrap(), Visibility::Hidden, "must stay hidden before the pose is actually confirmed applied");

    for _ in 0..4 {
        app.update();
    }

    assert_eq!(*app.world().get::<Visibility>(entity).unwrap(), Visibility::Inherited, "must be revealed once the pose is confirmed applied");
    assert!(!app.world().get::<AnimationController>(entity).unwrap().awaiting_reveal, "awaiting_reveal must clear once revealed");
    let anim = active_animation(&mut app, entity, "ClipA").expect("ClipA must be playing");
    assert!(anim.is_paused(), "the revealed pose must actually be the correct frozen one, not an intermediate state");
}

#[test]
fn out_of_range_fraction_is_clamped() {
    let mut app = setup_test_app();
    app.init_asset::<bevy::animation::AnimationClip>();
    app.update();

    let entity = spawn_test_entity(&mut app);
    push_request(&mut app, entity, AnimationRequest {
        clip_or_id: "pose_a".to_string(),
        start_at_fraction: Some(1.5),
        freeze: true,
    });

    for _ in 0..4 {
        app.update();
    }

    let active = app.world().get::<ActiveOverride>(entity).unwrap();
    assert_eq!(active.seek_fraction, Some(1.0), "1.5 must clamp to 1.0, not pass through or panic");
}

#[test]
fn nan_fraction_does_not_poison_the_pose() {
    // f32::clamp passes NaN straight through (NaN compares false against both bounds) — RON
    // accepts a NaN literal, so this isn't hypothetical without an explicit is_finite() guard.
    let mut app = setup_test_app();
    app.init_asset::<bevy::animation::AnimationClip>();
    app.update();

    let entity = spawn_test_entity(&mut app);
    push_request(&mut app, entity, AnimationRequest {
        clip_or_id: "pose_a".to_string(),
        start_at_fraction: Some(f32::NAN),
        freeze: true,
    });

    for _ in 0..4 {
        app.update();
    }

    let active = app.world().get::<ActiveOverride>(entity).unwrap();
    assert_eq!(active.seek_fraction, Some(0.0), "NaN must launder to 0.0, not pass through");
}

#[test]
fn reseeking_the_same_already_current_clip_takes_effect() {
    // Playback only re-triggers transitions.play() on `current != last_played` — a no-op for
    // "seek the SAME clip to a different fraction" without `pending_seek` forcing the replay.
    let mut app = setup_test_app();
    app.init_asset::<bevy::animation::AnimationClip>();
    app.update();

    let entity = spawn_test_entity(&mut app);
    push_request(&mut app, entity, AnimationRequest {
        clip_or_id: "pose_a".to_string(),
        start_at_fraction: Some(0.25),
        freeze: true,
    });
    for _ in 0..4 {
        app.update();
    }
    let first = active_animation(&mut app, entity, "ClipA").unwrap();
    assert!((first.seek_time() - 0.25 * CLIP_A_DURATION).abs() < 0.01);

    // Same override id (same resolved clip "ClipA") — active.clip and controller.current do NOT
    // change, so only `pending_seek` can make this take effect.
    push_request(&mut app, entity, AnimationRequest {
        clip_or_id: "pose_a".to_string(),
        start_at_fraction: Some(0.75),
        freeze: true,
    });
    app.update();
    app.update();

    let second = active_animation(&mut app, entity, "ClipA").unwrap();
    assert!(
        (second.seek_time() - 0.75 * CLIP_A_DURATION).abs() < 0.01,
        "re-seek to a new fraction on the same clip must take effect; seek_time was {}", second.seek_time()
    );
}

#[test]
fn switching_away_from_a_frozen_clip_resumes_it_first() {
    // AnimationTransitions::play's own fade-out guard skips a paused outgoing clip entirely — a
    // frozen clip left paused would otherwise never decay out of AnimationPlayer and stay
    // permanently blended against whatever plays next.
    let mut app = setup_test_app();
    app.init_asset::<bevy::animation::AnimationClip>();
    app.update();

    let entity = spawn_test_entity(&mut app);
    push_request(&mut app, entity, AnimationRequest {
        clip_or_id: "pose_a".to_string(),
        start_at_fraction: Some(0.5),
        freeze: true,
    });
    for _ in 0..4 {
        app.update();
    }
    assert!(active_animation(&mut app, entity, "ClipA").unwrap().is_paused());

    push_request(&mut app, entity, AnimationRequest::from("pose_b"));
    for _ in 0..4 {
        app.update();
    }

    let clip_a_after = active_animation(&mut app, entity, "ClipA")
        .expect("ClipA's ActiveAnimation should still be present mid fade-out, not force-removed");
    assert!(
        !clip_a_after.is_paused(),
        "the old frozen clip must be resumed before switching away, or it never fades out"
    );
}

#[test]
fn freeze_survives_a_forced_replay_of_the_same_controller_state() {
    // Simulates animation.rs's documented GLTF-hierarchy-respawn recovery path (a WASM-specific
    // case where Bevy's SceneSpawner replaces the animated hierarchy, forcing a second
    // transitions.play() later in the entity's lifetime) by directly resetting the same fields
    // that path resets, without touching ActiveOverride — if seek/freeze were consumed-and-
    // cleared on first apply instead of durable, this second play would silently restart the
    // clip from t=0, unfrozen.
    let mut app = setup_test_app();
    app.init_asset::<bevy::animation::AnimationClip>();
    app.update();

    let entity = spawn_test_entity(&mut app);
    push_request(&mut app, entity, AnimationRequest {
        clip_or_id: "pose_a".to_string(),
        start_at_fraction: Some(0.5),
        freeze: true,
    });
    for _ in 0..4 {
        app.update();
    }
    assert!(active_animation(&mut app, entity, "ClipA").unwrap().is_paused());

    {
        let mut controller = app.world_mut().get_mut::<AnimationController>(entity).unwrap();
        controller.graph_initialized = false;
        controller.last_player_entity = None;
        controller.last_played = String::new();
    }
    for _ in 0..4 {
        app.update();
    }

    let active = app.world().get::<ActiveOverride>(entity).unwrap();
    assert_eq!(active.seek_fraction, Some(0.5), "seek_fraction must survive the forced re-init");
    assert!(active.frozen, "frozen must survive the forced re-init");

    let anim = active_animation(&mut app, entity, "ClipA").expect("ClipA must be playing again after re-init");
    assert!(anim.is_paused(), "must still be paused after the forced replay, not reset to playing from t=0");
    assert!(
        (anim.seek_time() - 0.5 * CLIP_A_DURATION).abs() < 0.01,
        "must still be seeked to the same fraction, not reset to 0.0; seek_time was {}", anim.seek_time()
    );
}

// ── AnimationPolicy.initial_override (resolve_initial_override) ────────────────────────────
//
// Real-playtest regression: without `initial_override`, a corpse's death pose relied on a
// PlayAnimationOn queued from the behavior file's entry_actions — a slower async path
// (behavior asset load -> entry_actions -> ActionQueue -> action_executor_system ->
// AnimationRequests -> next resolver tick) than the animation policy's own, more direct load.
// The corpse visibly fell, snapped back to standing as the untended base.idle loop wrapped,
// then fell again once the real request finally won. `resolve_initial_override` is a pure
// function — these are direct unit tests, no ECS/App needed.

fn policy_with_overrides(initial_override: Option<&str>, overrides: Vec<AnimationOverrideDef>) -> AnimationPolicy {
    AnimationPolicy {
        base: BaseAnimations {
            idle: "ClipA".to_string(),
            walk: "ClipA".to_string(),
            run: "ClipA".to_string(),
            jump_loop: "ClipA".to_string(),
        },
        clips: HashMap::new(),
        overrides,
        default_transition_ms: Some(0),
        animation_sources: vec![],
        initial_override: initial_override.map(|s| s.to_string()),
    }
}

fn death_override_def(start_at_fraction: Option<f32>, freeze: bool) -> AnimationOverrideDef {
    AnimationOverrideDef {
        id: "death".to_string(),
        clip: "Death01".to_string(),
        priority: 150,
        cancel_on_move: false,
        stop_action: None,
        looping: false,
        duration: None,
        transition_ms: None,
        start_at_fraction,
        freeze,
    }
}

#[test]
fn initial_override_resolves_the_matching_override_with_its_own_seek_and_freeze() {
    let policy = policy_with_overrides(Some("death"), vec![death_override_def(Some(1.0), true)]);

    let resolved = resolve_initial_override(&policy, 0.0).expect("must resolve — \"death\" exists");

    assert_eq!(resolved.clip.as_deref(), Some("Death01"));
    assert_eq!(resolved.seek_fraction, Some(1.0));
    assert!(resolved.frozen);
    assert!(!resolved.looping);
}

#[test]
fn no_initial_override_set_resolves_to_none() {
    // The overwhelming majority case — every non-corpse policy. Must be silent (no warning);
    // the caller's correct response is to fall back to base.idle exactly as before this feature.
    let policy = policy_with_overrides(None, vec![death_override_def(Some(1.0), true)]);
    assert!(resolve_initial_override(&policy, 0.0).is_none());
}

#[test]
fn initial_override_naming_a_nonexistent_id_resolves_to_none_not_a_panic() {
    // A typo in initial_override (e.g. "deth") must degrade to the same base.idle fallback as
    // leaving it unset, not panic or silently resolve to the wrong override — see
    // resolve_initial_override's own warn! for the loud half of this contract.
    let policy = policy_with_overrides(Some("deth"), vec![death_override_def(Some(1.0), true)]);
    assert!(resolve_initial_override(&policy, 0.0).is_none());
}

#[test]
fn initial_override_without_its_own_seek_fraction_still_resolves_frozen_at_frame_zero() {
    // AnimationOverrideDef.freeze is independent of start_at_fraction, same as the runtime
    // request path (see freeze_without_start_at_fraction_pauses_at_frame_zero above).
    let policy = policy_with_overrides(Some("death"), vec![death_override_def(None, true)]);

    let resolved = resolve_initial_override(&policy, 0.0).expect("must resolve — \"death\" exists");

    assert_eq!(resolved.seek_fraction, None);
    assert!(resolved.frozen);
}
