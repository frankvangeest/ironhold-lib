---
name: render-only-reactive-capabilities
description: Pattern for cosmetic capabilities that react to state (e.g. CurrentTarget) without touching ActionQueue; target_indicator is the canonical example
metadata:
  type: project
---

Render-only reactive capabilities react to runtime state and spawn cosmetic, non-addressable entities — they sit OUTSIDE the Message→Action pipeline.

Canonical example: `capabilities/target_indicator.rs` (`TargetIndicatorPlugin`), added ~2026-06. Ground-ring decal that tracks the player's selected target. **Updated by `per_player_split_screen_targeting.md`:** it now tracks each player's own `PlayerTarget` component (one independent ring per player, via `Changed<PlayerTarget>`), not the single shared `CurrentTarget` resource — `CurrentTarget` still exists as "the primary player's `PlayerTarget`, mirrored" (see [[targeting_currenttarget_mirror]]), but target_indicator reads the per-player source directly now. This is a widening of the pattern, not a violation of it — still no `ActionQueue` access, still cosmetic/non-addressable.

Established conventions for this class of capability:
- It does NOT push to `ActionQueue`. It only reads state (per-player `PlayerTarget`, `LoadedTargetIndicator`, `SpawnRegistry`, `TargetRingVisibilityMode`) and spawns/moves/despawns a visual per player. Correct — it's not a gameplay-logic source.
- Its entity is intentionally NOT routed through `tag_spawned_entity`. It gets a bare `LevelEntity` (for scene-transition cleanup) but no `SpawnId`/`PrefabKey` and is NOT registered in `SpawnRegistry`, because it's cosmetic and non-addressable. This is a justified deviation from the "all spawns go through tag_spawned_entity" rule — that rule is about *addressable* entities.
- Config is a resolved resource (`LoadedTargetIndicator(Option<...>)`) populated in `scene_loader.rs` from a scene-RON block, cleared to `None` in the `Action::LoadScene` arm of `action_executor.rs`. Mesh/material cached in `Local<>`, keyed on `resource.is_changed()` — valid across scene loads because `insert_resource` always marks changed.

**Why:** prevents a future contributor from "fixing" the missing SpawnId/tag_spawned_entity routing, or from adding ActionQueue access to make it "configurable".

**How to apply:** when reviewing a new cosmetic/visual capability (selection rings, highlights, hover glows, screen markers), expect this shape and don't flag the absent tag_spawned_entity / ActionQueue as violations. DO flag it if the capability starts driving gameplay (then it belongs in the pipeline).

Known v1 limitation logged for target_indicator: ring uses fixed world-Y (`offset_y` replaces target Y, not added) — detaches vertically on slopes/platforms.
