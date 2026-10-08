# Fix: Airborne ground re-acquisition (a platform below a rising jump must not count as "landed")

_Status: Draft — plan-review 2026-10-02: needs more design work (see "Plan-review" section at the end)_
_Planned at: `9a05f6b` (2026-10-02)_

Backlog item: **Bugs** - "Jumping near a short raised platform spuriously grounds the player mid-ascent" (entry first committed at `5f6f2db`, 2026-09-02, found at `feature/prop-ground-veto`).
Related, **referenced not absorbed**: Bugs "A wall that's part of the same collider entity as the walkable floor..." (see Decision 6) and the drafted `planning/features/step_offset_auto_step.md` (see Coordination).
Written autonomously while Frank was away; not plan-reviewed. **Nothing below was compiled or run**; the reproduction is by static code reading plus the backlog entry's own earlier empirical harness (Task 1 re-establishes it).

## Reproduction status against current HEAD (workflow step 1) - READ FIRST
**The bug still exists; the mechanism is unchanged; the fix is NOT the one the backlog entry suggested.** Checked with `git log --oneline 5f6f2db..HEAD -- player.rs animation_resolver.rs physics.rs`: only `db0bb2b` (wall-friction toggle), `6f720de` (fixed timestep v1) and `f8c97f3` (docs) touched the area. `git diff 5f6f2db..HEAD -- player.rs` (comments stripped) shows only: `FIXED_TICK_RATE` import, `det_math::acos`/`quat_from_rotation_y`, the `Friction` toggle (reads `raw_grounded`, does not change it). `ground_cast`, `is_walkable_contact`, `ground_sensor_reach`, the coyote block, the reset block and `animation_resolver.rs` are logically identical. The fixed timestep only removed the *dual-clock* explanation for the reset's physical checks (`player.rs:540-551` comment); it did not change them.

Mechanism, citing current lines (`crates/ironhold_core/src/capabilities/player.rs`):
1. `:365` `max_time_of_impact: lift + ground_cast_length`, i.e. the sensor reaches `ground_sensor_reach() = collider_radius + ground_cast_length` (`:29`) = **0.7 m** below the feet at defaults (0.4 + 0.3).
2. `:478` `raw_grounded = hit.is_some_and(walkable)`: nothing looks at vertical velocity or at whether the player is rising.
3. `:552-561` after `jump_air_grace` expires, `raw_grounded && jumps_used > 0` resets `jumps_used` when `velocity.linvel.y <= 0.0 || risen_since_liftoff >= ground_sensor_reach()`. Mid-ascent the second clause is true as soon as the player has climbed 0.7 m, so **any walkable surface within 0.7 m below the feet resets the jump count while still rising**. Grace is ~17 ticks (`jump_air_grace_ticks`), the climb of 0.7 m takes ~9, so grace never protects this.
4. `:515-517` `!was_grounded && loco.is_grounded` pushes `jump_exit`; `animation_resolver.rs` step 3 accepts it over the playing `jump_enter` because both are `priority: 200` (`candidate.priority >= active.priority`; `local_coop_demo/.../player_locomotion.ron:21-35`, `3rd_person_game_demo/.../player_policy.ron:20-30`).
Concrete numbers for the repro: `jump` default apex 1.8 m (`jump_velocity` 5.94, `GRAVITY` 9.81), `cube_obstacle_room10` top at 1.2 m. Feet at 1.0-1.9 m are within 0.7 m above the top, so the whole upper two thirds of the ascent reads as "grounded on the cube".
Two reachability facts that matter for the fix: (a) the continuous-surface case is **not** only a cube: a 0.3 m curb hops the same way, because the floor (feet < 0.7) hands off to the curb top (feet up to 1.0) with no false tick, so a "may only retain, never acquire" rule (backlog candidate (a)) is bypassed by surface hand-off; (b) the cast also accepts walkable hits whose contact point is *above* the feet (`ground_cast` rescue, `:429`) - a player grazing a cube edge whose capsule is penetration-nudged by Rapier gets a walkable top-face hit with `witness1.y` up to ~0.3 m above the feet.
Confidence the bug reproduces: **high (~90%) by code reading**, 100% per the backlog's earlier real-physics harness. The residual 10% is only "does the visible symptom still look the same under the 64 Hz fixed step"; Task 1 settles it by writing the failing tests first.

## What
No new RON fields. A jump in flight can no longer be "landed" by a surface that is merely within the generous 0.7 m sensor reach below a *rising* player. The jump clip is not cut short, `jumps_used` is not reset before a real landing, and a second Jump press mid-ascent no longer fires a free impulse when `double_jump_enabled: false`. Landing on the platform still resets and still plays `jump_exit`, on the way down exactly as a landing on flat ground does today.

## Why
Character-controller correctness; also the root of the "double-jump height exploit" noted in the backlog entry. Godot (`snap_to_floor` only when not moving up), Quake/Source (`PM_CategorizePosition` velocity gate) and Unreal (`IsValidLandingSpot`) all refuse to treat a proximity probe as ground while the body is ascending.

## Approach

### Decisions
| # | Question | Decision | Rejected |
|---|---|---|---|
| 1 | Where to fix | **Physics, at the single source of truth: `raw_grounded`'s inputs (`ground_cast`).** The animation then needs no change: `jump_exit` already fires only from a `raw_grounded`-derived edge (`:515`), so one corrected signal fixes consequence (1) and (2) together. | Animation-only gate (`jump_exit` only while `linvel.y < 0`): leaves the `jumps_used` reset exploit untouched and creates a second, divergent definition of "landed" - exactly the two-mechanisms-disagree class `src/CLAUDE.md` ("Real bug hit while adding coyote-time") warns about. Raising `jump_enter` priority above `jump_exit`: breaks every authored policy, and `jump_enter` is re-fired by double jump. |
| 2 | Rule | While **rising past the liftoff reach** (`jumps_used > 0 && linvel.y > 0 && risen_since_liftoff >= reach`), the ground cast uses `GroundProbe::AscentContact`: reach shrinks to a *contact gap* (`ASCENT_CONTACT_GAP`, 0.05 m) and the walkable-rescue is disabled (hit must be genuinely underfoot, `witness1.y <= feet.y + collider_radius*0.5`). Otherwise `GroundProbe::Full`, byte-for-byte today's cast. | Backlog (a) "retain-only while rising": defeated by surface hand-off (curb) and edge-graze hits, needs a stored last-tick bit, and would strand a walker on a slope after one false tick (vy pinned > 0, never re-acquired). Backlog (b) "skin-only reach whenever airborne": makes `raw_grounded` false ~2 ticks after liftoff, so a fast double-tap fires the second jump at ground level (the exact regression `double_jump_still_requires_genuine_airborne_height_on_flat_ground` and the coyote rationale in `src/CLAUDE.md` exist to prevent), and it moves *every* fall's landing clip ~0.7 m later. Gating on `risen >= reach` keeps the first 0.7 m of ascent (the liftoff-overlap window, already owned by `jump_air_grace`) byte-identical. Hover-height or `linvel.y`-only gates: cannot tell a continuously rising slope (vy pinned positive by contact) from a platform below. |
| 3 | Why contact gap discriminates | On a climbing slope the body is physically touching the surface (gap ~0-0.02) while vy is pinned positive - this is what the `risen_since_liftoff` branch exists for, and it still fires. A platform below a ballistic ascent is 0.05-0.7 m away and is simply not detected. Descent (`vy <= 0`) is untouched, so a normal landing on flat ground or on the cube is unchanged. | Requiring contact in *all* modes: would delay every normal landing/animation. |
| 4 | New state | **None.** The gate is a pure function of `jumps_used`, `jump_liftoff_y`, `linvel.y` and feet Y, all already on `CharacterController`/`Velocity`. `jump_liftoff_y == None` (should not coexist with `jumps_used > 0`) selects `Full`, the old behaviour. | A `raw_grounded`-last-tick bit on `LocomotionState` (backlog suggestion): unneeded, and a new field on a `Reflect` component is more surface than a pure function. |
| 5 | API shape | `ground_cast(..)` stays as a thin wrapper calling `ground_cast_with(.., GroundProbe::Full)`, so `prop_ground_veto_tests.rs::probe()` (the only external caller) needs no edit. | Adding a parameter to `ground_cast` (churns the test probe and the step-offset plan's "unchanged" criterion for no gain). |
| 6 | Compound-collider sibling bug | Reference only. In `AscentContact` the loop can exclude more entities (walkable-but-not-underfoot hits), by whole `Entity` like today; if that over-excludes a compound prop's floor part the result is `raw_grounded == false` while already rising past the liftoff reach, i.e. **fails safe** (the player is airborne anyway). | Fixing per-collider exclusion here. |

### Pseudocode (the exact conditions)
```rust
const ASCENT_CONTACT_GAP: f32 = 0.05;            // m below feet; tunable after Task 1's slope measurement
pub enum GroundProbe { Full, AscentContact }

// player_movement_system, replacing the single ground_cast call at :477 (feet_pos is GlobalTransform y as today)
let rising_past_liftoff = controller.jumps_used > 0
    && velocity.linvel.y > 0.0                     // same threshold the reset's `<= 0.0` uses: no gap between the two
    && controller.jump_liftoff_y.is_some_and(|y| feet_pos.y - y >= ground_sensor_reach(&controller));
let probe = if rising_past_liftoff { GroundProbe::AscentContact } else { GroundProbe::Full };
let hit = ground_cast_with(context, entity, feet_pos, &controller, probe);

// ground_cast_with, the only two lines that differ from today's body:
let max_gap = match probe { Full => controller.ground_cast_length + controller.collider_radius,
                            AscentContact => ASCENT_CONTACT_GAP };
max_time_of_impact: GROUND_CAST_SKIN + max_gap,    // == lift + ground_cast_length for Full (lift = radius + skin): identical
...
let accept = is_underfoot || (probe == Full && is_walkable_contact(controller, candidate.details));
```
Everything downstream (`raw_grounded`, coyote, reset block, `can_jump`, `Friction` toggle, `jump_exit` edge, `jump_air_grace`) is untouched and reads the corrected `raw_grounded`. Resulting behaviour: flat-ground jump - `raw_grounded` falls false at the same tick as today (gap crosses 0.7 m exactly when the gate arms); platform below - not detected while ascending, detected as soon as `vy <= 0`; continuous slope - contact gap ~0 so reset via `risen_since_liftoff` as today.

### Known residuals (document, do not engineer around)
- **Apex hover.** The cube's top is 0.6 m below the 1.8 m apex, inside the 0.7 m reach. At `vy <= 0` the cast sees it, so `jumps_used` resets and `jump_exit` plays ~0.35 s before touchdown. This is the same rule that makes a normal flat landing start 0.7 m up, not a regression; it only reads differently because the apex is over a platform. If Frank dislikes it, the follow-up is a shorter *descending* reach, which changes every landing and is out of scope here.
- **Edge graze.** A Rapier penetration-nudge on a cube edge produces ~2-3 ticks where the body is touching the platform while rising; that counts as contact (indistinguishable from a slope). Accepted: touching a surface may re-arm the jump.
- **Steep walkable slopes** (authored limit > ~60 deg): while rising past liftoff, the walkable-rescue is off, so a very steep contact point above the feet tolerance is ignored and the reset waits for `vy <= 0`. `max_walkable_slope_deg` default is 45; `walkable_slope_steeper_than_the_ground_cast_underfoot_tolerance_is_still_grounded` has `jumps_used == 0` and is unaffected.
- **Uphill spam-jump cadence slows** (expected, arguably more correct): a 20 deg slope separates the body 0.05-0.27 m after each jump, which the old 0.7 m reach counted as grounded; the reset now comes at re-contact instead of at grace expiry (~30 vs ~17 ticks). `steep_slope_rejump_cadence_is_bounded_not_a_hover_exploit` only bounds from above and `steep_slope_jump_no_longer_locks_permanently` needs `jumps > 1` in 200 ticks, both should still pass; Task 1 records the before/after cadence.

### Coordination with `step_offset_auto_step.md`
- **Land this fix first.** It is ~40 lines in `player.rs`, no schema change, and it fixes the state (`raw_grounded`/`jumps_used`) the step plan consumes. The step plan's gate is `raw_grounded && jumps_used == 0`; this fix only changes behaviour when `jumps_used > 0`, so neither can trigger the other, in either order.
- This fix must not: touch XZ/Y position, write `Transform`, change `is_walkable_contact`, or add a `CharacterController` field (56 literal sites). Step-up must not: change `ground_cast_with`/`GroundProbe`, or lift while `jumps_used > 0` (it already does not). The step plan's acceptance line "`ground_cast`... unchanged in the diff" needs rebasing onto the wrapper if this lands first (the wrapper's body is the old function, only renamed); flag it when the step plan is promoted.
- A step lift (Y write while `jumps_used == 0`) leaves `jump_liftoff_y == None`, so `rising_past_liftoff` is false there.

## Tasks
- [ ] **Reproduce first, red tests before any fix** (Task 1): write the tests below and confirm the cube/curb cases fail on HEAD; paste the observed failing tick numbers into this file. If they pass on HEAD, stop and close the backlog entry as stale instead.
- [ ] `capabilities/player.rs`: `ASCENT_CONTACT_GAP`, `GroundProbe`, `ground_cast_with` (+ `ground_cast` wrapper), the `rising_past_liftoff` computation, doc comments citing this plan. Diff-review gate: reset block, coyote block, `can_jump`, `Friction` toggle byte-identical.
- [ ] Tests, new `tests/airborne_ground_reacquisition_tests.rs`, harness copied from `player_slope_jump_tests.rs` (`TimestepMode::Fixed { dt: 1/64 }` + `TimeUpdateStrategy::ManualDuration(1/64)` inserted manually; one `app.update()` = one tick; `GroundKind::{Cuboid, TriMesh}` floor; extra static `Collider::cuboid` for the platform). Every case on **both** floor kinds (`tests/CLAUDE.md` TriMesh rule). Per-tick record: `linvel.y`, `jumps_used`, `is_grounded`, drained `AnimationRequests`, `player.jumped` count.
  - `room10_cube_jump_does_not_reset_or_land_mid_ascent`: 1.2 m cuboid, player walking at it and jumping `jump_velocity` 5.94; for every tick with `linvel.y > 0` after tick 12: `jumps_used == 1`, no `jump_exit` queued.
  - `curb_hop_does_not_reset_mid_ascent` (0.3 m curb, surface hand-off case that defeats the backlog's gate (a)).
  - `spam_jump_next_to_platform_fires_exactly_one_jump_per_flight` with `double_jump_enabled: false` (the exploit); also with `true`, `max_jumps 2`: exactly two, the second only once genuinely airborne.
  - `landing_on_the_cube_still_resets_and_plays_jump_exit_once`: after `vy <= 0` `jumps_used == 0`, exactly one `jump_exit`, ends grounded at y ~1.2.
  - `apex_trajectory_matches_open_ground_control` (apex within epsilon of the no-platform control; physics itself unchanged).
  - `ground_cast_ascent_contact_probe`: direct `ground_cast_with` calls - `Full` finds a platform 0.4 m below, `AscentContact` returns `None`; both find a resting contact; `AscentContact` rejects a walkable top face 0.3 m above the feet.
  - `uphill_climbing_jump_still_resets_via_risen_since_liftoff` on 20 deg, cuboid **and** trimesh (the continuously-climbing-slope guard), plus the cadence recording.
  - `determinism_two_runs_identical` (per-tick `(y, linvel.y, jumps_used)` bit patterns).
- [ ] Existing suites unmodified and green, one file at a time with `df -h` between: `player_slope_jump_tests` (20), `prop_ground_veto_tests` (11), `wall_friction_tests`, `local_coop_tests`, `gamepad_binding_tests`, then the rest. `ron_lint`/`ron_validation` only if an asset is touched (none planned).
- [ ] Docs: `docs/dev/player-ground-detection.md` (new paragraph under "Jump reset cannot rely on a ground-check edge": the ascent-contact rule, why it is gated on `risen >= reach`, the three rejected alternatives in one line each); `tests/CLAUDE.md` table row + test count; update the `LocomotionState.is_grounded` doc if it still cites this backlog item. `docs/20_data_formats.md`: one sentence on `ground_cast_length` ("while rising past the jump's liftoff reach only a surface within ~5 cm counts as ground"). No RON schema change.
- [ ] Backlog: on merge, relocate the Bugs entry to Done and move this file to `planning/features/done/`; add a `claude_suggestions.md` entry for the descending-reach/apex-hover follow-up if Frank wants it.
- [ ] Reviews in one parallel message: `alignment-reviewer`, `system-architect`, `debug-detective` (brief: slope pogo cadence, edge graze, double-jump window); `wasm-perf-reviewer` is skippable (same single cast per tick, no new allocation; the wider exclusion loop is still capped at 4); `ux-gamedesigner-reviewer` only for the docs sentence.

## ironhold_cli / schema impact
None expected: no `schema/` type, `Action` or event changes, so `query.rs` is unaffected. `cargo check -p ironhold_cli` is still run as the unconditional gate. The existing `jump_cannot_clear_ground_sensor` / `coyote_time_exceeds_jump_airtime` checks reason about the full reach, which is unchanged for the first 0.7 m of ascent and for descent.

## Determinism notes
Pure function of fixed-tick state (`jumps_used`, `jump_liftoff_y`, `linvel.y`, `GlobalTransform` Y) and two constants; no `Time` deltas, no RNG, no new transcendentals (`is_walkable_contact` still uses `det_math::acos`). The strict `linvel.y > 0.0` comparison is on the value Rapier wrote the previous tick, identical native/WASM under `enhanced-determinism`.

## Playtest checklist
`local_coop_demo` -> room10 (`cube_obstacle_left`/`right`), then `3rd_person_game_demo`. `python serve.py` on a different port than `test_web.py`; dev build with `--features inspector` (F9 collider wireframes).
1. Walk at the cube and hold Jump: the jump clip plays through the whole ascent; no early landing pose.
2. Press Jump repeatedly during the ascent: only one jump fires (single `player.jumped`), apex height equals jumping on open ground.
3. Land on top of the cube: one landing clip; Jump works again immediately after; walk off and fall: landing clip as before.
4. Jump beside the cube without clearing it and slide down its side: no spurious landing, no free second jump.
5. `3rd_person_game_demo` hills (<= 30 deg) and ledges: hold forward + spam Jump uphill; confirm it still re-jumps (a bit slower than before is expected); walk off ledges: coyote and landing unchanged.
6. P2 (primitive capsule body) behaves identically to P1.

## Open questions
1. **`ASCENT_CONTACT_GAP` = 0.05 m.** Unmeasured; Task 1 should log the contact gap while climbing a 20/30 deg slope and while pogoing, and set the constant just above the 99th percentile. OK to tune it in the PR, or do you want it as a `MovementConfig` field (not recommended: designers cannot reason about it)?
2. **Apex hover over a platform** (residual 1): accept the landing clip starting at apex over a platform inside the 0.7 m reach, or schedule the larger "shorter descending reach" change?
3. **Uphill spam-jump cadence** drops from ~17 to ~30 ticks per jump on a 20 deg slope. Accept (a jump now needs a real re-contact), or keep the old cadence by exempting `jumps_used` resets below some slope angle?

## Acceptance criteria
- Given the room10 cube (or a 0.3 m curb) and a jump toward it, then for every tick with `linvel.y > 0` `jumps_used` stays 1, no `jump_exit` is queued, and a second Jump press fires nothing when `double_jump_enabled: false`; on both cuboid and trimesh floors.
- Given the same jump landing on the platform, then after `linvel.y <= 0` `jumps_used` resets and exactly one `jump_exit` plays; apex height equals the open-ground control.
- Given a continuously climbed 20 deg slope, then `jumps_used` still resets via `risen_since_liftoff` and `jumps > 1` over 200 spam ticks.
- Given `jumps_used == 0` (walking, falling, coyote jump), then the ground cast is byte-identical to today (`Full`).
- All existing physics suites pass unmodified; two identical runs are bit-identical; `cargo check -p ironhold_cli` passes; docs updated.

## Plan-review (2026-10-02)

Run by `/plan-review` while Frank was away: system-architect and ux-gamedesigner-reviewer, in parallel, read-only, claims checked against the code. **Combined verdict: needs more design work** — the blocking items below must be folded into this plan (and Frank's answers to the open questions recorded) before it can be marked Ready. The reports are reproduced verbatim (headings demoted one level).

### System-architect review

## Plan review (system-architect): `planning/features/airborne_ground_reacquisition.md`

Reviewed against `integration` @ `34803b1` (2026-10-02). Read-only; no cargo run.

#### Verdict
**Needs more design work.** The diagnosis is right and the fix is in the right place: `raw_grounded`'s input, one probe, no new state, no schema change. It respects the crates, works on WASM, and is deterministic. But the chosen rule has a hole for double jumps (it measures from the latest liftoff). Its acceptance criteria contradict its own "edge graze" residual in the room10 scenario this fix targets. It also underplays the apex residual, which re-arms the jump rather than only being cosmetic. And one of the "red-first" tests cannot be red on HEAD. All four have small, concrete fixes, and none needs a different approach.

**Mechanism re-proof against HEAD (step 1).** Confirmed by reading the code:
- `player.rs:29`: `ground_sensor_reach = collider_radius + ground_cast_length`.
- `:358-365`: lift `r + 0.01`, `max_time_of_impact = lift + ground_cast_length`, so the reach is exactly 0.7 m below the feet.
- `:427-429`: the walkable-rescue accepts above-feet walkable hits.
- `:478`: `raw_grounded` has no velocity term.
- `:498-506`: coyote only delays leaving the ground.
- `:516-517`: `jump_exit` is pushed on the `loco.is_grounded` rising edge, which equals the `raw_grounded` rising edge because coyote never delays a return.
- `:552-561`: the reset fires on `raw && jumps_used>0 && (vy<=0 || risen>=reach)` after grace.
- `:667-671`: `can_jump`.
- `:682`: `jump_liftoff_y` is overwritten on **every** jump fire, double jumps included.
- `animation_resolver.rs:268/295/317`: `candidate.priority >= active.priority`.

The claim that the animation side needs no change holds: `jump_exit` comes only from that edge.

---

#### Blocking

**B1. A double jump re-opens the 0.7 m `Full` window, so the same bug comes back on every airborne jump.**
- **Evidence.** `player.rs:682` sets `jump_liftoff_y = Some(current y)` on every jump fire. The plan's gate is `controller.jump_liftoff_y.is_some_and(|y| feet_pos.y - y >= reach)`, so after a double jump `risen` restarts from 0 and the probe goes back to `Full` for the next 0.7 m of ascent.
- **Scenario.** The player rises past the cube with the gate armed. `raw` is false, so a double jump is allowed (`can_jump` airborne branch, `:670`). The double jump fires and the probe falls back to `Full`. The cube top is within 0.7 m below, so `raw_grounded` goes true. If coyote has already expired, `loco` goes false→true and `jump_exit` is pushed **mid-ascent** again.
- The reset itself stays blocked by the double jump's own `jump_air_grace`, so this is the animation half of the bug, not the jump-count exploit.
- No shipped project sets `double_jump_enabled: true` (grep finds none), but the field is designer-authorable. The plan's own `spam_jump_…` case with `true, max_jumps 2`, plus AC1, would expose this.
- **Fix (one token, still a pure function).** A jump with `jumps_used >= 1` can only fire from the airborne branch, which requires `!raw_grounded` (`:667`: the grounded branch with coyote requires `jumps_used == 0`). So a second or later jump never has a liftoff-overlap window to protect:
  ```rust
  let rising_past_liftoff = controller.jumps_used > 0
      && velocity.linvel.y > 0.0
      && (controller.jumps_used > 1
          || controller.jump_liftoff_y.is_some_and(|y| feet_pos.y - y >= ground_sensor_reach(&controller)));
  ```
  Add a test: double jump fired beside or over the cube, then no `jump_exit` while `vy > 0`, on both floor kinds.

**B2. AC1 contradicts the "Edge graze" residual, and the room10 repro the fix targets involves a real edge contact.**
- **Evidence.** The backlog's earlier real-physics harness (recorded in the architect memory for this bug) saw a one-tick `vel_y 3.355 → 4.185` jump. That is about +0.98 m/s of Rapier penetration-recovery impulse against −0.153/tick of gravity, so the capsule **actually clipped the cube's top edge** around ticks 16-18, the same window where grace expires (17 ticks).
- **Geometry.** The capsule's bottom sphere centre is `feet + 0.4` (harness compound offset 0.9, half-height 0.5). The cast ball is that same sphere lifted by 0.01, so an edge contact is hit at toi ≈ skin, well inside `ASCENT_CONTACT_GAP`.
  - The hit normal tilts by φ from vertical. For φ ≤ 45° it passes `is_walkable_contact`.
  - The contact point sits `0.4·(1−cos φ) ≤ 0.12` above the feet, which is under the 0.2 tolerance (`:380`), so `is_underfoot` is true and the rescue is not even needed.
  - Result: `AscentContact` **accepts** the edge. `raw` goes true, `jump_exit` fires, and if the contact lasts until grace expiry, `jumps_used` resets mid-ascent, which is the exploit.
- **Conflict.** The plan accepts exactly this as a residual ("touching a surface may re-arm the jump"). AC1 / Task 1's `room10_cube_jump_does_not_reset_or_land_mid_ascent` says "for every tick with `linvel.y > 0`, `jumps_used == 1`, no `jump_exit`". Both cannot hold. The red test may stay red after the fix, or pass only because of the chosen approach geometry.
- **Fix (choose one in the plan before coding).**
  - **(a) Accept.** Rewrite AC1, the room10 test and playtest step 1 as "no reset or `jump_exit` while rising **except on ticks where the cast's `time_of_impact <= ASCENT_CONTACT_GAP`**". Have Task 1 record whether the room10 repro has such ticks. If it does, tell Frank in the plan that the visible symptom he reported may only partly go away.
  - **(b) Close it (recommended to evaluate in Task 1).** In `AscentContact`, also reject a hit the body is **moving away from**: `velocity.linvel.dot(n̂) > SEPARATION_EPS`, with `n̂ = normal1.normalize_or_zero()`, which is already computed for `is_walkable_contact`.
    - On a continuously climbed slope, Rapier's contact solver keeps `v·n ≈ 0`, so the slope-reset guard survives.
    - On an edge graze right after a recovery impulse, `v·n` is strongly positive, so it is rejected.
    - It remains a pure function of fixed-tick state and needs no new field.
    - Keep the 0.05 m gap as well. `v·n` alone would accept a slope that is closing in from 0.7 m away.

**B3. The "apex hover" residual is not only cosmetic. It re-arms the jump, so the spam test, playtest step 2 and the "Why" claim are wrong as written.**
- **Evidence.**
  - At `vy <= 0` the probe is `Full` (unchanged). The cube top is inside the 0.7 m reach at the apex: about 0.33-0.6 m below it, depending on damping (see N2).
  - So `:554-560` resets `jumps_used` at the apex, since grace expired long ago and `vy <= 0`.
  - On the same tick, `:667-668` makes `can_jump == true` (raw true, `jumps_used == 0`).
  - With Jump held or spammed (`step()`/`spam_jump()` press every tick, `player_slope_jump_tests.rs:277-281`), a **full-strength second jump fires at the apex**. `velocity.linvel.y = vel` *sets* rather than adds, so the reachable height from the floor beside a 1.2 m cube is apex + full jump.
  - That is *higher* than today's spam result: today the free jump fires at the tick-18 mid-ascent reset (feet about 1.3 m), not at the apex. For a spamming player the fix moves the free jump later and higher. It does not remove it.
- This is the same rule that lets a player re-jump in the last 0.7 m of any descent on flat ground, so it is consistent rather than new. But the plan presents residual 1 as only "the landing clip starts early", and the "Why" says this fixes "the root of the double-jump height exploit".
- `spam_jump_next_to_platform_fires_exactly_one_jump_per_flight` will count 2 unless "flight" ends at the reset, which makes it a tautology. Playtest step 2 ("press Jump repeatedly during the ascent: only one jump fires, apex height equals open ground") will look like a failure as soon as a press lands at or after the apex.
- **Fix.**
  - Restate residual 1 as "landing clip **and** jump re-arm start up to 0.7 m above any surface on descent, including the apex over a platform. Net gain over 'land then jump' ≈ the gap at re-arm, the same bonus flat ground already gives".
  - Bound the spam test's assertion window to ticks with `vy > 0`, and add an explicit test that documents the apex re-arm, so a future descending-reach change has something to flip.
  - Reword playtest step 2 to "press only during the rise".
  - Drop or qualify the "exploit fixed" claim in Why/What.
  - Reframe Open question 2 for Frank in these terms, since it is a gameplay decision, not only an animation one.

**B4. The 0.3 m curb test cannot be red on HEAD, and Task 1's stop rule then gives an ambiguous outcome.**
- **Evidence (default params, `jump_velocity` 5.94, 0.7 m reach, grace = `ceil(2·0.1323·64)` = 17 ticks).**
  - The curb top (0.3) is within reach only while feet ≤ 1.0 m. Feet pass 1.0 m at t ≈ 0.19 s, about tick 12-13 (later with damping).
  - So `raw` stays true continuously from the floor, through the curb hand-off, up to about tick 12. That is inside the 17-tick grace, so the reset is unreachable.
  - `loco` never goes false before the curb is lost, so there is no false→true edge and no `jump_exit`.
  - The curb only reappears on descent, which is correct behaviour.
- Hand-off only reproduces mid-ascent when the surface top is still within reach at grace expiry, i.e. top > feet(tick 17) − 0.7 ≈ 0.5-0.58 m, and ≤ 0.71 m for a seamless floor hand-off. That means a top of about **0.6 m**, a narrow 1-2 tick band, so the test is fragile.
- The same arithmetic weakens the plan's curb argument against backlog gate (a). Gate (a) is still adequately rejected on its other grounds: it needs a stored bit and can strand a walker on a slope.
- **Fix.**
  - Move the curb case out of the red set into the "green before and after" regression set, or raise it to a 0.6 m ledge with the tick math written next to it.
  - Restate the stop rule: "stop only if the **cube** case passes on HEAD".
  - Correct the "Reproduction status (a)" sentence ("a 0.3 m curb hops the same way").

---

#### Non-blocking

- **N1. "Byte-identical / same tick on flat ground" is true only up to the boundary tick.**
  - `jump_liftoff_y` is the resting `GlobalTransform` y. If Rapier rests the capsule a few mm *into* the floor, `Full` still sees the floor until `risen ≈ 0.7 + penetration`, but the gate switches to `AscentContact` at exactly `0.7`. So `raw` can drop one tick earlier than today. That makes the double-jump branch available one tick earlier at about 0.70 m instead of about 0.70x m, which is harmless.
  - Either word it as "±1 tick", or arm the gate at `reach + ASCENT_CONTACT_GAP` so `Full` has certainly lost the floor. The margin to the above-feet walkable-rescue risk (feet ≈ 0.79 for a 1.2 m cube) shrinks from about 0.09 m to about 0.04 m, which is still more than half a tick at that speed, so check it in Task 1.
- **N2. The numbers are undamped.**
  - Production `MovementConfig` default `linear_damping` is 0.5 (`schema/catalog.rs:1540`, applied at `entity_spawner.rs:1128`), and so is the harness (`player_slope_jump_tests.rs:176`).
  - The measured control apex was about 1.53 m, not 1.8. Residual 1's "0.35 s before touchdown" and "feet at 1.0-1.9 m" figures are therefore overstated.
  - Test thresholds like "after tick 12" should be derived from `risen >= reach`, not a hard-coded tick.
- **N3. Step-offset coordination.**
  - The claim that neither plan can trigger the other holds in this fix's direction.
  - But the step plan's premise that `raw_grounded && jumps_used == 0` "excludes the whole jump" (`step_offset_auto_step.md:112-113`) is false during the last ≤0.7 m of *every* descent, and now (B3) from the apex over a platform. That is pre-existing, not caused here. Flag it on the step plan so step-up can't lift mid-air beside a riser while descending.
  - Sequencing "this first" is right: smaller, no new component, and it fixes the state the step plan reads. The wrapper-rename note about the step plan's "`ground_cast` unchanged" criterion is correct.
- **N4.** `jump_liftoff_y == None` → `Full` is the opposite convention from the reset's `unwrap_or(f32::INFINITY)` (`:557`). It is unreachable today because the only writers are `:560`/`:682`/spawn `None` (`entity_spawner.rs:1113`). Add a `debug_assert!` or a one-line comment so a future teleport/respawn writer doesn't silently pick a different meaning.
- **N5. Docs to update beyond what the plan lists.**
  - `docs/dev/player-ground-detection.md` describes the pogo cadence as about one re-jump per grace window (~0.26 s) and has the "landing on a nearby raised platform … un-rejumpable for the remainder of the window" consequence. Both change, since resets now come at re-contact.
  - The `LocomotionState.is_grounded` doc, as the plan already notes.
  - Add the apex re-arm (B3) to the new paragraph.
- **N6. Rejected alternatives.**
  - (a) and (b) are handled, (a) on stored-state/slope-strand grounds; see B4 for the curb argument.
  - Not addressed: the "two-tier reach with hysteresis" variant (full reach while `raw_last_tick || coyote > 0`, short otherwise). It is the other precedent-backed option and it would also remove the apex residual (B3).
  - Add one Decisions line on why it is rejected: it needs a stored bit and the coyote window can still bridge to a cube. That keeps a future reviewer from reopening it.
- **N7.** `determinism_two_runs_identical` in one process proves run-to-run stability, not native-vs-WASM equality. That is fine as a regression guard, but don't cite it as cross-platform evidence. The gate adds only comparisons on values that already exist, with no transcendentals, `Time` or RNG. That is consistent with D3 and with `player_movement_system` running in `FixedUpdate` after `mark_dirty_trees`, so `feet_pos` is fresh.
- **N8. Harness feasibility is good.** `setup_case_full` (`player_slope_jump_tests.rs:105-186`) already builds `TimestepMode::Fixed` + `ManualDuration(1/64)`, a cuboid or trimesh floor, a settled capsule, `step()` with Move+Jump, `drain_animation_requests`, and a `player.jumped` count.
  - Copying it and adding one `Collider::cuboid(0.6,0.6,0.6)` at y=0.6 is straightforward.
  - The harness defaults to `is_running: true` with `run_speed` 10. To keep the player over the cube through the ascent (needed for B2/B3 to show up), approach from about 0.5 m and/or use walk speed. Pin the start offset and speed in the test, and log `toi` and normal per tick so the B2 decision has data.
- **N9.** The added exclusion iterations in `AscentContact` fail safe: hitting the cap of 4 returns `None`, i.e. airborne while rising, which matches the plan's Decision 6. The cost is still one cast per tick in the common case. Skipping wasm-perf-reviewer is justified.

#### Open questions for Frank
1. **Edge graze (B2):** accept that touching the cube edge while rising can still re-arm the jump and cut the clip, or add the separation-velocity check (`v·n > ε` → not ground while ascending)? **Recommendation:** add it if Task 1 shows room10 has contact ticks. Otherwise your original symptom may only partly go away.
2. **Apex re-arm (B3, reframing the plan's Q2):** a jump pressed at or after the apex over a platform fires a full jump from up to about 0.7 m above it, the same as the pre-touchdown re-jump on flat ground. **Recommendation:** accept it for this fix as a consistent rule and document it. Schedule the "shorter descending reach / landing requires near-contact" change separately, because it changes every landing.
3. **`ASCENT_CONTACT_GAP` (plan Q1):** agree it should be an engine constant, not a `MovementConfig` field, alongside `GROUND_CAST_SKIN` / `JUMP_AIR_GRACE_SAFETY`, tuned from Task 1's measurements.
4. **Uphill cadence (plan Q3):** accept the slower cadence (about 17 → about 25-30 ticks). It is more correct and both existing cadence tests bound in the right direction.

### UX-gamedesigner review

## UX / game-design plan-review: `planning/features/airborne_ground_reacquisition.md`

Reviewed against `integration` at `34803b1` (2026-10-02). Designer-facing surfaces checked: `docs/20_data_formats.md` MovementConfig table (~2288-2325) and the reserved `jump_enter`/`jump_exit` callout (~3610-3617), `docs/60_contributing.md` validate checks (~279-287), and the shipped demo geometry and prefabs for every project with a player.

#### Verdict
**Needs more design work.** The scope is small. The fix itself is the right one from a designer's point of view: no new RON field, one definition of "landed", and flat-ground jumps, coyote time and normal landings stay the same. Both blockers are in the **playtest checklist**. One step points at a demo that has none of the terrain it asks you to test. Another step's pass condition can't be seen in the browser. As written, the most noticeable feel change (uphill re-jump cadence) would ship without anyone playing it.

##### What a designer will actually feel (per shipped demo)
| Demo / place | Geometry | Before | After | Feels different? |
|---|---|---|---|---|
| `local_coop_demo` room10, `cube_obstacle_left/right` (top at 1.2 m, default apex 1.8 m) | Raised cube | Landing clip fires mid-ascent; jump re-arms in flight (free higher jump) | Jump clip plays through the whole ascent. The landing clip starts **at apex**, about 0.35 s before touchdown (residual 1) | Yes, better. The apex-hover early landing pose is still visible |
| `3rd_person_game_demo` `loot_display_01` (platform top 0.2 m, chest top ~1.0 m), `chest_01` (top 0.8 m), `anvil_01` | Low props | Mid-ascent false landing when jumping next to or onto them | Fixed. Chest top 1.0 is 0.8 m below the 1.8 apex, outside the 0.7 m reach, so no apex hover here | Yes, better |
| `3rd_person_game_demo` ground | **Flat 100x100 cuboid, no hills** | n/a | n/a | No. The plan's "hills <= 30 deg" step can't be done here |
| `quick_scene` heightmap terrain (`player_warrior`, the hillside behind the earlier downhill-creep playtest) | Real slopes | Uphill spam-jump re-arms at grace expiry (~17 ticks) | Re-arms at re-contact (~30 ticks) | **Yes, slower uphill pogo.** This is the one real feel change |
| `primitive_world`, `particles_demo`, `effect_mayhem_demo`, `stats_demo` (`double_jump: true`) | Mostly flat | Mid-ascent reset next to geometry could grant a 3rd jump | Exactly 2 | Only near raised geometry. Flat-ground double jump must stay byte-identical |
| Everything else: walking off ledges, coyote jump, falling | any | | `jumps_used == 0` uses the `Full` probe, same as today | No |

Double-jump height, `coyote_time_secs` and jump height are unchanged by construction. The gate only arms once the player has risen 0.7 m with `jumps_used > 0`. No authored value changes meaning. The `jump_cannot_clear_ground_sensor` and `coyote_time_exceeds_jump_airtime` validate checks and the docs/20 "A jump must clear..." callout all stay correct.

#### Blocking

1. **Playtest step 5 targets the wrong demo.** `3rd_person_game_demo/scenes/main.scene.ron` has a single flat `ground_plane` cuboid (100x100, top at y=0). It has no hills and no ledges. The uphill cadence change (open question 3), the climbing-slope guard (`risen_since_liftoff`) and the steep-slope residual would all go unplayed.
   **Fix:** split step 5 in two.
   - **5a, `quick_scene`:** walk up the heightmap hillside, hold forward and spam Jump. It must still re-jump with no lock. Expect it to be slower than before (roughly one jump per half second instead of a fast pogo), and note the result. Also walk and run downhill, then jump: landing must be unchanged.
   - **5b, `3rd_person_game_demo`:** jump next to and onto `loot_display_01` (low platform), `chest_01` and `anvil_01`. The jump clip must play through and land normally on top. Keep "walk off the loot platform edge: coyote jump still works" as the ledge check, because that platform is the only ledge in the demo.

2. **Step 2's pass condition can't be observed, and `double_jump: true` has no coverage.** "Only one jump fires (single `player.jumped`)": nothing in `local_coop_demo` reacts to `player.jumped`. The only binding in any shipped project is `primitive_world/logic/state_machine.ron:50` (`PlaySound(key: "jump")`). Room10's P2 also has no rig, so P2 can't show a landing clip either (room10's own `animation_gap_hint` says so). Step 6 ("P2 behaves identically") can therefore only check jump count and height, not animation.
   **Fix:**
   - Reword step 2 to something you can see: "spam Jump during the ascent next to the cube: peak height stays the same as an open-ground jump, with no second boost".
   - Add a **`primitive_world`** step, where `double_jump: true` and the jump sound is the audible canary. On flat ground, tap Jump twice quickly and then twice slowly. You should hear exactly two jump sounds per flight, with the second one coming at the same moment as before (the plan claims the first 0.7 m and flat-ground behaviour are byte-identical, so this is the no-regression check). Then jump next to any raised prop. Never more than two sounds before landing.
   - Scope step 6 to "P2 jump count and height match P1. P2 has no landing clip by design."

#### Non-blocking

3. **Docs: the planned sentence uses engine words.** "While rising past the jump's liftoff reach only a surface within ~5 cm counts as ground" means nothing to a designer. "Liftoff reach" is not a term docs/20 uses anywhere. Suggested wording, appended to the existing "A jump must clear..." blockquote (docs/20 ~2311-2325) rather than squeezed into the table row:
   > Once a jump has risen more than `collider_radius + ground_cast_length` and is still going up, the engine only counts a surface the player is actually touching as ground. A platform or ledge passing underneath a rising jump no longer cuts the jump animation short or refunds the jump. Landing is detected on the way down as usual.

4. **Docs: say when the landing clip starts.** The apex-hover residual is really a fact designers need when they author a `Jump_Land` clip. The landing clip (`jump_exit`) starts when the player comes within `collider_radius + ground_cast_length` of the surface below (0.7 m by default) **on the way down**, not at touchdown. After a normal jump that is ~0.13 s before touchdown. Over a platform that sits just below the jump's peak it can be up to ~0.35 s, starting near the apex. Add one sentence to the reserved `jump_enter`/`jump_exit` callout (docs/20 ~3610). It tells a designer why their landing pose shows up in mid-air and that a longer or anticipation-heavy `Jump_Land` clip hides it.

5. **Docs: `ground_cast_length` row advice now points the wrong way.** The row (docs/20 ~2303) still says "increase for uneven terrain or fast vertical movement". After this fix, a larger value mainly makes **every** landing clip start earlier and widens the apex-hover band. Add: "larger values also start the landing animation earlier, see the note below". This is the same still-open gap I flagged in an earlier review: the row doesn't cross-link to the invariant blockquote.

6. **`docs/STATUS.md` / backlog wording.** When the Bugs entry moves to Done, add one line to the Done entry naming the apex-hover residual and the uphill cadence change. A designer who notices either later should find "known, accepted", not file a fresh bug.

7. **Checklist: add a "before" capture.** Frank should spam-jump uphill on `quick_scene` once on the current `pkg/` before installing the dev build. The plan expects the cadence to drop (~17 to ~30 ticks), and "a bit slower" is impossible to judge without a baseline.

8. **Apex-hover over the room10 cube is the only shipped instance.** With the default 1.8 m apex, only a surface 1.1-1.8 m high triggers it. room10's 1.2 m cube is in that band. None of `3rd_person_game_demo`'s props are (highest top ~1.0 m). That is worth stating in the residual so Frank isn't surprised when room10 still shows an early landing pose after "the fix".

#### Open questions for Frank

1. **Should `ASCENT_CONTACT_GAP` (0.05 m) be authorable?** **No. Keep it an internal constant tuned in the PR.** A designer can't predict "contact gap while ascending" or see it, and no one can reason about it at authoring time. Exposing it creates another entangled field in the jump/ground-cast invariant that already couples five authored fields (`jump`, `double_jump_height`, `collider_radius`/`primitive.radius`, `collider_height`, `ground_cast_length`). One caveat for Task 1: also measure the gap with an unusually small and an unusually large `collider_radius` and at `run_speed`. If the 99th percentile scales with any of them, derive the constant from that field instead of exposing a new one.

2. **Accept the landing clip starting at apex over a platform?** **Accept it for this fix, and log the "shorter descending reach" follow-up in `claude_suggestions.md`.** It is strictly better than today (the landing clip used to fire mid-*ascent*). It only shows up with platforms 1.1-1.8 m high at the default jump, and it is the same rule every normal landing already uses. A shorter descending reach changes the timing of *every* landing animation in every project, so it deserves its own plan and playtest. Pair the acceptance with docs item 4 so designers know the rule.

3. **Accept the slower uphill spam-jump cadence?** **Accept it. Do not add a slope-angle exemption.** The old ~17-tick uphill pogo was faster than a normal flat-ground jump (~1.2 s of airtime), which was itself a bug. A ~30-tick re-arm that needs real re-contact is closer to what a player expects. A below-some-angle exemption would bring back a second, divergent definition of "landed", the exact failure class this plan avoids. It would also need a new threshold that no designer could tune. Two conditions: the docs/20 blockquote's existing "slower, stepped re-jump cadence" wording must still describe what Frank sees on `quick_scene` (blocker 1), and the before/after numbers from Task 1 must go into the plan file.
