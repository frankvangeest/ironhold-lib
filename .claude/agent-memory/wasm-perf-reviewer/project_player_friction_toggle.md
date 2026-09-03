---
name: project-player-friction-toggle
description: player_movement_system's per-tick Friction toggle — why its change guard is bit-exact-effective, rapier's PostUpdate schedule coalescing collider-material syncs to 1/frame, and the MessageReader-drain-in-FixedUpdate quirk that partially defeats the fix during browser hitches
metadata:
  type: project
---

`player_movement_system` (capabilities/player.rs, `FixedUpdate`) carries `Option<&mut Friction>` as
the 9th query element and writes `friction.coefficient` to `PLAYER_IDLE_FRICTION` (0.15) while
`raw_grounded && !loco.moving`, else `0.0`. Added by `feature/wall-friction` to kill the Coulomb
wall-crush bug. Facts verified by inspection of bevy_ecs 0.18 / bevy_rapier3d 0.33 / rapier3d 0.31:

**bevy_rapier's physics sets default to `PostUpdate`, NOT `FixedUpdate`.** `capabilities/physics.rs`
uses `RapierPhysicsPlugin::<NoUserData>::default()` and never calls `.in_fixed_schedule()`. So
`apply_collider_user_changes` (the `Changed<Friction>` consumer) runs **once per rendered frame**.
Any number of `Friction` DerefMut writes across a FixedUpdate catch-up burst coalesce into exactly
one collider-material sync per player per frame — the collider-write cost is frame-bounded, not
tick-bounded. Useful for any future `Changed<T>`-synced rapier component (`Restitution`, `Damping`,
`CollisionGroups`, `Collider`).

**`Collider::set_friction` is a plain field store** (`self.material.friction = coefficient`,
rapier3d-0.31 `geometry/collider.rs:213`) — no dirty bitflag, no broad/narrow-phase invalidation, no
body wake. The `Changed<Friction>` arm costs two stores plus a `HashMap<Entity, Handle>` lookup.
Also: rapier does **not** skip friction constraint rows at `μ == 0` (checked
`contact_with_coulomb_friction.rs` / the two-body constraint builders) — zeroing the coefficient is
solver-cost-**neutral**, it only zeroes the impulse budget. Don't claim a solver win from it.

**`Option<&mut T>` is free relative to a hard `&mut T` in bevy_ecs 0.18.** `matches_component_set`
returns `true` unconditionally (archetype match set is unchanged by adding it),
`IS_DENSE = T::IS_DENSE`, and `IS_ARCHETYPAL = true` — so the dense table fast path is preserved.
The `matches` bool is computed once per archetype/table in `set_archetype`/`set_table` and cached in
`OptionFetch`; per-entity cost is one branch. `Friction` is Table storage (no `SparseSet` attr).
`update_component_access` still registers the *write* access (needed for soundness), so it only
costs native multithreaded parallelism — irrelevant on web, and no other `ironhold_core` system
touches `Friction` anyway.

**The 0.001 epsilon guard is bit-exactly effective, because the value space has exactly 2 members.**
`raw_grounded` and `loco.moving` are both `bool`, so the target is always literally `0.15f32` or
`0.0f32`; `PLAYER_IDLE_FRICTION` is the single source of truth shared with the spawn site
(`entity_spawner.rs::spawn_player_entity_core`), so the stored and target bit patterns are identical
and the diff is exactly `0.0`. bevy_rapier only ever reads `&Friction` (never writes it back), and
nothing else in the engine mutates it — there is **no** floating-point-noise path. Steady state is
genuinely zero writes. If collider friction ever becomes RON-authorable (backlog Icebox), the
epsilon becomes a silent-clamp footgun and should be swapped for an exact `!=`.

**`MessageReader` in a `FixedUpdate` system is drained by the FIRST run of a catch-up burst.**
`player_movement_system` reads `InputActionMessage` once at the top, outside the query loop; on a
browser hitch the system runs N times in one frame and runs 2..N see **zero** input events, so
`move_vec == ZERO`, `loco.moving = false`, and `idle_drag` (default 0.8) is applied. Consequence for
the friction toggle: during a burst while grounded and holding Move, μ flips 0.0 (tick 1) → 0.15
(ticks 2..N), so the wall-crush mechanism partially re-arms on the residual `idle_drag`-decayed
approach speed. Still a **strict improvement** over the pre-fix always-0.15 behavior (it removes the
largest, full-commanded-speed term), and the airborne "hang while falling" case is fully fixed
(`raw_grounded` false → μ=0 on every tick). But this is a browser-specific residual — catch-up
bursts are far more common on web than native. See [[project-ground-cast-loop]] for the general
FixedUpdate amplification model and [[project-player-spawn-unification]] for the related
`TimestepMode::Variable` vs 64 Hz FixedUpdate mismatch.

**How to apply:** treat the friction block itself as free (~5 wasm ops × players × ticks/frame,
dwarfed by the ground shape-cast in the same loop). No renderer/WebGPU/WebGL2/uniform surface at
all — pure Rapier material state. Zero binary-size delta (one `f32` const, no new deps). New test
binary `wall_friction_tests` needs adding to root `CLAUDE.md`'s one-at-a-time test loop list.
