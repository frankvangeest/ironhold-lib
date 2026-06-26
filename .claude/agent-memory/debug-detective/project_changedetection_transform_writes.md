---
name: per-frame-changedetection-transform-writes
description: world_label_screen_pos_system writes Transform.translation unconditionally every frame, dirtying every nameplate anchor + Text2d/mesh children — a change-detection / transform-propagation stutter source
metadata:
  type: project
---

`world_label_screen_pos_system` in `crates/ironhold_core/src/lib.rs` writes `t.translation.x` and `t.translation.y` **unconditionally** every frame (the `match camera.world_to_viewport {...}` Ok arm), even when the camera and tracked entity are static and the projected screen position is identical. It correctly guards `Visibility` and `TextFont.font_size` writes, but NOT `Transform`.

Each `WorldLabel` anchor parents several `Text2d` + `Mesh2d` children (name, shadow, one bg + one fill quad per stat bar). Dirtying the parent `Transform` forces transform propagation + Text2d glyph re-layout + Mesh2d re-extraction for the whole subtree every frame. The nameplate feature multiplies the anchor count (one per visible enemy/NPC), so a 3rd_person_game_demo idle scene re-runs this for ~8 subtrees per frame — the constant idle stutter.

**Why:** CLAUDE.md "Change-detection discipline" mandates guarding render-affecting writes so change detection only fires on real change. The `Transform` write here violates that rule; the font/visibility writes in the same loop already follow it (lines ~427, 436, 439), so the fix is to apply the same guard to translation.

**How to apply:** Fix is to compare against current translation before writing, e.g. only assign when `(t.translation.x - new_x).abs() >= 0.5 || (t.translation.y - new_y).abs() >= 0.5` (sub-pixel threshold, mirrors the 0.5 font guard). Check siblings: `damage_popup_system` and any other system iterating `WorldLabel`/`Transform` for the same unconditional-write pattern. Related: [[webgpu-preprocessing-warning]] (the WebGPU warnings are a red herring, not the cause).
