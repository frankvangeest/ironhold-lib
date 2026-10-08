# Feature: Step-Offset / Auto-Step for Small Ledges (`MovementConfig.step_height`)

_Status: Draft — plan-review 2026-10-02: needs more design work (see "Plan-review" section at the end)_
_Planned at: `9f23098` (2026-10-02)_

Backlog item: **Step-offset / auto-step for small ledges** (`## Engine / Runtime`). Related, **referenced not absorbed**: Bugs
"Jumping near a short raised platform spuriously grounds the player mid-ascent" (`planning/features/airborne_ground_reacquisition.md`,
suggested, not yet written) and Bugs "A wall that's part of the same collider entity as the walkable floor..." (see Decision 7).
Also feeds the Queued `parkour_demo` item (a steps lane belongs there once it exists).
Written autonomously while Frank was away; not plan-reviewed yet. **Nothing below was compiled or run**; the one empirical
unknown (how tall a ledge the capsule already climbs unaided) is Task 1, not an assumption.

## What
A designer sets one optional field on a player prefab and the character walks up small ledges instead of stalling on them:

```ron
movement: (
    walk_speed: 5.0,
    step_height: 0.35,   // metres; 0.0 (default) = off, byte-for-byte today's behaviour
),
```

When the player is walking on the ground into a vertical-ish obstacle no taller than `step_height`, with a walkable top surface the
body fits on, the body is lifted onto that surface and walks on. Taller ledges, walls, unwalkable slopes, ledges with no headroom,
and anything while jumping/airborne behave exactly as today. No velocity, jump-count, coyote or friction state is touched.

## Why
Curbs, stair risers, low props and terrain lips either block the player dead or force a jump. Unity (`stepOffset`), Unreal
(`MaxStepHeight`) and Godot (`max_step_height`) all provide this. It was deferred out of `uphill_jump_lock.md` (Frank's second
playtest) and is a prerequisite for believable interiors/stairs in any `Actor`-player project.

## Findings (verified against code at `9f23098`)
1. **The player is a Dynamic Rapier body driven by velocity writes.** `RigidBody::Dynamic`, capsule collider
   (`Collider::compound` wrapping `capsule_y(cap_half, cap_radius)` offset `(0, cap_half+cap_radius, 0)`), `LockedAxes::ROTATION_LOCKED`,
   `Friction` (`entity_spawner.rs:1121-1136`). `player_movement_system` writes `linvel.x/z` every tick (`player.rs:598-617`), gravity
   handles Y. The entity origin is the **feet**, so capsule centre = feet + `player_height/2`. This path is shared by GLB **and**
   primitive players (`cap_half` at `entity_spawner.rs:1070`), so only dimension *derivation* diverges, not the collider shape.
2. **The system order already supports a pre-step position write.** `lib.rs:312-322`: `... player_movement_system, player_view_box_clamp_system,
   ..., mark_dirty_trees` `.chain().before(PhysicsSet::SyncBackend)`. bevy_rapier syncs bodies from `Changed<GlobalTransform>`
   (`plugin/systems/rigid_body.rs:58`), and `player_view_box_clamp_system` (`player.rs:694-713`) is the precedent for writing
   `Transform.translation` on this body. A Y write in `player_movement_system` is therefore applied by the same tick's physics step.
3. **`ground_cast`/`is_walkable_contact` are the shared, most playtest-sensitive code** (`player.rs:281`, `:350`) and have a documented
   trap that a step probe also hits: a cast that starts in resting contact with the floor reports a `time_of_impact == 0` hit and
   `cast_shape` returns only the single nearest hit, so side/floor contacts beat the real one (`player.rs:302-349`, bounded
   re-query `MAX_GROUND_CAST_CANDIDATES = 4`, exclusion by whole `Entity` via `QueryFilter::predicate`).
4. **Jump/coyote state is keyed on `raw_grounded`, `jumps_used`, `jump_liftoff_y`** (`player.rs:470-562`, `:667-685`; `src/CLAUDE.md`
   "Jump reset cannot rely on a ground-check edge"). `jump_liftoff_y` is `Some` only while `jumps_used > 0`. A Y position change
   while `jumps_used == 0` cannot reach `risen_since_liftoff`.
5. **Friction is `0.0` whenever the player commands movement** (`player.rs:643-648`), so a step-face contact cannot add Coulomb
   braking; the wall-friction fix is unaffected by anything done only while moving.
6. **`CharacterController` has ~56 struct-literal sites** (tests: `local_coop_tests` 18, `gamepad_binding_tests` 12, `action_tests` 9, ...;
   `grep "CharacterController {"`). The backlog already avoided adding fields to it for the same reason (airborne-reacquisition
   entry: "store on `LocomotionState`, not `CharacterController`"). `MovementConfig` has a `Default` impl and no struct-literal sites.
7. **NPCs have no ground state.** `npc_behavior_system` writes `linvel.x/z` only (`npc.rs:203-520`); no `CharacterController`, no
   `raw_grounded`, no sensor cast.
8. **Rapier's own autostep exists but is kinematic-only.** `bevy_rapier3d-0.33.0/src/control/character_controller.rs:69,133`
   (`autostep: Option<CharacterAutostep>` on `KinematicCharacterController`, which computes an *effective translation* for a kinematic body).
   Nothing in `src/` uses `KinematicCharacterController` (grep).
9. **Determinism context:** `enhanced-determinism` is on (`Cargo.toml:16`); `is_walkable_contact` already routes `acos` through
   `det_math` (`player.rs:297`); Rapier steps at `FIXED_TICK_RATE` 64 Hz in `FixedUpdate`.
10. **Existing suites that must stay green untouched:** `player_slope_jump_tests` (20), `prop_ground_veto_tests` (11),
    `wall_friction_tests` (5), `local_coop_tests`, `gamepad_binding_tests` (all construct `CharacterController` literals).

## Approach

### Decisions
| # | Question | Decision | Rejected |
|---|---|---|---|
| 1 | Algorithm | **Custom pre-step "probe and lift" in `FixedUpdate`**, all queries via `RapierContext::cast_shape` with the player's own capsule: (a) forward cast at floor-skin height finds a *blocker*; (b) up cast measures headroom; (c) lifted forward cast proves the path is clear; (d) down cast finds the walkable top; then `Transform.translation.y += rise`. Position write only (Finding 2). | `KinematicCharacterController` autostep (Finding 8): needs a Kinematic body, i.e. rewriting gravity, pushing dynamic props, `Friction`, `ExternalImpulse`, `ground_cast`, coyote and every tuned test. Velocity-impulse lift (set `linvel.y = rise*64`): the step-face contact is still active at tick start and the solver removes the forward velocity, so the result depends on solver internals. Capsule rounding/chamfer/bigger hemisphere: `collider_radius` is shared by the ground ball and every derived formula (`ground_sensor_reach`, `jump_air_grace_ticks`); changes feel for everyone. |
| 2 | Config home | New component `StepOffset { height, capsule_half_segment }` inserted by `spawn_player_entity_core` **only when `step_height > 0`**; `player_movement_system` gets `Option<&StepOffset>`. | A field on `CharacterController` (56 literal edits, Finding 6). `Option` is mandatory for the same "required component silently drops the entity from the whole query" trap documented for `Friction`/`PlayerTarget` (`player.rs:636-642`). |
| 3 | Default | `step_height: 0.0` (off). | A non-zero default would change every shipped project's feel at once; see Open question 1. |
| 4 | Slope limit for the landing surface | Reuse `max_walkable_slope_deg` through `is_walkable_contact`. No `max_step_slope` field. | A second slope limit gives two definitions of "floor" that can disagree (the exact class of bug `uphill_jump_lock.md` fixed). |
| 5 | Lift magnitude | Lift by the **measured** `rise` (clamped by headroom), never a fixed `step_height`. | Fixed-size lift: creeps a player up a tall curb in repeated ticks. With measured rise a curb taller than `step_height` is rejected every tick (the lifted forward cast is blocked), so no double-step. |
| 6 | Who | **Players only (v1).** | NPCs (Finding 7) need their own capsule dims and per-NPC per-tick casts on WASM; the probe is written as a free function so a follow-up can reuse it. |
| 7 | Relation to the two ground bugs | **Reference, do not absorb.** Step logic is a pure *consumer* of `raw_grounded` and `jumps_used`; it changes neither `ground_cast` nor `raw_grounded`. | Absorbing the reacquisition fix: it edits `raw_grounded`'s computation (reach shrink, retain-only while rising), a different, riskier mechanism with its own mandatory TriMesh regression set. Both can land in either order: the step gate (`raw_grounded && jumps_used == 0`) stays valid under the reacquisition fix. The compound-collider bug has the same limitation in the probe, but it fails **safe** (an over-excluded blocker means no lift, never a wrong lift). |

### Schema and types
```rust
// schema/catalog.rs, MovementConfig (+ Default impl + default fn)
/// Tallest ledge (metres) the player auto-climbs when walking into it. 0.0 = off. Default: 0.0.
#[serde(default = "default_step_height")] pub step_height: f32,

// capabilities/step_up.rs (new)
#[derive(Component)] pub struct StepOffset { pub height: f32, pub capsule_half_segment: f32 }
pub fn find_step_up(ctx: &RapierContext, entity: Entity, feet: Vec3, dir: Vec3 /*unit, XZ*/,
                    travel: f32, radius: f32, slope_limit: &CharacterController, step: &StepOffset) -> Option<f32> /*rise*/
```
`find_step_up` takes the controller only for `is_walkable_contact`. `ground_cast`, `is_walkable_contact` and `ground_sensor_reach`
stay byte-identical; `player.rs` gains one `Option<&StepOffset>` query item and one call site.

### Algorithm (`find_step_up`), constants `STEP_CAST_SKIN 0.02`, `STEP_MIN_RISE 0.03`, `STEP_LANDING_MARGIN 0.05`, `STEP_LIFT_SKIN 0.01`
Preconditions at the **end** of the per-player loop body (after the jump block, so a jump fired this tick makes `jumps_used > 0`):
`rapier_context` present, `raw_grounded`, `controller.jumps_used == 0`, `step.height > 0`, commanded `speed * (1/FIXED_TICK_RATE) > 1e-4`
(a rooted player with input held does not step). `travel = speed / FIXED_TICK_RATE`; `dir` = the normalized `move_vec` already computed.
Probe shape = `Collider::capsule_y(capsule_half_segment, collider_radius)` placed at `feet + (0, cap_half + radius, 0)`.
Every cast uses `QueryFilter::new().exclude_rigid_body(entity).exclude_sensors()` and, like `ground_cast`, a bounded re-query loop
(max 4) excluding rejected entities. **Any ambiguity returns `None` (no lift).**
1. **Blocker:** cast from `feet + STEP_CAST_SKIN` along `dir`, distance `travel`. Hit counts only if `!is_walkable_contact(normal)`,
   `normalize(normal1)·dir < -0.1` (rejects the parallel side-wall `toi == 0` case from Finding 3) and its witness height
   `<= feet.y + step.height + STEP_CAST_SKIN`. The skin lift is what keeps the resting floor from returning a `toi == 0` hit.
2. **Headroom:** cast up `step.height`; `H = min(step.height, toi - STEP_LIFT_SKIN)`; `H <= STEP_MIN_RISE` rejects.
3. **Clear path:** from the lifted pose cast along `dir` for `radius + travel + STEP_LANDING_MARGIN`; any hit rejects (taller wall / next riser).
   This distance puts the capsule *axis* just past the step edge, so the next cast lands on the top, not on the corner (a corner
   contact has a tilted normal and would wrongly read unwalkable; Rapier's autostep needs `min_width` for the same reason).
4. **Landing:** from the lifted+forward pose cast down `H + STEP_CAST_SKIN`. Accept only if `is_walkable_contact`, then
   `rise = H - toi`; reject `rise < STEP_MIN_RISE`.
5. **Apply:** `transform.translation.y += rise + STEP_LIFT_SKIN`. XZ and `Velocity` untouched; gravity seats the 1 cm gap.
After the lift the capsule's bottom hemisphere clears the edge corner by construction (corner-to-centre distance is `sqrt(2)*radius`
for a face-touching capsule), so the next tick's forward velocity carries the body onto the top with no residual contact braking.

### Interactions (what must not regress)
- **Coyote / `is_grounded` / `jumps_used` / `jump_liftoff_y` / `jump_air_grace`:** never read-modified. The gate uses `raw_grounded` (not the
  coyote-buffered value, per `capabilities/CLAUDE.md` "Ground detection hard rules") and `jumps_used == 0`, which excludes the whole jump and its reset window, so a mid-ascent
  "grounded" reading near a platform (the Bugs entry) can never trigger a lift.
- **Wall friction toggle:** untouched; lift runs only while moving, when `Friction` is already `0.0` (Finding 5).
- **`mark_dirty_trees`/stale `GlobalTransform`:** the lift happens last in the loop; nothing later in the tick reads this entity's `GlobalTransform`
  (the view-box clamp reads `Transform`). The chain's `mark_dirty_trees` then propagates it before `SyncBackend`.
- **View-box clamp:** unaffected (Y untouched by it; lift is Y only).
- **Rapier Fixed timestep:** distances use `1/FIXED_TICK_RATE`, never `Time` deltas.
- **Determinism:** pure function of Transform, last tick's Rapier state, commanded input and authored constants; takes the *nearest* hit (no
  query-order dependence), the exclusion `Vec` keeps insertion order, angle test is `is_walkable_contact` (`det_math::acos`). No new
  transcendentals, no RNG, no wall-clock.
- **Cost:** moving + grounded + opted-in players only: 1 cast per tick when nothing blocks, 4 when a step is taken. Negligible next to `ground_cast`;
  `wasm-perf-reviewer` still applies (new per-tick physics query).
- **Step down:** not snapped. Walking down stairs stays "grounded" because a riser drop is inside the sensor reach (`collider_radius + ground_cast_length`
  = 0.7 m at defaults) and coyote covers the rest. Verify in playtest; step-down snap is a v2 candidate, not needed here.

### Known limits (document, do not engineer around in v1)
- Treads shallower than roughly `collider_radius` cannot hold the capsule; the next riser blocks step 3. Author stairs with tread >= ~0.5 m or use a ramp.
- Very thin tops (a 0.1 m rail) are accepted if walkable; the player then slides off. Playtest decides whether a `min_width` check is needed.
- A step onto a moving/bobbing platform uses last tick's positions only; no relative-velocity handling.
- Steps onto **dynamic** props are allowed (Unity does the same); a pushable crate shorter than `step_height` is climbed rather than pushed.

## Tasks
- [ ] **Baseline measurement first** (no feature code): a throwaway harness in the `wall_friction_tests` style walking at walk/run speed into
  cuboid curbs of 0.05..0.5 m on `Collider::cuboid` **and** `Collider::trimesh` ground; record the tallest curb the capsule already climbs
  unaided. Put the table in this file. If it is already >= 0.3 m, shrink the feature's scope/default accordingly (Open question 1).
- [ ] `schema/catalog.rs`: `MovementConfig.step_height` + `Default` + doc comment (what 0 means, units, tread-depth limit).
- [ ] `capabilities/step_up.rs` + `capabilities/mod.rs`: `StepOffset`, `find_step_up`, module docs citing Findings 3 and 4.
- [ ] `player.rs`: `Option<&StepOffset>` query item, end-of-loop call site only. No other line changes (diff-review gate: `ground_cast`,
  `is_walkable_contact`, the coyote block and the jump block are unchanged).
- [ ] `entity_spawner.rs` (`spawn_player_entity_core`, shared by GLB and primitive players): insert `StepOffset` when `step_height > 0`,
  clamped to `player_height * 0.5`; `scene_loader.rs::warn_invalid_step_height` (negative/NaN/non-finite -> `warn!`, treated as off; above
  the clamp -> `warn!`), called next to `warn_negative_coyote_time_secs` (`entity_spawner.rs:1091`).
- [ ] CLI `crates/ironhold_cli/src/commands/validate.rs` (beside `invalid_walkable_slope_limit`, ~L3185): `--strict` warning
  `invalid_step_height` (negative/NaN) and `step_height_exceeds_collider_limit`. **`cargo check -p ironhold_cli` is mandatory** (new schema field).
- [ ] Tests, new `tests/step_up_tests.rs` (one `app.update()` = one tick, `setup_test_app()`, patterned on `wall_friction_tests.rs`; every case on
  cuboid **and** trimesh ground per `tests/CLAUDE.md`):
  - climbs curbs <= `step_height` at walk and run speed; ends standing on top (`Transform.y` and the Rapier body position, not just `Transform`);
  - rejects curb > `step_height` over 300 ticks with no Y creep (no double-step); rejects no-headroom (low ceiling), unwalkable-slope top, tall wall;
  - ramps and walkable slopes: zero lifts (Y trajectory equals `step_height: 0` control within epsilon);
  - **`step_height` omitted/0.0: trajectory bit-identical to the control** and no `StepOffset` component present;
  - jump held next to a curb: no lift while `jumps_used > 0`, `jumps_used` reset and `jump_exit` timing identical to control;
  - side-wall parallel to travel does not trigger; blocker + wall directly behind the step rejects; rooted player (speed 0, input held) does not step;
  - moving into a curb for 5 s: single lift, no oscillation (Y monotone after the lift); stair set with tread 0.5 m climbs all risers;
  - diagonal and strafe input; prop-pressed-against-player case from `prop_ground_veto_tests` still grounded and still jumps;
  - determinism: two identical runs produce identical per-tick `(x,y,z)` bit patterns;
  - `ron_validation.rs` round-trip of `step_height` present/absent.
- [ ] Full existing physics suite one file at a time, unmodified: `player_slope_jump_tests`, `prop_ground_veto_tests`, `wall_friction_tests`,
  `local_coop_tests`, `gamepad_binding_tests`, then the rest (`df -h` between files per root `CLAUDE.md`).
- [ ] Demo content: `local_coop_demo` `room10` gets a **steps lane** per player (flat, known geometry, already the friction repro room): curbs 0.15 / 0.30 /
  0.45 m, a 3-riser stair (0.2 m risers, 0.5 m treads), a 0.2 m riser with 0.25 m treads (expected NOT climbed, labelled), and a 1.2 m cube
  (existing `cube_obstacle_room10`, negative control). New primitive prefabs in `prefabs.ron`; enable `step_height: 0.35` on the room10 player prefabs
  and on `3rd_person_game_demo`'s player prefabs (terrain-bump control; no new geometry there). Later mirrored into `parkour_demo`.
- [ ] Docs: `docs/20_data_formats.md` movement table (~L2300 near `max_walkable_slope_deg`: field row, limits above, interaction note), `docs/dev/player-ground-detection.md` and `crates/ironhold_core/src/capabilities/CLAUDE.md`
  (new "Step-up" subsection after the coyote section: the Decision 7 rule, the `raw_grounded && jumps_used == 0` gate, why `StepOffset` is a component),
  `tests/CLAUDE.md` table row. If the CLAUDE.md "ground-sensor" prose mentions blocked ledges, update it.
- [ ] Reviews in one parallel message after tests: `alignment-reviewer`, `system-architect`, `debug-detective` (explicit brief: ledge-edge jitter, mid-jump
  gate, compound colliders), `ux-gamedesigner-reviewer` (RON + docs + demo), `wasm-perf-reviewer`. `ron_lint` + `ron_validation` after the RON edits;
  WASM dev build with `--features inspector` (F9 collider wireframes are how you judge the lift).
- [ ] `planning/claude_suggestions.md` entries at implementation: NPC step-up (reuse `find_step_up`), step-down snap, optional `player.stepped_up` event.

## Playtest checklist
Project: `local_coop_demo` -> room10 (then `3rd_person_game_demo`). `python serve.py`; use a different port than `test_web.py`.
1. Walk then run into each curb (0.15 / 0.30 / 0.45): the first two climb with one visible step, the 0.45 stops like a wall. Tune `step_height` to see the boundary move.
2. Hold forward against the 0.45 curb for 10 s: no vertical creep, no jitter, no slow rise.
3. Stair lane (0.5 m treads): smooth ascent, no per-riser stall; descend: no falling animation flicker. The 0.25 m-tread stair is documented as not climbed.
4. Jump next to a 0.30 curb: jump height and landing animation as before the feature (compare with `step_height: 0.0`); no pop mid-ascent.
5. Walk up a hillside (`3rd_person_game_demo`) and a 30 degree ramp: feel identical to before.
6. Press into the 1.2 m cube and a wall corner: no teleporting; jump still works while pressed against a prop.
7. Both split-screen players in room10 step independently; camera pop on a step is acceptable or not (Open question 3).
8. F9: after a step the capsule bottom sits on the top surface, not inside the edge.

## Open questions
1. **Default `step_height`.** Plan ships `0.0` (opt-in) so no shipped project changes; demos opt in at `0.35`. Unity's default is `0.3`. Flip the engine default after the
   playtest (and after Task 1's baseline), or keep opt-in permanently?
2. **NPC scope.** Players only in v1 (Finding 7). Monsters will still stall on curbs; accept, or make NPC step-up part of this feature (adds per-NPC casts on WASM)?
3. **Visual pop.** The lift is an instant Y write of up to `step_height` (engine-standard, but the orbit camera follows the entity and render interpolation is deliberately
   not used). Accept, or add camera-side Y smoothing as a follow-up if the playtest shows a visible hitch?
4. **Event.** Emit a `GameEvent::Trigger("player.stepped_up")` for designer sounds/animations? Plan says no (stairs would spam it); say if wanted.

## Acceptance criteria
- Given `step_height: 0.35` and a 0.30 m cuboid curb with a walkable top, when the grounded player walks into it at walk or run speed, then within 2 ticks the body stands on
  top (Transform and Rapier body), with `jumps_used`, coyote state, `Friction` and `Velocity` untouched.
- Given a 0.45 m curb, a wall, a low ceiling, or an unwalkable-slope top, then no lift occurs and no Y creep accumulates over 300 ticks.
- Given `step_height` omitted or `0.0`, then trajectories are bit-identical to today and no `StepOffset` exists; every pre-existing test passes unmodified.
- Given a jump pressed next to a curb, then no lift occurs while `jumps_used > 0` and jump/landing timing is unchanged.
- Given two identical runs, then per-tick positions are bit-identical (no `Time`, no query-order dependence).
- `ground_cast`, `is_walkable_contact`, the coyote block and the jump block are unchanged in the diff; `cargo test -p ironhold_core` (one file at a time) and
  `cargo check -p ironhold_cli` pass; docs and `ironhold_cli validate` checks updated; Task 1's baseline table recorded in this file.

## Plan-review (2026-10-02)

Run by `/plan-review` while Frank was away: system-architect and ux-gamedesigner-reviewer, in parallel, read-only, claims checked against the code. **Combined verdict: needs more design work** — the blocking items below must be folded into this plan (and Frank's answers to the open questions recorded) before it can be marked Ready. The reports are reproduced verbatim (headings demoted one level).

### System-architect review

## system-architect plan-review: `planning/features/step_offset_auto_step.md`

Reviewed against `integration` @ `34803b1` (no `crates/` change since the plan's `9f23098`). Read-only; nothing compiled.
Plan claims spot-checked and confirmed: player body/collider (`entity_spawner.rs:1076,1117-1124`, compound capsule on the
player entity itself), FixedUpdate chain + `mark_dirty_trees` before `SyncBackend` (`lib.rs:312-322`), `ground_cast`/
`is_walkable_contact` (`player.rs:281,350`), jump/coyote/reset (`player.rs:498-562,667-685`), friction toggle
(`player.rs:643-648`), view-box clamp precedent (`player.rs:694-713`), `warn_negative_coyote_time_secs` site
(`entity_spawner.rs:1091`), CLI neighbour (`validate.rs:3204,3229`), `MovementConfig` has `deny_unknown_fields` + a
hand-written `Default` (`catalog.rs:1435-1437,1516`), ~54 `CharacterController {` literals (plan says ~56, fine).

#### Verdict
**Needs more design work.** The algorithm is the right shape and the footprint (optional component, opt-in default,
players only, no `CharacterController` field, no edit to `ground_cast`) is sound. Two items have to be fixed in the plan
text before code: the step gate does not mean "standing on the ground", and the casts are specified loosely enough that
a straight implementation of the plan would reject every real step. Both are text fixes, not redesigns.

#### Blocking

**B1. `raw_grounded && jumps_used == 0` is a proximity test, not a contact test. The "can never trigger mid-air" claim
and "land in either order" (Decision 7) are false.**
- `raw_grounded` (`player.rs:477-478`) is "a walkable hit within `collider_radius + ground_cast_length` = 0.7 m below
  the feet". It is true for the last 0.7 m of any fall, and nothing there looks at velocity.
- Reachable today, with `jumps_used == 0`:
  (a) **walk off a ledge** (`jumps_used` is already 0) and fall alongside a wall-step or curb: within 0.7 m of the
  floor below, the gate passes and the player is lifted mid-fall onto a ledge up to `step_height` above the feet. That is
  an unplanned mantle, and `linvel.y` stays negative after the lift, so they slam down on the ledge.
  (b) **On HEAD, mid-ascent**: the airborne-reacquisition bug *is* `jumps_used` resetting to 0 while still rising
  (`player.rs:552-561`, mechanism in `airborne_ground_reacquisition.md:16`). So "jump next to a curb, no lift while
  `jumps_used > 0`" does not hold on HEAD. After the reset the step gate opens mid-jump.
  (c) **After the airborne fix**, the "apex hover" residual (`airborne_ground_reacquisition.md:62`) still resets
  `jumps_used` at `vy <= 0` while up to 0.7 m above a platform, which again opens the gate in mid-air.
  (d) **Hover oscillation**: if the lift happens but XZ progress is then cancelled (e.g. `player_view_box_clamp_system`
  zeroing velocity at the box edge, which runs *after* the lift), the floating player is still `raw_grounded` from the
  floor below. They fall back below the step top and get lifted again, a vertical pumping that the "no Y creep over 300
  ticks" test (taller curbs only) does not cover.
- **Correction to the plan text:**
  - Keep the `(Entity, ShapeCastHit)` from the existing `ground_cast` call (`player.rs:477`); this is a local-variable
    change, not an edit to `ground_cast`.
  - Add a genuine-contact gate: `hit.time_of_impact - (collider_radius + GROUND_CAST_SKIN) <= STEP_CONTACT_GAP` (about
    0.05 m). `GROUND_CAST_SKIN` must move from fn-local to module scope so `step_up.rs` can see it. A penetrating hit
    (toi 0) counts as contact.
  - Rewrite the Interactions bullet and Decision 7. The gate is `raw_grounded && jumps_used == 0 && in_contact`, and
    only `in_contact` makes it independent of the airborne fix.
  - Add tests: "walk off a ledge beside a 0.3 m wall-step: no lift during the fall", and "lifted, then XZ blocked: no
    repeated lift". Run both on cuboid and trimesh.
  - Sequencing: agree with the airborne plan, **airborne fix lands first**. Ideally both plans use one named
    "touching" gap constant in `player.rs` rather than two near-identical literals (`ASCENT_CONTACT_GAP` vs a new
    `STEP_CONTACT_GAP`).

**B2. Cast poses, penetration semantics and the `feet` source are under-specified, and each one decides correctness.**
- **`stop_at_penetration`.** `ShapeCastOptions::default()` has `stop_at_penetration: true`
  (`parry3d-0.25.3/src/query/shape_cast/shape_cast.rs:141`). In `shape_cast_support_map_support_map.rs:36-53`, any cast
  whose start pose already overlaps a collider returns a `toi == 0` hit whatever the direction. In the steady state the
  player is pressed into the step (the previous tick's solver contact, with Rapier's allowed penetration), and the
  unlifted capsule rests on the floor. So with defaults, **step 2 (headroom, cast up) returns toi 0 → `H = -0.01` →
  rejects every real step.** The 2 cm skin helps against the floor but does nothing against a near-vertical riser face
  (h ≥ ~r), where a vertical lift does not separate the capsule from the face.
  The plan must state per cast:
  - Blocker (1): `stop_at_penetration: true`. We *want* the pressed-against blocker reported at toi 0.
  - Headroom (2), clear path (3), landing (4): `stop_at_penetration: false`. parry then drops a toi-0 contact whose
    `normal1·vel >= 0` (separating), which is exactly the floor and the step corner when casting up.
  - Keep `compute_impact_geometry_on_penetration: true` on all of them.
  - Verify on TriMesh too: each triangle goes through the support-map path, so the option applies there as well.
- **Explicit poses.** Write every pose as an offset from one `feet`, for example:
  `P0 = feet + Y*SKIN` (blocker origin); headroom cast from `P0`; `PL = P0 + Y*H`; clear-path cast from `PL`;
  `PF = PL + dir*(radius + travel + MARGIN)`; landing cast from `PF` down `H + SKIN`;
  `rise = (PF.y - toi) - feet.y`.
  As written, step 1 starts at `feet + 0.02` but step 4 computes `rise = H - toi`, which silently assumes the lifted
  pose is `feet + H`. If an implementer lifts from `P0`, the rise comes out 2 cm short and the 1 cm `STEP_LIFT_SKIN` no
  longer clears the edge. A penetrating lift is not benign on this body: Rapier turns penetration into recovery
  velocity (the airborne investigation measured +0.98 m/s from a cube-edge clip), which shows up as a visible pop.
- **`feet` must be `transform.translation`, not `global_transform.translation()`.** bevy_rapier's
  `writeback_rigid_bodies` writes `Transform` only (`bevy_rapier3d-0.33.0/src/plugin/systems/rigid_body.rs:403+`).
  `GlobalTransform` is propagated in the *next* tick's `SyncBackend`, so on the second tick of a multi-tick frame (≈4/s
  at 60 Hz, `lib.rs:285-295`) GT is one step stale. Meanwhile the query pipeline has the post-step collider positions
  and step 5 writes the fresh `Transform`. If the probe uses GT and the lift is applied to Transform, `rise` is off by
  one tick of vertical motion. Walking *down* a slope into a step, that means landing up to ~4-5 cm below the top
  (run speed on a 30° slope), i.e. penetration, which brings back the pop above.
  `Transform` is the authoritative post-Writeback pose for a root dynamic body (the player is a root). Say so, and say
  why it deliberately differs from `ground_cast`'s GT input.

#### Non-blocking

1. **Derive the probe shape from the body's own `Collider`, and drop `StepOffset.capsule_half_segment`.** The player's
   `Collider::compound` (with its `(0, cap_half + cap_radius, 0)` offset) sits on the player entity
   (`entity_spawner.rs:1119-1123`). Pass `collider.raw.as_ref()` with origin = `feet`. Then the probe is the body by
   construction: there is no duplicated dimension to drift when collider sizing changes, and the GLB-vs-primitive
   derivation split (`entity_spawner.rs:997-1009`) cannot desync it. Use `Option<&Collider>` (same required-component
   trap reasoning as `Friction`), so `StepOffset { height }` is the only new state.
2. **Corrections in "Interactions":**
   - "Nothing later in the tick reads this entity's `GlobalTransform`" is wrong. `npc_behavior_system` reads the
     player's GT later in the same chain (`npc.rs:215`). It sees a one-tick-stale Y after a lift, which is harmless and
     identical to the view-box-clamp precedent, but the sentence should say that.
   - `mark_dirty_trees` does not propagate anything; it only flags. Propagation is bevy_rapier's
     `RapierTransformPropagateSet` in `SyncBackend` (`plugin/plugin.rs:140-141,313`). The body then moves via
     `apply_rigid_body_user_changes` → `rb.set_position` (`rigid_body.rs:~246`), because the new GT differs from
     `last_body_transform_set`.
3. **Finding 8 is overstated.** `RapierContextSimulation::move_shape` (`bevy_rapier3d-0.33.0/src/plugin/context/mod.rs:874`)
   runs the KCC, autostep included, as a pure query on any shape. No kinematic body is needed. Rejecting it is still
   reasonable: it needs `&mut` simulation context plus `RapierQueryPipelineMut`, it runs a full slide loop, and its XZ
   would disagree with the dynamic solver. Record those reasons instead. Consider using it as a **test oracle** in
   Task 1 or the step tests, to sanity-check `find_step_up`'s accept/reject boundary against Rapier's own `max_height`
   / `min_width`.
4. **Blocker walkability vs. low curbs.** A capsule pressed against a curb of height `h < r` touches the top *corner*.
   The contact normal's angle from up is `acos((r-h)/r)`, which is walkable (≤ 45°) for `h ≤ r(1 - cos 45°)` ≈
   **0.117 m** at default `r = 0.4`. So the blocker test rejects those, and the plan depends on the solver riding the
   rounded bottom up them unaided. Task 1's baseline must sample densely around 0.08-0.15 m and confirm there is no gap
   band (corner reads walkable, yet the solver stalls). If there is one, the blocker test needs a contact-height
   criterion instead of walkability.
5. **Insertion logic is not exercised by the planned harness.** `wall_friction_tests.rs:73` hand-spawns the player, so
   "no `StepOffset` when `step_height` is 0/omitted" and the `player_height * 0.5` clamp are never run through
   `spawn_player_entity_core`. Factor a pure `step_offset_from(&MovementConfig, player_height) -> Option<StepOffset>`
   with unit tests, plus one scene-load fixture test under `assets/projects/integration_tests/`.
6. **CLI/runtime mirror.** Put the validity predicate in `schema/`, e.g. `MovementConfig::step_height_issue(player_height)`,
   called by both `warn_invalid_step_height` and `validate.rs`. The clamp depends on `player_height`, which comes from
   `movement.collider_height` (GLB) or `params.height` (primitive), so the CLI must resolve it the same way or the two
   bands diverge. Use `!(x >= 0.0)` for the NaN case (matches the `coyote_time_secs` precedent at `validate.rs:~3215`).
7. **Clamp size.** `0.5 * player_height` = 0.9 m at defaults is far above `collider_radius`. It works algorithmically
   (the clear-path cast is the real guard), but a designer who types 0.8 gets a 0.8 m instant pop. Consider a strict
   warning above `collider_radius` ("taller than the capsule's rounded base; expect a visible pop") rather than a
   tighter hard clamp.
8. **Determinism: write down the strengths explicitly.** Casts read the query pipeline as of the *last* step, so one
   player's lift is invisible to another player's probe in the same tick, and player iteration order cannot change the
   outcome. The Transform write goes through the same GT→`compute_transform`→`set_position` path that every turning
   tick already uses (`player.rs:593-596`), so it adds no new glam-backend exposure. Both belong in the `src/CLAUDE.md`
   "Step-up" subsection. Also relevant to `gameplay_fixed_tick_pipeline.md` §5: once the executor runs in FixedUpdate
   after movement, a same-tick `ResetToSpawn`/teleport simply overwrites the lift. That is fine; note it.
9. **Rebase note.** If the airborne fix lands first, the acceptance line "`ground_cast` ... unchanged in the diff"
   becomes "`ground_cast_with` / `GroundProbe` unchanged" (the airborne plan flags this itself at `:69`).
10. **Docs.** In `docs/dev/player-spawn-sites.md`, note that `StepOffset` is the first *conditional*
    shared-post-dispatch component (present only when `step_height > 0`). A future "every player has X" audit should
    not treat its absence as a bug.
11. **WASM/perf.** OK as stated: 1 capsule cast per moving, in-contact, opted-in player per tick, and ≤ 4 casts (≤ 16
    worst case with re-queries) on a step tick. `excluded_this_tick` stays alloc-free until the first push. The B1
    contact gate further reduces calls (no probing in the air). The `wasm-perf-reviewer` pass in the plan is still
    appropriate.

#### Open questions for Frank

1. **Default `step_height`.** Recommend keeping `0.0` / opt-in for v1 and deciding per project after Task 1's baseline
   and the room10 playtest. An engine-wide flip changes the feel of every shipped project at once, which deserves its
   own playtest pass.
2. **NPC step-up.** Recommend deferring, as planned. `find_step_up` as a free function keeps the door open. Revisit only
   when a real NPC route stalls.
3. **Visual pop.** Recommend accepting it for v1, consistent with the "no render interpolation" decision in
   `src/CLAUDE.md`. Add camera-side Y smoothing only if the playtest shows a hitch. If it does, smooth the *camera*, not
   the body.
4. **`player.stepped_up` event.** Recommend no for v1 (stair spam, and no consumer yet). Log it in `claude_suggestions.md`.
5. **(New, from B1)** Should a step ever fire when not in contact, i.e. a deliberate ledge-mantle while falling?
   Recommend **no** for this feature. If wanted, it is a separate "mantle" feature with its own animation hook, not a
   side effect of step-offset.

### UX-gamedesigner review

## UX / game-designer plan-review: `planning/features/step_offset_auto_step.md`

Reviewed against `integration` @ `34803b1` (plan written at `9f23098`). Read-only; nothing compiled.
Surfaces cross-checked: `docs/20_data_formats.md` MovementConfig table (~L2288-2325) and NPC block (~L3230-3260),
`docs/15_authoring_tools.md` validate/`--strict` tables, `docs/STATUS.md` L51, `local_coop_demo` room9/room10 +
prefabs, `3rd_person_game_demo` prefabs, `blank_project` prefabs.

#### Verdict

**Needs more design work** (small and targeted). The schema shape is right for designers: one optional `f32` on `movement:`,
`0.0` = off, players only, no new event. It matches house style: unitless metre fields (`collider_radius`, `ground_cast_length`)
have no suffix, `0.0 = off` matches `coyote_time_secs`, the bad-value handling (warn and treat as off) matches
`negative_coyote_time_secs`, and reusing `max_walkable_slope_deg` instead of adding a second slope limit is the right call.
Two things need fixing before code: (1) the docs tasks put the designer-relevant rules in `CLAUDE.md` instead of `docs/`, and skip
two designer-facing surfaces; (2) the "tread >= ~0.5 m" stair rule, which is the main thing a designer wants this feature for, is
an unmeasured guess, and by the plan's own algorithm it is probably too small.

#### Blocking

**B1. The docs tasks miss designer-facing surfaces and put the behaviour rules in a file designers can't read.**
The Docs task lists `docs/20` (one row + limits + interaction note), `docs/dev/player-ground-detection.md`, `crates/ironhold_core/src/capabilities/CLAUDE.md` and `tests/CLAUDE.md`.
Designers can't open either `CLAUDE.md`. Add the following:
- `docs/15_authoring_tools.md` strict-only table: add one bullet each for `invalid_step_height` and
  `step_height_exceeds_collider_limit`, in the same shape as the `negative_coyote_time_secs` bullet (L281). Every existing
  MovementConfig check has a bullet there, so a new code without one breaks the pattern designers look up when a validate run fails.
- `docs/20` MovementConfig row: write it as designer rules, not mechanism. It must say all of these explicitly:
  - it only acts while the player is grounded **and** commanding movement; it never acts while jumping or airborne, and a rooted
    (`player_speed` 0) player with input held does not step;
  - **players only**: NPCs ignore it and stall on the same curb (see B1's NPC bullet below);
  - the clamp is half the player's capsule height. GLB players take that from `collider_height` and primitive players from
    `primitive.height`, so name both fields. The same GLB/primitive split is already spelled out in the `collider_radius`/`collider_height` rows;
  - walking **down** is not snapped. It relies on the ground sensor reach (`collider_radius + ground_cast_length`, 0.7 m at defaults), so a
    riser taller than that reach plays the fall/land animation on the way down;
  - dynamic props **and small NPCs** with a walkable top shorter than `step_height` get climbed rather than pushed. A capsule
    collider's apex normal is vertical, so it reads as walkable. A 0.3 m "rat" NPC under `step_height: 0.35` becomes a stepping stone;
  - the stair tread-depth rule (see B2).
- `docs/20` NPC section (~L3230-3260): add one sentence: "NPCs do not auto-step; `step_height` is player-only. An NPC chasing the
  player stops at any curb the player stepped over, so use ramps on NPC chase paths." Without it, the curb-kiting exploit reads as an engine bug.
- Cross-references in the existing rows: `coyote_time_secs` (L2305) currently claims it "absorbs ... small ledges", and the
  `ground_cast_length` row (L2303) tells designers to raise it for uneven terrain. Both should point at `step_height` as the real
  tool for ledges, so designers stop over-tuning sensor fields to fake stepping. That over-tuning is the direct route back into the
  documented `jump_cannot_clear_ground_sensor` trap.
- `docs/STATUS.md` L51 Player movement row: its field list is already stale (no `coyote_time_secs`, `max_walkable_slope_deg`,
  `ground_cast_length`, ...). At minimum add `step_height` and say the movement feature now covers small ledges.
- Add a "How do I make stairs/curbs walkable?" lead-in (one short paragraph plus a RON snippet) above or below the table. A designer
  searches for "stairs", not for "step_height".

**B2. The stair-tread rule is unmeasured, depends on radius and speed, and probably excludes real-world stairs. Measure it before
publishing it or building demo geometry from it.**
The "Known limits" section says to author treads >= ~0.5 m, and the room10 demo and a test case both use 0.5 m treads. In the
algorithm, step 3 sweeps the *lifted* capsule forward by `radius + travel + 0.05` and rejects on any hit, including the next riser.
The capsule starts with its axis about `radius` behind riser 1. At radius 0.4 that means the next riser must sit roughly
`2*radius + travel + margin` beyond riser 1, about 0.9 m at walk and more at run. `travel` is 0.078 m at 5 m/s and 0.13-0.16 m at
8.5-10 m/s. The lift only partly offsets this, because a hemisphere lifted by `H` still dips below a riser top at `2*rise`. My numbers
are rough. The point is that 0.5 m is not established, and real-world stairs (about 0.18 rise, 0.28 tread) almost certainly won't
climb at all for a 0.4-radius capsule.
Stairs are the reason the plan gives for this feature ("prerequisite for believable interiors/stairs"). A designer who builds normal
stairs and sees the player stop dead at riser 2, with no warning, will decide the feature is broken.
Fix:
- Extend Task 1 (baseline) to also record the **minimum climbable tread** at walk and run speed for radius 0.4 (shipped default and
  `blank_project`) and 0.3. Put the table in the plan.
- Publish the rule in `docs/20` using the designer's own fields, e.g. "tread depth >= about 2 x `collider_radius` (`primitive.radius`) + 0.2 m".
- Build the room10 stair lanes from the measured numbers, so the "should climb" lane really climbs.
- If the measured tread is unacceptable for interiors, decide now, not after playtest, between two options: (a) make step 3 tolerate a
  next riser that is itself <= `step_height` above the landing; or (b) document the standard game workaround, a ramp collider under
  stair visuals. Option (b) needs verification first: I could find no documented way for a designer to author an invisible
  collider-only primitive in `docs/20`. If none exists, option (b) is not available to designers today.

#### Non-blocking

**N1. Demo content: what a designer sees.**
- `3rd_person_game_demo` has **one** player prefab, not several (`prefabs.ron:16`, movement has only `coyote_time_secs`). Turning on
  `step_height: 0.35` there "with no new geometry" gives designers a field with no visible effect, and it quietly changes how the
  player handles terrain lips. Either add two or three curb/rock-lip props near spawn with a short world label, or don't opt that
  project in for v1. If you do opt in, add an inline comment on the field (`// metres; auto-climb ledges up to this height - see docs/20 MovementConfig`).
  Per the dev-path memory, the comment should point at docs/20, not `planning/`.
- `player_p1_split_ring` is shared by **room9 and room10** (room10's own comment says "reused verbatim from room9"). Enabling
  `step_height` on it changes room9 too, and nothing tells the reader. Either accept it and add a comment on the prefab saying both
  rooms are affected, or clone a `_step` variant. The split-switch duplication pattern makes cloning costly, so accepting plus a comment is fine.
- Room10 layout: spawns are at x=+/-4 walking to -z (click targets at z=-6, cubes at z=-1.5), and portals are at z=+/-15 on x=0.
  Put the steps lanes on the outer flanks (around x=+/-10, running along z) so they don't block the routes to either portal. Mark
  each element with short ASCII world labels following the established short-token pattern, e.g. `0.15`, `0.30`, `0.45 too tall`,
  `tread 0.25: no`. Do not add another screen `Label`: room10 already has 6 hint lines (y 20-240), and any new line must stay under
  the verified 82-char ceiling and use ASCII hyphens, not em-dashes.
- Room10 is a good home on the merits. Its whole claim is GLB-vs-primitive parity, and stepping is one more parity proof. Say so in
  the `room_hint`/scene comment, so the lanes don't read as unrelated clutter.
- `blank_project` (the starter template) player prefab has `movement: (walk_speed: 5.0, run_speed: 8.0)`. If the default stays `0.0`,
  add a commented line there (`// step_height: 0.3,  // auto-climb curbs/stairs up to this height (metres)`). Opt-in features are
  only discoverable if the copy-from-here file mentions them.

**N2. Naming: consider `max_step_height`.**
`step_height: 0.35` reads to many designers as "every step lifts 0.35 m". The plan itself relies on the opposite (Decision 5: the lift
is the measured rise, never `step_height`). `max_step_height` matches Godot, pairs with the sibling `max_walkable_slope_deg`, and
removes the ambiguity. Rename it now if you're going to; it gets expensive after it ships.
`step_height_exceeds_collider_limit` describes the internals. `step_height_exceeds_half_player_height` (or
`max_step_height_exceeds_half_player_height`) tells the designer what to change.

**N3. Two more designer-facing coupling checks, both advisory `--strict`:**
- `step_height` > `collider_radius + ground_cast_length`: the player climbs risers it can't walk back down cleanly (fall and land
  animation flicker on descent, since there is no step-down snap). This is the same reach invariant as `jump_cannot_clear_ground_sensor`.
  It matters because the docs already tell designers to *lower* `ground_cast_length` to fix jump re-arm.
- `step_height` >= the resolved jump apex: anything the designer meant as a jump-up becomes walk-up, which trivialises platforming and
  the queued `parkour_demo`. This one is genuinely a design choice, so `--strict` is the right tier.

**N4. The 0.5 x height clamp is generous.** At defaults it allows 0.9 m, which is hip height and would read as teleporting onto tables.
The clamp is a sanity guard, not a feel recommendation, so state a recommended range in the docs row (e.g. "typical 0.2-0.4").

**N5. Playtest checklist.**
- Step 8 relies on F9 (inspector build). That is fine for Frank's dev playtest, but say so explicitly so nobody adds it to designer-facing docs.
- Add: walk into a short dynamic prop and a short NPC to confirm the documented "climbed, not pushed" behaviour; descend the stair lane
  at **run** speed (the step-down flicker case); confirm room9 still feels unchanged (shared prefab); and walk down a riser close to
  the `collider_radius + ground_cast_length` reach.
- Step 7 (camera pop): judge it on the 3-riser stair, not a single curb. Repeated pops are where it reads as stutter.

**N6. Primitive vs GLB.** The plan handles this correctly: one collider path, only dimension derivation differs. The docs row just has to
name both source fields (see B1). No separate demo is needed beyond room10's existing GLB + primitive pair.

#### Open questions for Frank (with recommendations)

1. **Default 0.0 vs ~0.3.** Recommend **0.0 for v1**, with the discoverability fixes above: the blank_project commented line, the
   docs/20 "How do I make stairs walkable?" lead-in, and the 3rd_person opt-in only if it has visible geometry. Re-decide after Task 1
   and the B2 tread measurement. If the capsule already climbs about 0.15-0.2 m unaided, a flipped default buys little. If you do flip
   it later, use ~0.25-0.3 (Unity's 0.3 matches what designers coming from Unity expect), and treat it as its own backlog item with a
   playtest pass over every shipped player prefab, because it changes platforming in primitive_world/particles_demo/stats_demo at once.
2. **NPC scope.** Recommend **players-only for v1**, provided the NPC limitation is documented in the NPC section (B1) and a backlog
   entry exists for NPC step-up (reusing `find_step_up`). Curb-kiting is a real exploit, but a level designer can avoid it with ramps.
   It isn't worth per-NPC WASM casts in this feature.
3. **Visual pop.** Recommend **accept for v1, gated on the stair-lane playtest**. The default Orbit camera has no follow smoothing, so
   the whole view jumps up to `step_height` in one tick, in both split-screen halves. `Follow` mode's `smoothing` already hides it. If
   the stair lane reads as stutter, the follow-up should smooth Y on the **camera side** (orbit target Y lerp), never on the body,
   because body smoothing would break the deterministic lift.
4. **`player.stepped_up` event.** Recommend **no for v1**. The only strong designer use is footstep or stair sounds, and those want a
   per-footfall cadence, which a per-riser event doesn't give. Log it in `claude_suggestions.md` as the plan already proposes. If it
   is ever added, name it `player.stepped_up` (consistent with `player.jumped`), and in co-op it needs a per-player payload, since
   two players share one event name today.
