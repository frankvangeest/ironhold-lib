use bevy::prelude::*;
use bevy::reflect::Reflect;
use std::collections::VecDeque;

use crate::schema::player::{
    AnimationOverrideDef,
    AnimationPolicy,
};

use crate::capabilities::animation::AnimationController;

/// Wrapper component: attach the (data) AnimationPolicy to the player entity.
#[derive(Component, Debug, Clone)]
pub struct AnimationPolicyComponent(pub AnimationPolicy);

/// Locomotion intent derived from input/movement.
/// The movement system writes this; the resolver reads it.
#[derive(Component, Reflect, Debug, Clone)]
#[reflect(Component)]
pub struct LocomotionState {
    pub moving: bool,
    pub running: bool,
    /// A *debounced* grounded signal, not the raw ground-sensor reading — written by
    /// `player_movement_system` from `raw_grounded` buffered through a short "coyote time" window
    /// (`CharacterController.coyote_time_secs`/`coyote_ticks_remaining`) so brief single-tick
    /// sensor noise (uneven terrain, a mesh seam) doesn't flicker consumers of this field. If you
    /// need the true, un-debounced sensor result (e.g. a future step-offset/auto-step feature —
    /// see `planning/backlog.md`), that's `player_movement_system`'s own `raw_grounded` local, not
    /// this field — it is not currently exposed outside that function. See
    /// `crates/ironhold_core/src/CLAUDE.md`'s "Coyote time" section for the full design.
    pub is_grounded: bool,
}

impl Default for LocomotionState {
    fn default() -> Self {
        Self {
            moving: false,
            running: false,
            is_grounded: true,
        }
    }
}

/// One queued animation command. `start_at_fraction`/`freeze` come from
/// `Action::PlayAnimationOn`'s optional fields (`None`/`false` = old behavior: play from the
/// start, keep playing). A bare clip/override/id name (from `Action::PlayAnimation`, or any
/// internal call site like a jump animation trigger) converts via `From<&str>`/`From<String>`
/// with both left at their no-op defaults.
#[derive(Debug, Clone)]
pub struct AnimationRequest {
    /// Raw command string — an override id, a semantic alias (`policy.clips` key), or a raw
    /// GLTF clip name. Resolved in that precedence order by `animation_resolver_system`.
    pub clip_or_id: String,
    pub start_at_fraction: Option<f32>,
    pub freeze: bool,
}

impl From<String> for AnimationRequest {
    fn from(clip_or_id: String) -> Self {
        Self { clip_or_id, start_at_fraction: None, freeze: false }
    }
}

impl From<&str> for AnimationRequest {
    fn from(clip_or_id: &str) -> Self {
        Self::from(clip_or_id.to_string())
    }
}

/// A small queue of animation commands.
/// These come from Action::PlayAnimation("...") / Action::PlayAnimationOn.
#[derive(Component, Debug, Default)]
pub struct AnimationRequests {
    pub queue: VecDeque<AnimationRequest>,
}

/// Runtime state for the currently-active override.
/// Single-active-clip model: one override at a time.
#[derive(Component, Reflect, Debug, Default, Clone)]
#[reflect(Component)]
pub struct ActiveOverride {
    pub id: Option<String>,
    pub clip: Option<String>,
    pub priority: i32,
    pub cancel_on_move: bool,
    pub stop_action: Option<String>,
    pub transition_ms: u64,
    pub expires_at: Option<f32>,
    pub looping: bool,
    /// Fraction (0.0–1.0, already clamped) of the clip's duration to start playback at, from
    /// the request that produced this override. `None` = start at 0.0 (unchanged pre-existing
    /// behavior). Durable — NOT consumed/cleared once applied — so it survives
    /// `animation.rs`'s GLTF-hierarchy-respawn recovery path re-triggering `transitions.play()`
    /// (see `AnimationController.pending_seek` doc comment for why a one-shot version of this
    /// would silently un-freeze a corpse on the web).
    pub seek_fraction: Option<f32>,
    /// If `true`, `animation_playback_system` pauses the clip exactly at `seek_fraction` instead
    /// of letting it continue playing. Forces `looping = false` when set (see
    /// `apply_seek_and_freeze`).
    pub frozen: bool,
}

impl ActiveOverride {
    fn clear(&mut self) {
        // `*self = Self::default()` rather than hand-listing every field: a cleared override is
        // defined as "identical to a freshly-spawned one" (both have `clip: None`, so `looping`
        // etc. are never actually read either way — see step 4 below), and this makes that
        // definition structural instead of something that can silently drift the next time a
        // field is added to this struct (as already happened once, when `seek_fraction`/`frozen`
        // were added alongside a hand-written `clear()` that had to be remembered separately).
        *self = Self::default();
    }
}

pub(crate) fn build_active_from_def(def: &AnimationOverrideDef, now: f32, default_transition_ms: u64) -> ActiveOverride {
    ActiveOverride {
        id: Some(def.id.clone()),
        clip: Some(def.clip.clone()),
        priority: def.priority,
        cancel_on_move: def.cancel_on_move,
        stop_action: def.stop_action.clone(),
        transition_ms: def.transition_ms.unwrap_or(default_transition_ms),
        expires_at: def.duration.map(|d| now + d),
        looping: def.looping,
        seek_fraction: None,
        frozen: false,
    }
}

/// Shared finalization step for all three `ActiveOverride`-construction branches below
/// (override-id lookup, semantic `clips` alias, raw clip name) — merges the requesting
/// `AnimationRequest`'s `start_at_fraction`/`freeze` into a freshly-built candidate. Factored
/// into one function specifically so a future field on `AnimationRequest` can't be wired into
/// only one or two of the three branches by accident (found in pre-implementation review: the
/// corpse use case resolves via the override-id branch, not the more obvious-looking raw-clip
/// branch, so an implementation that only touched the latter would silently drop the feature's
/// own primary use case).
pub(crate) fn apply_seek_and_freeze(mut candidate: ActiveOverride, req: &AnimationRequest) -> ActiveOverride {
    if let Some(fraction) = req.start_at_fraction {
        // f32::clamp only panics on an inverted range; it passes NaN straight through (NaN
        // compares false against both bounds), which would otherwise reach `set_seek_time` and
        // produce a NaN-poisoned pose. RON accepts a `NaN` literal, so this isn't hypothetical.
        let clamped = if fraction.is_finite() { fraction.clamp(0.0, 1.0) } else { 0.0 };
        if clamped != fraction {
            warn!(
                "PlayAnimationOn: start_at_fraction {} for clip {:?} is outside [0.0, 1.0] — clamping to {}",
                fraction, candidate.clip, clamped
            );
        }
        candidate.seek_fraction = Some(clamped);
    }

    candidate.frozen = req.freeze;
    if req.freeze {
        // Pausing and looping are contradictory; freeze always wins.
        candidate.looping = false;
    }

    if candidate.looping {
        if let Some(fraction) = candidate.seek_fraction {
            if fraction >= 1.0 {
                warn!(
                    "PlayAnimationOn: start_at_fraction {} on a looping clip {:?} wraps to ~0.0 \
                    (seek_time %= clip_duration) — likely not what was intended outside a one-shot pose",
                    fraction, candidate.clip
                );
            }
        }
    }

    candidate
}

/// Resolves `AnimationPolicy.initial_override` (if set and valid) into the `ActiveOverride` it
/// should produce — applied synchronously at policy-attach time by
/// `entity_spawner.rs::animation_policy_loader_system` (and mirrored by test harnesses that
/// bypass that system's async loading, e.g. `corpse_loot_interact_tests.rs::spawn_real_corpse`),
/// instead of waiting for a `PlayAnimationOn` request to arrive via the slower behavior-file/
/// action-pipeline path. Returns `None` both when `initial_override` is unset (silently — the
/// normal case for every non-corpse policy) and when it names an override that doesn't exist
/// (loudly, via `warn!` — almost certainly a typo); either way the caller's correct fallback is
/// the same: use `policy.base.idle`.
pub fn resolve_initial_override(policy: &AnimationPolicy, now: f32) -> Option<ActiveOverride> {
    let id = policy.initial_override.as_ref()?;
    match policy.overrides.iter().find(|d| &d.id == id) {
        Some(def) => {
            let default_transition_ms = policy.default_transition_ms.unwrap_or(0);
            let synthetic_req = AnimationRequest {
                clip_or_id: id.clone(),
                start_at_fraction: def.start_at_fraction,
                freeze: def.freeze,
            };
            Some(apply_seek_and_freeze(build_active_from_def(def, now, default_transition_ms), &synthetic_req))
        }
        None => {
            warn!(
                "AnimationPolicy.initial_override {:?} does not match any override id — falling back to base.idle",
                id
            );
            None
        }
    }
}

/// Resolve locomotion + requests into a single `AnimationController.current`.
///
/// Field ownership (see also `animation_playback_system`'s doc comment):
/// - This system owns `AnimationController.current`/`transition_ms`/`should_loop` and sets
///   `pending_seek = true` — the ONE exception is `animation.rs`'s missing-node-index recovery
///   path, which also writes `current` (to fall back to idle) as a last-resort safety net; that
///   path does not go through this resolver at all.
/// - `animation_playback_system` owns `last_played`/`graph_initialized`/`node_indices`/
///   `last_player_entity`, and clears `pending_seek` once a pending seek/freeze has been applied.
pub fn animation_resolver_system(
    time: Res<Time>,
    mut query: Query<(
        &AnimationPolicyComponent,
        &LocomotionState,
        &mut AnimationRequests,
        &mut ActiveOverride,
        &mut AnimationController,
    )>,
) {
    let now = time.elapsed_secs();

    for (policy_comp, loco, mut requests, mut active, mut anim_ctrl) in &mut query {
        let policy = &policy_comp.0;
        let default_transition_ms = policy.default_transition_ms.unwrap_or(0);

        // 1) Expire time-based overrides.
        if let Some(exp) = active.expires_at {
            if now >= exp {
                active.clear();
            }
        }

        // 2) Cancel-on-move.
        if loco.moving && active.cancel_on_move {
            active.clear();
        }

        // 3) Apply queued commands.
        while let Some(req) = requests.queue.pop_front() {
            let cmd = req.clip_or_id.clone();
            // Only a seek/freeze request needs to force a replay of an already-current clip
            // (animation.rs's `current != last_played || pending_seek` gate) — an ordinary
            // re-request of the same override (e.g. rapid "attack_light" re-presses) keeps its
            // pre-existing behavior of NOT restarting the clip, since nothing here actually
            // asked for that.
            let wants_seek = req.start_at_fraction.is_some() || req.freeze;

            // If this command is declared as a stop_action on any policy override it is a
            // sentinel value, not a real clip name. Clear the active override when it matches,
            // then drop the command so it never reaches the raw-clip branch and pollutes
            // `controller.current` with an unplayable name.
            let is_stop_sentinel = policy.overrides.iter()
                .any(|d| d.stop_action.as_deref() == Some(cmd.as_str()));
            if is_stop_sentinel {
                if active.stop_action.as_deref() == Some(cmd.as_str()) {
                    active.clear();
                }
                continue;
            }

            // override id
            if let Some(def) = policy.overrides.iter().find(|d| d.id == cmd) {
                let candidate = apply_seek_and_freeze(build_active_from_def(def, now, default_transition_ms), &req);
                if active.clip.is_none() || candidate.priority >= active.priority {
                    *active = candidate;
                    // Assign, not just set-if-true: if a non-seek request wins the priority
                    // gate in the same frame right after a seek request lost it (or vice versa),
                    // pending_seek must reflect the WINNING candidate, not linger true/false from
                    // whatever was processed earlier this same resolver pass.
                    anim_ctrl.pending_seek = wants_seek;
                } else {
                    debug!("Animation override {:?} dropped — priority {} < active priority {}", cmd, candidate.priority, active.priority);
                }
                continue;
            }

            // semantic alias
            if let Some(clip) = policy.clips.get(&cmd) {
                let candidate = apply_seek_and_freeze(ActiveOverride {
                    id: Some(cmd.clone()),
                    clip: Some(clip.clone()),
                    priority: 0,
                    cancel_on_move: false,
                    stop_action: None,
                    transition_ms: default_transition_ms,
                    expires_at: None,
                    looping: true,
                    seek_fraction: None,
                    frozen: false,
                }, &req);
                if active.clip.is_none() || candidate.priority >= active.priority {
                    *active = candidate;
                    anim_ctrl.pending_seek = wants_seek;
                } else {
                    debug!("Animation alias {:?} dropped — priority {} < active priority {}", cmd, candidate.priority, active.priority);
                }
                continue;
            }

            // raw clip name
            let candidate = apply_seek_and_freeze(ActiveOverride {
                id: None,
                clip: Some(cmd.clone()),
                priority: 0,
                cancel_on_move: false,
                stop_action: None,
                transition_ms: default_transition_ms,
                expires_at: None,
                looping: true,
                seek_fraction: None,
                frozen: false,
            }, &req);
            if active.clip.is_none() || candidate.priority >= active.priority {
                *active = candidate;
                anim_ctrl.pending_seek = wants_seek;
            } else {
                debug!("Animation clip {:?} dropped — priority {} < active priority {}", cmd, candidate.priority, active.priority);
            }
        }

        // 4) Choose final clip + metadata.
        let (mut chosen_clip, chosen_looping, chosen_transition_ms) = if let Some(clip) = &active.clip {
            (clip.clone(), active.looping, active.transition_ms)
        } else if !loco.is_grounded {
            (policy.base.jump_loop.clone(), true, default_transition_ms)
        } else if loco.moving {
            let clip = if loco.running {
                policy.base.run.clone()
            } else {
                policy.base.walk.clone()
            };
            (clip, true, default_transition_ms)
        } else {
            (policy.base.idle.clone(), true, default_transition_ms)
        };

        // 4b) Validate against the graph. Once graph_initialized=true, node_indices is
        // populated with only the clips that actually exist in the GLB. If the chosen clip
        // is absent (e.g. a raw-clip-name override from an unrecognised PlayAnimation
        // command), clear the bad override and fall back to idle to prevent the playback
        // system's "permanent trap" from freezing all animations in T-pose.
        if anim_ctrl.graph_initialized
            && !anim_ctrl.node_indices.is_empty()
            && !anim_ctrl.node_indices.contains_key(&chosen_clip)
        {
            warn!(
                "Animation clip {:?} not found in graph ({}) — clearing override, falling back to idle",
                chosen_clip, anim_ctrl.gltf_path
            );
            active.clear();
            chosen_clip = policy.base.idle.clone();
        }

        // 5) Write current + transition settings (single-writer).
        anim_ctrl.transition_ms = chosen_transition_ms;
        anim_ctrl.should_loop = chosen_looping;
        if anim_ctrl.current != chosen_clip {
            anim_ctrl.current = chosen_clip;
        }
    }
}
