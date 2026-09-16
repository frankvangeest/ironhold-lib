---
name: fixed-timestep-det-math-pattern
description: Reviewing engine-internal timestep/determinism changes (Rapier in FixedUpdate, det_math/libm) — why "no RON knob" is the right verdict, the FIXED_TICK_RATE three-way-drift gap, and the recurring stale-comment-claims-it-was-rewritten failure
metadata:
  type: project
---

Reviewed 2026-09-15 on `feature/deterministic_fixed_timestep` (v1 of
`planning/features/deterministic_fixed_timestep.md`). Same rubric family as
[[system-ordering-determinism-pattern]] and [[derived-physics-constant-pattern]].

**Verdict shape for this class: ALIGNED with warnings.** Physics tick rate, system schedule
assignment, and which math backend computes a sine are *not* designer concerns — a
`fixed_timestep:`/`motion_schedule:` RON field would be an anti-feature (it would let a project
silently desync from `jump_air_grace_ticks()`/`coyote_ticks()`, which are baked against the tick
rate). Confirm and move on; spend the budget on the drift/doc gaps below.

**`FIXED_TICK_RATE` is a *two*-way source of truth, not three.** It now lives in
`capabilities/physics.rs` (`pub const`, re-exported publicly by `capabilities/mod.rs`'s
`pub use physics::*`) and drives (1) `TimestepMode::Fixed.dt` and (2) `player.rs`'s
`jump_air_grace_ticks`/`coyote_ticks`. It does **not** drive the schedule itself — nothing calls
`insert_resource(Time::<Fixed>::from_hz(...))` anywhere in the crate, so the actual `FixedUpdate`
rate is still Bevy's undeclared 64Hz default and the constant only *documents* it. A Bevy upgrade
that changed that default would silently desync all three. Check for this on any future tick-rate
work; the one-line fix is the insert.

**Recurring failure, third sighting in this codebase: a doc comment asserts a sibling doc was
updated when it wasn't.** `src/CLAUDE.md` claimed `player_slope_jump_tests.rs`'s
`grace_expiry_does_not_reset_early_when_real_physics_time_lags_ticks` doc comment "was rewritten
alongside this fix"; the test (and `setup_case_full`'s own doc) still described production as
`TimestepMode::Variable` in `PostUpdate`. Always open the cited file rather than trusting the
claim — the plan's own task list had the rewrite as an explicit unchecked item.

**Test harness hardcodes `1.0 / 64.0` in 4+ sites** (`player_slope_jump_tests.rs`,
`prop_ground_veto_tests.rs` x1, `wall_friction_tests.rs` x2) even though
`ironhold_core::capabilities::physics::FIXED_TICK_RATE` is public and importable. The
`TimeUpdateStrategy::ManualDuration(1 tick)` + "one `app.update()` == one FixedUpdate tick" harness
is the pattern these three physics test files now share — replicate it, don't reinvent, when adding
a fourth.

**Designer-visible side effects worth listing in any such review** (none need RON exposure, all
need playtest/doc coverage): rendering is now stepped at the tick rate with no interpolation
(`TransformInterpolation` was *not* added — native is framepace-capped to 60fps against a 64Hz
tick, so there is a beat; a >64Hz browser shows real stepping); `motion_system` moved
`Update`→`FixedUpdate`, so `MotionDef.rotate`/`bob` on the 5 projects that author `motion:`
(`stats_demo`, `primitive_world`, `particles_demo`, `effect_mayhem_demo`, `entity_logic_demo`)
render at 64Hz. `MotionDef` has **no entry in `docs/20_data_formats.md` at all** (pre-existing gap)
— its only designer-facing docs are the schema doc comments in `schema/catalog.rs`.

**Playtest follow-up fix (reviewed 2026-09-16): `crate::utils::fresh_global_transform`.** New
engine-internal helper (`src/utils.rs`, `pub mod utils`) returning `GlobalTransform::from(*transform)`
for root entities, falling back to the raw (stale) `GlobalTransform` when `Option<&ChildOf>` is
`Some`. Applied in `world_label_screen_pos_system` (`lib.rs`, camera + tracked entity) and
`target_indicator_system` (`capabilities/target_indicator.rs`, 2 sites), plus
`world_label_screen_pos_system.after(camera_blend_system)`. Correctly RON-invisible: it only changes
*which* transform snapshot is read, never the offset/`screen_offset`/`depth_scale` math applied to
it, so no authored value needs retuning. Three things to re-check on any future use:
1. **The real footgun is the caller's query, not the function.** The function self-guards on
   `child_of`, but a caller that omits `Option<&ChildOf>` and passes `None` silently treats a
   parented entity's *local* Transform as a world transform (large wrong position, not a 1-tick lag).
   The doc comment does not say "always query and pass `ChildOf`; never pass `None` as a placeholder".
2. **The doc's root-ness justification is inaccurate.** Only *nested Actor/Prop* prefabs inside a
   composite are spawned as roots, and the reason is Rapier reading pre-propagation
   `GlobalTransform` (`scene_loader.rs:3181-3190`, `backlog.md` item "composite prefab child
   positions and physics wrong for nested Actor/Prop") — **not** label freshness. Nested `Primitive`
   anchors and primitive children *are* `add_child`ed (`scene_loader.rs:3214`, `:3327`).
3. **Unfixed structural twin: `fading_decal_system`** (`capabilities/decal.rs:107-119`) follows a
   `TrackedDecal` entity in XZ from a raw `GlobalTransform` in `Update` — identical shape to the
   `target_indicator_system` site that *was* fixed. A designer-authored decal and target ring on the
   same entity can now visibly disagree by one tick.

**Ordering gap to check if it recurs:** `motion_system` and the 7-system gameplay chain are both
`.before(PhysicsSet::SyncBackend)` but unordered *relative to each other*, and both take
`&mut Transform` — an ambiguity of exactly the kind the change's own comment says it exists to
remove. Benign today (disjoint entity sets in practice) but should be `.chain()`ed or explicitly
ordered.
