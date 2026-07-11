---
name: split-screen-doc-gap
description: Split-screen multi-viewport correctness fixes update only internal CLAUDE.md, not designer-facing docs/20_data_formats.md — recurring gap
metadata:
  type: project
---

Internal split-screen "single-camera assumption" correctness fixes (world labels, nameplates, stat labels, world stat bars, particles, click-select) consistently list their only doc target as `crates/ironhold_core/src/CLAUDE.md`'s known-limitations note + `planning/claude_suggestions.md` — both internal, non-designer-facing. `docs/20_data_formats.md` is the designer-facing home and gets skipped.

**Why:** These are framed as "engine-internal correctness fixes, no new RON schema surface," so the authors treat them as invisible to designers. Precedent: `planning/features/done/world_label_split_screen_positioning.md` shipped this way (docs task updated only CLAUDE.md).

**How to apply:** For a fix with no new field but a designer-*visible* behavior asymmetry, push for a `docs/20_data_formats.md` note. Strongest case: the `split_screen_camera_followups.md` v2 scope-cut — Ascii world stat bars + stat labels duplicate per viewport, but Pixel bars, damage popups, and nameplate anchors do NOT. Docs (line ~3120) explicitly encourage coexisting Ascii+Pixel bars on one prefab, so the asymmetry is reachable. There is an established precedent for per-style limitation blockquotes right there (line 3083: "Pixel bar depth scaling ... not yet implemented for the Pixel style") — a split-screen note belongs in the same spot. The world-stat-widgets section (~3021) and nameplate section (~449, `max_distance`) are the two landing zones.
