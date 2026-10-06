---
name: ui-flex-group-walker-pattern
description: Group UI node + walk_ui_nodes rule; reviewing any new UiNodeDef variant or new scene.ui scan; CLI severity split (error vs --strict warning) for layout diagnostics
metadata:
  type: project
---

`UiNodeDef::Group(GroupDef)` (feature/ui_flex_group, reviewed 2026-10-06) made `scene.ui` a tree.

**Rule:** every consumer of `scene.ui` must use `walk_ui_nodes`/`walk_ui_nodes_pathed` (schema/scene_v2.rs), never `for el in &scene.ui`. Only legit flat iterations: the two top-level spawn loops in scene_loader.rs (spawning recurses itself) and `ui_count` in query.rs (top-level by design, R6). A new `UiNodeDef` variant with children must be added to `UiNodeDef::children()` — the single place the walkers learn nesting.

**Diagnostics shape:** one shared pure fn `ui_layout_diagnostics(scene)` returns path-named (`ui[2].children[0]`) diagnostics with severity. Engine `warn!`s all of them at load; CLI reports Error-severity in default `validate`, Warning-severity ONLY under `--strict` (so `collapsed_group`, `panel_nested_in_group`, `percent_under_auto`, `inert_*` are invisible to a plain `ironhold validate`). The --strict summary line still says "N unused definitions" even though layout warnings are now mixed in.

`GameSceneV2::validate()` is now wired into CLI (`invalid_scene`), fail-fast one error per scene.

**Why:** prior to this, nesting would have silently blinded radar_handles pre-pass and every ActionBar dup-key check.
**How to apply:** when reviewing a new UiNodeDef variant or a new scene-level UI check, grep `scene\.ui` / `&scene.ui` in both crates; any flat loop is a finding. Related: [[diagnostic-only-feature-pattern]], [[uimaterial-ui-node-pattern]].
