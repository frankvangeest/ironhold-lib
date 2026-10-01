# Investigation: non-deterministic `HashMap`/`HashSet` iteration order in gameplay systems

_Audit run: 2026-10-01, `crates/ironhold_core/src` at `afc76de` (debug-detective, read-only; no cargo run)._
_Backlog item: Beta 0.5 ▸ "Audit gameplay systems for non-deterministic `HashMap`/`HashSet` iteration order"._
_Status: **audit complete; fixes not started — tracked as backlog items D1-D5 plus two Bugs (Beta 0.5 section / `## Bugs`).** Findings below feed the fix plan and the cross-platform determinism harness's expected-divergence list._

## Why this matters

Lockstep multiplayer and replay need every machine to compute identical results. Rust's `std`
`HashMap`/`HashSet` re-seed their hasher per map instance, so iteration order differs between runs,
between instances within one process, and between native and WASM. The physics-timestep determinism
work (`planning/features/deterministic_fixed_timestep.md`) did not look at this. Swapping to a
fixed-hasher map is **not** a fix by itself: order would still depend on insertion history and
capacity, with no documented cross-platform guarantee for native vs. wasm32.

## Summary

- ~70 map/set declaration sites, all `std::collections` (≈47 named container types + ≈23
  function-local temporaries). No `hashbrown`/`FxHashMap`/`EntityHashMap` of the crate's own.
- **Already deterministic:** `StatMap` (`IndexMap`, `stats.rs:299`); `SpawnRegistry.entities`,
  `LoadedSpawnPoints`, `LoadedCameraModes` (`BTreeMap`, `scene_manager/mod.rs:334/342/362`);
  `ActionQueue`, `DelayedEventQueue`, `PendingEntitySpawns` (`Vec`/`VecDeque`).
- **Keyed-lookup only (no ordering risk):** `GameVariables`, `HandledIntentSlots`, `NpcHitQueue`,
  `NpcDeadQueue`, `NpcReviveQueue`, `BuiltMaterials`, `LoadedModifiers`, both `icon_atlases`,
  `AnimationController.node_indices` (except one log line), `named_colors`, `stat_overrides`
  (except one warn loop), `CustomMaterialDef.textures`, runtime `AssetCatalog` lookups, and assorted
  local `seen`/`claimed`/`visited` sets.
- **Iterated: 22 site groups** — 4 order-dependent affecting gameplay, 9 order-dependent cosmetic
  only, 9 order-independent, 1 unsure.

## Iterated sites

| file:line | container (key) | loop purpose | class | reason | proposed fix |
|---|---|---|---|---|---|
| `capabilities/action_bar.rs:342` | `PendingIntentActions` (slot key) | `drain()` → `action_queue.push`, cooldown insert, `action_bar.activated` event | **GAMEPLAY** | push order into `ActionQueue` and event order follow hash order of slot keys | `IndexMap` (insertion = slot query order) or `BTreeMap` |
| `capabilities/stats.rs:114` | `LoadedStats` (String) | `iter_mut` → `fire_threshold_crossings` writes `GameEvent`s | **GAMEPLAY** | threshold events reach the FSM in hash order; transitions are first-match and sequential (`fsm_interpreter.rs:91-121`) | `IndexMap`/`BTreeMap` (catalog order) |
| `capabilities/stats.rs:15` | `LoadedStats` | `iter_mut` → `tick_modifiers` writes `stat.modifier.expired:*` | **GAMEPLAY** | same mechanism | same |
| `runtime/input.rs:45` | `LoadedKeyBindings` (key name) | writes `UiEvent::ButtonPressed` per just-pressed bound key | **GAMEPLAY** | two keys pressed in one frame → two event orders → two FSM outcomes | `BTreeMap` (or pre-resolved sorted `Vec<(KeyCode, String)>`) |
| `runtime/input.rs:145` | `LoadedGamepadBindings` (button name) | `find_map`, first match wins | **GAMEPLAY (low)** | one pad pressing two bound buttons in one frame; the comment at `input.rs:85` admits this | `BTreeMap` |
| `capabilities/action_bar.rs:110` | `CooldownMap` | `retain`, per-entry decrement | independent | per-entry, pure | none (convert for the lint) |
| `capabilities/stats.rs:66`, `:85` | `LoadedStats` | effective-value recompute, regen | independent | per-entry only | none |
| `runtime/input.rs:211`, `:226`, `:229` | `stable_secs`/`stuck_secs`/`warned` (Entity) | `retain` with pure predicate | independent | pure predicate | none |
| `scene_manager/project_loader.rs:226` | `model_fixes` | `extend` into another map | independent | builds an unordered collection | none |
| `scene_manager/project_loader.rs:298` | `StatCatalog.stats` | collect into `LoadedStats` | independent | builds an unordered collection | becomes the `LoadedStats` fix |
| `scene_manager/scene_loader.rs:134`, `:150` | scene key/gamepad bindings | overlay insert | independent (`load_errors` order cosmetic) | insert per key | none |
| `capabilities/animation.rs:125`, `:137` | `clips` → `clip_names`; Bevy `named_animations` | build set / merged map | independent (warn order cosmetic) | builds unordered collections | none |
| `runtime/material_factory.rs:255`, `:265` | `colors`/`floats` | uniform packing | independent | keys already `sort()`ed | none |
| `capabilities/particle_renderer.rs:401` | `groups` | per-group mesh rebuild | independent | per-group only | none |
| `capabilities/particle_renderer.rs:327` | `buckets` (GroupKey) | spawns pool group entities/materials | cosmetic | only entity/handle-to-group mapping changes | `BTreeMap` (derive `Ord` on `GroupKey`) |
| `scene_manager/scene_loader.rs:175` | `AssetCatalog.materials` | `MaterialFactory::build`, handle allocation order | cosmetic | asset handles only | `BTreeMap` catalog |
| `scene_manager/scene_loader.rs:2345` | `ItemCatalog.items` | icon-sheet load/layout order | cosmetic | load order only | `BTreeMap` catalog |
| `scene_manager/scene_loader.rs:3482`, `:3502` | `audio`/`decals` | preload issue order | cosmetic | I/O order only | `BTreeMap` catalog |
| `schema/catalog.rs:166-176,636,702`; `schema/items.rs:22`; `schema/stats.rs:72,91` | catalogs | `validate()` returns the first error found | cosmetic | which error is reported varies, not whether one is | `BTreeMap` catalogs (also stabilises CLI output) |
| `project_loader.rs:113,126,250,262`; `entity_spawner.rs:129`; `animation.rs:400` | bindings / `stat_overrides` / `node_indices` | `warn!`/log loops | cosmetic | logging only | none |
| `capabilities/animation.rs:150` | `clip_names` HashSet | `graph.add_clip` → `AnimationNodeIndex` assignment | **unsure** (probably cosmetic) | name→index mapping stays consistent; open question is whether graph node order changes blend accumulation in cross-fades | `BTreeSet` (free) |

## Gameplay-affecting findings, ranked

1. **`flush_pending_intent_system` drains `PendingIntentActions` in hash order** (`action_bar.rs:342`).
   Two slots firing in one frame is a designed-for case (per-player bars in co-op, click+key,
   chords). *Scenario:* health 80/100; slot "1" heals `+50`, slot "2" sacrifices `-60`;
   `apply_delta` clamps per step (`stats.rs:189-195`) — heal first: 80→100→40; sacrifice first:
   80→20→70. Two slots that both `Spawn` with no `id` also swap their counter-derived ids, and
   `SPAWNS_PER_FRAME = 2` FIFO decides which lands this frame.
2. **Global-stat threshold and modifier-expiry events are emitted in `LoadedStats` hash order**
   (`stats.rs:114`, `:15`). The FSM advances `LogicState` immediately, first match wins.
   *Scenario:* one frame pushes `health` past `BelowOrEqual(0)` (`player.died`, `playing→dead`) and
   `mana` past its threshold (`mana.empty`, `playing→oom`) — `player.died` first → `dead`;
   `mana.empty` first → `oom` and the `playing→dead` transition no longer matches (the player
   survives on that machine).
3. **`global_input_system` writes UI events in `LoadedKeyBindings` hash order** (`input.rs:45`).
   Matters only if lockstep/replay injects recorded key state through `ButtonInput`.
   *Scenario:* Escape→`pause` and I→`toggle_inventory` in one tick — pause first leaves the
   state-scoped inventory binding unmatched; inventory first opens it, then pauses.
4. **`unclaimed_gamepad_trigger_system`'s `find_map`** (`input.rs:145`) — only when one unclaimed
   pad presses two bound buttons in one frame. Low likelihood.

General note: `RandomState` re-seeds per process **and per map instance**, so none of these replay
identically even on one machine.

## Bevy query/schedule-order findings (not HashMaps, same effect)

- **A. `Update` event writers are ordered only against the interpreter, not against each other**
  (`lib.rs:233-251`, `:325-334`): `button_system`, `icon_button_click_system`, `global_input_system`,
  `unclaimed_gamepad_trigger_system`, the stat chain, `interactable_system`, `tick_delayed_events_system`
  and the action-bar/targeting chain are each only `.before(fsm_interpreter_system)`. They all take
  `ResMut<Messages<…>>` so they serialise, but their relative order is left to the multi-threaded
  executor — frame event order, hence FSM transitions (same mechanism as finding 2), can differ run to
  run on native, and native vs. web (single-threaded) can disagree. **Rated the most likely real
  divergence source overall; needs runtime confirmation** via
  `ScheduleBuildSettings { ambiguity_detection: LogLevel::Warn }` on `Update` in a test.
- **B. Behavior activation timing depends on asset loading** (`entity_spawner.rs:564-607`):
  `resolve_pending_behaviors_system` activates a `.behavior.ron` and fires its initial
  `entry_actions` on whichever frame the asset finishes loading (varies per machine); events arriving
  earlier are missed on one machine but not another, and the `BehaviorHandle` archetype row order
  (which drives item C) depends on load timing. Dialogue assets (`dialogue.rs:138`) and GLTF
  readiness have the same shape.
- **C. `entity_fsm_interpreter_system` pushes per-entity actions in query order**
  (`entity_fsm_interpreter.rs:44`) — several entities reacting to one event push `Spawn` (counter
  ids), `ModifyStat` on a shared stat (clamp, as finding 1), or transitions in query order.
- **D. `npc_behavior_system` (`npc.rs:236`) and `stat_threshold_system`'s `StatMap` loop
  (`stats.rs:117`) emit events in NPC/entity query order** (e.g. two NPCs dying in one tick).
- **E. `npc_hit_relay_system` uses `player_query.iter().next()` as "the attacker"** (`npc.rs:177`) —
  in co-op the NPC investigates whichever player is first in query order, not the player who hit it.
  Order-dependent **and simply wrong** (a plain bug independent of determinism).
- **F. `interactable_system` fires for every in-range interactable in query order**
  (`interactable.rs:68-91`) — two containers in range produce two `OpenContainer`s; the last wins
  `active_container`.
- **G. Tab-targeting ties fall back to query order** (stable sort on distance only,
  `targeting.rs:321`; click-select keeps the first result on an exact pixel tie, `:242`). Consistent
  with the known `local_coop_tests` equidistant `enemy_a`/`enemy_b` flake (not root-caused; per-thread
  wall-clock `Time` in tests is a second candidate). Fix: tie-break on `SpawnId`.
- **H. `Action::Despawn` and `SetDespawnTimer` resolve their target by scanning the `SpawnId` query
  with `find`** (`action_executor.rs:294-298`, `:327-331`) — spawn ids share a flat namespace and can
  collide, then query order picks which entity dies. Fix: resolve through `SpawnRegistry` (a
  `BTreeMap` already holding the authoritative mapping).
- **I. `action_bar_input_system` walks slots in query order** (`action_bar.rs:181`) — sets the order
  of `intent.slot.*`/`action_bar.pressed` events, and makes the last slot win `pending.insert` when
  two bars share a key (already `validate`-flagged).
- **Entity-index tie-breaks** (`sort_by_key(e.index())` for gamepads at `input.rs:104`/`:201`;
  `camera_priority_key`) are local-hardware/camera concerns, acceptable — but **never use `Entity`
  ordering as a tie-break inside simulation logic**: indices are not stable across machines once any
  spawn is asset-load-driven (GLTF children).

## Corrections after architect triage (2026-10-01)

- **Finding A is narrower than first stated.** Both interpreters always read all `UiEvent`s first,
  then `GameEvent`s, then `SceneEvent`s (`fsm_interpreter.rs:35-52`, `entity_fsm_interpreter.rs:24-40`),
  so the order between a UI writer and a Game writer can never change an FSM outcome. Only writers
  of the **same** message type that race each other matter (UI: `button_system`,
  `icon_button_click_system`, `global_input_system`, `unclaimed_gamepad_trigger_system`; Game: the
  stat chain, `interactable_system`, `tick_delayed_events_system`, the action-bar/targeting chain).
  It also duplicates two already-queued items, now merged into backlog item **D3**.
- **Finding G's "known `local_coop_tests` flake" has no evidence** — no backlog or
  `claude_suggestions.md` entry mentions it, and the only equidistant setup
  (`local_coop_tests.rs:3886-3887`) asserts `is_some()`, so a tie cannot flip it. Disregard that
  sentence. The tie-break in `targeting.rs:321` (`partial_cmp`, ties fall back to query order) is still real.
- Everything else in the audit was re-verified against the code and holds.

## Recommended systemic prevention

1. **Deterministic collection aliases.** Add `crate::collections` with
   `pub type DetMap<K, V> = indexmap::IndexMap<K, V>;` and `DetSet` (`indexmap` with serde is already a
   dependency, `Cargo.toml:19`); iteration follows insertion order regardless of hasher. For schema
   catalogs prefer `BTreeMap`/`BTreeSet` (native serde support; also stabilises `validate()` and CLI
   output). Caveat: `IndexMap::remove` is `swap_remove` and reorders — use `shift_remove`, or prefer
   `BTreeMap` for maps that see removals (`CooldownMap`, `PendingIntentActions`).
2. **Lint.** Workspace `clippy.toml` with `disallowed-types` for `std::collections::HashMap`/`HashSet`
   and `bevy::platform::collections::HashMap`/`HashSet`, pointing at the alias; allow only on
   genuinely lookup-only hot-path maps via `#[allow(clippy::disallowed_types)]` plus a comment.
3. **Touches:** ≈30 type sites across ≈18 files (`schema/{catalog,items,stats,project,scene_v2,material,player}.rs`,
   `lib.rs`, `capabilities/{action_bar,animation,inventory,npc,particle_renderer,player,stat_display,nameplate,target_indicator}.rs`,
   `runtime/{input,material_factory,model_spawner}.rs`, `scene_manager/{mod,scene_loader,entity_spawner}.rs`);
   `GroupKey` needs `Ord` if `BTreeMap` is chosen; `ironhold_cli` schema consumers recompile (the
   `cargo check -p ironhold_cli` gate). The 4 gameplay sites are the only urgent ones.
4. **What the lint cannot catch (query/ambiguity order):**
   - (a) ambiguity detection at `Warn` on `Update`/`FixedUpdate` in one integration test, failing on
     any ambiguity between message writers (overlaps the Queued "Schedule-graph assertions + first
     named `SystemSet`" item);
   - (b) a convention: any loop over a query that emits events, pushes actions or spawns must sort by
     `SpawnId` first;
   - (c) a cheap in-process determinism regression test: run one scripted input sequence in two
     `App`s and assert identical `ActionQueue`/`GameEvent` sequences — different `RandomState` keys
     per map instance make this catch every HashMap site above without cross-process plumbing.

## Open / undetermined

- Whether the multi-threaded executor actually varies the order of the ambiguous writers in
  finding A (needs the ambiguity report or a stress test).
- Whether animation-graph node order (`animation.rs:150`) changes blended pose bits, and whether any
  future gameplay reads bone transforms (none found).
- Whether lockstep will inject input through `ButtonInput` (decides if findings 3-4 matter).
- Root cause of the `local_coop_tests` flake: query order vs. wall-clock `Time` across parallel test
  threads.
- Larger, non-HashMap determinism gap (already its own item): `Update`-scheduled gameplay timers use
  variable `Time` delta — see Beta 0.5 ▸ "Move the gameplay-logic pipeline and its timers onto the
  fixed tick".

## Suggested follow-ups (not yet logged as backlog items)

- Fix the 4 gameplay-affecting map sites (small, mechanical; `PendingIntentActions`, `LoadedStats`,
  `LoadedKeyBindings`, `LoadedGamepadBindings`) with a regression test per site.
- Add the `crate::collections` aliases + `clippy.toml` lint and convert the remaining sites.
- Ambiguity-detection test + the "sort by `SpawnId`" convention (finding A, C, D, F, G, I).
- Plain bugs surfaced: **E** (`npc_hit_relay_system` picks the wrong player in co-op) and **H**
  (`Despawn`/`SetDespawnTimer` resolve by `SpawnId` query scan instead of `SpawnRegistry`), **G**
  (tie-break targeting on `SpawnId`).
