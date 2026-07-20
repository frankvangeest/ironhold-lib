---
name: levelentity-recursive-sweep-double-despawn
description: scene_loader level sweep despawns LevelEntity parents AND their LevelEntity children with recursive despawn(), double-despawning children whenever a parent is iterated first — source of "generation N" warnings on portal/scene transitions
metadata:
  type: project
---

`spawn_scene_v2` (crates/ironhold_core/src/runtime/scene_manager/scene_loader.rs, ~line 136) tears down a scene with:
```rust
for entity in level_entities.iter() { commands.entity(entity).despawn(); }
```
where `level_entities: Query<Entity, With<LevelEntity>>` has NO `Without<ChildOf>` filter. Bevy 0.18 `despawn()` is recursive (despawns descendants). Several widget builders spawn a `LevelEntity` **parent whose children are ALSO `LevelEntity`, attached via `add_child`**:
- `nameplate_setup_system` (nameplate.rs ~120-208): anchor + shadow/name_text/bg/fill children.
- `spawn_world_stat_bar_widget` Pixel/Icon/Textured styles (stat_display.rs ~581-637+): anchor + border/bg/fill (or per-cell) children. (Ascii style + `stat_label` are top-level, no children — SAFE.) Split-screen duplicates each set `MAX_SPLIT_PLAYERS` times (per `WorldLabelRank`).

The query returns parent AND children. When a parent is iterated before its children (archetype-iteration-order dependent → **intermittent**), despawning it recursively kills the children, then the loop reaches each now-dead child → `commands.entity(child).despawn()` on a stale entity → `WARN ... Entity ... is invalid; its index now has generation N`. A Pixel-bar child index recycled across ~8 scene loads shows as e.g. generation 9.

**Why:** classic Bevy 0.16+ hazard — recursive `despawn()` over a query snapshot that contains entities in parent/child relationships with each other. **Benign in effect** (idempotent deferred despawn: warn only, no panic/corruption — see [[project_idempotent_despawn_untestable_by_state]]), but noisy and a real structural bug.

**NOT the hot-join feature.** Confirmed 2026-07-20: reproduced by navigating local_coop_demo's portal chain (main→room2..room8 = ~8 `Action::LoadScene` sweeps), whose player prefabs use `world_stat_bar: (style: Pixel(...))`. The hot-join diff (Action::JoinPlayer, spawn_split_camera_for_player, drain_spawn_queue_system) never touches scene_loader/stat_display/nameplate; hot-join cameras are top-level (no children) so add no parent/child hazard.

**How to apply / recommended fix:** change the level sweep (and the overlay sweeps: scene_loader ~105, action_executor UnloadOverlay ~79 / ToggleOverlay ~95) to `try_despawn()` — the low-ceremony silent-no-op variant the core CLAUDE.md (§"Despawning: prefer try_despawn()") already prescribes for exactly this shape.

**OverlayEntity does NOT share the recursive hazard (verified 2026-07-20).** Unlike `LevelEntity`, overlay UI *descendants are never separately tagged* — `OverlayEntity` is inserted on exactly two SIBLING roots ("Overlay Backdrop" scene_loader ~1067, and "UI Root" ~1086/~1153); their `with_children` subtrees (Panel, ui elements, via `spawn_ui_element_node`) carry no `OverlayEntity`. So the overlay `for entity in overlay_entities.iter()` sweep iterates two independent roots → recursive despawn of each hits no other swept entity → **no recursive double-despawn possible**. The `try_despawn()` on the *scene_loader* overlay sweep is therefore purely defensive (harmless, but its "OverlayEntity-tagged sibling descendants" rationale comment is factually wrong). The *action_executor* UnloadOverlay/ToggleOverlay `try_despawn()` IS justified, but by a different mechanism — two overlay-dismissing actions in one executor run share the same deferred query snapshot (same as `Action::Despawn`/`StopMusic`), not by recursion. When fixing/reviewing overlay despawns, cite the same-run-double-dispatch reason, not recursion. NOTE: a `HashSet` dedup does NOT fix this (the child is a distinct entity killed via recursion, not a duplicate query hit) — unlike [[project_target_indicator_double_despawn]]. A `Without<ChildOf>` query filter also works but is unsafe if any `LevelEntity` child has a non-`LevelEntity` parent (would leak); `try_despawn()` is robust regardless. Related: [[project_loadscene_teardown_atomicity]], system-architect/deferred_despawn_double_queue.md.
