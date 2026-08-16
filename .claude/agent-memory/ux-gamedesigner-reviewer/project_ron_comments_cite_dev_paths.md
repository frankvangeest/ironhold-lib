---
name: ron-comments-cite-dev-paths
description: Recurring authoring-clarity defect — RON comments in assets/ point designers at planning/*.md and Rust doc comments, which are outside the designer's artifact boundary
metadata:
  type: project
---

Comments inside `assets/projects/**/*.ron` repeatedly send the reader to resources a designer does
not have. Observed concentration in `3rd_person_game_demo` (prefabs.ron and scenes/main.scene.ron):
"see planning/backlog.md '<item title>'", "see planning/claude_suggestions.md", "see
`StatLabelDef.screen_offset`'s doc comment", "see scene_loader.rs's Button spawn arm".

**Why:** designers receive only `assets/`, `docs/`, `README.md`, and a prebuilt WASM build. They
have no `planning/` folder and no Rust source, so these pointers dead-end. They also rot faster
than `docs/` links (backlog items get renamed/archived on completion).

**How to apply:** when reviewing any RON diff, flag comment references to `planning/`, `crates/`,
or Rust type/field doc comments, and recommend rewriting them as a `docs/20_data_formats.md`
section reference (the doc has stable anchor headings, e.g. "Label depth scaling
(`LabelDepthScaleDef`)"). A one-line "why" in the RON plus a docs pointer is the right shape;
in-file cross-references to another prefab in the same file ("see the player world_stat_bar's
comment above") are fine and should not be flagged.
