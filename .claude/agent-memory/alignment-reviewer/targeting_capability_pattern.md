---
name: Targeting capability + {target} substitution pattern
description: How the targeting capability (click/Tab selection) wires into the data-driven pipeline; the SetTarget/capability GameVariable asymmetry footgun; consult when reviewing targeting or CurrentTarget changes
type: project
---

The targeting capability (`capabilities/targeting.rs`) selects entities by **screen-space projection** (`camera.world_to_viewport`), NOT mesh raycasting — deliberate, because Bevy mesh picking raycasts bind-pose geometry and misses animated/skinned GLB characters. `SELECT_PIXEL_RADIUS` (70px) is still a hardcoded engine constant (acceptable UX tuning). `SELECT_AIM_HEIGHT` (1.0m) is now the *fallback only* — it is per-prefab overridable via `PrefabDef.select_aim_height: f32` (`#[serde(default = "default_select_aim_height")]` = 1.0, schema/catalog.rs). At spawn, `tag_spawned_entity` inserts a `SelectAimHeight(f32)` component alongside `ClickSelectable` (only when click_selectable); `click_select_system` + `debug_selectables_system` read `Option<&SelectAimHeight>` with `.map_or(SELECT_AIM_HEIGHT, |h| h.0)`. This is the canonical example of correctly promoting a hardcoded UX constant to a per-prefab data field: component (not resource) for per-entity state, threaded through the single `tag_spawned_entity` helper (so all 7 spawn paths + dynamic Action::Spawn get it for free), backward-compatible default, debug gizmo reads the same value as the click logic. Used by 3rd_person_game_demo enemy_snake (0.4) / enemy_spider (0.6).

**Designer entry points (all RON-reachable):**
- `PrefabDef.click_selectable: bool` and `PrefabDef.targetable: bool` (both `#[serde(default)]`, backward-compatible).
- `InputMap.target_next: String` (default "Tab") and `InputMap.target_range: f32` (default 30.0) in the player prefab `inputs` block. Note: browsers intercept Tab for focus, so web projects should override to e.g. `"KeyT"`.
- `Action::SetTarget(String)` / `Action::ClearTarget` (tuple/unit variants → positional RON).
- `{target}` substitution in `rewrite_target()` (message_interpreter.rs) — fills the current `CurrentTarget` spawn ID into action fields. Runs in all 3 interpreter systems (rules, FSM, entity-FSM). Supported fields mirror `{self}`: key, entity, event, id, spawn_point, and SetVariable's value.

**Marker insertion — three spawn paths (same footgun as TriggerZone/PrefabKey):**
`click_selectable`/`targetable` markers + `PrefabKey`(catalog key, distinct from `SpawnId` instance id) must be inserted in ALL THREE scene_loader.rs branches: composite primitive, single-mesh primitive, and GLB actor/prop. The GLB actor branch historically omitted `SpawnId`+SpawnRegistry registration entirely — verify it inserts `SpawnId`, `PrefabKey`, AND `spawn_registry.entities.insert(...)`, or targeting + all id-targeted actions silently miss GLB scene entities.

**ASYMMETRY FOOTGUN (flag if reintroduced):** The capability writes three GameVariables directly (`target_display`, `target_name`, `target_id`) via `write_target_vars`/`clear_target_vars` so UI labels update without rule wiring (same self-managed-state pattern as action_bar). BUT `Action::SetTarget`/`ClearTarget` in action_executor.rs only set `CurrentTarget` + emit events — they do NOT write those GameVariables. So a rule-driven `SetTarget("orc_01")` updates the target but leaves a `bind: "target_display"` label stale, while click/Tab selection updates it. Either route SetTarget/ClearTarget through the same var-writing helper, or document that designers must pair `SetTarget` with `SetVariable("target_display", ...)`. This is a designer-reachability gap: the same logical outcome (set target) produces different UI depending on whether it came from input or from a rule.

**GLB player movement footgun:** the GLB player path (`entity_spawner.rs::spawn_player_entity`) must insert `SpeedMultiplier(1.0)` — `player_movement_system`'s query requires it, and without it the GLB player is silently filtered out and never moves. The primitive player path inserts it in scene_loader.rs. Both paths must stay in sync.

**PER-PLAYER TARGETING (Phase 1, reviewed 2026-07-13, ALIGNED):** `CurrentTarget` resource was
KEPT (not deleted) and redefined as "the primary player's PlayerTarget, mirrored". New
`PlayerTarget(Option<String>)` component (capabilities/player.rs) on each player entity, inserted
at BOTH spawn sites (entity_spawner.rs::spawn_player_entity_core for GLB; scene_loader.rs inline
for primitive/capsule). "Primary" = `PlayerIndex(0)` OR no `PlayerIndex` at all (primitive path
never gets one) — see `is_primary_player()` helper. `{target}` substitution + action_bar cost-gate
read `CurrentTarget` UNCHANGED → single-player regression genuinely preserved (verified by tests
test_legacy_target_vars_populate_when_single_player + test_only_primary_player_target_mirrors).
`apply_player_target`/`clear_player_target` helpers gate CurrentTarget-mirror + target.changed/
target.cleared emission on `is_primary`; blank the legacy target_display/name/id vars whenever
`is_multiplayer` (raw player-count >= 2, NOT gated on real split-screen). New designer surface:
`GameSceneV2.target_hud: Option<TargetHudDef{show:TargetHudDisplay(Full/NameOnly/IdOnly),font_size,
color}>` → LoadedTargetHud resource (mod.rs, init lib.rs, cleared LoadScene action_executor.rs,
populated scene_loader.rs). Two new camera.rs systems (target_hud_spawn/_update) mirror the
split_viewport_player_label precedent exactly (per SplitViewportSlot, Added<> spawn, chained
.after split_screen_viewport). NO ActionQueue anywhere; rings/HUD are pure cosmetic views.
target_indicator.rs rewritten: TrackingTarget now carries owner:Entity; rings tinted by
PLAYER_LABEL_COLORS when 2+ players (Frank's explicit design choice, palette not RON-tunable = OK
per corner-label precedent). WARNINGS (both documented, non-blocking): (1) `target.clicked:{id}`
still emitted unconditionally for non-primary players (only target.changed*/target.cleared are
primary-gated) — a `{target}`-using rule matched on target.clicked resolves {target} to the PRIMARY
player, wrong entity; documented in src/CLAUDE.md. (2) legacy vars blank on ANY 2+ player scene
incl. party (non-split) mode, but target_hud only spawns on SplitViewportSlot cams → a party-mode
2p scene loses the legacy target Label AND gets no HUD replacement.

**Target indicator (ground ring) — CONFIRMED ALIGNED, NOT pipeline-routed (reviewed 2026-06-17):** `capabilities/target_indicator.rs::target_indicator_system` reads `CurrentTarget` + `LoadedTargetIndicator` and directly spawns/moves/despawns a flat unlit quad. It correctly does NOT push to `ActionQueue` and emits NO events — it is a pure visual *view* of `CurrentTarget` resource state, same self-managed-state exception as targeting's GameVariable writes and StatRadar/action_bar. The game-logic decision (what selecting means) already lives in `SetTarget`/`ClearTarget` + `target.changed`/`target.cleared`; the ring is downstream of it. Use this as the canonical example when asked "is it OK for a capability to manage entities directly?": YES iff the output is a derived view a designer would never want to attach rewards/conditions to. Designer path: `GameSceneV2.target_indicator: Option<TargetIndicatorDef>` (schema/scene_v2.rs) + `decals:` catalog key (reuses existing `AssetCatalog.decals`, also used by `Action::ProjectDecal`). Resolved in scene_loader.rs (key→path via catalog), stored in `LoadedTargetIndicator`, cleared to `None` on `LoadScene` in action_executor.rs. Known perf wart (not a blocker): mints a fresh `Plane3d` mesh + `StandardMaterial` + re-loads the texture on every target change — WASM pipeline-compile stall + asset leak on rapid switching; cache-once is the fix.
