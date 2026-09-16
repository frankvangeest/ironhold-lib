use std::path::PathBuf;
use bevy::log::info;
use bevy::prelude::{ChildOf, GlobalTransform, Transform};

/// Returns a `GlobalTransform` guaranteed fresh for *this* frame/tick, for an entity known to be
/// root-level (no parent) at the call site — the caller must actually query
/// `Option<&ChildOf>` and pass the real value; never hardcode `None` as a shortcut, since that
/// would make a parented entity's *local* `Transform` get treated as its world transform (wrong,
/// not just stale). Every real call site today (players, cameras, NPC actors) is genuinely
/// root-level: the one place this engine deliberately spawns a gameplay-tracked child as a root
/// instead of parenting it is nested GLB Actor/Prop prefabs (`scene_loader.rs`, for an unrelated
/// Rapier-init-ordering reason, not this one) — nested *Primitive* prefab anchors and their
/// children genuinely are parented (`add_child`), and correctly take this function's fallback
/// path.
///
/// Bevy's own `PostUpdate` propagation pass still runs every rendered frame regardless of any of
/// this — reading `GlobalTransform` from an `Update`-scheduled system has *always* been one frame
/// behind the `Transform` a same-frame `Update` system just wrote (pre-existing, not introduced
/// by the fixed-timestep feature below). What changed is that a physics-driven entity's
/// `GlobalTransform` used to go stale in lockstep with the render-driving camera's own staleness
/// (both were exactly one *rendered frame* behind), so the errors cancelled; once physics moved
/// to a fixed 64Hz tick separate from the render/camera cadence
/// (`planning/features/deterministic_fixed_timestep.md`, v1), a frame that advances physics by
/// two ticks breaks that cancellation and a rigid screen-space projection (world-space UI, a
/// camera's `look_at`) pops by one tick of motion. Found as the root cause of a nameplate/
/// health-bar stutter regression during that feature's v1 playtest — see the plan file for the
/// full derivation. Do not call this in a physics-timing context (e.g. `ground_cast`) — the
/// `mark_dirty_trees` addition to the `FixedUpdate` chain is the correct, complete fix there
/// instead, since that context needs the propagation to actually happen, not to be bypassed.
///
/// For a root entity, `GlobalTransform::from(*transform)` is exactly what propagation would
/// compute, with zero lag — not an approximation. Falls back to the real `GlobalTransform`
/// component when the entity has a parent; that fallback is genuinely stale-and-wrong for this
/// frame's render (not "stale but correct"), it's simply the best available answer without doing
/// full propagation ourselves, and no real call site hits it today.
///
/// `transform` is `Option`, not a hard requirement: `Transform` implies `GlobalTransform` via
/// Bevy's required-components (every gameplay-spawned entity has both), but the reverse doesn't
/// hold — an entity can carry `GlobalTransform` alone (several test fixtures do exactly this for
/// simplicity). Making `&Transform` a hard query requirement at a call site would silently drop
/// such an entity out of the query *entirely*, not just skip the freshness optimization — a real
/// regression caught by the existing test suite when this fix was first written requiring
/// `&Transform` instead of `Option<&Transform>`.
pub fn fresh_global_transform(
    transform: Option<&Transform>,
    global: &GlobalTransform,
    child_of: Option<&ChildOf>,
) -> GlobalTransform {
    match (transform, child_of) {
        (Some(t), None) => GlobalTransform::from(*t),
        _ => *global,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::Entity;

    #[test]
    fn root_entity_with_transform_is_freshened() {
        let t = Transform::from_xyz(1.0, 2.0, 3.0);
        let stale = GlobalTransform::from(Transform::from_xyz(0.0, 0.0, 0.0));
        let fresh = fresh_global_transform(Some(&t), &stale, None);
        assert_eq!(fresh, GlobalTransform::from(t));
    }

    #[test]
    fn root_entity_without_transform_falls_back_to_global() {
        let stale = GlobalTransform::from(Transform::from_xyz(5.0, 6.0, 7.0));
        let result = fresh_global_transform(None, &stale, None);
        assert_eq!(result, stale);
    }

    #[test]
    fn parented_entity_with_transform_falls_back_to_global_not_local() {
        // A parented entity's `Transform` is local-space — using it directly would be wrong,
        // not just stale, so this must fall back to the (possibly stale) real `GlobalTransform`.
        let local = Transform::from_xyz(100.0, 100.0, 100.0);
        let world = GlobalTransform::from(Transform::from_xyz(1.0, 2.0, 3.0));
        let child_of = ChildOf(Entity::PLACEHOLDER);
        let result = fresh_global_transform(Some(&local), &world, Some(&child_of));
        assert_eq!(result, world);
    }

    #[test]
    fn parented_entity_without_transform_falls_back_to_global() {
        let world = GlobalTransform::from(Transform::from_xyz(1.0, 2.0, 3.0));
        let child_of = ChildOf(Entity::PLACEHOLDER);
        let result = fresh_global_transform(None, &world, Some(&child_of));
        assert_eq!(result, world);
    }
}

pub fn find_assets_folder() -> PathBuf {
    let mut current = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    info!("Current Working Directory: {:?}", current);

    // Search up to 5 levels parent directories
    for _ in 0..5 {
        let assets = current.join("assets");
        if assets.exists() && assets.is_dir() {
            return assets;
        }
        if !current.pop() {
            break;
        }
    }
    
    // Fallback if not found
    PathBuf::from("assets")
}
