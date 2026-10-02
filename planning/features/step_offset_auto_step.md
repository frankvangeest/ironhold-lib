# Feature: Step-Offset / Auto-Step for Small Ledges (`MovementConfig.step_height`)

_Status: Draft_
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
  coyote-buffered value, per `src/CLAUDE.md`) and `jumps_used == 0`, which excludes the whole jump and its reset window, so a mid-ascent
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
- [ ] Docs: `docs/20_data_formats.md` movement table (~L2300 near `max_walkable_slope_deg`: field row, limits above, interaction note), `crates/ironhold_core/src/CLAUDE.md`
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
