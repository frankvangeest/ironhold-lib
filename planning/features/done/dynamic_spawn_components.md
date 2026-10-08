# Dynamic Spawn — Missing Components (motion, stat_label, world_stat_bar)

Planned at: df8c94b (2026-06-14)

## Problem

`spawn_prefab_instance` (called by `drain_spawn_queue_system` for `Action::Spawn`) handles
behavior, stat_templates, interactable, and trigger_zone correctly, but silently skips three
prefab-derived components that scene-placed entities receive:

| Component | Effect when missing |
|---|---|
| `motion` | rotate / bob animation never starts on dynamically spawned entities |
| `stat_label` | no floating world-space health label above entity |
| `world_stat_bar` | no GLB health bar above entity |

A scene-placed enemy gets all three. A rule-spawned `Action::Spawn` enemy gets none — no error,
no warning.

## Fix Strategy

### `motion`

`MotionDef` is entirely prefab-derived (no per-entity-def field). Move the motion `insert` from the
primitive and GLB branches in `scene_loader.rs` into `spawn_prefab_instance`. The call sites in
scene_loader that already call `spawn_prefab_instance` can then drop their own motion inserts.

### `stat_label` and `world_stat_bar`

These currently work by pushing spawn descriptors onto `StatLabelSpawnQueue` /
`WorldStatBarSpawnQueue` `Vec`s in `scene_loader.rs`, which are drained at end of the scene load
frame. Dynamic spawns don't participate in that frame-end drain, so they're silently skipped.

**Chosen approach — `Added<StatMap>` reactive system:**

Add a new system `attach_dynamic_stat_ui_system` that runs after `drain_spawn_queue_system` and
queries `(Entity, &PrefabKey, Added<StatMap>)`. For each newly-added `StatMap` it:
1. Looks up the `PrefabDef` from `LoadedPrefabCatalog`
2. If the prefab has `stat_label`, pushes to `StatLabelSpawnQueue`
3. If the prefab has `world_stat_bar`, pushes to `WorldStatBarSpawnQueue`

This is identical to what `scene_loader.rs` does for placed entities — same queues, same drain
path — so label/bar rendering is pixel-for-pixel identical regardless of how the entity was
spawned.

**Why not push directly from `spawn_prefab_instance`?**

`spawn_prefab_instance` is a `Commands`-based function, not a system. It can insert components but
cannot access resources like `StatLabelSpawnQueue` without threading mutable references through
every caller. The reactive system approach keeps `spawn_prefab_instance` free of system resource
dependencies.

## Files to Touch

| File | Change |
|---|---|
| `entity_spawner.rs` | move `motion` insert here from scene_loader branches |
| `scene_loader.rs` | remove motion inserts from GLB and primitive paths; they're now in helper |
| new system `attach_dynamic_stat_ui_system` | react to `Added<StatMap>` + `PrefabKey`, push to queues |
| `runtime/mod.rs` or wherever systems are registered | add new system after `drain_spawn_queue_system` |

## Schema / RON Impact

None — no new fields, no version bump.

## Acceptance

- A dynamically spawned `enemy_snake` via `Action::Spawn` shows a health bar, stat label, and
  patrol motion identical to a scene-placed instance
- Integration test: spawn entity with `motion_def` via `Action::Spawn`, assert `Motion` component
  is present
- Integration test: spawn entity with `stat_templates` + `world_stat_bar` via `Action::Spawn`,
  assert world stat bar appears (entity count in stat bar system > 0)
- Existing scene-placed stat label and world stat bar tests still pass

## Notes moved from `crates/ironhold_core/src/CLAUDE.md` (2026-10-08, core CLAUDE.md split)

These paragraphs were in the crate `CLAUDE.md` and are kept here verbatim; that file now carries only the condensed current-state rule. Wording such as "above"/"below" refers to the old file.

### b:1646: Spawn queue: `SPAWNS_PER_FRAME = 2`
<!-- moved-from-claude-md: b:1646 -->

## Dynamic spawning

### Spawn queue
`Action::Spawn` does **not** call `spawn_prefab_instance` inline. Instead it pushes a `QueuedSpawn` struct (pre-resolved: prefab def, model path, transform, spawn ID, project root) onto `PendingEntitySpawns`. `drain_spawn_queue_system` runs at the end of the interpreter chain and processes at most `SPAWNS_PER_FRAME = 2` entries per frame.

This caps wave-spawn WebGPU pipeline-compile stalls. On WASM, every new mesh+material combination causes a synchronous `device.createRenderPipeline()` call on first render (~100–300 ms each). Limiting to 2 spawns per frame keeps the per-frame stall under ~600 ms instead of seconds for large waves.

For single-entity spawns the queue is transparent: action_executor pushes, drain_spawn_queue processes, all within the same `app.update()` call.

`PendingEntitySpawns` is cleared on `Action::LoadScene` so no orphaned spawns execute after a scene transition.

### b:1657: Component parity: dynamic vs scene-placed (nameplate gating)
<!-- moved-from-claude-md: b:1657 -->

### Component parity with scene-placed entities

Dynamically spawned entities (via `Action::Spawn`) receive the same prefab-driven components as scene-placed entities:

- **`motion`** — inserted inside `spawn_prefab_instance` (GLB path), so any prefab with `motion:` gets rotation/bob automatically on dynamic spawn.
- **`stat_label` / `world_stat_bar`** — `drain_spawn_queue_system` pushes a `DynamicStatUiEntry` to `DynamicStatUiQueue` for each spawn whose prefab declares these widgets. `drain_dynamic_stat_ui_system` (runs in the same chained set, one slot after `drain_spawn_queue_system`) drains the queue and spawns the label/bar entities. The net result is a one-frame deferral — imperceptible in practice.
- **`NameplateTag`** — inserted at spawn time when `should_insert_nameplate(prefab.nameplate, show)` returns true (`scene_manager/mod.rs`, beside `tag_spawned_entity`): `nameplate: Some(false)` always suppresses; otherwise `show` or an explicit `nameplate: Some(true)` opt-in enables it. **`show` differs by entity type** — NPCs/props use `scene.show_nameplates`/`nameplate_config.enabled`; `Player`-tagged entities (see below) use the independent `show_player_nameplate` / `nameplate_config.player_enabled` instead, never `show_nameplates`. This is the single source of truth for all 6 nameplate-gating call sites (`scene_loader.rs` ×5 — 4 NPC/prop + 1 primitive-player, `entity_spawner.rs` ×1, `action_executor.rs` ×1 for the character-select dynamic player spawn) — do not re-inline the predicate at a new call site. `nameplate_setup_system` queries `Added<NameplateTag>` every `Update` frame and spawns the anchor + `Text2d` + pixel bar quads for any newly-tagged entity — scene-placed entities, dynamically spawned actors, and wave-spawned enemies all use the same path; it also re-checks `player_enabled` vs `enabled` per-entity (via `Option<&Player>`) since the tag alone doesn't carry which toggle governs it. Bar fills are driven by the existing `world_pixel_bar_update_system`. `NameplateSceneConfig` (resource) is populated from `GameSceneV2` on each scene load and cleared on `LoadScene`. `nameplate_visibility_system`'s `faction_filter` (HostileOnly/FriendlyOnly/All) is an NPC/prop-only categorization — `Player` entities bypass it entirely (same treatment as a `Some(true)` override: distance-only), since faction hostility doesn't apply to "should I see my own name."
- **`Player` / `PlayerOwnership`** (`capabilities/player.rs`) — marker inserted unconditionally wherever a player entity is spawned (`spawn_player_entity` for GLB, inline in `scene_loader.rs` for the primitive-player path). `PlayerOwnership::{Local, Remote}` is always `Local` today (no multiplayer exists yet) — reserved so nameplate/UI/camera systems can distinguish "me" from "other players" once Beta 0.6 (LAN co-op) lands, without another schema pass. See `planning/features/player_nameplate_visibility.md`.
- **`Action::ToggleOwnNameplate`** (v2 of the above) — flips `PlayerNameplatePreference` (a `Resource`, `init_resource`'d alongside `NameplateSceneConfig`), emitting `nameplate.own_shown`/`nameplate.own_hidden`. Consumed only by `nameplate_visibility_system`'s per-frame `Player`-entity branch — `nameplate_setup_system`'s spawn-time gate is untouched, since toggling only needs to flip `Visibility`, not insert/remove `NameplateTag`. An explicit per-prefab `nameplate: Some(true)`/`Some(false)` on the player prefab always wins over this preference, same precedence as `show_player_nameplate`. Re-seeded from `show_player_nameplate` on every scene load in `scene_loader.rs` (does NOT persist across scene transitions — a deliberate simplicity choice, matching `player_enabled`'s own per-scene-authored behavior rather than `AudioState`'s session-persistent pattern).
- **`interactable` / `trigger_zone` / `colliders` / `stat_templates` / `behavior`** — all inserted inside `spawn_prefab_instance` for both scene and dynamic paths.
