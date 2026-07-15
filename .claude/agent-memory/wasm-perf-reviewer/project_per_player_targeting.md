---
name: project-per-player-targeting
description: Phase 1 per-player targeting — PlayerTarget component replaces global CurrentTarget; 4 targeting systems + 2 new camera HUD systems, all capped at MAX_SPLIT_PLAYERS=4
metadata:
  type: project
---

Phase 1 of per_player_split_screen_targeting.md: targeting moved from a single global `CurrentTarget` resource to a per-player `PlayerTarget` component (on every `CharacterController`/`Player` entity). `CurrentTarget` still exists but is mirrored only for the primary player (PlayerIndex 0 or None). Player count is capped at MAX_SPLIT_PLAYERS=4 engine-wide.

Per-frame hot-path characteristics (all in `capabilities/`):

- **targeting.rs `tab_targeting_system`** (Update): now loops ALL `CharacterController` entities (was `.iter().next()`, O(1) → O(players≤4)). `controllers.iter().count()` runs unconditionally each frame (≤4, trivial). The expensive `candidates` Vec collect+sort over `Targetable` entities only runs for a player whose own `target_next` key is `just_pressed` — not per-frame. Fine.
- **targeting.rs `click_select_system`** (Update): the extra work (`player_targets.iter().count()` scan + `.find()` primary fallback + camera OrbitCamera field) is gated behind an actual mouse click — near-zero on idle frames. Fine.
- **targeting.rs `target_auto_clear_system`** (Update): loops all controllers each frame (≤4), one HashMap `registry.entities.get` + visibility lookup per player only when that player has a target. Cheap.
- **target_indicator.rs `target_indicator_system`** (Update): unconditional per-frame "move rings" loop over `Query<(Entity,&TrackingTarget)>` (pre-existing). New `all_players.iter().count()` runs every frame the system is active BUT only after early-returns for `indicator_cfg`/`cached_mesh` — so free on scenes with no `target_indicator:` block. Ring spawn/despawn gated on `Changed<PlayerTarget>` (per-transition, not per-frame). Multiplayer rings tinted by PLAYER_LABEL_COLORS. Ring material = pre-existing StandardMaterial (AlphaMode::Blend, unlit, double_sided, cull_mode None, depth_bias 64.0) — NO new pipeline variant.
- **camera.rs `target_hud_spawn_system`** (Update): `Added<SplitViewportSlot>`-filtered, ≤4 fires per scene load. Early-returns if no `target_hud:` cfg. Per-spawn.
- **camera.rs `target_hud_update_system`** (Update, chained): unconditional per active split camera per frame. Builds a `format!()` String each frame when the owning player has a target (NOT cached in a Local — the one real per-frame allocation, ≤4 small strings/frame, only when a target is selected; zero alloc on idle/no-target because it early-returns hidden first). Text/Node/Visibility writes ALL guarded (`text.0 != new_text`, `node.left != new_left`, etc.) so no taffy relayout churn. Comparable to the stat_display.rs format!-before-guard pattern already logged. Non-blocking.

WASM verdict: zero new deps; ring texture `shared/textures/decals/ring_thick.png` is 4 KB; new schema is a plain RON struct+enum (`TargetHudDef`/`TargetHudDisplay`, no GPU uniform, no alignment concern). Negligible binary-size impact. `LoadedTargetHud` resource is a direct clone of the scene field on load (no catalog/texture lookup) — trivial startup cost.

Related: [[project_target_indicator_system]], [[project_split_screen_hud_labels]] (target_hud_update_system copies its now-correct guarded viewport-tracking idiom), [[project_local_coop_input_camera]], [[project_stat_widget_split_duplication]] (same format!-per-frame class).
