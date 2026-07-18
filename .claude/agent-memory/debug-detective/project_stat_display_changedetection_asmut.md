---
name: project-stat-display-changedetection-asmut
description: In stat_display.rs world-bar update systems, `Mut<Sprite>`/`Mut<T>` guards are defeated if you call `.as_mut()` before the value comparison — DerefMut fires and marks the component Changed every frame regardless
metadata:
  type: project
---

`stat_display.rs` has sibling per-frame "world bar update" systems that all claim "Writes are
guarded for change-detection efficiency" in their doc comments. The guard is only real if you
read the current value via an **immutable** deref first and `DerefMut` only when a change is
actually needed.

**Why:** `world_pixel_bar_update_system` does this correctly — it reads `transform.scale.x`
(immutable Deref, no change tick) inside its `if` guard and only assigns `transform.scale.x = ...`
(DerefMut → sets Changed) when the value differs. So an unchanged pixel bar never dirties its
Transform.

`world_icon_bar_update_system` (added in the world-icon-stat-bar feature) does NOT — it calls
`sprite.texture_atlas.as_mut()` to reach the guard's comparison. `.as_mut()` takes `&mut`, which
forces `DerefMut` on the `Mut<Sprite>` *before* the `if atlas.index != want_index` check runs, so
every cell Sprite is marked `Changed` every frame even when nothing changes (up to ranks×cells =
4×20 sprites per bar in split-screen). The inner guard prevents the assignment but not the
change-tick. Correct pattern: read `sprite.texture_atlas.as_ref().map(|a| a.index)` first, then
only `get_mut`/`.as_mut()` when it differs.

**How to apply:** When reviewing or writing any `stat_display.rs` update system (or any per-frame
system that guards writes for change-detection), verify the comparison reads through an immutable
path — `.as_ref()`, plain field read — and that `.as_mut()`/`get_mut()`/`&mut` field access only
happens on the branch that truly mutates. Same failure class as
[[project_changedetection_transform_writes]] (unconditional Transform writes re-propagating to
children). Also note `Assets::get_mut(handle)` queues an `AssetEvent::Modified` even if you don't
change the asset — the pixel system's color path has this pre-existing, accepted.
