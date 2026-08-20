# Feature: Uphill jump lock fix

_Status: Draft_
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

**Replaces the edge-detection reset** (`player.rs:191-195` — `was_grounded`/`!was_grounded &&
is_grounded` becomes dead code and is removed) **with a level-gated reset**:
```rust
if controller.jump_air_grace > 0 {
    controller.jump_air_grace -= 1;
} else if loco.is_grounded && controller.jumps_used > 0 {
    requests.queue.push_back("jump_exit".to_string());
    controller.jumps_used = 0;
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

### Design-time diagnostic (new, addressing the broader misconfiguration class)

Independently of the grace fix, a project can still author a `jump` height whose apex never clears
`collider_radius + ground_cast_length` at all (the flat-ground low-jump-height case above) — the
grace counter bounds this to "at most one full jump's hang time before the reset fires," which is
correct but not instant. Add a scene-load `warn!` (mirroring `warn_missing_player_stat_templates`'s
shape) plus a matching `ironhold_cli validate` check: when a player prefab's resolved jump apex
(`v²/(2·GRAVITY)`, i.e. `resolve_jump_velocity`'s inverse) is ≤ `collider_radius +
ground_cast_length`, warn that the jump will not cleanly detach from the ground sensor and suggest
raising `jump` or lowering `ground_cast_length`.

## Tasks
- [ ] Bump `GRAVITY` (`scene_loader.rs:2823`) to `pub(crate)`; import into `player.rs`
- [ ] Add `jump_air_grace: u16` to `CharacterController`; update the ~13 test-file struct literals
      across `crates/ironhold_core/tests/*.rs` that construct `CharacterController` directly (no
      `Default` impl exists — the compile break will be loud and complete, which is correct; do
      **not** add a `Default` impl as part of this fix, that's a separate, unrelated cleanup —
      log it to `planning/claude_suggestions.md` if still worth doing after)
- [ ] Replace the `was_grounded`/edge-detection reset (`player.rs:191-195`) with the level-gated
      `jump_air_grace` countdown; delete the now-dead `was_grounded` local
- [ ] Set `jump_air_grace` from the derived formula at both jump-firing sites (grounded first jump
      and airborne double jump, `player.rs` ~line 254-265) — both must set it, since a double jump
      re-arms the same detach-timing problem from a new height
- [ ] Add the scene-load `warn!` + `ironhold_cli validate` check for jump-apex-vs-sensor-reach
      (see "Design-time diagnostic" above)
- [ ] Fix `tmp_slope_jump.rs`'s single-jump-tick bug (`let jumping = tick >= 20` spams every tick;
      fix to fire once) before converting it into a permanent test
- [ ] Move the fixed/trimmed harness into a new dedicated file
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
- [ ] Audit `tests/action_tests.rs`'s `test_player_jump_emits_game_event` (~line 142, deliberately
      runs with no Rapier context so `is_grounded` is unconditionally `true`) and
      `tests/scene_lifecycle_tests.rs`'s equivalent headless-grounded test (~line 139) against the
      new level-gated reset — confirm they still pass and still test what they claim to
- [ ] Document the invariant in `crates/ironhold_core/src/CLAUDE.md` beside the existing "Physics &
      movement must use `FixedUpdate`" rule: *`jumps_used` reset is level-gated by a physically-
      derived grace counter, not a `!was_grounded && is_grounded` edge, because the ground-check
      cannot guarantee ever reporting `false` on steep-enough terrain* — the non-obvious constraint
      a future movement change could otherwise reintroduce
- [ ] Tests (full suite + `cargo check -p ironhold_cli`)
- [ ] Remove `tmp_slope_jump.rs` once its content is migrated into the permanent test file

## Playtest checklist
- `3rd_person_game_demo` — the original reported repro: run at any hill, spam jump while
  ascending, confirm jump keeps working (not just once) and there's no perceptible extra hang time
  on flat ground
- `terrain_demo` — heightmap terrain gives a continuous spread of real slope angles; confirm no
  lock at any point while running across varied terrain
- `primitive_world` — has both `double_jump_enabled: true` **and** a sound bound to
  `player.jumped` (`logic/state_machine.ron`) — the best canary for both the double-jump-height
  regression and audible re-jump spam while climbing a slope; confirm jump sound doesn't machine-gun
  while holding jump against an incline
- `local_coop_demo` (rooms 3/9/10) — verify with two players/two controllers, since
  `CharacterController` state is per-entity; confirm neither player's jump state leaks into the
  other's

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
  `ironhold_cli validate` error flag the misconfiguration.
- Given `double_jump_enabled: true`, when a player jumps and immediately double-taps jump again,
  then the second jump fires at genuine airborne height, not at ground level — unchanged from
  today's behavior.
