# Fix: Airborne ground re-acquisition (a platform below a rising jump must not count as "landed")

_Status: Draft_
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
- [ ] Docs: `crates/ironhold_core/src/CLAUDE.md` (new paragraph under "Jump reset cannot rely on a ground-check edge": the ascent-contact rule, why it is gated on `risen >= reach`, the three rejected alternatives in one line each); `tests/CLAUDE.md` table row + test count; update the `LocomotionState.is_grounded` doc if it still cites this backlog item. `docs/20_data_formats.md`: one sentence on `ground_cast_length` ("while rising past the jump's liftoff reach only a surface within ~5 cm counts as ground"). No RON schema change.
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
