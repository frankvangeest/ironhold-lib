# Player ground detection, jumping and friction

How `capabilities/player.rs` decides what counts as ground, when a jump is allowed, and why friction is conditional.

> Moved from `crates/ironhold_core/src/CLAUDE.md` on 2026-10-08 (the core CLAUDE.md split). The hard rules for this area stay in the folder `CLAUDE.md` files; this page keeps the long-form reference and history verbatim.

## Jump reset can't rely on a ground-check edge; slope walkability gate

<!-- b:779 -->

### Jump reset cannot rely on a ground-check edge (`planning/features/done/uphill_jump_lock.md`)

`player_movement_system`'s ground detection (`capabilities/player.rs`) is a fixed-reach downward
shape-cast (`collider_radius + ground_cast_length`), re-evaluated fresh every tick — **it cannot be
assumed to ever report `is_grounded = false`**. On a slope steep enough (~12°+ at shipped
defaults), the incline's rising surface closes the vertical gap a jump's Y-velocity opens *faster
than gravity does*, so the cast keeps reporting contact on every single tick, forever. The same
failure is reachable on perfectly flat ground too, with no slope involved: any player prefab whose
`jump`/`double_jump_height` apex doesn't clear `collider_radius + ground_cast_length` hits the
identical lock (a scene-load `warn!` — `scene_loader.rs::warn_jump_cannot_clear_ground_sensor` —
plus a matching `ironhold_cli validate` error, `jump_cannot_clear_ground_sensor`, flag this
misconfiguration).

**A slope-normal walkability gate is the first line of defense — added after real playtesting
found the mirror-image bug: an unbounded re-jump exploit while falling/sliding down a steep
decline.** The ground shape-cast's hit (`ShapeCastHit::details.normal1`, requesting
`compute_impact_geometry_on_penetration: true` in `ShapeCastOptions` so the normal is populated
even for the common near-zero-clearance resting case) is checked against
`CharacterController.max_walkable_slope_deg` (from `MovementConfig`, default 45°, matching Unity's
`CharacterController.slopeLimit` / Unreal's `WalkableFloorAngle` / Godot's `floor_max_angle`) —
**a surface steeper than that never counts as grounded at all**, regardless of proximity. Root
cause of the downhill bug: on any incline the sensor can't cleanly detach from (whether ascending
*or* descending), `velocity.linvel.y` is trivially `<= 0.0` for the entire descending portion, so
without this gate the tick/velocity/height reset logic described later on this page would re-arm `jumps_used` on *every*
tick once grace expires — an unbounded re-jump exploit that gets worse the longer the fall/slide
continues, exactly mirroring the original uphill lock but in the opposite direction. The gate fixes
this at the source rather than special-casing it in the reset logic: an unwalkably steep surface
is simply never "ground," so the character is correctly airborne/sliding on it and the whole reset
mechanism described later on this page never engages. **This does not replace the grace/velocity/liftoff-height mechanism
described later on this page, for *walkable* slopes** (≤ `max_walkable_slope_deg`, e.g. an ordinary 12–30° hill) — a
walkable incline's contact is genuinely correct, continuous grounding while climbing or
descending it, so the sensor still can't cleanly detach there, and the bounded "pogo" cadence
tradeoff documented further down still applies to that range. See
`player_slope_jump_tests.rs::unwalkable_slope_does_not_allow_endless_rejump_while_sliding` and
`::walkable_slope_pogo_cadence_is_unaffected_by_the_slope_limit_check`.

## `jumps_used` reset: grace ticks, velocity, liftoff height; dual-clock history

<!-- b:815 -->

Because of this, `jumps_used` is **not** reset on a `!was_grounded && is_grounded` edge (an
edge-triggered reset can starve permanently if the edge never fires). The landing *animation*
request (`jump_exit`) still fires on that real edge, unchanged from before this fix — a plain
fall (e.g. walking off a ledge, `jumps_used` already `0`) must still play the landing clip. The
`jumps_used` **reset** is a separate, level-gated check, re-evaluated every tick:

1. `CharacterController.jump_air_grace` — a `FixedUpdate` tick countdown, set at jump-fire time
   from `jump_air_grace_ticks()`, derived analytically from the jump's own velocity, `GRAVITY`
   (`scene_loader.rs`, `pub(crate)` specifically so this formula can share it), and the
   controller's own `collider_radius`/`ground_cast_length` — never a separate hand-tuned constant
   that could drift out of sync with a project's authored values. While `> 0`, a grounded reading
   isn't even considered as a possible landing.
2. Once grace hits `0`, the reset additionally requires **either** `velocity.linvel.y <= 0.0`
   (the jump's ballistic ascent has genuinely ended) **or** the entity having risen at least
   `collider_radius + ground_cast_length` above `CharacterController.jump_liftoff_y` (its Y <!-- audit:ok -->
   position at jump-fire time) — proof the sensor's overlap with the liftoff pose can no longer
   explain a grounded reading.

**Both of those two extra checks are physical quantities, not clock-derived — this is deliberate,
not redundant with the tick counter.** As of `planning/features/deterministic_fixed_timestep.md`
(v1), Rapier's own physics stepping runs on `TimestepMode::Fixed` in `FixedUpdate`
(`capabilities/physics.rs`) — the *same* clock `jump_air_grace` is counted against, so the
dual-clock lag scenario this section used to describe (`TimestepMode::Variable` in `PostUpdate`
drifting from `FixedUpdate`'s tick count at a low framerate or a `Time<Virtual>::max_delta`-clamped
hitch) can no longer happen. The two checks are kept anyway as defense-in-depth against any other
source of a slower-than-assumed ascent — see
`player_slope_jump_tests.rs::grace_expiry_does_not_reset_early_when_real_physics_time_lags_ticks`,
whose doc comment was rewritten alongside this fix: its `physics_dt != 1.0/64.0` setup no longer
represents a scenario reachable in the shipped game, but its underlying assertions (the
`jump_liftoff_y`/velocity belt-and-braces check) remain valid and are kept. The two checks
serve different terrain: `velocity.linvel.y <= 0.0` covers a jump whose ascent has genuinely ended
(flat ground, or a jump too short to ever clear the sensor); the liftoff-height check covers a
*continuously climbing* slope, where the contact solver keeps `linvel.y` pinned positive (matching
the climb rate) for as long as the player keeps walking uphill — `linvel.y <= 0.0` alone would
never fire there, but net height risen since the jump still grows the whole time.
`CharacterController.jump_liftoff_y`'s only clearing path is a successful reset; if a future
change ever removes the `velocity.linvel.y <= 0.0` half of that `||`, double check nothing can
leave a stale `jump_liftoff_y` behind after a non-jump position change (e.g. a teleport) — today
this is benign only because a teleport also zeroes `Velocity`, which the surviving `<= 0.0` clause
happens to catch.

## Reset must read `raw_grounded`, never coyote-buffered `is_grounded`

<!-- b:856.ref -->

**The `jumps_used`/`jump_liftoff_y` reset logic described earlier on this page intentionally never reads the coyote-buffered
`LocomotionState.is_grounded` — it reads a separate `raw_grounded` local, computed identically to
before coyote-time existed, in both the Rapier-context and no-physics branches.** An earlier version
of this fix (pre-coyote-time) forced `is_grounded = false` for a fixed window instead — rejected in
plan review because (1) `can_jump`'s airborne branch (`double_jump_enabled && jumps_used <
max_jumps`) would then go live immediately after a ground jump, letting a fast double-tap consume
the second jump at ground level instead of at a real airborne height, and (2) a fixed window's
safety margin is a function of authorable content (`jump` height, `collider_radius`,
`ground_cast_length`) and no single constant is safe across every project's authored values. If
touching this system again: `animation_resolver.rs`'s jump/land clip selection and `can_jump`'s
branch selection may legitimately read the coyote-buffered `loco.is_grounded` (that's the whole
point of the buffer — smoothing *feel*), but the `jumps_used` reset's grace/velocity/liftoff-height
gate must keep reading `raw_grounded`, never the buffered value.

**Real bug hit while adding coyote-time, kept here as the concrete cautionary example:** feeding the
coyote-buffered `loco.is_grounded` into the reset condition broke
`grace_expiry_does_not_reset_early_when_real_physics_time_lags_ticks` — `jumps_used` reset to `0`
while `linvel.y` was still clearly positive (~4.16). Root cause: right at the tick the sensor first
genuinely detaches, the coyote buffer keeps `loco.is_grounded` artificially `true` for several more
ticks; if the liftoff-height threshold (`risen_since_liftoff >= ground_sensor_reach()`) also happens
to cross in that same window — a check specifically designed to fire *while still rising*, for the
continuously-climbing-slope case — the reset fires prematurely on an ordinary flat-ground jump
nowhere near any slope. Two mechanisms, each correct for its own purpose, reintroduced exactly the
bug the other one already fixed. Fixed by the `raw_grounded`/`loco.is_grounded` split described
earlier on this page — a debounce that smooths one consumer's *feel* must never leak into another consumer's
*correctness*-critical timing check.

## Coyote time: debounced grounding

<!-- b:883 -->

### Coyote time — debounced grounding for uneven terrain (`planning/features/done/uphill_jump_lock.md`)

Playtesting `3rd_person_game_demo` surfaced a third, distinct problem from the two earlier ones: walking
over ordinary uneven terrain (bumps, small ledges, barely-there slope) made the character flicker
into the falling state constantly, even though no jump-lock or hover-exploit logic was involved —
just single-tick gaps in the raw ground shape-cast as the character crossed terrain irregularities
too small to be a real "leave the ground" event. Other engines solve exactly this with a debounce
buffer, universally nicknamed **coyote time** (Wile E. Coyote not falling until he looks down):
delay the grounded→airborne *transition* by a short window, refreshed every tick the sensor
genuinely reports contact.

`CharacterController.coyote_ticks_remaining` (a `FixedUpdate` tick countdown, refreshed to
`coyote_ticks(controller.coyote_time_secs)` every tick `raw_grounded` is `true`) buffers
`LocomotionState.is_grounded` — **and only that**: `raw_grounded` still exists as its own local,
computed once per tick before the buffer is applied, and is what the `jumps_used` reset logic reads
(see the earlier sections of this page). The buffer widens two things on purpose: how long the falling animation is suppressed
after a real-but-brief loss of contact, and how late a jump input can land after leaving a platform
edge and still fire (the classic "coyote time" forgiving-jump-timing benefit, not just an
anti-flicker fix). `MovementConfig.coyote_time_secs` (default `0.1`s) is a real designer-facing
tuning field, unlike `jump_air_grace` — see Q2 in the feature plan for why `jump_air_grace` is
deliberately *not* authorable while this is. `0.0`/negative both disable the buffer (negative is
flagged by `warn_negative_coyote_time_secs` + `ironhold_cli validate --strict`'s
`negative_coyote_time_secs`, since it's more likely a typo than an intentional "off"). A value
*too large* relative to the time the ground sensor actually reports "ungrounded" during the jump
(narrower than the raw ballistic airtime `2 * jump_velocity / GRAVITY` — the sensor keeps reading
"grounded" for as long as height is at or below `collider_radius + ground_cast_length`, both on <!-- audit:ok -->
the way up and down) has no load-time `warn!` counterpart, but is caught by `ironhold_cli validate
--strict`'s `coyote_time_exceeds_jump_airtime`.

## `can_jump`: coyote buffer unlocks only the first jump

<!-- b:912 -->

**`can_jump`'s two branches read different grounded signals on purpose — this is not the same
mistake as feeding the buffer into the `jumps_used` reset, but it looks similar enough that three
independent post-implementation reviews all found the same real bug in the first version of this
fix.** The grounded (first-jump) branch is gated by `raw_grounded || (coyote_ticks_remaining > 0 &&
jumps_used == 0)` — deliberately buffered, since first-jump coyote-forgiveness is the whole point.
The airborne (double-jump) branch is reached only when that combined condition is false, and its own
`double_jump_enabled && jumps_used < max_jumps` check has **no** buffering in it at all — it depends
purely on the branch *not* having taken the grounded path, which for `jumps_used > 0` means purely
on `raw_grounded`. The first version of this fix instead gated the grounded branch on the fully
buffered `loco.is_grounded` with no `jumps_used == 0` qualifier — mutually exclusive with the
airborne branch, so for the entire coyote window after a real ground jump (`jumps_used == 1`),
*neither* branch was reachable: a double-jump press was silently swallowed until the buffer expired,
up to permanently for a large `coyote_time_secs`. If touching `can_jump` again: the coyote buffer
must only ever unlock a *first* jump (`jumps_used == 0`), never gate whether a *second* jump is
reachable — that must always come down to `raw_grounded` alone, exactly as if coyote-time didn't
exist.

## Ground shape-cast must `.exclude_sensors()` and `normalize_or_zero()`

<!-- b:929 -->

**The ground shape-cast must exclude sensors, and must `normalize_or_zero()` a hit's normal before
using it — both found by a real playtest, not by the reviews.** `QueryFilter::new().exclude_rigid_
body(entity).exclude_sensors()` (`capabilities/player.rs`): without `.exclude_sensors()`, a nearby
prop's `trigger_zone` (a ghost `Collider::ball` + `Sensor` child, `entity_spawner.rs`'s
`attach_prefab_features`) could be swept by the ground cast just like real geometry. The cast ball
starts embedded in a large, nearby sensor sphere (the sensor's own radius, e.g. 2.5m for
`3rd_person_game_demo`'s chest, easily contains it from up to `radius + collider_radius` away), so
the resulting `time_of_impact == 0` "penetrating" hit beats the real floor's small-but-nonzero toi —
and the ball-in-ball EPA normal at that embedded position is radial, i.e. near-horizontal,
misclassified as an unwalkable wall by the slope-walkability gate described earlier on this page. Symptom: the player played
the falling animation while standing on ordinary flat ground, for as long as they stood within
range of *any* nearby `trigger_zone` prop. Matches the existing `.exclude_sensors()` precedent in
`capabilities/npc.rs`'s line-of-sight raycast — a sensor is a ghost collider by definition and must
never count as floor. Second, independently-found bug in the same code path: a penetrating hit's
`normal1` is not always unit length (measured ~0.52 for the ball-in-sensor case just described) — a bare
`.dot(Vec3::Y).acos()` on that computes `acos(|n| * cos(theta))`, not the real angle theta, silently
biasing every penetrating-hit angle toward 90°. Fixed by `.normalize_or_zero()`-ing the normal
first; a fully degenerate (zero) result dots to 0 (90°, unwalkable), matching the existing
"no computable normal" treatment for a `details: None` hit. See
`crates/ironhold_core/tests/prop_ground_veto_tests.rs`.

## `ground_cast` re-query loop, "underfoot", `is_walkable_contact`, known gap

<!-- b:950 -->

**A solid (non-sensor) prop/wall pressed directly against the player could also veto the floor —
fixed by re-querying (`ground_cast`), not by touching `is_walkable_contact` (the walkability check
itself).** This feature's slope-walkability gate is what made a solid prop/wall's normal matter at
all: on `main`, the ground cast was proximity-only (`hit.is_some()`), so no collider's normal was
ever load-bearing. A prop tall enough to reach the cast ball's centre (feet + `collider_radius` +
skin ≈ 0.41m) has a penetrating (`time_of_impact == 0`) contact when the player stands pressed
against it — this always beats the real floor's non-zero toi, so `cast_shape` (which only ever
returns the single nearest hit) reported the wall instead of the floor, and the wall's
near-horizontal EPA normal then correctly-but-wrongly failed the walkability check. Silently
disabled jump entirely for any project with `double_jump_enabled: false` (every shipped project's
default) — not just a wrong animation, since `can_jump`'s only reachable branch then requires
`raw_grounded`.

Fixed by extracting the walkability check into `is_walkable_contact()` and the whole ground
shape-cast (origin lift, `ShapeCastOptions`, `QueryFilter`) into `ground_cast()` — both `pub fn`s in
`capabilities/player.rs`, shared by `player_movement_system` and its test probe
(`prop_ground_veto_tests.rs::probe()`, which now calls `ground_cast` directly instead of hand-
duplicating it, closing a real "the test and the engine can silently drift" risk a review flagged).
`ground_cast` re-queries in a bounded loop (`MAX_GROUND_CAST_CANDIDATES = 4`), excluding (via
`QueryFilter::predicate`, only attached once there's actually something to exclude) any hit that is
**both** not underfoot **and** not walkable, until an accepted candidate is found or every
candidate this tick is exhausted:

- **"Underfoot"** — the contact point (`ShapeCastHitDetails.witness1`, confirmed genuinely
  world-space for `cast_shape` despite a misleading doc comment inherited from parry — verified
  against `bevy_rapier3d-0.33.0/src/plugin/context/mod.rs`'s `RapierContext::cast_shape` doc
  comment) is at or below `feet_pos.y + collider_radius * 0.5`. A hit with no computable <!-- audit:ok -->
  contact point (`details: None`) defaults to underfoot — its fate is decided by the walkability
  check either way, which itself treats a detail-less hit as unwalkable unless the `90.0` escape
  hatch is set.
- **Both conditions, not "not underfoot" alone**, is load-bearing — a first version of this fix
  rejected any non-underfoot hit unconditionally and was caught by system-architect review before
  landing: `collider_radius * 0.5` alone imposes a hidden `acos(1 - 0.5) = 60°` walkable-slope
  ceiling *independent of* `max_walkable_slope_deg` (both the tolerance and a slope's contact-height
  offset scale with `collider_radius`, so the ceiling angle doesn't depend on it) — a project
  authoring a steeper walkable slope, or a player merely spawned slightly inside geometry (a
  deep-penetration contact after a teleport/`at_entity` placement), would have had its own genuine
  floor contact wrongly excluded, turning a previously-grounded tick ungrounded. Requiring **both**
  conditions to fail before excluding makes the loop monotone — the underfoot check can only ever
  *rescue* a hit `is_walkable_contact` alone would have rejected (the wall case, the actual bug),
  never *reject* one `is_walkable_contact` alone would have accepted. This is also what makes the
  `max_walkable_slope_deg >= 90.0` escape hatch restore this project's exact pre-fix proximity-only
  behavior even after this loop was added: at `90.0` every hit is walkable, so the very first
  candidate is always accepted with zero exclusions.

**Known remaining gap, tracked as its own bug (`planning/backlog.md` ▸ Bugs), not covered by this
fix:** `QueryFilter::predicate` excludes by whole `Entity`, so a wall that's part of the *same*
collider entity as the walkable floor beneath it — any compound-collider prop with a tall-enough
component shape, not only a raised terrain edge carved into one `TriMesh` — still excludes both
together, reproducing the identical full-jump-lock symptom. No shipped project's compound colliders
currently combine a tall component with player-reachable floor geometry this way.

See `prop_ground_veto_tests.rs::solid_prop_taller_than_cast_ball_centre_no_longer_vetoes_when_pressed_against`
(and its `_on_trimesh_terrain` sibling) plus `player_slope_jump_tests.rs::walkable_slope_steeper_than_the_ground_cast_underfoot_tolerance_is_still_grounded`.

## `jump_air_grace_ticks` NaN clamp; accepted pogo consequences

<!-- b:1005 -->

`jump_air_grace_ticks()` clamps its input velocity via `f32::max(0.0, vel)` (which also launders a
NaN velocity — from a negative/misconfigured `jump`/`double_jump_height` — to `0.0`) and floors
its result at 1 tick, so a near-zero or invalid jump height degrades to a bounded (if rapid)
re-arm rather than either a NaN-poisoned permanent lock or a same-tick event storm. The design-time
`warn!`/`ironhold_cli validate` check (described later on this page) should catch that authoring mistake before it ships;
this is defense-in-depth for if it doesn't.

One accepted, physically-unavoidable consequence: on a slope steep enough that the cast never
truthfully detaches, holding jump produces a bounded "pogo" cadence (roughly one re-jump per grace
window, ~0.26s at shipped defaults) rather than either the old permanent lock or an unbounded
hover exploit — see `player_slope_jump_tests.rs`'s cadence-bound test. Two secondary, accepted
consequences of that same tradeoff: a jump whose real airtime is shorter than the grace window
(e.g. landing on a nearby raised platform) plays its landing animation on the real edge but stays
un-rejumpable for the remainder of the window (bounded, ≤ the grace duration); and on a
never-detaching slope, `jump_exit` never fires at all (no real edge ever happens) while
`jump_enter` re-arms every pogo cycle, which can visibly pin the takeoff animation for as long as
the player holds jump uphill.

## Collider divergence; Friction 0.15 idle-only after the wall-friction fix

<!-- b:1224 -->

**Deliberately NOT unified in v1** (kept as pre-existing behavioral divergence, not a bug):
collider sizing — GLB derives it from `movement`-config-driven capsule dimensions; primitive
derives it from the prefab's own `shape`/`params`. This divergence is unrelated to Friction (described later on this page)
and stays.

**Unified in v2:** the `Friction` component (`entity_spawner.rs`'s `spawn_player_entity_core`) is
now inserted unconditionally for every player regardless of `model_source` — previously
primitive-only, leaving a GLB player's capsule at Rapier's default 0.5/`Average` friction while an
otherwise-identical primitive player got its own coefficient/`Min`. The coefficient is **`0.15`, not
`0.0`** — the two-scene playtest (room10 cube-edge + `quick_scene` hillside) that verified this
found `0.0` eliminated edge-catching but let an idle player creep downhill on sloped terrain
indefinitely (movement writes `velocity.linvel` directly each tick, so friction was never doing much
*while moving*; the risk was always specifically the idle case). `0.15` (still `combine_rule: Min`)
holds a slope while remaining low enough to avoid noticeably reintroducing edge-catching. `idle_drag`
(`MovementConfig`, `capabilities/player.rs`) bounds any residual creep further but cannot zero it on
its own (no grounded gate — pushing it very low also cancels air momentum after a jump), so it is a
secondary tuning knob, not the primary defense. No separate friction field was added to
`MovementConfig` — `0.15` is a fixed engine constant, not per-prefab-authorable (logged to
`planning/backlog.md`'s Icebox as a possible future physics-material field).

**Made conditional in the wall-friction velocity-crush fix:** the `0.15` coefficient described earlier is no
longer applied unconditionally — `player_movement_system` now syncs `Friction.coefficient` to
`PLAYER_IDLE_FRICTION` (`capabilities/player.rs`, the same `0.15`, now a single named constant
shared with `spawn_player_entity_core`'s initial value) only while `raw_grounded && !loco.moving`,
and to `0.0` otherwise. Root cause: Rapier's Coulomb friction at a *wall* contact resists motion
across its full tangent plane, which in 3D includes vertical — and since movement writes
`velocity.linvel.x/z` every tick (an impulse-sized command, not a force), the friction impulse per
physics step scaled with the player's full commanded approach speed into the wall, independent of
mass or frame rate. This crushed jump height by up to ~83% when a jump was held into a wall, and
let a merely-falling player "hang" against a wall at ~1/5 free-fall rate purely from holding
movement into it — no jump required. The fix does not change the idle-grounded case described
earlier on this page at all (same `0.15`, same `Min` combine rule, same slope-holding behavior) — it only stops
that coefficient from ever being live while the player is moving or airborne, which is exactly the
case the earlier paragraph already notes friction "was never doing much" in anyway. See
`crates/ironhold_core/tests/wall_friction_tests.rs` and `planning/backlog.md`'s former "Moving into
a wall while airborne crushes vertical velocity via Coulomb friction" entry.
