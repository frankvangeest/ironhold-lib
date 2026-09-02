---
name: wall-friction-velocity-crush
description: Third character-controller bug class — Coulomb friction at a wall contact eats the player's jump/fall because re-commanding linvel.xz into the wall every tick makes the normal impulse (and hence the friction budget) proportional to commanded speed, not to dt; exact solver citations, the mass- and dt-independent Δv_y = μ·v_cmd law, why NPCs are immune, and why "friction only when grounded AND idle" is the cheap correct fix
metadata:
  type: project
---

Root-caused 2026-09-01 during a `feature/prop-ground-veto` consultation. **Distinct from both
[[ground_cast_penetrating_normal]] (which *hit* the cast returns) and
[[airborne_ground_reacquisition]] (whether an airborne hit should be trusted).** Those two are
query-side; this one is entirely **solver-side** — `ground_cast` behaves perfectly throughout.

**The law (verified analytically to <1% against a real-Rapier harness).** For a player pressed
against a wall while `player_movement_system` re-writes `velocity.linvel.x/z = move_vec * speed`
every tick:

    Δv_y lost per physics step  =  μ_eff × v_commanded_into_wall

**Mass-independent and dt-independent.** Derivation: the normal constraint must cancel the full
commanded approach velocity every step, so λ_n = m·v_cmd (an impulse ∝ velocity, *not* ∝ force·dt
as for a body merely resting against a wall). Rapier's friction bound is
`let limit = self.limit * normal_part.impulse;`
(rapier3d-0.31 `dynamics/solver/contact_constraint/contact_with_coulomb_friction.rs:452`), and in
3D the tangent basis is `[tangent1, dir1.cross(tangent1)]` (same file, :447) — a **full 2D tangent
plane**, which for a horizontal wall normal **contains Y**. So Δv_y = μ·λ_n/m = μ·v_cmd; m cancels.

At shipped defaults (μ_eff 0.15, `walk_speed` 5.0, 64 Hz): **0.75 m/s per step = 48 m/s² ≈ 4.9 g**
of vertical braking — 5× gravity. At `run_speed` 10.0 it is 96 m/s² ≈ 9.8 g. Consequence: a
`jump_velocity: 6.0` jump that reaches 1.53 m in the open reaches **~0.26 m** when Move-into-wall is
held, dying in ~5 ticks. Numeric check (tick 0): predicted 0.75 (friction) + 0.153 (gravity) +
0.039 (damping, rapier is `linvel * 1/(1 + dt·damping)`, `rigid_body_components.rs:749`) = 0.942 vs
**observed 0.935**. Control with no Move input: predicted 0.198, observed 0.197.

**Diagnostic fingerprints** (use these to identify a recurrence, they distinguish it from anything
force- or solver-accuracy-based): unchanged under `substeps` 1 vs 8 (total per-step normal impulse
is invariant); **halving the physics rate roughly doubles the apex** (per-*step* loss is constant);
zero effect when jumping without Move input; scales with `walk_speed`/`run_speed`, not with mass.

**μ_eff is 0.15 and props never opt out.** `CoefficientCombineRule::combine` picks
`rule1.max(rule2)` over the discriminants `Average=0, Min=1, Multiply=2, Max=3, ClampedSum=4`
(rapier3d-0.31 `dynamics/coefficient_combine_rule.rs:53`). The player's `Min` therefore always wins
against a prop's default `Average`, giving `min(0.15, 0.5) = 0.15` for **every** untouched prop —
no prop in the repo sets friction at all.

**NPCs are structurally immune, and that is precedent, not accident.** All three NPC spawn sites
(`entity_spawner.rs:318`, `scene_loader.rs:463`, `scene_loader.rs:682`) use
`Friction { coefficient: 0.0, combine_rule: Min }`. Only `spawn_player_entity_core`
(`entity_spawner.rs:1113`) uses 0.15.

**This is old and was recently made 3.3× *better*, not worse.** Before the
`player_model_source_unification` v2 change, a GLB player got no `Friction` component at all →
rapier default 0.5/`Average` on both sides → μ_eff 0.5 → 2.5 m/s of vertical loss **per tick**
(jump annihilated in one step). `src/CLAUDE.md` ~line 1113 documents that the 0.15 exists *solely*
to stop idle downhill creep and explicitly concedes "friction was never doing much *while
moving*" — which is exactly the escape hatch for the fix.

**Recommended fix (cheap, targeted, precedented): make the coefficient conditional on
`!loco.moving && raw_grounded`, written from `player_movement_system` with change-detection
guarding** (`Friction` syncs via `Changed<Friction>` in bevy_rapier's `apply_collider_user_changes`,
so write only on transition). ~15 lines, no new physics query, no schema change, no WASM surface,
preserves the documented purpose of the 0.15 exactly. Direct precedent: **Quake/Source apply
`PM_Friction`/`CGameMovement::Friction()` only when on ground** — wall contact never applies
friction there either.

The general fix is **collide-and-slide on the commanded vector** (project XZ onto near-horizontal
contact planes before writing `linvel`), which every real character controller does *instead of*
letting a dynamic solver clean up: Unity `CharacterController.Move()` (kinematic PhysX sweep, no
PhysicMaterial applies), Unreal `SlideAlongSurface`/`ComputeSlideVector` = `VectorPlaneProject`
plus an explicit `HandleSlopeBoosting` guard that forbids a wall from altering vertical speed while
falling, Godot `move_and_slide()`'s `motion.slide(normal)` loop, Quake `PM_ClipVelocity`/`PM_FlyMove`.
`ContactManifoldView::normal()` is **world-space** (bevy_rapier3d-0.33 `plugin/narrow_phase.rs:202`;
rapier3d `geometry/contact_pair.rs:322`) and reachable via `contact_pairs_with(entity)` on the same
`RapierContextSystemParam` `ground_cast` already uses — gotcha: the normal points collider1→collider2,
so check which side the player is on and flip.

**Do NOT migrate to `KinematicCharacterController` for this.** It exists in the vendored crate
(`bevy_rapier3d-0.33/src/control/character_controller.rs`, with `slide`/`autostep`/`snap_to_ground`/
`max_slope_climb_angle`) and is the textbook-correct destination, but it deletes `Velocity`,
`Damping`, `ExternalImpulse` knockback, and the entire `ground_cast` + coyote + walkable-slope +
underfoot-veto stack that two consecutive features just built. Real caveat to name out loud: the
engine **is** incrementally reimplementing a KCC inside a dynamic body (coyote, slope gate, sensor
reach, wall veto, now slide projection). The honest crossover trigger is wanting step-offset/stairs,
moving platforms, or one-way platforms — not this bug.

**Symptom collision worth remembering:** the `prop-ground-veto` fix and this bug produce the *same*
player complaint ("jump doesn't work next to a wall") via different mechanisms. The veto fix makes
the jump *fire*; this bug then eats it. Anyone playtesting the veto fix against wall geometry will
conclude it failed unless told.
