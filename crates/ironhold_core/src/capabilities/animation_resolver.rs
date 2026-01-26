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
#[derive(Component, Reflect, Debug, Default, Clone)]
#[reflect(Component)]
pub struct LocomotionState {
    pub moving: bool,
    pub running: bool,
}

/// A small queue of animation commands (strings).
/// These come from Action::PlayAnimation("...").
#[derive(Component, Debug, Default)]
pub struct AnimationRequests {
    pub queue: VecDeque<String>,
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
    pub expires_at: Option<f32>,
    pub looping: bool,
}

impl ActiveOverride {
    fn clear(&mut self) {
        self.id = None;
        self.clip = None;
        self.priority = 0;
        self.cancel_on_move = false;
        self.stop_action = None;
        self.expires_at = None;
        self.looping = true;
    }
}

fn build_active_from_def(def: &AnimationOverrideDef, now: f32) -> ActiveOverride {
    ActiveOverride {
        id: Some(def.id.clone()),
        clip: Some(def.clip.clone()),
        priority: def.priority,
        cancel_on_move: def.cancel_on_move,
        stop_action: def.stop_action.clone(),
        expires_at: def.duration.map(|d| now + d),
        looping: def.looping,
    }
}

/// Resolve locomotion + requests into a single `AnimationController.current`.
///
/// IMPORTANT: this system should be the only writer of `AnimationController.current`.
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
        while let Some(cmd) = requests.queue.pop_front() {
            // stop action cancels active override
            if let Some(stop) = &active.stop_action {
                if stop == &cmd {
                    active.clear();
                    continue;
                }
            }

            // override id
            if let Some(def) = policy.overrides.iter().find(|d| d.id == cmd) {
                let candidate = build_active_from_def(def, now);
                if active.clip.is_none() || candidate.priority >= active.priority {
                    *active = candidate;
                }
                continue;
            }

            // semantic alias
            if let Some(clip) = policy.clips.get(&cmd) {
                let candidate = ActiveOverride {
                    id: Some(cmd.clone()),
                    clip: Some(clip.clone()),
                    priority: 0,
                    cancel_on_move: false,
                    stop_action: None,
                    expires_at: None,
                    looping: true,
                };
                if active.clip.is_none() || candidate.priority >= active.priority {
                    *active = candidate;
                }
                continue;
            }

            // raw clip name
            let candidate = ActiveOverride {
                id: None,
                clip: Some(cmd),
                priority: 0,
                cancel_on_move: false,
                stop_action: None,
                expires_at: None,
                looping: true,
            };
            if active.clip.is_none() || candidate.priority >= active.priority {
                *active = candidate;
            }
        }

        // 4) Choose final clip.
        let chosen_clip = if let Some(clip) = &active.clip {
            clip.clone()
        } else if loco.moving {
            if loco.running {
                policy.base.run.clone()
            } else {
                policy.base.walk.clone()
            }
        } else {
            policy.base.idle.clone()
        };

        // 5) Write current (single-writer).
        if anim_ctrl.current != chosen_clip {
            anim_ctrl.current = chosen_clip;
        }
    }
}

