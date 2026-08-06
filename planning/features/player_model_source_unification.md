# Feature: Player Model Source Unification ("multiplayer with 1")

_Status: In Progress (v1 Done 2026-07-19, v2 Ready — confirmation pass complete 2026-08-06, v3
Queued)_
_Planned at: `6e38aa1` (2026-07-17)_
_v2 fleshed out at: `1fcef14` (2026-07-31) — moved back from `features/done/` per the multi-phase
convention in `planning/CLAUDE.md` (only the final phase's completion moves a multi-phase file to
`done/`; v1 alone shipping was not that). v2 revised after plan-review at `2026-08-01`: system-
architect found the prefab-strategy gap (retrofitting either room7 or an under-specified existing
prefab would either violate the no-retrofit rule or hard-fail CLI validation) and, more
importantly, that the Friction fix is a terrain-regression risk, not a low-risk one-liner;
ux-gamedesigner-reviewer converged on the same prefab-strategy gap independently and added the
demo's docs/animation-gap/room-chain requirements._

**Confirmation pass (2026-08-06, system-architect + ux-gamedesigner-reviewer, ahead of cutting the
`feature/{slug}` branch).** Both reviewers confirmed the 2026-08-01 findings were correctly folded
in, but independently caught the same new drift: `per_viewport_target_ring_visibility.md` shipped
*after* the 2026-08-01 pass (`95d68a9`, 2026-07-31) and introduced `SplitScreenDef.own_viewport_only`
— which `player_p1_split` (this plan's original GLB-half prefab choice) deliberately does **not**
set (`prefabs.ron:1149-1153` says so explicitly), while v2's own acceptance criteria ("a ring
appears on *their* target only... P2's own `target_hud` updates while P1's does not") describe
exactly the `own_viewport_only: true` behavior. As drafted, the plan would have shipped a demo that
contradicts its own acceptance criteria. **Fix applied below: room10 is now based on room9 (the
`_ring` prefab family), not room3.** Three smaller findings also folded in: a color-identity task
for room10 (ground/portal accent), an explicit statement that the return portal reuses
`portal_to_room9` verbatim (room9's own precedent — no new prefab), and a corrected "two portals
past the hot-join room" distance (was mis-stated as one). See the revised Approach/Tasks/Acceptance
criteria below for the exact changes.

**Plan-review note (2026-07-17):** Both reviewers returned Needs-more-design-work on the first
pass; both sets of findings are now folded into the plan above. **system-architect**: caught that
v1's task list understated real cost — primitive body construction needs mesh/material/catalog
resources `spawn_player_entity_core` doesn't have, and 2 of its 3 call sites (terrain-deferred,
character-select dynamic-spawn) structurally can't get them without a separate resource-promotion
effort → v1 rescoped to the immediate scene-load path only, with v3 added for the deferred paths;
also caught the dispatch discriminant was wrong (`prefab.kind`, not `shape`/`children` presence)
and that the character-select task was based on a false premise (`Action::Spawn` already actively
rejects primitive player prefabs today, not merely "untested"). **ux-gamedesigner-reviewer**:
confirmed the zero-RON-surface claim is genuinely true (verified against all shipped projects), but
found v1 as first scoped was unverifiable by playtest (its headline gains only show with 2+
players, deferred to v2) and that it would trade one silent designer footgun for a subtler one →
pulled a minimal 2-primitive-player proof forward into v1, added docs tasks (a stale `docs/20`
"primary player" example, and a new designer-facing note on which `PrefabDef` fields now apply to
players vs. still silently don't), and rewrote several acceptance criteria to be browser-observable.
Verified independently against current source (line numbers, `terrain_demo`/`quick_scene` project
contents) while incorporating both reviews, ahead of cutting the `feature/{slug}` branch.

## Phases

| Phase | Backlog item | Status | Completed |
|---|---|---|---|
| v1 | `PlayerModelSource` enum — collapse the primitive/capsule player path into `spawn_player_entity_core`, scoped to the immediate scene-load path | Done | 2026-07-19 |
| v2 | Fuller `local_coop_demo` demonstration (mixed primitive + GLB) + `Friction` reconciliation | Revised — recommend confirmation pass | — |
| v3 | Resource promotion so primitive players also work via terrain-deferred spawn and character-select dynamic spawn | Queued | — |

## What
Today there are two structurally separate ways a player character gets its body: a GLB-model
player routes through `assemble_player_config` → `spawn_player_entity_core` (which also gives it
`PlayerIndex`, its own `StatMap`, material overrides, animation, and multi-player camera policy);
a primitive/capsule-shaped player (`kind: "primitive"` prefab tagged `["player"]`) is built by a
completely separate ~165-line inline block in `scene_loader.rs` that never touches
`spawn_player_entity_core` at all. v1 collapses these into one pipeline — a `PlayerModelSource`
enum on `PlayerConfig` that `spawn_player_entity_core` dispatches on for body construction only,
with every downstream player feature (camera policy, `PlayerIndex`, per-player `StatMap`, split-
screen, the `player_stat_widgets` feature once it ships) becoming reachable by *any* player
regardless of whether its body is a GLB model or a primitive shape. v2 is the new capability that
unification unlocks: primitive-bodied players actually participating in local co-op (multiple
primitive players, or a primitive player alongside a GLB player, sharing split-screen/party camera
like GLB players already do).

This is the structural half of Frank's "eventually we'll want single-player to just be
multiplayer-with-1" framing (2026-07-17 conversation) — not a networking feature, a code-path
convergence that removes an entire recurring bug class (see Why).

## Why
Every recent local-coop feature (`per_player_split_screen_targeting`, `per_player_stat_pools`, and
the in-progress `player_stat_widgets`) has had to explicitly re-derive "the four player-construction
sites" and wire each new field through all of them by hand — a real, recurring source of silent
per-field divergence (documented in `crates/ironhold_core/src/CLAUDE.md`'s "The four player-
construction sites" section). Investigating that pattern for `player_stat_widgets`
(system-architect consultation, 2026-07-17) found the root cause isn't camera-assignment logic —
`spawn_players_and_camera` already unifies party/split/dynamic/grid camera policy for however many
`PlayerConfig`s it's given — it's that **the primitive player path is a different body-construction
mechanism that was never folded into that pipeline at all**, so it structurally cannot receive
anything `spawn_player_entity_core` provides.

Concretely, verified by reading both paths (`entity_spawner.rs:735-873` vs.
`scene_loader.rs:705-870`), a primitive-bodied player today gets:
- `Player`, `PlayerOwnership::Local`, `PlayerTarget::default()`, `SpawnId`/`PrefabKey` (via
  `tag_spawned_entity`), and nameplate — same as a GLB player (these *are* duplicated correctly
  today, just by hand).
- **No** `PlayerIndex` — split-screen's "P1"/"P2" corner HUD label and per-player-targeting's
  primary-player check can't distinguish a primitive player from another one.
- **No** `stat_templates` → `StatMap` — the field is captured on the GLB path's `PlayerConfig` but
  the primitive path's inline block never reads `prefab.stat_templates` at all, so a primitive
  player can never have its own action-bar cost pool (`per_player_stat_pools`).
- **No** `material` (`PendingMaterialOverride`) — same gap the `per_player_stat_pools` review
  found and fixed for the GLB path; never fixed for primitive because there's no shared code to fix.
- Structurally **at most one** primitive player per scene — the scene loader collects it into a
  bare `Option<(...)>` tuple (`scene_loader.rs:173`), not a `Vec`, and a code comment at line
  169-170 states outright that "the primitive/capsule player path below is separate and stays
  single-player-only." This is the literal opposite of "multiplayer with 1" — today primitive
  players are hard-capped at exactly one, permanently, by data structure, not by a deliberate
  design choice.

To be precise about scope: **neither** player path calls `attach_prefab_features`
(`entity_spawner.rs:40`) — `behavior`/`interactable`/`dialogue`/`inventory`/`trigger_zone` on a
player prefab silently no-op on *both* the GLB and primitive paths today, equally. That gap is
real but out of scope here (see "Explicitly out of scope"); this feature is about the fields that
already differ *between* the two player paths, not the fields both paths equally lack.

**Caveat, stated plainly so it isn't misread as MP progress:** this converges *code paths* for
local, same-machine co-op. It has no bearing on real networked multiplayer readiness, which is
blocked by unrelated things (cross-platform Rapier float determinism, a `SimClock` chokepoint) —
those remain a separate, later workstream.

## Approach

### v1 — `PlayerModelSource` enum, collapse the primitive body-construction path

**Schema (Rust-only — `PlayerConfig` is never deserialized directly from scene RON, confirmed
during `per_player_stat_pools` and again during the `player_stat_widgets` consultation, so this is
purely additive/internal, zero RON migration):**

```rust
// schema/player.rs
pub enum PlayerModelSource {
    Glb(String), // replaces the old bare `model_path: String`
    Primitive {
        shape: crate::schema::catalog::PrimitiveShapeKind,
        params: crate::schema::catalog::PrimitiveParams,
        children: Vec<crate::schema::catalog::ChildPrimitiveDef>,
    },
}
```

`PlayerConfig.model_path: String` becomes `PlayerConfig.model_source: PlayerModelSource`. (Any
code reading `.model_path` today — `spawn_player_entity_core`'s `gltf_path`/`gltf_handle`
construction — moves into the `Glb` match arm unchanged.)

**`assemble_player_config`** (the existing single source of truth for building a `PlayerConfig`
from a `PrefabDef` — already used by both the scene-load GLB collector and the character-select
dynamic-spawn path) gains the branch: **`prefab.kind == PrefabKind::Primitive`** →
`PlayerModelSource::Primitive { .. }`; otherwise → `PlayerModelSource::Glb(prefab.model.clone())`.
(Correction from an earlier draft of this plan, caught by system-architect: dispatching on
`prefab.shape.is_some()`/`!prefab.children.is_empty()` is wrong, because a valid primitive prefab
may have `shape: None` **and** empty `children` — `scene_loader.rs:251` already defaults a missing
`shape` to `PrimitiveShapeKind::Capsule3d` for exactly this case — which that heuristic would
misclassify as `Glb` and then fail on an empty `prefab.model`. `prefab.kind` is the correct,
already-authoritative discriminant, exactly matching how `scene_loader.rs:247-249` distinguishes
a primitive player from a GLB one today.)

**`spawn_player_entity_core`** dispatches on `player_config.model_source` for body construction
only:
- `Glb(path)` arm — today's logic unchanged (`model_spawner.spawn_instance`, GLTF handle,
  animation policy loading).
- `Primitive { shape, params, children }` arm — rebuilds what the inline block in
  `scene_loader.rs:724-832` does today: `build_primitive_mesh`, `primitive_material`, the compound
  capsule collider (including the zero-`Friction` component the primitive path has and the GLB
  path doesn't — see v2's cleanup note), the mesh child, and `spawn_primitive_children` for
  cosmetic children. **Visibility note:** `build_primitive_mesh` and `spawn_primitive_children` are
  currently private `fn`s in `scene_loader.rs`; `primitive_material` is already `pub(crate)`. Both
  need to become at least `pub(super)` so `entity_spawner.rs` (a sibling module under
  `scene_manager`) can call them — a mechanical visibility change, not a logic change.
  **Implementation note (system-architect finding, post-implementation):** the old inline block
  hardcoded `Damping { linear_damping: 0.5, angular_damping: 0.5 }`; the unified arm reads
  `mv.linear_damping`/`mv.angular_damping` instead (same as the GLB arm always did).
  `MovementConfig` defaults both to 0.5 and `primitive_world`'s `player_capsule` regression
  baseline doesn't override either, so this is behavior-identical for that specific prefab — but
  it's a capability improvement, not a pure no-op: any *other* primitive player prefab that sets a
  custom damping value now actually gets it, which it silently didn't before.

Everything *after* body construction in `spawn_player_entity_core` (`tag_spawned_entity`,
`Player`/`PlayerOwnership`/`PlayerIndex`/`PlayerTarget`, the `StatMap` build, nameplate, material
override, animation-policy attach) already runs unconditionally today for GLB players and needs no
per-variant branching — it becomes reachable by primitive players simply by virtue of routing
through the same function.

**Resource-threading blocker, and why v1 is deliberately scoped to the immediate scene-load path
only** (per system-architect, 2026-07-17 — this is the actual reason this feature is riskier than
`player_stat_widgets`, and the original draft of this plan didn't call it out explicitly enough).
Unlike GLB body construction (which only needs `asset_server`/`model_spawner`, already threaded
through `spawn_player_entity_core`), primitive body construction needs `&mut Assets<Mesh>`,
`&mut Assets<StandardMaterial>`, `&Assets<CustomMaterial>`, the **per-scene-load built-materials
map** (`mats.built.0` — constructed fresh as a local value inside `spawn_scene_v2`, not a
`Resource`), `project.primitive_default_color`, and the asset/prefab catalogs
(`ChildSpawnCtx`, `scene_loader.rs:2901`). `spawn_player_entity_core` is called from **three**
places, and only one of them has these resources in scope:
1. **`spawn_scene_v2`'s immediate scene-load path** (via `spawn_players_and_camera`) — has
   everything (`mats`, `asset_catalog`, `prefab_catalog`, `project`) already in scope. ✅ v1 scope.
2. **`spawn_delayed_players_system`** (terrain-deferred spawn, runs after terrain finishes loading,
   a separate system) — has none of it today; could in principle gain some via `SystemParam`s, but
   the built-materials map specifically has no persistent home to read it back from later. ❌ v1
   does not touch this.
3. **`drain_spawn_queue_system`** (`Action::Spawn`/character-select dynamic spawn) — same gap, and
   moot anyway: `action_executor.rs:111`'s `asset_catalog.models.get(&prefab_def.model)` lookup
   already rejects a primitive prefab (`model == ""`) with a "model key not found" warning **before
   `assemble_player_config` is ever reached** — a primitive character-select player is actively
   rejected today, not merely untested (see the corrected Tasks entry below). ❌ v1 does not touch
   this either.

**v1 therefore only unifies primitive players spawned via the immediate, non-terrain scene-load
path** (the only path with all three player-construction sites — GLB scene-load, primitive
scene-load, and their shared camera policy — actually converging on shared resources today).
Terrain-deferred and dynamic-spawn primitive players are explicitly deferred to v3 (a real,
distinct resource-architecture problem — promoting the built-materials map to something
re-derivable outside the one-shot scene-load pass — not just more plumbing), and each of those two
paths gets a scene-load-time `warn!` (and `ironhold_cli validate` error) if a primitive player
prefab is used in a scene with `terrain: Some(...)`, or if `Action::Spawn` targets a primitive
player prefab — both cases already fail today (a primitive player currently spawns immediately
regardless of terrain per the existing inline block's placement, so a terrain+primitive-player
combination is untested territory, not a v1 regression; `Action::Spawn` already rejects primitive
prefabs outright), so the warning documents an existing limit rather than introducing a new one.

**Scene loader (`scene_loader.rs`):** delete the inline primitive-player block
(`~705-870`). The primitive-player collector (currently the singular
`primitive_player: Option<(...)>` at line ~172-173) is replaced by calling
`assemble_player_config` for that prefab too and pushing the result onto the *same*
`player_configs: Vec<PlayerConfig>` list the GLB collector already builds — removing the
structural single-primitive-player cap as a direct side effect, not a separate task — **but only
when `scene.terrain.is_none()`**; when terrain is present, a primitive player prefab logs the v3
deferral warning above and does not spawn (matching today's "primitive players don't participate
in terrain-delayed spawn" gap, now made explicit instead of accidental). The single call to
`spawn_players_and_camera(&player_configs, ...)` (already handling 1..N GLB players' camera policy)
now handles any mix of GLB and primitive players identically for the non-terrain case.

**v1 also includes a minimal 2-primitive-player proof, not deferred to v2** (per
ux-gamedesigner-reviewer, 2026-07-17): v1's headline claims — a primitive player gets its own
`PlayerIndex`/`StatMap`, and the single-primitive-player cap is gone — are otherwise only
observable with 2+ primitive players, which would leave the riskiest part of v1 (the `Vec`
collector change) merged with zero test coverage until v2. `local_coop_demo` has no primitive
player prefab today (every existing player prefab there is GLB), so add one minimal 2-primitive-
player split-screen scene (cloning an existing split room, swapping both player prefabs for
`kind: Primitive` capsules) as part of v1, plus one integration test asserting the two primitive
players get distinct `PlayerIndex` values and independent `StatMap`s. This is a small, cheap add
(the split-screen scene infrastructure already exists) and is the only thing that actually falsifies
"v1 removed the structural cap" rather than merely asserting it in prose.

### v2 — primitive-player local co-op polish + cleanup (follow-on, after v1 is playtested stable)

**Fleshed out 2026-07-31, ahead of formal plan-review**, since the original draft above was too
thin to review against (a one-sentence "fuller demo" with no concrete room/prefab plan, and a
Friction question with no proposed resolution).

**Demo scope — a *new* room, not a retrofit of v1's `room7`.** `room7` (v1's 2-primitive-player
proof) should stay exactly as it is — a minimal, focused regression baseline proving the
structural single-primitive-player cap is gone, nothing more. Mutating it to also carry
per-player-targeting/stat-pool wiring would conflate "does the cap-removal still work" with "does
the fuller feature set work," the same anti-pattern this whole local-coop batch has consistently
avoided (see `room9`'s own precedent: a sibling copy, not a retrofit, when demonstrating a new
combination of existing mechanics). Proposed: a new `local_coop_demo` room (**room10**, two portals
past the hot-join demo room — room8 → room9 → room10, corrected from an earlier draft's "one
portal past") pairing **one primitive-bodied player and one GLB player** (not two more
primitives — the "mixed" pairing is the actual point, since v1's proof already covers
two-primitives-together) — reusing the `target_indicator:`/`target_hud:`/per-player `ActionBar`
wiring pattern `room3`/`room9` already established. This validates the actual headline claim — a
primitive player participates in every per-player mechanic exactly like a GLB player does, side by
side in the same scene — rather than re-asserting it in prose.

**Corrected base prefab pair (2026-08-06 confirmation pass) — room10 is based on room9's
`own_viewport_only` prefab family, not room3's default-visibility one.** The GLB half must reuse
`player_p1_split_ring` (not `player_p1_split`, the original draft's choice) — only
`player_p1_split_ring` sets `camera.split.own_viewport_only: true`
(`prefabs.ron:1186-1190`), and v2's own acceptance criteria describe exactly that behavior ("a ring
appears on *their* target only... P2's own `target_hud` updates while P1's does not"). Reusing
`player_p1_split` verbatim, as originally drafted, would have shipped a demo whose actual on-screen
behavior (every ring visible in both viewports, room3's default) directly contradicted its own
acceptance criteria. The primitive half's new prefab must mirror `player_p2_split_ring`'s input
wiring (`gamepad_index: 1`, `target_next: "KeyM"`, `look_left: "Comma"`/`look_right: "Period"`,
`ArrowUp`/`ArrowDown`/`ArrowLeft`/`ArrowRight`/`Enter`/`ShiftRight`) applied to a primitive body
(extending `player_p2_primitive`'s shape/collider, per the composed-`children:` note below) — same
naming precedent room9 set for its own siblings, e.g.
`player_p2_primitive_split_ring`/`player_p2_primitive_target` (naming TBD during implementation).

**Revision (2026-08-01, after plan-review — system-architect + ux-gamedesigner-reviewer):** both
reviews independently confirmed mixing GLB+primitive players in one split scene is architecturally
free (nothing in the split-screen/camera/`RenderLayers` system assumes a uniform model source), but
both also found the original draft's "reuse the exact wiring pattern" claim glossed over three real
gaps. Incorporated below.

**Prefab strategy — new sibling prefabs, not a mutation of `room7`'s existing ones (both reviews,
independently).** `room7`'s `player_p1_primitive`/`player_p2_primitive` author no `target_next`,
`look_left`/`look_right`, or `gamepad_index` at all — none of the per-player targeting/look/gamepad
wiring room3's pattern needs. Adding those fields to the existing prefabs would mutate room7's own
regression baseline (forbidden, per the "don't retrofit room7" decision above); a **new** pair —
`player_p1_split_target`/`player_p2_primitive_target` (naming TBD during implementation, following
the `_ring`-suffix precedent room9 already set for the same reason) — is the correct, precedented
shape. Concretely:
- **GLB half (corrected 2026-08-06)**: reuse `player_p1_split_ring` — **not** `player_p1_split`,
  the original draft's choice. Only `player_p1_split_ring` sets `camera.split.own_viewport_only:
  true` (`prefabs.ron:1186-1190`), which is required for room10's own acceptance criteria (own-
  viewport-only ring visibility) to actually hold — `player_p1_split` demonstrates room3's default
  (every ring in every viewport) and reusing it here would contradict this room's own claims. It
  already owns the split switch, `target_next: "KeyT"`, `look_left`/`look_right`, `gamepad_index:
  0`, a 100-mana pool, and the `action_bar_p1`/`gamepad_key: "RightTrigger"` pairing. Zero new
  prefab needed for this half.
- **Primitive half (corrected 2026-08-06)**: a new prefab extending `player_p2_primitive`'s
  body/collider shape with the same input wiring `player_p2_split_ring` uses — `target_next:
  "KeyM"`, `look_left: "Comma"`/`look_right: "Period"`, and **`gamepad_index: 1`** — the last one
  is not optional decoration: reusing room9's `ActionBar` pattern (which authors `gamepad_key:
  "RightTrigger"` on both bars) against a primitive prefab with no `gamepad_index` is a **hard
  `ironhold_cli validate` failure** (`gamepad_key_without_gamepad_index`), confirmed still live in
  `validate.rs:549` as of this confirmation pass — this is a blocking correctness requirement, not
  a nice-to-have. `orbit_button: "None"` is mandatory for any split-screen player (mouse can't drive
  2+ cameras), so omitting `look_left`/`look_right` would also leave the primitive player
  structurally unable to turn their own camera — undercutting the "participates exactly like a GLB
  player" claim in the most visible possible way. Note (system-architect, 2026-08-06): pad binding
  is now session-locked to whichever gamepad connects first for each seed index (`BoundGamepad`/
  `gamepad_bind_system`, shipped in `gamepad_player_binding_hardening.md` after this plan was first
  drafted) — add a one-line controls-hint note that on a real dual-controller WASM playtest, P1's
  pad should connect before P2's, or vice versa, matching whichever `gamepad_index` each is seeded
  with; this doesn't change the RON, just the on-screen hint text.
- **Player ordering**: GLB player first (owns the split switch, matching room3's known pattern);
  primitive player as P2 — the genuinely uncovered configuration (`room7`'s primitive player is
  already P1/primary; a primitive body as a *non-primary* player, with its own ring tint, its own
  `target_hud` viewport, and an `owner_player: 1` action bar resolving against a primitive body's
  `StatMap`, is what's actually unproven).
- **Visual identity (ux-gamedesigner-reviewer)**: give the primitive player a composed multi-part
  body via `children:` (e.g. a cuboid head + cylinder limbs, steel/silver tone) rather than a bare
  capsule. Two independent reasons, not just polish: a plain capsule beside a rigged GLB humanoid
  reads as "programmer placeholder next to the real character," undermining the room's own message;
  and **no shipped player prefab anywhere in this repo uses `children:`** today, even though v1's
  `Primitive` arm already calls `spawn_primitive_children` — this room is a genuine first real-world
  proof of composed primitive player bodies, not just cosmetic. Add a RON comment noting the
  children are cosmetic only (the physics capsule still comes from `primitive.radius`/`height`).
- **Animation gap, must be pre-empted on screen, not silently discovered (ux-gamedesigner-reviewer)**:
  no primitive player has an `animation_policy`, so the primitive half will slide rather than walk.
  In a room whose message is "everything works the same for both," an unexplained sliding character
  argues against the thesis unless it's named up front — add an on-screen hint (see UI text below).
- **Obstacle (system-architect)**: the room as scoped contains no shared obstacle for the Friction
  comparison task to actually use — author a couple of `Cuboid` blocks both players can walk past.

**Friction reconciliation — corrected scope and risk assessment (system-architect; this
supersedes the original draft's "one-line, low-risk" framing).** The mechanism itself is smaller
than described: post-v1 there is no separate "GLB collider construction site" — `Collider::compound`
is inserted once, unconditionally, in the shared post-dispatch block, and primitive-only friction
sits immediately below it behind a single `if let PlayerModelSource::Primitive { .. }` guard. The
change is **deleting that guard**, not adding a line.

**But the actual risk is not the cube-edge case the original draft compared against — it's
terrain**, and this was missed entirely in the first pass. `Friction { coefficient: 0.0,
combine_rule: CoefficientCombineRule::Min }` outranks Rapier's default `Average` against *every*
surface a player touches, including terrain (`capabilities/terrain.rs`'s collider carries no
`Friction` component at all, i.e. Rapier's default 0.5/`Average`). Friction was never doing much
*while moving* (movement writes `velocity.linvel` directly each tick) — the only two things it ever
did were resist edge-snagging and **hold an idle player in place on an incline**. Removing it
therefore risks an idle GLB player **creeping downhill** on any sloped terrain. `quick_scene` is a
live, shipped case: a GLB player (`player_warrior`) stands on real fbm terrain with real elevation.
No human-controlled body has ever run zero-friction on terrain in this engine — every existing
zero-friction body (NPCs, primitive players) is either AI-driven or confined to flat-ground demo
projects. The original draft's tie-break ("if the playtest shows no difference, default to add it")
is unsafe as written, because a flat `local_coop_demo` room *will* show no difference and would
wrongly greenlight a `quick_scene` regression.

**Corrected plan: a two-scene playtest, not one.** (1) The mixed room10's cube-edge comparison
(the original upside case), and (2) `quick_scene`'s hillside — stand an idle GLB player on a
visible slope, before and after, and confirm no creep. If (2) fails, the fallback is **not**
"revert to primitive-only" but a **low, non-zero coefficient** (e.g. `0.15`, still `combine_rule:
Min`) — enough static friction to hold a shallow slope while still eliminating edge-snag, a third
option the original draft didn't enumerate. Whichever way it lands, add a note to the
`MovementConfig` docs table (`docs/20_data_formats.md`) — currently the only friction-adjacent
knobs documented are `idle_drag`/`linear_damping`/`angular_damping`, and a designer has no way to
discover that collider friction isn't a per-prefab-authorable field at all.

**Additional fallback surfaced during the 2026-08-06 confirmation pass (system-architect):**
`idle_drag` (`MovementConfig`, default `0.8`, applied at `capabilities/player.rs:240-241`) is a
*second*, independent idle-hold mechanism this plan hadn't enumerated — steady-state downhill creep
approximates `gravity_component * dt / (1 - idle_drag)`, so `idle_drag` alone is a real fourth
option alongside "leave Friction at 0", "low non-zero Friction coefficient", and (not viable per the
above) "revert to primitive-only". If the two-scene playtest shows creep, try tuning `idle_drag`
on `quick_scene`'s player prefab before reaching for a hardcoded non-zero Friction constant — it's
already a per-prefab-authorable RON field, whereas a Friction coefficient today is not. If a
non-zero Friction constant does turn out to be necessary, consider promoting it to a real
`MovementConfig.friction: Option<f32>` field (default `None` = today's zero-friction-for-primitives/
default-friction-for-GLB behavior) rather than a bare hardcoded Rust constant, so it's consistent
with every other per-prefab movement knob and doesn't reintroduce a designer-unreachable value.

**Resolved by real playtest (2026-08-06, Frank).** Both scenes were tested with `Friction {
coefficient: 0.0 }`: room10's cube-edge comparison was clean (no catching), but `quick_scene`'s
hillside showed real, confirmed downhill creep — the risk this section's risk assessment predicted,
not a hypothetical. Fixed via the **low, non-zero coefficient** option (`0.15`, `combine_rule:
Min`), applied uniformly to every player regardless of `model_source` — **not** by tuning
`idle_drag`, since `idle_drag` only asymptotically bounds creep rather than eliminating it, and
lowering it far enough to matter also cancels horizontal air momentum right after releasing input
mid-jump (no grounded gate). No `MovementConfig.friction` field was added — `0.15` is a fixed engine
constant for now (logged to `planning/backlog.md`'s Icebox as a possible future physics-material
field, since nothing has asked for per-prefab friction tuning yet).

### v3 — resource promotion for terrain-deferred and dynamic-spawn primitive players

The harder, structurally distinct follow-on flagged by system-architect: promote whatever
primitive body-construction needs (the built-materials map, `primitive_default_color`, mesh/material
asset access) into a form `spawn_delayed_players_system` and `drain_spawn_queue_system` can reach —
neither is a simple parameter-threading exercise like v1's, since the built-materials map today is
one-shot state computed fresh per scene load, not persistent resource state. Only worth doing once
a real project actually wants a terrain scene or a character-select flow with a primitive-bodied
player option — no current project does. Until v3 ships, the v1 warnings make this limitation
explicit and diagnosable rather than a silent gap.

## Explicitly out of scope
- Wiring `attach_prefab_features` (`behavior`/`interactable`/`dialogue`/`inventory`/`trigger_zone`)
  onto either player path. Both paths lack it equally today; fixing it is a separate, independent
  feature with its own design question (e.g. does "behavior" — NPC state-machine driven — mean
  anything for an input-driven `Player`? probably not; but `inventory`/`trigger_zone` plausibly do).
  Not blocking this unification and shouldn't be bundled with it.
- `player_stat_widgets.md` (`stat_label`/`world_stat_bar` for players) — a separate, already-Ready,
  smaller feature that ships independently and first; v1 here does not depend on it, but a
  primitive player will inherit its wiring for free once both have shipped, since both route
  through the same `DynamicStatUiQueue` push from `spawn_player_entity_core`.
- Real networked multiplayer (determinism, `SimClock`) — unrelated workstream, see Why.

## Tasks
- [x] `PlayerModelSource` enum in `schema/player.rs`; `PlayerConfig.model_path` → `model_source`
      (note: `PlayerModelSource` needs its own `#[derive(Deserialize)]`, or `PlayerConfig`'s
      `Deserialize` derive — vestigial today, nothing actually deserializes it — is dropped instead)
- [x] `assemble_player_config`: branch on `prefab.kind == PrefabKind::Primitive` (not
      `shape`/`children` — see the corrected discriminant in Approach) to build the correct
      `PlayerModelSource` variant
- [x] Bump `build_primitive_mesh`/`spawn_primitive_children` visibility to `pub(super)` (scene_loader.rs)
- [x] `spawn_player_entity_core`: dispatch body construction on `model_source`; move the primitive
      body-construction logic from the inline `scene_loader.rs` block into the `Primitive` arm.
      Thread the needed `Assets<Mesh>`/`Assets<StandardMaterial>`/`Assets<CustomMaterial>`/
      built-materials-map/`primitive_default_color`/catalog params through this function and its
      **scene-load-path** callers only (`spawn_players_and_camera`, called from `spawn_scene_v2`
      where these are already in scope) — not through `spawn_delayed_players_system` or
      `drain_spawn_queue_system` (see the resource-threading blocker in Approach; those two are v3)
- [x] `scene_loader.rs`: delete the inline primitive-player block; fold the primitive-player
      collector into the same `assemble_player_config` → `player_configs: Vec<PlayerConfig>` flow
      as the GLB collector when `scene.terrain.is_none()`, removing the single-primitive-player
      structural cap for the non-terrain case
- [x] Scene-load `warn!` + `ironhold_cli validate` error: a primitive player prefab combined with
      `scene.terrain: Some(...)` (v3-deferred, not a v1 regression — see Approach)
- [x] **Corrected from an earlier draft** (system-architect caught this): `action_executor.rs`'s
      `Action::Spawn` handler already rejects a primitive-shaped player prefab today (the
      `asset_catalog.models.get(&prefab_def.model)` lookup at `:111` fails on the empty `model`
      string before `assemble_player_config` is ever reached) — this is **not** a "verify it's a
      no-op" task, it's an existing, active rejection. v1 leaves this as-is (out of scope, deferred
      to v3); add a `warn!` at the rejection site clarifying *why* (primitive players aren't yet
      supported via character-select) instead of the current generic "model key not found" message.
      **Hardened further during review** (debug-detective finding): the rejection now fires
      unconditionally on `kind == Primitive && tags: ["player"]`, *before* the model lookup, not
      only when that lookup happens to fail — a primitive player prefab with a resolvable `model`
      key would otherwise have sailed past the guard and panicked at spawn time. Regression test
      added (`spawn_tests.rs`).
- [x] Tests — regression coverage that `primitive_world`'s existing `player_capsule` prefab
      (`assets/projects/primitive_world/prefabs/prefabs.ron`, the definitive single-primitive-player
      regression baseline) is pixel/behavior-identical after v1 (verified by playtest plus
      independent field-by-field code review from system-architect/debug-detective — damping,
      friction, and collider sizing all confirmed unchanged for this specific prefab); new coverage
      that a primitive player now gets `PlayerIndex`/`StatMap`/`material` like a GLB player; new
      coverage (see the v1 2-primitive-player proof above) that two primitive players in one scene
      get distinct `PlayerIndex` values and independent `StatMap`s; a test confirming the
      terrain+primitive-player warning fires (both scene-load `warn!` and `ironhold_cli validate`
      error, the latter via a new CLI fixture); a regression test for the hardened `Action::Spawn`
      rejection above.
- [x] `local_coop_demo`: add the minimal 2-primitive-player split-screen scene described above (v1,
      not v2) as the playtest aid proving the structural cap is actually gone (`room7.scene.ron`,
      reachable via room6's portal chain; playtest-confirmed 2026-07-19)
- [x] Docs — `crates/ironhold_core/src/CLAUDE.md`'s "The four player-construction sites" section
      (this collapses sites 2 and 4 into the shared pipeline for the non-terrain case; rewrite the
      section to reflect the new, smaller set of divergence-risk sites, and note the v3-deferred
      terrain/dynamic-spawn limitation)
- [x] Docs — `docs/20_data_formats.md`: fix the stale "primary player" example (~line 477-479),
      which currently cites "the primitive/capsule player path" as the canonical example of a
      player with no `PlayerIndex` at all — after v1 this is no longer true (for the non-terrain
      case) and the example is actively misleading. Keep the "or no `PlayerIndex` at all" fallback
      clause (still meaningful for the v3-deferred terrain case) but drop the primitive-path example.
- [x] Docs — add a short designer-facing note (a RON comment on `primitive_world`'s `player_capsule`
      prefab and/or `local_coop_demo`'s new primitive player prefab, plus a line in `docs/20`'s
      player-prefab section, ~line 1804) enumerating which `PrefabDef` fields now take effect on a
      `tags: ["player"]` prefab (`player_index`, `material`, `stat_templates`, `stat_label`/
      `world_stat_bar` once `player_stat_widgets` ships) versus which still silently no-op
      (`behavior`, `interactable`, `dialogue`, `inventory`, `trigger_zone` — the explicitly-out-of-
      scope gap), and noting the terrain/dynamic-spawn limitation deferred to v3. Without this, v1
      replaces one silent footgun (primitive players are limited) with a subtler one (some fields/
      contexts work now, others still don't, with no visible boundary)
- [ ] v2: new `local_coop_demo` **room10** pairing a new primitive-player-target prefab (P2) +
      reused **`player_p1_split_ring`** (P1) — **corrected 2026-08-06: not `player_p1_split`**, see
      Approach's confirmation-pass note (only `_ring` sets `camera.split.own_viewport_only: true`,
      which room10's own acceptance criteria require) — with a composed multi-part (`children:`)
      steel/silver body for the primitive half, `gamepad_index: 1` and `player_p2_split_ring`'s
      input wiring set (required — see Approach's blocking `gamepad_key_without_gamepad_index`
      finding), and a couple of `Cuboid` obstacles for the Friction comparison task below. Also add
      a new `ground_room10` prefab and a `portal_to_room10` accent color — pick a steel/silver-blue
      tone consistent with the primitive body's composed identity (existing rooms have already
      claimed amber/orange (room3), cyan/teal (room4), rose/magenta (room5), gold/amber-yellow
      (room6), emerald/green (room7), violet (room8), crimson (room9) — steel/silver-blue is
      unused).
- [ ] v2: five UI Labels on room10 — room hint (mixed-bodies framing), controls (including a
      one-line WASM dual-controller connect-order hint — see Approach's `BoundGamepad` note), a
      parity statement ("same targeting, same action bar, same per-player mana — only the body
      differs"), targeting/ability hint, and an explicit animation-gap hint ("P2 has no rig, so it
      slides instead of walking — the only real difference") — see Approach for the animation-gap
      reasoning
- [ ] v2: room9's `room_hint` needs a new sibling Label for the room10 exit (its own line, not an
      extension of the existing ~71-char line); a `portal_to_room10` prefab + `rules.ron` entry.
      The **return** trip needs no new prefab — reuse `portal_to_room9` verbatim, same as room9
      reused `portal_to_room8` (`room9.scene.ron:99-101`'s "same event name... no new prefab
      needed" precedent, confirmed still the pattern as of this confirmation pass).
      `scene.ready:room9` **and** `scene.ready:room10` Log rules (room9's own is currently missing
      from a prior room's addition — fix both while touching this file)
- [ ] v2: **two-scene** Friction playtest (see Approach) — room10's cube-edge comparison AND
      `quick_scene`'s hillside idle-creep check; implement whichever the comparison actually
      supports (zero-friction / low-non-zero-friction fallback / no change), then document the
      decision (not just the mechanism) in `crates/ironhold_core/src/CLAUDE.md`'s "Deliberately NOT
      unified in v1" note (rewritten, not deleted — the collider-*sizing* divergence there is
      unrelated and stays) and add a `MovementConfig` docs note that friction isn't a per-prefab
      field
- [ ] v2: update `docs/20_data_formats.md`'s "Special tag: player" section, which currently names
      `room7` as *the* canonical primitive-player example and explicitly calls out room3 as "a
      primitive-free example of a live, spendable pool" — both claims go stale the moment room10
      ships; also verify and document whether a primitive player prefab used as a
      `join_prefab_keys` hot-join entry works, warns, or silently fails (a third dynamic-spawn
      context the existing terrain/character-select documentation doesn't cover, and room10 sits
      one portal past the hot-join demo room)
- [ ] v2: integration test — a mixed `Glb` + `Primitive` `PlayerConfig` pair in one scene produces
      distinct `PlayerIndex`es, independent `StatMap`s, and two split cameras; add a one-line
      assertion that a GLB player carries `Friction` if the comparison task adds it
- [ ] v3: promote the built-materials map / primitive construction resources so
      `spawn_delayed_players_system` and `drain_spawn_queue_system` can also spawn primitive
      players (terrain-deferred and character-select respectively) — a distinct resource-
      architecture problem, not v1/v2-sized plumbing

## Open questions
- None outstanding for v1 — the two real open questions from the previous draft (the dispatch
  discriminant, and whether character-select needed a "verify" step) were resolved as concrete
  corrections above per system-architect's review; v3's resource-promotion approach is intentionally
  left undesigned until a real project needs it.
- **(resolved, post-review)** v2's `Friction` decision needs a **two-scene** playtest comparison
  (room10's cube-edge case AND `quick_scene`'s terrain-slope case), not the one-scene comparison
  originally proposed — see Approach for why terrain is the actual risk, not cube edges.
- **(resolved, post-review)** Demo prefab strategy: new sibling prefabs (not a `room7` retrofit,
  not a bare mutation of `player_p2_primitive`), GLB-first/primitive-as-P2 ordering, composed
  multi-part primitive body — see Approach.

## Acceptance criteria
- Given `primitive_world`'s existing `player_capsule` prefab (the single-primitive-player
  regression baseline, in a non-terrain scene), when v1 ships, then that player's visible behavior
  (movement, camera, collision) is unchanged — **browser-observable**: play `primitive_world`,
  confirm movement/camera/collision feel identical to before.
- Given a primitive-shaped player prefab with `material: <key>` set, when that player spawns after
  v1, then the material visibly applies (**browser-observable**: the capsule renders in the
  overridden color/material, which it silently did not before) — the same fix `per_player_stat_pools`
  made for GLB players, now closing the gap for primitive players too.
- Given a primitive-shaped player prefab with `stat_templates` set, when that player spawns after
  v1, then it gets its own `StatMap` exactly like a GLB player does today (e.g. its own action-bar
  cost pool) — closing the gap without any player-specific special-casing.
- Given the new v1 `local_coop_demo` 2-primitive-player split-screen scene (non-terrain), when both
  players spawn, then each shows a distinct "P1"/"P2" HUD corner label (**browser-observable**) and
  has an independent `StatMap` — proving the structural single-primitive-player cap is actually
  gone, not just asserted.
- Given a scene with 2+ players where at least one is primitive-shaped (non-terrain), when v1
  ships, then `spawn_players_and_camera`'s existing party/split/dynamic/grid camera policy applies
  to all of them identically, with no separate code path for the primitive one(s).
- Given a scene with `terrain: Some(...)` and a primitive-shaped player prefab, when v1 ships, then
  a clear `warn!`/`validate` error explains the player won't spawn (v3-deferred), rather than a
  silent failure or a confusing crash.
- Given room10, when both players spawn, then the GLB player shows "P1" and the primitive player
  shows "P2" in their own viewport corners (**browser-observable**).
- Given room10's primitive P2, when they press their target key (or click a sphere), then a ring
  appears on *their* target only, restricted to *their own viewport* (room10 deliberately reuses
  the `_ring` prefab pair — `player_p1_split_ring`'s `own_viewport_only: true` — per the
  **2026-08-06 confirmation-pass fix**: an earlier draft of this room instead reused plain
  `player_p1_split`, which does *not* set `own_viewport_only`, and would have contradicted this
  exact criterion), tinted with P2's colour, and P2's own `target_hud` readout updates while P1's
  does not (**browser-observable**).
- Given room10's primitive P2, when they fire their action-bar slot with a target selected, then
  P2's own mana bar drops and P1's is unaffected, and vice versa for P1's slot (**browser-observable**
  — proves per-player stat pools work identically regardless of body type).
- Given both players in room10, when each walks past the same cube obstacles, then neither body
  type catches or sticks differently from the other (**browser-observable** — the cube-edge half of
  the Friction comparison).
- Given a GLB player standing idle on `quick_scene`'s sloped terrain, when the Friction change
  ships at its resolved coefficient (`0.15`, not the initially-tried `0.0`), then they do not creep
  downhill (**browser-observable regression check, confirmed 2026-08-06** — the terrain half of the
  Friction comparison, and the one the original v2 draft didn't test for. `0.0` failed this exact
  check on first playtest; `0.15` was the fix).
