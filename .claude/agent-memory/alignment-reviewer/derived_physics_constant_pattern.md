---
name: derived-physics-constant-pattern
description: uphill_jump_lock (v2 grace / v3 slope limit / v4 coyote time / v5 sensor exclusion / v6 prop ground veto / v7 wall friction) — the derive-vs-expose-vs-invariant-vs-conditional tiers for physics values, the single CharacterController construction site, the can_jump branch-exclusivity footgun, and the recurring stale-doc + missing-test-registry pattern
metadata:
  type: project
---

Reviewed 2026-08-20 (v2 grace fix), 2026-08-21 (v3 slope limit), 2026-08-22 (v4 coyote time),
2026-08-23 (v5 `.exclude_sensors()` + `normalize_or_zero()` on the ground cast) — all on
`feature/uphill-jump-lock`. Plan: `planning/features/uphill_jump_lock.md`.

**Third tier added at v5 — *neither* derive nor expose: an unconditional engine invariant, with no
RON knob AND no design-time diagnostic.** "A Rapier `Sensor` is never floor" is the canonical
example (`player_movement_system`'s ground `cast_shape` `QueryFilter`). Discriminators to reuse:
(1) the value is boolean-correct, not tunable — there is no project for which "my trigger volume
should be standable" is meaningful, since a sensor has no physical presence to stand on; (2) every
comparable engine hardcodes the same behavior (Unity's CharacterController ignores triggers, Godot
shape queries default `collide_with_areas: false`, Unreal overlap-only bodies miss ground traces) —
none expose it per-volume; (3) a "should validate warn about a big `trigger_zone` radius?" proposal
should be **rejected** once the bug is structurally fixed: there is no derivable correctness
boundary for a trigger radius (region triggers legitimately span tens of metres, schema has no upper
bound), so the check would be an arbitrary magic threshold in the engine — exactly the anti-pattern.
Contrast with this feature's three *accepted* diagnostics (`jump_cannot_clear_ground_sensor`,
`invalid_walkable_slope_limit`, `negative_coyote_time_secs`), which all detect conditions with a
derivable or definitional boundary. **Two designer-authorable routes produce sensors** —
`PrefabDef.trigger_zone` (sensor *child*, `entity_spawner.rs::attach_prefab_features`) and
`PrimitiveDef.sensor: true` (collectibles, e.g. `primitive_world`'s `coin_collectible`) — so any
sensor-related fix must be reasoned about against both, not just `trigger_zone`.

**The two-sided rule this feature established for physics constants — cite both halves together:**
- *Derive, don't expose*, when the value is **internal correctness bookkeeping** whose correct
  value is a function of already-authored geometry. `CharacterController.jump_air_grace` (`u16`
  FixedUpdate tick countdown, `capabilities/player.rs`) is computed per jump-fire by
  `jump_air_grace_ticks(vel, controller)` from `collider_radius + ground_cast_length` (both
  `MovementConfig` RON fields) + jump velocity + `GRAVITY`. A knob here would let a designer set it
  too low and silently reintroduce the bug; the designer still gets indirect control via
  `ground_cast_length`. The `Friction` 0.15 precedent covers bare constants; this covers
  geometry-dependent ones.
- *Expose as a real RON field*, when the value is **gameplay-facing tuning with a semantic every
  comparable engine exposes**. `MovementConfig.max_walkable_slope_deg` (default 45.0) mirrors Unity
  `slopeLimit` / Unreal `WalkableFloorAngle` / Godot `floor_max_angle`;
  `MovementConfig.coyote_time_secs` (default 0.1, `schema/catalog.rs` ~1408-1419 +
  `default_coyote_time_secs()`) is the universally-implemented coyote-time feel knob. Three
  decisions in one feature are *consistent, not contradictory* — the discriminator is "does the
  designer have a gameplay intent about this number?", not "is it physics?".

**`spawn_player_entity_core` (entity_spawner.rs ~979-998) is still the ONE `CharacterController {`
construction site** (verified again at v4 — `grep "CharacterController {" src/` returns only it plus
the struct def). Every player spawn path (both scene-load sites, `Action::Spawn` character-select,
hot-join) routes through the single `assemble_player_config`. `PrefabComponents.movement` is
`#[serde(default)]` (catalog.rs ~1302) + `MovementConfig` has a full `Default` impl, so a new
`MovementConfig` field reaches all four paths by touching only those two places — the exception to
the usual 3-spawn-path footgun in [[prefab-marker-three-spawn-paths]].

**FOOTGUN found at v4 — `can_jump`'s two branches are mutually exclusive, so anything that extends
`loco.is_grounded` also *suppresses* double jump for exactly that long** (`capabilities/player.rs`
~474-478): grounded branch is `jumps_used == 0`, else branch is `double_jump_enabled && jumps_used <
max_jumps`. During the coyote window after a ground jump, `jumps_used == 1` *and* `is_grounded ==
true`, so neither branch permits a jump — and jump input is `just_pressed`-only
(`runtime/input.rs` ~372-379), never buffered, so the press is silently swallowed (~94ms at the
0.1s default, proportionally worse for any larger authored value). Both doc sites claimed the
buffer "only" delays the falling animation and widens the edge-jump window. Any future change that
holds `is_grounded` true longer must re-check this branch pair.

**v6 (`feature/prop-ground-veto`, reviewed 2026-09-01) — the ground cast is now a bounded re-query
loop, and it confirmed a *fourth* recurring shape: the escape hatch's own docs go stale.**
`player_movement_system` loops up to `MAX_GROUND_CAST_CANDIDATES = 4` `cast_shape` calls, using
`QueryFilter::predicate` to exclude any already-seen hit whose `details.witness1.y >
feet_pos.y + collider_radius * 0.5` (a side/wall contact, not floor). Both new constants correctly
land on the *derive-don't-expose* tier: `4` is a query-cost safety bound with no gameplay semantic,
and the `0.5` factor scales off the authored `collider_radius`, so the designer keeps indirect
control exactly like `jump_air_grace`. **Do not re-litigate these as missing RON knobs.**
Two things this pass got wrong that are worth checking on every future ground-cast change:
- **`docs/20_data_formats.md`'s `max_walkable_slope_deg` row (~line 2286) is the single
  designer-facing home for every ground-cast caveat** — it accumulated a "**Known limitation:** a
  solid prop or wall … not yet fixed" paragraph at v5 that v6 (the fix) did not remove. Any
  ground-cast fix must edit that row, not just the two `CLAUDE.md`s and the schema doc comment.
- **The `90.0` escape hatch's documented meaning keeps narrowing without the docs following.**
  Three sites claim `90.0` = "every hit counts as ground regardless of angle / exact pre-fix
  proximity-only behavior" (docs/20 ~2286, `schema/catalog.rs` ~1476, and now player.rs's own
  comment ~379-382). After v6 the underfoot filter runs *before* the slope check unconditionally, so
  a non-underfoot (wall) contact is no longer a ground candidate even at `90.0` — the hatch is now
  "any *underfoot* hit". Repeated pattern: an escape hatch documented once as "restores old
  behavior exactly" and then silently qualified by the next fix in the same code path.
- **Entity-granularity limit worth remembering:** the predicate excludes whole *entities*, so a
  single collider that is both wall and floor (the zero-thickness `TriMesh` terrain, one entity)
  can be dropped entirely by one non-underfoot contact. The fix therefore only covers separate prop
  entities, not terrain-integrated cliffs — the "known limitation" text should be rewritten to this
  residual case rather than deleted.

**v7 (`feature/wall-friction`, reviewed 2026-09-03) — a *fourth* tier: the same constant, applied
conditionally.** `PLAYER_IDLE_FRICTION = 0.15` (`capabilities/player.rs` ~52, `pub(crate)`) is now
the shared source of truth for the spawn-time `Friction` in `entity_spawner.rs::spawn_player_entity_core`
(~1110) *and* a per-tick sync in `player_movement_system` (~608-613):
`if raw_grounded && !loco.moving { PLAYER_IDLE_FRICTION } else { 0.0 }`. Reusable reasoning for
"should this gate be RON-authorable?" — **no**, when the alternative branch is a *bug* rather than a
style: a velocity-driven controller that re-writes `linvel.x/z` every tick must never expose itself
to solver Coulomb friction while commanding motion (a wall's 3D tangent plane includes vertical), and
Unity/Unreal/Godot/Quake all either never expose character motion to solver friction or apply ground
friction only. Same tier as "a Sensor is never floor". Do not re-litigate.
- **No RON friction field exists anywhere** — grepped `src/schema/` and `assets/projects/**`:
  zero hits. So the player's `Min` combine rule makes the fix undefeatable by designer-authored
  surfaces (a prop with no `Friction` gets Rapier's default 0.5/`Average`; `Min` outranks `Average`
  → effective μ = the player's 0.0). A fix that had instead required `friction: 0` on wall prefabs
  would have been the blocking shape. Icebox item "RON-authorable collider friction" is the tracked
  future promotion — when it lands, the constant's home should move to `schema/catalog.rs` as a
  `default_*` fn (like `default_coyote_time_secs`), not stay in `capabilities/`.
- **The three designer-facing 0.15 doc claims all went stale in the same pass** (the recurring
  pattern from v6, now confirmed twice): `docs/20_data_formats.md` `idle_drag` row (~2282, "fixed at
  `0.15` for every player") and `max_walkable_slope_deg` row (~2286, "any actual sliding is an
  incidental side effect of the player's fixed `Friction` (`0.15`)" — now literally false while
  moving), plus `crates/ironhold_core/src/CLAUDE.md`'s v2 friction paragraph (~1114-1127). Check all
  three on any future friction change.
- **Test-registry omission recurred a third time**: a new `tests/*.rs` file was absent from both the
  root `CLAUDE.md` test-loop list (~87) and the `tests/CLAUDE.md` table. `uphill_jump_lock.md` ~611
  records the same omission for `prop_ground_veto_tests.rs`. Always check both registries for a new
  test file.
- **`local_coop_tests.rs` ~6542-6553 asserts `(0, 0.15), (1, 0.15)`** on real-spawn-path players.
  It only still passes because that harness never runs `player_movement_system`; it is now an
  assertion about the *spawn-time* value, not the carried one. Any future harness change that adds
  the movement system flips it to 0.0 for an airborne/moving player.

**Open gaps at v4 review (non-blocking, flag if still present):**
- **`coyote_time_secs` has no validation twin** while its sibling `max_walkable_slope_deg` got
  both (`warn_invalid_walkable_slope_limit` in scene_loader.rs ~2824 + `invalid_walkable_slope_limit`
  in `ironhold_cli/src/commands/validate.rs` ~854-867). Negative/NaN is silently laundered to zero
  ticks by `coyote_ticks` (player.rs ~66), and any value under ~1/128s silently `.round()`s to a
  0-tick (i.e. absent) buffer. The player-prefab loop in validate.rs ~796-867 is the natural home.
- **`coyote_time_secs: 0.0` is the undocumented opt-out** (restores exact pre-coyote behavior) —
  same shape as the `max_walkable_slope_deg: 90.0` escape hatch, which *was* documented at v3.
  Recurring pattern: this feature keeps adding fields whose "disable me" value is only discoverable
  from the code.
- **New knobs are proven only by direct-`CharacterController`-literal tests.**
  `tests/player_slope_jump_tests.rs` builds the controller by hand (~123-141), so no test covers
  `MovementConfig.<field>` → `CharacterController.<field>` wiring; dropping the
  `coyote_time_secs: mv.coyote_time_secs` line in entity_spawner.rs would keep every test green.
  `max_walkable_slope_deg` is at least a fixture *parameter*; `coyote_time_secs` is hardcoded 0.1.
- **No shipped project authors either new field** (grepped `assets/projects/**`) — every project
  silently takes the default and a designer can only discover the knob from `docs/20_data_formats.md`.

**Resolved since earlier reviews** (don't re-report): the `jump_exit`-on-a-plain-fall regression
(player.rs ~376, plus a dedicated regression test); `jump_cannot_clear_ground_sensor` documented in
`docs/20_data_formats.md` + `docs/60_contributing.md`; `max_walkable_slope_deg` range validation
(runtime warn + CLI `--strict`) and its `90.0` escape hatch, both now documented.

**Still-open duplication (logged to claude_suggestions, not a blocker):** `GRAVITY`, jump-height
resolution, and the `1.8`/`0.4` collider defaults are re-derived independently in
`scene_loader.rs`, `player.rs`, and `validate.rs` (literal `unwrap_or(1.8)`/`unwrap_or(0.4)`) — a
`MovementConfig::resolved_collider_*` helper in `schema/catalog.rs` would be CLI-reachable, same
reasoning as the `PrefabDef::is_player()` move in [[diagnostic-only-feature-pattern]].
