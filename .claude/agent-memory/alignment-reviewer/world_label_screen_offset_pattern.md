---
name: world-label-screen-offset-pattern
description: StatLabelDef/WorldStatBarDef.screen_offset — the pixel-space widget-stacking field, which WorldLabel construction sites expose it vs hardcode ZERO, and the missing per-prefab nameplate offset that blocks the pattern for short creatures
metadata:
  type: project
---

`screen_offset: (f32, f32)` (`#[serde(default)]`, `schema/catalog.rs` — `StatLabelDef` ~960,
`WorldStatBarDef` ~992) is a **pixel-space** offset applied AFTER world-to-screen projection.
Added by `feature/nameplate-zoom-spacing` round 2 (2026-08-16). It exposes to RON an internal
`WorldLabel.screen_offset: Vec2` field (`scene_manager/mod.rs` ~383) that already existed but was
only ever non-zero for damage-popup shadows.

**Why it exists** — a single entity's nameplate + `stat_label` + `world_stat_bar` are THREE
independent `WorldLabel` entities, each perspective-projected separately. Slightly different world
`offset`s (2.4 / 2.8 / 3.1) project to a pixel gap that GROWS as the camera closes in (~40px gap
observed in a playtest screenshot). Designer rule: **give co-located widgets on one entity the SAME
world `offset`, and stack them with `screen_offset`.**

**Applied in `world_label_screen_pos_system` (`lib.rs` ~673-675)**, multiplied by
`depth_scale_factor(label.depth_scale, dist)` so the pixel stack shrinks in lockstep with the
widgets. This is what makes the fix correct rather than a second drift source: below
`reference_distance` the factor is clamped at 1.0 (constant px gap — the zoomed-IN bug case);
beyond it the factor is `ref/dist`, which coincidentally matches how a world gap would have
projected — so the whole stack behaves as one rigid, uniformly-scaling unit at every zoom.
Sign convention: `new_y = half_h - vp.y + screen_offset.y * s` → **positive Y is up** (matches
`offset`).

**Coverage matrix — which `WorldLabel` sites accept a RON `screen_offset`:**
| Site | RON-authorable? |
|---|---|
| `stat_display.rs` all 6 sites (stat_label, Ascii bg+fill, Pixel/Icon/Textured anchors) | YES — `Vec2::from(def.screen_offset)` |
| `nameplate.rs` ~137 anchor | NO — hardcoded `Vec2::ZERO`, `NameplateOptionsDef` has no such field |
| `scene_loader.rs` ~926 (`world_labels:`) / ~963 (entity `label:`) | NO — hardcoded `Vec2::ZERO` |
| `action_executor.rs` ×4 (damage popup / floating text) | NO — engine-internal shadow offsets |

The nameplate being pinned at `screen_offset == 0` is workable-by-design (it's the reference the
stat widgets stack against), but note the asymmetry: `WorldLabelDef`/`EntityLabelDef` have
`depth_scale` but NOT `screen_offset`; `StatLabelDef`/`WorldStatBarDef` have `screen_offset` but
NOT `depth_scale`. Neither type has both. An entity carrying BOTH a `label:` and a nameplate
cannot be pixel-stacked.

**The real remaining designer-reachability gap: `nameplate_options.offset` is SCENE-WIDE with no
per-prefab override.** So a mixed-height cast can't fully follow the pattern —
`3rd_person_game_demo`'s `enemy_snake` deliberately shares `offset: 1.4` between its two stat
widgets (its own body midpoint) while the scene nameplate sits at 2.4, so snake still has
nameplate-vs-stat-widget drift. Logged in `planning/claude_suggestions.md` ▸ Nameplate System
(2026-08-16) as needing a real schema addition (per-prefab nameplate offset). If a future review
sees a "widget cluster still drifts" complaint on a short creature, this is the cause.

**Migration arithmetic used in `prefabs.ron` comments: "~72 px/metre at reference_distance 12.0".**
Derivable as `(viewport_h/2) / (d * tan(fov/2))` ≈ `1000/2 / (12 * tan30°)` — i.e. it is
**viewport-height- and FOV-dependent**, not a portable engine constant, and the RON comments don't
say so. Only needed to reproduce a pre-existing world-offset stack as pixels; for a NEW creature
the designer just tunes `screen_offset` in pixels directly (same unit space as `font_size` and
`Pixel`/`Textured` bar `size`, which is the strongest alignment argument for the whole design).

**Zero CLI/test coverage for the new field**: `ironhold_cli` never mentions `screen_offset` (it
deserializes `PrefabCatalog` straight from `ironhold_core::schema`, so `#[serde(default)]` means
adding the field is not a CLI break); every `screen_offset` in `crates/ironhold_core/tests/` is a
`ZERO`/`(0.0,0.0)` fixture fill-in — nothing asserts a non-zero value propagates RON → `WorldLabel`
→ screen position.
