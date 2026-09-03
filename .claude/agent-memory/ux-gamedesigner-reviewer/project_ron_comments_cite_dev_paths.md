---
name: ron-comments-cite-dev-paths
description: Recurring authoring-clarity defect — RON comments in assets/ point designers at planning/*.md and Rust doc comments, which are outside the designer's artifact boundary
metadata:
  type: project
---

Comments inside `assets/projects/**/*.ron` repeatedly send the reader to resources a designer does
not have. The `3rd_person_game_demo` citations originally logged here (planning/backlog.md,
planning/claude_suggestions.md, `StatLabelDef.screen_offset`'s doc comment, scene_loader.rs's
Button spawn arm) no longer grep-match in that project — but the pattern is still very much alive
elsewhere, e.g. (current, verified): `camera_modes/prefabs/prefabs.ron` ("planning/features/
done/flycam_scene_conflicts.md", "this whole project is a living planning/features/camera_modes.md
demo"), `entity_logic_demo/prefabs/prefabs.ron` ("entity_spawner.rs" internal function names,
"planning/features/monotonic_entity_id.md"), `local_coop_demo/prefabs/prefabs.ron` (several
`planning/features/*.md` citations plus "see the `warn!`s at scene_loader.rs and
action_executor.rs"), `local_coop_demo/scenes/main.scene.ron` ("planning/backlog.md's still-queued
..."), `local_coop_demo/scenes/room10.scene.ron`/`room11.scene.ron`, and
`primitive_world/scenes/main.scene.ron` / `stats_demo/scenes/main.scene.ron` (both citing
`entity_spawner.rs`'s internal default functions). Re-check with a fresh grep before citing any
specific line — these rot fast as files get edited.

**Why:** designers receive only `assets/`, `docs/`, `README.md`, and a prebuilt WASM build. They
have no `planning/` folder and no Rust source, so these pointers dead-end. They also rot faster
than `docs/` links (backlog items get renamed/archived on completion).

**How to apply:** when reviewing any RON diff, flag comment references to `planning/`, `crates/`,
or Rust type/field doc comments, and recommend rewriting them as a `docs/20_data_formats.md`
section reference (the doc has stable anchor headings, e.g. "Label depth scaling
(`LabelDepthScaleDef`)"). A one-line "why" in the RON plus a docs pointer is the right shape;
in-file cross-references to another prefab in the same file ("see the player world_stat_bar's
comment above") are fine and should not be flagged.
