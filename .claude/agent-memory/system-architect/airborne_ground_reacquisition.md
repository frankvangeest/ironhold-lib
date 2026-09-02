---
name: airborne-ground-reacquisition
description: The "proximity sensor re-acquires a NEW surface while the character is still airborne and rising" bug class — exact tick math for the local_coop_demo cube repro, why it's pre-existing (not a prop-ground-veto or uphill-jump-lock regression), the jump_exit equal-priority animation swap that makes it visible, and the verified Unity/Unreal/Godot/Quake precedent for fixing it
metadata:
  type: project
---

Worked out 2026-09-01 during a consultation on `feature/prop-ground-veto`. **Distinct from the
wall-veto bug class in [[ground_cast_penetrating_normal]]** — that one is about *which* hit
`cast_shape` returns; this one is about whether *any* hit should be trusted while airborne.

**The mechanism.** `ground_cast`'s reach is `collider_radius + ground_cast_length` (0.7 m at
defaults, see [[ground_detection_jump_invariants]]). It answers "is there a walkable surface within
0.7 m below the feet" — which is *correct* but is not the same question as "have you landed". A
character arcing over/onto a nearby platform gets a geometrically-true grounded reading while still
rising. **Pre-existing since before the walkable-slope gate** (on old `main`, `raw_grounded =
hit.is_some()` — same result), so it is not a regression from `uphill_jump_lock` or
`prop_ground_veto`.

**Verified repro arithmetic** (`local_coop_demo` `cube_obstacle_room10`: `size (1.2,1.2,1.2)` at
`translation (-3.3, 0.6, -1.5)` ⇒ **top face at y = 1.2**; harness `jump_velocity: 6.0`,
`coyote_time_secs: 0.1`, start y = 0.02, 64 Hz):
- Clears the 0.7 m reach at `t = (6 − sqrt(36 − 2·9.81·0.7))/9.81 = 0.1306 s` ⇒ **raw goes false
  ~tick 8**; coyote (6 ticks) ⇒ **`loco.is_grounded` false ~tick 15**. Matches the observed log.
- `jump_air_grace_ticks(6.0)` = `ceil(2 · 0.1306 · 64)` = **17 ticks** ⇒ the reset's `else if`
  branch first becomes reachable at **tick 18**. The observed "reset at tick 18" is grace expiry,
  not a cube interaction — the cube only supplies the `raw_grounded == true` that lets it fire.
- At tick 18 feet y ≈ 1.342 ⇒ cube top is **0.14 m below the feet**, inside the 0.7 m reach.
  `linvel.y = 3.72 > 0`, so the reset fires through `risen_since_liftoff (1.32) >=
  ground_sensor_reach() (0.7)` — the branch added for the *climbing-slope* case.
- The one-tick `vel_y 3.355 → 4.185` jump is ~+0.98 m/s of Rapier penetration-recovery impulse
  (gravity alone is −0.153/tick) — the capsule **actually clipped the cube's top edge**, it is not
  a non-contact fly-by. That also explains apex 1.97 vs the 1.53 control.

**Why it looks like "the jump halts half way" — confirmed by code, not hypothesis.**
`player.rs`'s landing edge (`if !was_grounded && loco.is_grounded { push "jump_exit" }`) fires
mid-ascent. `animation_resolver.rs` accepts an override on `candidate.priority >= active.priority`
(**>=**, not `>`), and every shipped policy gives `jump_enter` *and* `jump_exit` `priority: 200` —
so `jump_exit` ("Jump_Land") **replaces** the in-flight "Jump_Start" 0.28 s into its 0.4 s window,
while the body is rising at 3.7 m/s. The older note in [[ground_detection_jump_invariants]] that "a
sub-0.4 s lie about `is_grounded` would be invisible in animation" is **wrong for this path** — it
only holds for the `!is_grounded → jump_loop` fallback branch, not for the sentinel-push edge.

**Second, unreported consequence:** `jumps_used` → 0 mid-air means a *second* full-strength jump is
available from apex even with `double_jump_enabled: false`. Jump input is `just_pressed`
(`runtime/input.rs:373`), so it needs a re-press — no auto-hover — but it is a real exploit.

**Recommended fix shape (not yet implemented).** Two complementary gates, both standard precedent:
1. **Continuity gate (Godot's `!p_was_on_floor`)** — a *newly acquired* contact can't count while
   rising. Needs one bool of last-tick raw state. Put it on **`LocomotionState`** (`raw_grounded`),
   not `CharacterController`: `LocomotionState` has a `Default` impl and **zero struct literals
   anywhere in the repo**, so it is zero test churn, whereas `CharacterController` has no `Default`
   and ~48 literal sites. It also closes the gap `LocomotionState.is_grounded`'s own doc comment
   admits ("the true, un-debounced sensor result … is not currently exposed").
2. **Two-tier reach (Unreal's `HeightCheckAdjust`, KCC's `GroundDetectionExtraDistance`)** — full
   0.7 m reach only while `raw_grounded_last_tick || coyote_ticks_remaining > 0`; a small skin-sized
   reach otherwise. **Key property: this only affects *re*-acquisition, never the initial detach**
   (the first false still requires clearing the full 0.7 m), so `jump_air_grace_ticks()`,
   `risen_since_liftoff >= ground_sensor_reach()`, `warn_jump_cannot_clear_ground_sensor` and the
   CLI validate check all stay numerically correct with no change. Gating on `|| coyote > 0` (not
   raw alone) is what stops a one-tick terrain-seam miss from latching the shrunken reach on.

Gate 1 alone is insufficient: it releases at apex, where the cube top can still be within 0.7 m
(0.655 m undamped) ⇒ a later, briefer false landing. Gate 2 alone leaves a residue for low jumps.
Don't derive the airborne reach from a new `MovementConfig` field — keep it a derived constant
alongside `GROUND_CAST_SKIN`/`JUMP_AIR_GRACE_SAFETY`; nobody should tune it.

**Do NOT "fix" this in `animation_resolver.rs`.** Making the resolver distrust a same-tick grounded
flip puts movement policy in the animation layer and would also suppress *real* landings. The `>=`
priority rule is correct; the input was wrong.

**Verified external precedent** (re-checked against sources, don't re-derive):
- **Godot** `scene/3d/physics/character_body_3d.cpp`:
  `void CharacterBody3D::_snap_on_floor(bool p_was_on_floor, bool p_vel_dir_facing_up) { if
  (collision_state.floor || !p_was_on_floor || p_vel_dir_facing_up) { return; } apply_floor_snap();
  }` with `bool vel_dir_facing_up = velocity.dot(up_direction) > 0;`. Both gates, exactly.
- **Quake / Source** `PM_CategorizePosition`: `if (pmove->velocity[2] > 180) pmove->onground = -1;`
  ("Shooting up really fast. Definitely not on ground."). Source uses `NON_JUMP_VELOCITY 140.0f`.
  A raw velocity gate with a threshold chosen above ramp-climb rate, below jump speed.
- **Unreal** `UCharacterMovementComponent`: the proximity probe (`FindFloor`) runs in
  `MOVE_Walking`; in `MOVE_Falling`, landing requires a real blocking hit from the move sweep, then
  `IsValidLandingSpot`, which rejects unwalkable normals, rejects penetrating hits with
  `Normal.Z < KINDA_SMALL_NUMBER`, and **rejects impacts above the capsule's lower hemisphere**
  (`Hit.ImpactPoint.Z >= Hit.Location.Z - PawnHalfHeight + PawnRadius`) — the same idea as this
  repo's `witness1` underfoot test. `FindFloor` also shrinks its own sweep when not already
  walking: `HeightCheckAdjust = (IsMovingOnGround() ? MAX_FLOOR_DIST + KINDA_SMALL_NUMBER :
  -MAX_FLOOR_DIST)`.
- **Unity KCC** (Photon/Rival lineage): `ForceUnground()` suppresses grounding for a duration on
  jump; extra ground-probe distance is added *only* when `LastGroundingStatus.IsStableOnGround`.
  Unity's built-in `CharacterController.isGrounded` is contact-derived from `Move()`, not a probe.
