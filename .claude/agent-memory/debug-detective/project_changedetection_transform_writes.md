---
name: per-frame-changedetection-transform-writes
description: world_label_screen_pos_system's per-frame writes are now all epsilon-guarded (translation 0.5px, font_size 0.5, anchor scale 0.005) — NameplateCameraDistance is the one deliberate exception
metadata:
  type: project
---

`world_label_screen_pos_system` (`crates/ironhold_core/src/lib.rs`) used to write `t.translation.x/.y`
**unconditionally** every frame, dirtying every nameplate anchor's whole `Text2d`/`Mesh2d` subtree and
causing idle stutter. **That is fixed** — verified 2026-08-15. Every render-affecting write in that loop
is now epsilon-guarded:

- `t.translation.x/.y` — write only when `|delta| >= 0.5` px
- `TextFont.font_size` — write only when `|delta| >= 0.5`
- `t.scale` (anchor-style labels, added by `feature/nameplate-zoom-spacing`) — write only when `|delta| >= 0.005`
- `Visibility` — compared before assignment

The one deliberate exception is `NameplateCameraDistance`, rewritten unconditionally every on-screen
frame (documented in its own doc comment; no system filters on `Changed<NameplateCameraDistance>` today).

**Why:** CLAUDE.md's "change-detection discipline" — a `DerefMut` on a `Mut<T>` marks it Changed
regardless of whether the value moved, and a dirty parent `Transform` re-propagates + re-lays-out the
entire child subtree.

**How to apply:** When reviewing any new per-frame write in this system (or a sibling like
`damage_popup_system`), the guard is the house convention — a new unguarded write is a regression, not a
new idea. Watch for the `.as_mut()`-before-compare mistake that silently defeats the guard
([[stat-display-changedetection-asmut]]). Related: [[webgpu-preprocessing-warning]],
[[label-depth-scale-three-mechanisms]].
