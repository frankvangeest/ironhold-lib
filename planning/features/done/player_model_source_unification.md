# Feature: Player Model Source Unification ("multiplayer with 1")

_Status: Done (v1 shipped 2026-07-19 — v2/v3 remain Queued design sketches, re-review before either starts)_
_Planned at: `6e38aa1` (2026-07-17)_

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
| v1 | `PlayerModelSource` enum — collapse the primitive/capsule player path into `spawn_player_entity_core`, scoped to the immediate scene-load path | Queued | — |
| v2 | Fuller `local_coop_demo` demonstration (mixed primitive + GLB) + `Friction` reconciliation | Queued | — |
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

With v1's minimal 2-primitive-player proof already merged, v2 is polish and the one deliberately-
deferred design decision: a fuller `local_coop_demo` demonstration (e.g. a mixed primitive + GLB
pairing, not just two identical capsules) exercising per-player targeting/stat pools end-to-end,
plus reconciling the one known behavioral inconsistency between the two paths found while reading
them for v1: the primitive path includes a zero-`Friction` component (`Friction { coefficient:
0.0, .. }`, prevents catching on cube edges) that the GLB path's collider does not — the NPC path
already has this same zero-friction component too (`entity_spawner.rs:304`), a partial precedent
worth weighing when deciding whether GLB players should get it too, or whether it stays
primitive/NPC-only by design (needs a quick playtest comparison, not a guess).

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
- [ ] v2: fuller `local_coop_demo` demonstration (e.g. mixed primitive + GLB pairing) exercising
      per-player targeting/stat pools end-to-end
- [ ] v2: decide and document the `Friction` component inconsistency (GLB vs. primitive collider)
- [ ] v3: promote the built-materials map / primitive construction resources so
      `spawn_delayed_players_system` and `drain_spawn_queue_system` can also spawn primitive
      players (terrain-deferred and character-select respectively) — a distinct resource-
      architecture problem, not v1/v2-sized plumbing

## Open questions
- None outstanding for v1 — the two real open questions from the previous draft (the dispatch
  discriminant, and whether character-select needed a "verify" step) were resolved as concrete
  corrections above per system-architect's review; v3's resource-promotion approach is intentionally
  left undesigned until a real project needs it.
- v2's `Friction` decision (see Approach) needs a playtest comparison, not a design-time guess —
  left as an open decision for v2, not v1.

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
- Given v2's fuller `local_coop_demo` demonstration (mixed primitive + GLB), when both players play
  split-screen, then per-player targeting and per-player stat pools work exactly as they do for two
  GLB players today.
