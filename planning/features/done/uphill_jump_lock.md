# Feature: Uphill jump lock fix

_Status: Done — playtest confirmed 2026-08-23, worktree HEAD `768a12c`_
_Planned at: `48edf00` (2026-08-19)_
_Plan reviewed: system-architect + ux-gamedesigner-reviewer (2026-08-19) — v1 approach rejected, v2 below incorporates both verdicts._

## Bug

`planning/backlog.md` ▸ Bugs ▸ **uphill jump lock**:
> when jumping against an uphill slope, the player can land in a state where `jump` never
> re-triggers: the character controller reports ground contact but the slope normal keeps the
> jump cooldown active. Suspected cause: Rapier's ground-contact normal threshold in the character
> controller or the jump cooldown not resetting when sliding contact ends. Reproduce:
> 3rd_person_game_demo, run toward any hill and spam jump while ascending.

No `found at` hash (predates that convention).

## Reproduction (confirmed against current HEAD, `48edf00`)

The backlog's own theory ("jump cooldown", "contact normal threshold") doesn't match anything
literally present in the code — there is no cooldown timer and no contact-normal check anywhere in
`player_movement_system`. Built a deterministic Rapier-physics regression harness (spawns a real
sloped `Collider::cuboid` + a player capsule matching `spawn_player_entity_core`'s exact
construction, steps `player_movement_system` + Rapier's physics integration across many
`FixedUpdate` ticks while moving into the incline) instead of relying on manual play-testing, since
the mechanism is physics-timing-dependent. Ran it against `crates/ironhold_core/src/capabilities/
player.rs`'s current code, real shipped defaults (`ground_cast_length: 0.3`, `collider_radius: 0.4`,
`jump_velocity: 5.94`, `double_jump_enabled: false`):

| Slope | Result |
|---|---|
| 0° (flat, control) | Repeated jumps work: `ever_ungrounded=true`, detaches ~9 ticks (~0.14s) after takeoff |
| 5°–10° | Still works normally |
| **12°+** | **Locks permanently after the first jump** — `ever_ungrounded=false` for the rest of the run (160+ ticks / 2.5s+), `jumps_used_final=1` forever |
| 15°/20°/25°, stop moving at t=60 | **Still locked** — does not self-heal even once the player stops climbing |
| 12°–25°, `ground_cast_length` retuned to 0.05 | Still locks at 15°+ — shrinking the cast reach is not a robust fix |

**Root cause**, with exact citations:
- Ground detection (`player.rs:161-189`) is a fixed-reach (`collider_radius` + `ground_cast_length`
  ≈ 0.7m combined at defaults) downward sphere-cast, re-evaluated fresh every tick, with no memory
  of "currently mid-jump."
- `jumps_used` resets **only** on the `!was_grounded && loco.is_grounded` landing edge
  (`player.rs:191-195`).
- `can_jump` (`player.rs:246-253`) requires `jumps_used == 0` while grounded.
- On an incline steep enough, the slope's rising surface closes the vertical gap the jump's
  Y-velocity impulse opens *faster than gravity does* — well within the sphere-cast's reach — so
  `is_grounded` never dips `false` even for one tick. The landing edge never fires, `jumps_used`
  stays stuck at 1, and since `double_jump_enabled` defaults to `false`, `can_jump` is permanently
  `false` for the rest of the session on that slope.

**Corrected math** (v1 of this plan had this wrong — caught by review, see "What changed" below):
measured flat-ground detach time is **~9 ticks / ~0.14s**, matching the analytic ballistic estimate
almost exactly: solving `h = v·t − ½g·t²` for the first time the player clears the sensor's
combined reach `h = collider_radius + ground_cast_length = 0.7m` at `v = jump_velocity = 5.94`,
`g = GRAVITY = 9.81` (`scene_loader.rs:2823`) gives `t = (v − √(v² − 2gh)) / g ≈ 0.132s`.

**This bug class is not slope-specific.** The same detach-time formula shows that *any* project
authoring a `jump` height whose ballistic apex (`v²/2g`) doesn't clear `collider_radius +
ground_cast_length` hits the identical permanent lock on perfectly flat ground — no slope required.
This is why the fix (below) is framed generally rather than as slope-specific compensation, and why
the regression test includes a flat-ground low-jump-height case, not just a slope matrix.

Harness: `crates/ironhold_core/tests/tmp_slope_jump.rs` (currently untracked, temporary). One real
bug in it, found by review, to fix before converting it to a permanent test: the per-tick jump
input (`let jumping = tick >= 20;`) spams Jump every tick from t=20 onward, contradicting its own
comment ("Send Jump on exactly ONE tick") — this doesn't invalidate the core finding (the
`ever_ungrounded=false` lock is real and independent of input spam pattern) but does mean the
"jumps_taken" counts in the harness's printed table reflect continuous-spam behavior, not a clean
single-jump-then-wait test. Fix this before finalizing the permanent regression test.

## What changed after plan review

Both `system-architect` and `ux-gamedesigner-reviewer` independently rejected the v1 approach
("force `LocomotionState.is_grounded = false` for a fixed 0.1-0.15s window after a jump") on
overlapping grounds:

1. **The v1 window duration was wrong and dangerously close to a real exploit.** 0.10-0.15s
   straddles the ~0.14s real flat-ground detach time — a window even slightly shorter would fire a
   *fake* landing-edge reset while the player is still genuinely rising on flat ground, handing
   every project unlimited pogo-jumping regardless of `double_jump_enabled`.
2. **Forcing `is_grounded` false leaks into `can_jump`'s branch selection** (`player.rs:249-253`),
   not just the landing-edge check — a `double_jump_enabled: true` player (4 shipped projects:
   `primitive_world`, `particles_demo`, `effect_mayhem_demo`, `stats_demo`) could consume their
   second jump at ground level on a fast double-tap, instead of at a real mid-air height. This is a
   real regression the v1 approach didn't isolate against.
3. **A fixed constant's correctness is coupled to authorable content** (`jump` height,
   `collider_radius`, `ground_cast_length` are all designer-facing) — no single fixed number is
   safe across every project's authored values, and a value tuned for shipped defaults could
   silently misbehave (either direction) for a project with different values.

## Approach (v2)

Reformulate the fix as a **level-triggered, physically-derived grace counter that gates only the
`jumps_used` reset** — never writes to `LocomotionState.is_grounded`, never changes which branch
of `can_jump` runs.

**New `CharacterController` field**: `jump_air_grace: u16` — a tick countdown, not a float-seconds
timer (see "Why ticks, not seconds" below).

**Replaces the edge-detection reset** (`player.rs:191-195`) **with a level-gated reset, keeping
the landing-animation edge separate:**
```rust
// Landing animation: unchanged from before this fix — fires on any real airborne->grounded
// edge, including a plain fall with jumps_used already 0.
if !was_grounded && loco.is_grounded {
    requests.queue.push_back("jump_exit".to_string());
}
// jumps_used reset: level-gated, not an edge (see "What changed after post-implementation
// review" below for why jump_air_grace alone needed the extra velocity/liftoff-height checks).
if controller.jump_air_grace > 0 {
    controller.jump_air_grace -= 1;
} else if loco.is_grounded && controller.jumps_used > 0 {
    let risen = controller.jump_liftoff_y.map(|y0| global_transform.translation().y - y0).unwrap_or(f32::INFINITY);
    if velocity.linvel.y <= 0.0 || risen >= controller.collider_radius + controller.ground_cast_length {
        controller.jumps_used = 0;
        controller.jump_liftoff_y = None;
    }
}
```
**On every jump firing** (both the grounded first jump and the airborne double jump, `player.rs`
~line 262), set the grace counter from the *same physical quantities the ground-check itself uses*
— so it can never desync from a project's authored `collider_radius`/`ground_cast_length`, and
needs no separate tuning constant to keep in sync with them:
```rust
let h = controller.collider_radius + controller.ground_cast_length;
let t_detach = (vel - (vel * vel - 2.0 * GRAVITY * h).max(0.0).sqrt()) / GRAVITY;
controller.jump_air_grace = seconds_to_ticks(JUMP_AIR_GRACE_SAFETY * t_detach); // SAFETY ≈ 2.0
```
At shipped defaults: `t_detach ≈ 0.132s` (matches the harness's measured ~0.14s), so
`grace ≈ 0.264s` — comfortably clear of the real detach time, and small against the ~1.2s of real
airtime every shipped project's jump already has, so flat-ground/shallow-slope feel is unaffected.
For a project whose jump can never ballistically clear `h` (`vel² ≤ 2·g·h` — the flat-ground
low-jump-height case above), the `.max(0.0)` clamp degrades `t_detach` to `vel/g` (time to apex),
so grace caps at the full up-and-down flight duration rather than growing unbounded — a bounded
fallback, not a guess.

**Why this resolves both review findings, not just patches around them:**
- `LocomotionState.is_grounded` is never written by this fix — animation-resolver's jump/land
  clip selection (`animation_resolver.rs:176`) is bit-for-bit unaffected on any terrain.
- `can_jump`'s grounded/airborne branch selection continues reading *real* contact state — a
  `double_jump_enabled` player's second jump still only fires at genuine airborne height, exactly
  as today. No new double-jump regression.
- The grace value is derived per-jump from the controller's own already-authored fields — no new
  constant to keep in sync with a project's `ground_cast_length`/`jump` tuning, and no designer
  needs to author or tune it (see Q2 below).

**Why ticks, not float seconds:** `player_movement_system` runs in `FixedUpdate`
(`crates/ironhold_core/src/lib.rs:281-289`, unaffected by this fix). Converting the derived grace
duration to an integer tick count once (at jump-fire time) and decrementing by 1 per tick avoids
two real hazards a float-seconds countdown would hit: (1) the regression harness drives
`player_movement_system` via `run_system_once` *outside* the real `FixedUpdate` schedule
(`tmp_slope_jump.rs:66` deliberately neuters `Time<Fixed>`), so `Res<Time>` there carries the
*real wall-clock* frame delta — a `time.delta_secs()` countdown would decrement by microseconds per
manual step and never expire inside the test; (2) tick counts are exactly comparable and
reproducible, which matters for this engine's Beta 0.5 deterministic-tick roadmap item, whereas
float-seconds accumulation is not.

**`GRAVITY`**: currently `scene_loader.rs:2823`'s `const GRAVITY: f32 = 9.81` is private to
`scene_manager`. Bump its visibility to `pub(crate)` and import it into `player.rs` rather than
duplicating the constant — single source of truth, no drift risk between the jump-velocity formula
and the grace-window formula.

### Q2 — fixed constant vs. RON field vs. derived (decided)

**Derived from the controller's existing fields, not a bare constant and not a new RON field.**
Both reviewers rejected a designer-authorable knob here — framing it as "how floaty does a jump
feel" is the wrong mental model; it's a minimum forced-airborne bookkeeping window bounded by
fields the designer *already* authors (`jump`, `collider_radius`, `ground_cast_length`), and a
knob would let a designer silently reintroduce this exact bug by setting it too low. The
`Friction` 0.15-coefficient precedent (fixed constant, no RON field, logged to Icebox) supports
*not adding a field*, but doesn't support a *bare* fixed constant either, since (unlike friction)
this window's correct value is a function of already-authorable geometry — hence "derived", not
"fixed at one number for every project."

## What changed after post-implementation review

Three parallel post-implementation reviews (`alignment-reviewer`, `system-architect`,
`debug-detective`) ran against the v2 implementation above. Real findings, all fixed before merge:

- **Alignment-reviewer (blocking): `jump_exit` silently stopped firing on a plain fall.** The
  first implementation gated the landing-animation request behind `jumps_used > 0`, so walking off
  a ledge without ever having jumped stopped playing the landing clip — a real, designer-visible
  regression the plan's own claim ("animation-resolver unaffected on any terrain") missed. Fixed by
  separating the animation edge-trigger from the `jumps_used` reset entirely (see the corrected
  code sketch above); added
  `player_slope_jump_tests::falling_off_a_ledge_still_plays_landing_animation_without_ever_jumping`.
- **System-architect (major): `jump_air_grace` alone is framerate-fragile.** The tick counter
  assumes `FixedUpdate` ticks and Rapier's own physics stepping advance in lockstep — true only
  when they share a clock. In this project they don't: Rapier runs `TimestepMode::Variable` in
  `PostUpdate` (`capabilities/physics.rs`), a separate, framerate-coupled clock. At a low enough
  real framerate (or one `Time<Virtual>::max_delta`-clamped hitch), grace could expire while real
  physics time — and thus real height gained — lags far behind what the tick count assumes,
  reopening the exact hover exploit the fix exists to prevent. Fixed by adding two physical (not
  clock-derived) backstops that gate the reset alongside grace: `velocity.linvel.y <= 0.0` (ascent
  genuinely over) OR net height risen since the jump (`CharacterController.jump_liftoff_y`) has
  cleared `collider_radius + ground_cast_length` (covers the continuously-climbing-slope case,
  where `linvel.y` stays pinned positive by the contact solver for as long as the player keeps
  walking uphill, so the velocity check alone regressed the original bug — caught by re-running
  the test suite after a first attempt at just the velocity check). Added
  `player_slope_jump_tests::grace_expiry_does_not_reset_early_when_real_physics_time_lags_ticks`
  (deliberately decouples the two clocks via a non-1/64s `physics_dt`) and
  `::jumps_used_resets_promptly_after_a_genuine_flat_ground_landing` (pins the level-check-not-edge
  design property directly).
- **Debug-detective (blocking): negative/NaN jump heights defeated both new checks.** A negative
  `jump: Fixed(height: ...)` or negative `RelativeToHeight(percent: ...)` makes the resolved
  velocity NaN; `NaN <= reach` is `false` in both the scene-load `warn!` and the CLI check, so the
  one misconfiguration that both crashes engine (`Dir3::new_unchecked` debug-assert on a NaN
  direction) and reproduces the original permanent-lock symptom (`jump_air_grace_ticks` also
  degrades NaN straight through) sailed past silently. Fixed: both checks use `!(apex > reach)`
  instead of `apex <= reach`; `jump_air_grace_ticks` clamps its velocity input via `f32::max(0.0,
  vel)` (which also launders NaN to `0.0`, per `f32::max`'s IEEE-754 semantics) and floors its
  result at 1 tick (closing a secondary finding: a near-zero jump height produced a ~32Hz
  `player.jumped` event storm instead of the intended bounded fallback).
- **Debug-detective (non-blocking, fixed anyway): test-harness robustness.** The jump-counter in
  `player_slope_jump_tests.rs` was correct only because `setup_test_app()`'s `Messages<GameEvent>`
  registration order happened to never rotate the message buffer — fragile, and silent if that
  ever changed (the file's hover-exploit upper-bound assertions would start passing vacuously).
  Fixed by explicitly calling `.update()` on the resource each tick and accumulating (`+=`)
  instead of relying on the accidental non-rotation. Also pinned the virtual clock
  (`TimeUpdateStrategy::ManualDuration(Duration::ZERO)`) so `GamePlugin`'s own `FixedUpdate`
  registration of `player_movement_system` can't also self-trigger off real wall-clock time
  alongside the harness's explicit `run_system_once` calls.
- **Debug-detective (non-blocking): CLI severity was inconsistent with the plan's own framing.**
  Independently flagged by both system-architect and debug-detective: the CLI check had been
  wired as a hard `CrossFileError` while the plan and the runtime `warn!` both frame this as
  "not a hard error" (the grace fallback keeps it working, just via a fallback rather than a real
  landing). Moved to the `--strict`-only `StrictWarning` tier instead, matching the runtime's own
  severity; CLI test split into `_without_strict_exits_0` / `_strict_exits_1`.
- **Logged, not fixed here** (non-blocking, narrower scope than this bug fix): moving `GRAVITY` +
  jump-height resolution + the `1.8`/`0.4` collider defaults into `schema/catalog.rs` so
  `scene_loader.rs`, `player.rs`, and `ironhold_cli` share one formula instead of duplicating the
  arithmetic (system-architect); running Rapier in the fixed schedule
  (`in_fixed_schedule()` + `TimestepMode::Fixed`) to close the framerate-mismatch root cause at
  the source rather than compensating for it per-system (system-architect); a landed-but-can't-jump
  window and a slope-pogo animation-pinning artifact, both bounded and cosmetic (debug-detective);
  a `double_jump`-on-a-non-detaching-slope test (debug-detective) — see
  `planning/claude_suggestions.md`.

### Design-time diagnostic (new, addressing the broader misconfiguration class)

Independently of the grace fix, a project can still author a `jump` height whose apex never clears
`collider_radius + ground_cast_length` at all (the flat-ground low-jump-height case above) — the
grace counter bounds this to "at most one full jump's hang time before the reset fires," which is
correct but not instant. Added a scene-load `warn!` (mirroring `warn_missing_player_stat_templates`'s
shape) plus a matching `ironhold_cli validate` check: when a player prefab's resolved jump apex
(`v²/(2·GRAVITY)`, i.e. `resolve_jump_velocity`'s inverse) does not clear `collider_radius +
ground_cast_length`, warn that the jump will not cleanly detach from the ground sensor and suggest
raising `jump` or lowering `ground_cast_length`. **Severity (revised after review):** the CLI side
is a `--strict`-only `StrictWarning`, not a hard `CrossFileError` — matching the runtime `warn!`'s
own "not a hard error" framing (the grace fallback keeps the jump working; this is a design-time
nudge toward the cleaner fix, not a broken feature).

## Tasks
- [x] Bump `GRAVITY` (`scene_loader.rs:2823`) to `pub(crate)`; import into `player.rs`
- [x] Add `jump_air_grace: u16` to `CharacterController`; update the ~13 test-file struct literals
      across `crates/ironhold_core/tests/*.rs` that construct `CharacterController` directly (no
      `Default` impl exists — the compile break will be loud and complete, which is correct; do
      **not** add a `Default` impl as part of this fix, that's a separate, unrelated cleanup —
      log it to `planning/claude_suggestions.md` if still worth doing after)
- [x] Replace the `was_grounded`/edge-detection reset (`player.rs:191-195`) with the level-gated
      `jump_air_grace` countdown; delete the now-dead `was_grounded` local
- [x] Set `jump_air_grace` from the derived formula at both jump-firing sites (grounded first jump
      and airborne double jump, `player.rs` ~line 254-265) — both must set it, since a double jump
      re-arms the same detach-timing problem from a new height
- [x] Add the scene-load `warn!` + `ironhold_cli validate` check for jump-apex-vs-sensor-reach
      (see "Design-time diagnostic" above)
- [x] Fix `tmp_slope_jump.rs`'s single-jump-tick bug (`let jumping = tick >= 20` spams every tick;
      fix to fire once) before converting it into a permanent test
- [x] Move the fixed/trimmed harness into a new dedicated file
      `crates/ironhold_core/tests/player_slope_jump_tests.rs` (not `action_tests.rs`, which
      exists specifically to run *without* a real physics world) — keep only focused,
      assertion-based tests, not the exploratory println matrix:
  - Flat ground: repeated jumps still work, cadence unchanged from pre-fix
  - Slope ≥12° (previously locked): jump usable again after landing — repeatedly, not just once
  - Slope re-jump cadence is bounded/comparable to flat cadence when holding jump — not an
    unbounded faster-than-flat pogo/hover (this is the "turns a lock into a hover" failure mode
    both reviews flagged; assert an upper bound on jumps-per-second, not just "more than one")
  - **Flat ground, low jump height** (e.g. `JumpConfig::Fixed { height: 0.2 }`, chosen so its
    apex doesn't clear `collider_radius + ground_cast_length`): also locks today, also fixed by
    this change — proves the fix is general, not slope-specific
  - `double_jump_enabled: true`: second jump still only fires at real airborne height, not at
    ground level immediately after the first jump (regression guard for review finding #2)
  - Add the new file's name to the root `CLAUDE.md` one-file-at-a-time test loop list (which is
    already missing `local_coop_tests`, `camera_modes_tests`, `gamepad_binding_tests` — drive-by
    fix while touching that list)
- [x] Audit `tests/action_tests.rs`'s `test_player_jump_emits_game_event` (~line 142, deliberately
      runs with no Rapier context so `is_grounded` is unconditionally `true`) and
      `tests/scene_lifecycle_tests.rs`'s equivalent headless-grounded test (~line 139) against the
      new level-gated reset — confirm they still pass and still test what they claim to
- [x] Document the invariant in `crates/ironhold_core/src/CLAUDE.md` beside the existing "Physics &
      movement must use `FixedUpdate`" rule: *`jumps_used` reset is level-gated by a physically-
      derived grace counter, not a `!was_grounded && is_grounded` edge, because the ground-check
      cannot guarantee ever reporting `false` on steep-enough terrain* — the non-obvious constraint
      a future movement change could otherwise reintroduce
- [x] Tests (full suite + `cargo check -p ironhold_cli`)
- [x] Remove `tmp_slope_jump.rs` once its content is migrated into the permanent test file

## Playtest checklist
- **`quick_scene` — critical, not optional.** This is the one gallery project with a real player
  standing on the real heightmap-terrain collider. Two independent post-implementation reviews
  found (and this branch's own new `TriMesh`-ground tests now cover) a bug where the ground
  shape-cast's fix could misclassify *all* terrain, including flat, as unwalkable — which would
  make jump not work **at all** here. Confirm jump works normally on `quick_scene`'s terrain
  before anything else in this checklist.
- `3rd_person_game_demo` — the original reported repro: run at any hill, spam jump while
  ascending, confirm jump keeps working (not just once) and there's no perceptible extra hang time
  on flat ground
- `3rd_person_game_demo`/`terrain_demo` — the playtest-found regression: jump off a cliff edge or
  any steep/unwalkable terrain and fall for an extended time while holding jump; confirm jump does
  **not** silently re-fire mid-fall (was: unbounded re-jump the longer the fall)
- `terrain_demo` — heightmap terrain gives a continuous spread of real slope angles; confirm no
  lock at any point while running across varied walkable terrain
- `primitive_world` — has both `double_jump_enabled: true` **and** a sound bound to
  `player.jumped` (`logic/state_machine.ron`) — the best canary for both the double-jump-height
  regression and audible re-jump spam while climbing a slope; confirm jump sound doesn't machine-gun
  while holding jump against an incline
- `local_coop_demo` (rooms 3/9/10) — verify with two players/two controllers, since
  `CharacterController` state is per-entity; confirm neither player's jump state leaks into the
  other's

## What changed after playtest

Frank's playtest found a real, distinct regression: the fix removed the permanent uphill lock, but
introduced an unbounded re-jump exploit while falling/sliding down a long enough decline ("endless
jump in the air"). Root cause: on any incline the ground sensor can't cleanly detach from — uphill
*or* downhill — `velocity.linvel.y` is trivially `<= 0.0` for the entire descending portion of the
fall, so once grace expired the OR-condition re-armed `jumps_used` on essentially every tick for as
long as the sensor stayed in contact. Same underlying defect as the original bug (a proximity-only
sensor can't tell "resting contact" from "nearby surface"), just biting in the opposite direction.

Frank asked how other engines solve this class of problem. Answer: they don't patch the jump-count
bookkeeping — they fix what counts as "grounded" using the contact's **surface normal**, not just
proximity (Unity `CharacterController.slopeLimit`, Unreal `WalkableFloorAngle`, Godot
`floor_max_angle`). Added a new designer-authorable `MovementConfig.max_walkable_slope_deg` (default
45°) and gated the ground shape-cast's result on the hit normal's angle from world-up — a surface
steeper than the limit never counts as grounded, uphill or downhill, closing the exploit at the
source rather than special-casing it in the reset logic. Unlike `jump_air_grace`
(deliberately *not* RON-authorable — see Q2 above), this genuinely is a designer-facing gameplay
tuning knob (how steep can the player climb before sliding), matching the equivalent field every
cited engine exposes, so it's a real `MovementConfig` field, not internal bookkeeping.

This does **not** replace the grace/velocity/liftoff-height mechanism for slopes at or below the
walkable limit (an ordinary 12–30° hill) — that terrain's continuous contact while climbing or
descending is genuinely correct grounding, so the sensor still can't cleanly detach there, and the
already-accepted bounded "pogo" cadence tradeoff still applies. The walkability gate only changes
behavior for terrain steeper than `max_walkable_slope_deg`, where the character now correctly
slides/falls instead of either locking (the original bug) or exploiting (this regression).

Added 4 new tests: `unwalkable_slope_never_reports_grounded`,
`unwalkable_slope_does_not_allow_endless_rejump_while_sliding` (the direct regression guard),
`walkable_slope_pogo_cadence_is_unaffected_by_the_slope_limit_check` (no regression on the already-
accepted walkable-slope tradeoff), and `custom_walkable_slope_limit_is_respected` (the new field is
genuinely per-project authorable). All 12 tests in `player_slope_jump_tests.rs` pass.

## What changed after the slope-normal fix's post-implementation review

Two parallel post-implementation reviews (`alignment-reviewer` clean/minor; `system-architect` and
`debug-detective` both independently blocking) ran against the slope-normal walkability gate above.
Both `system-architect` and `debug-detective` found — independently, with their own reproductions
against real `rapier3d`/`parry3d` — the same **critical, ship-blocking bug**:

**The ground shape-cast was centered at the player's feet, which sit exactly on the surface at
rest — so the cast always started already embedded ("penetrating") in whatever was below.** On a
solid convex shape (this file's own test slopes, `Collider::cuboid`) EPA still happens to resolve
the minimum-translation vector straight up, matching the true surface normal by geometric
coincidence. On this project's **real terrain collider** — `capabilities/terrain.rs`'s
`ComputedColliderShape::TriMesh(TriMeshFlags::default())`, a zero-thickness triangle mesh — there
is no "up" to resolve through from a buried point; the shortest way out is sideways, so the
returned normal came back ~90° from vertical **regardless of the triangle's actual slope**,
misclassifying *any* terrain as unwalkable — including perfectly flat terrain. Both reviewers
measured this against real Rapier (not just static analysis) and confirmed: standing still on flat
`quick_scene`-style terrain would never register as grounded, meaning **the player could never
jump at all** on any terrain-based project (`quick_scene`, `primitive_world`, `local_coop_demo`,
`integration_tests`) — strictly worse than the bug this whole feature exists to fix. This bug was
invisible to every test in this file, since all of them used solid `Collider::cuboid` ground, the
one geometry family where the bug happens not to manifest.

**Fix:** lift the shape-cast's start point above the surface by the ball's own radius (plus a
small `GROUND_CAST_SKIN` margin), instead of casting from the bare feet position, and extend
`max_time_of_impact` by the same amount so the total downward reach below the feet is unchanged
(`collider_radius + ground_cast_length`, exactly as before — every formula derived from that
combined reach needed no change). This guarantees the cast begins genuinely separated from the
surface, so EPA/GJK always resolves the real contact normal — verified against both solid and
`TriMesh` geometry via 6 new tests (`standing_still_is_grounded_on_flat_trimesh_terrain`,
`walkable_trimesh_slope_is_grounded_and_repeated_jumps_work`,
`unwalkable_trimesh_slope_never_reports_grounded`, plus two `Cuboid`-ground positive controls and
a positive control added to `custom_walkable_slope_limit_is_respected` — all flagged as missing by
the same two reviews). `player_slope_jump_tests.rs` now has 16 tests, half of them against a real
`Collider::trimesh` ground built to match the terrain capability's own collider construction, not
just the convex-shape family that let this bug ship undetected the first time.

Also fixed, smaller findings from the same two reviews:
- `max_walkable_slope_deg`'s documented `90.0` "disable this check" escape hatch didn't actually
  work (a hit with no computable normal detail still counted as ungrounded even at the maximum
  limit) — added an explicit short-circuit so `>= 90.0` always counts any hit as ground.
- The `MovementConfig.max_walkable_slope_deg` Rust doc comment overclaimed slope physics
  ("the player slides/falls on it") the engine doesn't implement — corrected to match the accurate
  `docs/20_data_formats.md` wording (`is_grounded`-only; no slowdown or slide force).
- No design-time validation existed for `max_walkable_slope_deg` outside `(0, 90]` — a value at or
  below 0 silently breaks grounding entirely (no surface ever walkable) with no diagnostic. Added
  `warn_invalid_walkable_slope_limit` (runtime) + `invalid_walkable_slope_limit` (`ironhold_cli
  validate --strict`), mirroring `jump_cannot_clear_ground_sensor`'s existing shape and severity.
- A real bug in my own test harness (not production code): `falling_off_a_ledge_still_plays_
  landing_animation_without_ever_jumping` manually teleported the player via a direct `Transform`
  mutation with no intervening `app.update()` — `player_movement_system` reads `GlobalTransform`
  (only updated by transform propagation, which runs as part of the normal schedule), so the very
  first ground-check after the fix's cast-origin change saw the *pre-teleport* position and
  short-circuited the test before the real fall ever happened. Fixed by forcing one `app.update()`
  (and explicitly resetting `LocomotionState.is_grounded`, since its `Default` is `true`) between
  the teleport and the measurement loop.
- Three findings logged as follow-ups rather than fixed here (see `planning/claude_suggestions.md`
  ▸ Physics / Movement): a wall can veto a legitimate floor contact underneath it (single-hit
  `cast_shape` can't distinguish "which of several simultaneous contacts is the floor"); no
  hysteresis at the walkable-angle boundary for a tessellated slope whose triangles straddle it;
  the per-tick EPA allocation cost of `compute_impact_geometry_on_penetration: true` on `TriMesh`
  terrain (flagged for a future `wasm-perf-reviewer` pass, not blocking).

## What changed after the second playtest (coyote time)

Frank's second playtest confirmed the slope-normal fix resolved both the original lock and the
hover/pogo exploit, but found a third, distinct false-positive: walking over `3rd_person_game_demo`'s
uneven terrain — bumps and small ledges well under `max_walkable_slope_deg`, not a real slope issue
— repeatedly flickered the character into the falling state. Frank asked how other engines avoid
this class of false positive. Answer: **coyote time** — a short debounce buffer on the
grounded→airborne transition (named for Wile E. Coyote not falling until he looks down), universal
across shipped character controllers, that absorbs exactly this kind of single-tick ground-sensor
noise. The alternative — step-offset/auto-step (Unity `stepOffset`, Unreal `MaxStepHeight`), which
lets the controller auto-climb small ledges instead of ever losing contact with them — solves a
related but different problem (walking *onto* raised geometry) and was logged to the backlog as a
separate, larger feature rather than implemented here (see `planning/backlog.md` ▸ Queued).

**Implemented:** `MovementConfig.coyote_time_secs` (default `0.1`s, designer-authorable — unlike
`jump_air_grace`, see Q2 above) plus `CharacterController.coyote_ticks_remaining`, a `FixedUpdate`
tick countdown refreshed to full every tick the raw ground shape-cast genuinely reports contact.
While the buffer is non-zero, `LocomotionState.is_grounded` (and therefore `can_jump`'s grounded
branch and `animation_resolver`'s clip selection) stays `true` even on a tick the raw sensor just
lost contact. Full design + the critical interaction bug this introduced and fixed are documented in
`crates/ironhold_core/src/CLAUDE.md` ▸ "Coyote time — debounced grounding for uneven terrain" —
summary: the coyote buffer must never feed the `jumps_used` reset's grace/velocity/liftoff-height
gate (a separate `raw_grounded` local was hoisted specifically to prevent this), because doing so
once caused a premature `jumps_used` reset mid-ascent on an ordinary flat-ground jump
(`grace_expiry_does_not_reset_early_when_real_physics_time_lags_ticks` caught this).

Added 2 new tests: `coyote_time_lets_a_jump_fire_briefly_after_leaving_the_ground` (the intended
forgiving-jump-timing benefit) and `coyote_time_does_not_mask_an_extended_fall_forever` (proves the
buffer is bounded — a real, extended fall still eventually reports ungrounded). All 18 tests in
`player_slope_jump_tests.rs` pass, along with the full `ironhold_core` suite (all 19 test files) and
`ironhold_cli`'s `cargo check` + full test suite (34 cross-file + 9 smoke tests).

A design-time diagnostic *was* added after all: not for an extreme value (any non-negative value
degrades gracefully, just a bigger/smaller buffer), but for a negative one, which silently
laundered to "disabled" with no feedback — `warn_negative_coyote_time_secs` (runtime) +
`negative_coyote_time_secs` (`ironhold_cli validate --strict`), mirroring
`invalid_walkable_slope_limit`'s shape. See "What changed after the coyote-time post-implementation
review" below.

## What changed after the coyote-time post-implementation review

Three parallel post-implementation reviews (`alignment-reviewer`, `system-architect`,
`debug-detective`) ran against the coyote-time addition above. All three independently converged on
the same real, blocking-severity finding, plus several smaller ones. `wasm-perf-reviewer` was
deliberately not run separately — system-architect's own pass already assessed the change's cost
(two `u16` ops and one branch per player per tick, no allocation) as negligible, and re-running a
dedicated perf pass for that delta wasn't warranted.

**All three reviews (blocking): `can_jump`'s exclusive branch selection silently swallowed a
double-jump press for the length of the coyote window, up to permanently for a large
`coyote_time_secs`.** `can_jump`'s grounded/airborne branches are mutually exclusive; the first
version of this fix gated the grounded branch on the fully coyote-buffered `is_grounded` with no
`jumps_used == 0` qualifier. After a real ground jump (`jumps_used == 1`), the grounded branch's own
`jumps_used == 0` check already blocks it — but the airborne (double-jump) branch also stayed
unreachable, because it required `!is_grounded`, and the coyote buffer kept reporting `is_grounded
== true` for the whole window even after the player had genuinely left the ground. Debug-detective
measured the concrete regression at shipped defaults: second-jump availability moved from ~9 ticks
(141ms) after liftoff to ~15 ticks (234ms) at the 0.1s default — and would have become **permanent**
at a large `coyote_time_secs` (e.g. 100.0s), since the airborne branch would never become reachable
at all. Fixed by qualifying the grounded branch's coyote-forgiveness specifically to `jumps_used ==
0`: `raw_grounded || (coyote_ticks_remaining > 0 && jumps_used == 0)`. This means the coyote buffer
can only ever unlock a *first* jump (its intended purpose), never gate whether a *second* jump is
reachable — for `jumps_used > 0` the branch choice now depends purely on `raw_grounded`, exactly as
if coyote-time didn't exist. Added
`player_slope_jump_tests::double_jump_fires_even_while_the_coyote_buffer_still_reports_grounded`,
which directly proves the second jump fires on the tick the buffered `is_grounded` is still `true`.

Smaller findings, all fixed:
- **(debug-detective) Test fragility, not a production bug:** `unwalkable_slope_never_reports_grounded`
  was measured to be within ~7% of a false failure — its `moving: true` config ran the player far
  enough up the 60°-slope test slab (120m wide) over 200 ticks to approach the slab's edge cap,
  whose normal is a legitimately walkable ~30° from vertical. Fixed by generously sizing both ground
  colliders (`Cuboid`/`TriMesh`, 60m → 400m half-extent) rather than disabling movement, since active
  movement while sliding down an unwalkable slope is the actual reported scenario.
- **(debug-detective) Test coverage gap:** the two coyote tests only bounded the window loosely
  (`[4, 16]` ticks and "eventually goes false within 30 ticks" respectively) — neither would have
  caught a regression that widened the window 3x. Tightened to assert the exact 6-tick boundary
  (seeded value via `coyote_ticks_remaining`, and exact tick-7 expiry) instead.
- **(system-architect) Missing doc comment:** `LocomotionState.is_grounded` had no doc comment at
  all despite now being a debounced *feel* signal, not the raw sensor result — a future consumer
  (e.g. the queued step-offset/auto-step backlog item) could easily read it expecting real-time
  ground truth. Added a doc comment explaining the distinction and pointing at `raw_grounded` (a
  `player_movement_system`-local, not currently exposed) for anyone who needs the un-debounced value.
- **(debug-detective) Stray blank line** in `ironhold_cli/src/commands/validate.rs` (pure diff
  noise from an earlier edit) — removed.
- **(system-architect + debug-detective) Stale/inaccurate doc comments:** a test comment claiming
  "this fix never touches `is_grounded` or `can_jump`'s branch selection" predated coyote-time and
  was no longer fully accurate; corrected to scope the claim correctly (still true for the
  `jumps_used > 0` branch this specific test exercises, now explained why).
- **(alignment-reviewer) Discoverability:** no shipped project authored `coyote_time_secs` at all.
  Added an explicit (default-matching, so no behavior change) `coyote_time_secs: 0.1` to
  `3rd_person_game_demo`'s player prefab — the project this was playtested against — so the field is
  discoverable by example, not just from docs.

Logged, not fixed here (see `planning/claude_suggestions.md` ▸ Physics / Movement): `CharacterController`
test-construction churn now warrants a shared `tests/support` helper (not a `Default` impl — that
call was re-confirmed still correct, for a stronger reason than before: the sole production
construction site's whole job is wiring every field from `MovementConfig`, and a `Default` there
would let a new field silently default instead of being plumbed); and the observation that a future
ground-sensor consumer (the queued step-offset/auto-step feature) must not assume `is_grounded`
means "the sensor says so right now" — it needs `raw_grounded`'s semantics, not today's only
exposed signal.

All 19 tests in `player_slope_jump_tests.rs` pass (18 pre-existing + 1 new regression guard), along
with the full `ironhold_core` suite (all 19 test files) and `ironhold_cli`'s `cargo check` + full
test suite (36 cross-file + 9 smoke tests, including 2 new `negative_coyote_time_secs` fixtures).
`python tools/asset_checker/check.py` clean (514 references). WASM dev build rebuilt.

## What changed after the third playtest (sensor veto)

Frank's third playtest found a third, distinct false-positive, unrelated to slope or coyote-time:
the falling animation triggered while walking on ordinary **flat** ground, specifically near a
physics prop (a chest sitting on a small platform in `3rd_person_game_demo`, per the reported
screenshot). Investigated via `debug-detective` before any fix was written (per this project's
bug-fix workflow) — confirmed root cause and built a reproduction harness before any code changed.

**Root cause:** the ground shape-cast's `QueryFilter` (`capabilities/player.rs`) excluded only the
player's own rigid body — `QueryFilter::new().exclude_rigid_body(entity)` — nothing else. Any
prefab with a `trigger_zone: (radius: r)` field spawns a ghost child collider (`Collider::ball(r)` +
`Sensor`, `entity_spawner.rs`'s `attach_prefab_features`) for interaction/collision detection. The
ground cast's downward-swept ball could start embedded inside a large, nearby trigger-zone sensor
sphere — up to `radius + collider_radius` away (≈2.9m for the reported chest's `trigger_zone: 2.5`)
— and because a `time_of_impact == 0` penetrating hit beats the real floor's small-but-nonzero toi,
the sensor won the cast instead of the floor. Its ball-in-ball EPA normal at that embedded position
is radial (near-horizontal) — unwalkable by construction under the slope-walkability gate added
earlier this feature, permanently vetoing the real floor for as long as the player stood anywhere
near the prop. This is a regression introduced by this feature, not a pre-existing bug: before the
walkable-slope gate existed, the same sensor hit was already being returned, but proximity-only
grounding (`hit.is_some()`) treated *any* hit as ground, so the wrong-collider bug was invisible.

A second, independent bug was found in the same investigation: on a penetrating hit, parry's EPA
normal is not always unit length (measured `|n| ≈ 0.52` for the ball-in-sensor case) — the angle
check's bare `.dot(Vec3::Y).acos()` computes `acos(|n| * cos(theta))`, not the real angle, silently
biasing every penetrating-hit angle toward 90°. Harmless when the surface really is steep, but would
misclassify a genuinely walkable penetrating contact as too steep.

**Fix:** two changes, both in the ground-detection block:
1. `.exclude_sensors()` added to the `QueryFilter` — a sensor is a ghost collider and must never
   count as floor, full stop. Matches the existing `.exclude_sensors()` precedent already in
   `capabilities/npc.rs`'s line-of-sight raycast. No RON schema change, no new designer-facing
   knob — this is an unconditional engine invariant, not a tunable.
2. `.normalize_or_zero()` on the hit's normal before the angle's dot product.

**Deliberately not fixed here:** a solid (non-sensor) prop tall enough to reach the cast ball's
centre and pressed directly against the player can still veto the floor — this needs actual contact
(a much narrower window than a sensor's multi-metre radius), was already predicted and logged in
`planning/claude_suggestions.md` (`"a wall can veto a legitimate floor contact underneath it"`)
before this playtest ever found the sensor case, and the correct general fix (reading multiple
candidate contacts instead of the single nearest `cast_shape` hit) is a larger change than this
regression fix's scope. Documented as a known limitation, not a dangling TODO, in the new test file.

New test file `crates/ironhold_core/tests/prop_ground_veto_tests.rs` (9 tests, real Rapier physics,
same harness style as `player_slope_jump_tests.rs`): covers standing/walking near a `trigger_zone`
at various distances, independence from the prop's rigid-body kind, a replica of the exact reported
`loot_display`+chest configuration, a direct math test of the normalize fix, and the two solid-prop
tests documenting the known-remaining limitation (one asserting it still applies, one confirming a
shorter prop never triggers it).

All 19 tests in `player_slope_jump_tests.rs` and all 10 in the new file (after review) pass, along
with the full `ironhold_core` suite (20 test files) and `ironhold_cli`'s `cargo check` + full test
suite.

## What changed after the sensor-veto fix's post-implementation review

Three parallel reviews ran. `alignment-reviewer` came back clean (two non-blocking doc/registry
gaps, fixed). `system-architect` found a real, more-severe-than-framed issue with how this round's
own "known limitation" was characterized; `debug-detective` found one genuine (currently
unreachable) bug elsewhere in the same feature plus several cosmetic nits.

- **(system-architect, major) The solid-prop/wall veto is a regression this feature introduced, not
  a pre-existing limitation, and it silently disables jump entirely, not just the animation.** On
  `main`, the ground cast was proximity-only, so no collider's normal was ever load-bearing; this
  feature's own slope-walkability gate is what made a solid prop's normal matter at all. With
  `double_jump_enabled: false` (every shipped project's default), `can_jump`'s only reachable branch
  requires `raw_grounded` — so pressed against a tall prop, jump does nothing, not just plays the
  wrong animation. Promoted from a `claude_suggestions.md` idea-list entry to a proper
  `planning/backlog.md` ▸ Bugs entry with a concrete repro (`local_coop_demo`'s portal frame posts)
  and a candidate fix (gate the veto on the hit's world-space contact point, via
  `ShapeCastHitDetails.witness1`, being at/below the feet rather than beside/above — not yet
  implemented or verified, since a wall's exact EPA contact-point geometry hasn't been empirically
  confirmed and this needs its own verification pass). Doc/test/CLAUDE.md framing corrected
  throughout to say "deliberately-deferred regression" instead of "known limitation".
- **(debug-detective, non-blocking, verified empirically) `coyote_time_secs` has a real upper bound
  that the code's own comments claimed didn't exist.** A value large enough relative to a jump's
  airtime (measured: `>= 1.0` at shipped jump defaults) can mask the entire jump's airborne/landing
  animation — `is_grounded` never goes false, so neither the airborne clip nor `jump_exit` ever
  fires. Physics itself is unaffected. No shipped project is anywhere near this (all author `0.1`).
  Doc comments in `scene_loader.rs`, `validate.rs`, and `docs/20_data_formats.md` corrected to stop
  claiming "no invalid range"; a possible future `--strict` check (comparing against resolved jump
  airtime) logged to `claude_suggestions.md`, not implemented now (no shipped project affected).
- **(debug-detective, mutation-tested) Confirmed the sensor-exclusion fix has real regression
  value**: reverting `.exclude_sensors()` and re-running fails 5 of the (then-)9 tests in
  `prop_ground_veto_tests.rs`, not coverage theater.
- Fixed: two doc-registry omissions (`prop_ground_veto_tests.rs` missing from the root `CLAUDE.md`
  test loop and `tests/CLAUDE.md`'s file table — both landed mid-review, since these gaps had
  already been created before the reviews ran and would have been caught on the next audit anyway),
  `trigger_zone`/`sensor` RON doc rows gaining a one-line ground-detection note, a test citation
  pointing at the wrong function/line, and the test file's docstring overclaiming the sensor
  ball-in-ball normal anomaly is unreachable "in production" (it's reachable, per debug-detective's
  own repro — it just no longer misbehaves once excluded).
- Added a 10th test, `standing_near_a_trigger_zone_sensor_stays_grounded_on_trimesh_terrain`, per
  `tests/CLAUDE.md`'s own "TriMesh vs Cuboid ground testing" rule (this exact lifted-origin cast has
  broken twice already on that geometry family specifically).
- Logged, not implemented: extracting the ground cast into a shared `ground_probe` helper so test
  code stops hand-duplicating it (both system-architect and debug-detective flagged this
  independently) — see `claude_suggestions.md`.

## Acceptance criteria
- Given a player running/walking up a slope steep enough to previously lock (12°+ at shipped
  defaults), when they jump, then they can jump again after landing — repeatedly, not just once.
- Given a player holding jump while continuously climbing a slope that never truly detaches from
  the ground sensor, then their re-jump cadence is bounded and comparable to flat-ground cadence —
  not faster (no pogo/hover exploit).
- Given flat ground or a shallow slope (≤10°) that already worked before this fix, when a player
  jumps, then behavior — including jump/land animation timing and jump sound cadence — is
  unchanged.
- Given a flat-ground player prefab whose jump height can't clear `collider_radius +
  ground_cast_length` (the non-slope instance of this same bug class), when they jump, then they
  can jump again after landing, bounded by the derived grace fallback — and a scene-load `warn!` +
  `ironhold_cli validate --strict` warning flag the misconfiguration.
- Given `double_jump_enabled: true`, when a player jumps and immediately double-taps jump again,
  then the second jump fires at genuine airborne height, not at ground level — unchanged from
  today's behavior.
- Given a slope steeper than `max_walkable_slope_deg` (default 45°), when a player jumps and then
  falls/slides down that incline for an extended time, then jump does not silently re-arm and
  fire again while still sliding — no unbounded re-jump exploit, however long the descent.
- Given a slope at or below `max_walkable_slope_deg` (an ordinary walkable hill), behavior is
  unchanged from the rest of this fix — bounded pogo cadence while continuously climbing, normal
  single jump-per-landing otherwise.
- Given a player walking over uneven terrain with only single-tick ground-sensor gaps (no real
  jump/fall), then the falling animation/state does not flicker on — the coyote buffer absorbs it.
- Given a player who genuinely walks off a ledge and falls for longer than `coyote_time_secs`, then
  the falling state still engages — the buffer delays, but never masks, a real fall.
- Given a player standing or walking on flat ground near a prop with a `trigger_zone`, then they
  remain grounded regardless of proximity — a nearby sensor never vetoes real floor beneath them.
