---
name: screen-offset-stacking
description: screen_offset (StatLabelDef/WorldStatBarDef) pixel-stacking pattern — shared-offset rule, mismatched defaults (2.4/2.5/2.8), the 72px-per-metre derivation caveat, and which projects still ship the anti-pattern
metadata:
  type: project
---

`screen_offset: (f32, f32)` was added to `StatLabelDef` and `WorldStatBarDef` (schema/catalog.rs,
`#[serde(default)]` → `(0.0, 0.0)`) in feature/nameplate-zoom-spacing round 2 (~2026-08-16). It is a
pixel-space offset applied AFTER perspective projection, in `world_label_screen_pos_system`
(lib.rs): `new_y = half_h - vp.y + screen_offset.y * depth_scale_factor(...)`.

**Verified semantics (check these before repeating any doc claim):**
- **+Y is UP on screen** (Camera2d space), matching `offset`'s up-axis. Negative stacks below.
- It **IS multiplied by the depth-scale factor**, so a doc claim of "does not scale with camera
  distance" is only true in a scene with no `label_depth_scale` block. The two doc locations
  (StatLabelDef field row vs. the "Label depth scaling" callout) contradicted each other on exactly
  this at review time.
- Units are **window logical pixels** — same pixel space as `font_size`, Ascii `font_size`, and
  Pixel/Icon/Textured `size`. That is the mental model to teach, NOT a metre→pixel conversion.

**The stacking rule and its landmine:** co-located widgets must share ONE world `offset` and stack
via `screen_offset`. But the three relevant defaults are all DIFFERENT —
`NameplateOptionsDef.offset` `(0,2.4,0)`, `StatLabelDef.offset` `(0,2.5,0)`,
`WorldStatBarDef.offset` `(0,2.8,0)` — so a designer who omits `offset` gets the drift bug by
default. Also `nameplate_options.offset` is **scene-wide with no per-prefab override**, so the
shared value is cross-file, and in `3rd_person_game_demo` the shared `2.4` is the *schema default*,
never written in `main.scene.ron` (prefab comments that say "the scene's nameplate_options.offset
(2.4)" send a designer looking for a line that does not exist).

**The ~72 px/metre figure in `3rd_person_game_demo/prefabs/prefabs.ron` is a migration artifact,
not a rule.** It comes from `viewport_height / (2 * d * tan(fov/2))` ≈ 720 / (2·12·tan22.5°) — i.e.
it only holds at a ~720px-tall window, the default 45° Orbit FOV, and `reference_distance: 12.0`.
Do not recommend it to designers authoring a NEW creature. The reproducible rule is additive: the
shipped enemies all land on an 18–21 px gap ≈ `stat_label.font_size` + a few px of padding
(zombie +29/+50 with font 14 & bar 7px; snake -7/+11 font 13 bar 6px; spider -22/-4 font 14).

**enemy_snake is a deliberate exception:** it shares its OWN offset `1.4` (short body) rather than
the nameplate's scene-wide `2.4`, so its nameplate is NOT part of the stack. Root cause is the
missing per-prefab nameplate offset override — logged in `planning/claude_suggestions.md`
("Nameplate System" section) but with no designer-facing explanation in `docs/`.

**Migrated (feature/nameplate-screen-offset-migration, 2026-08-17):** `primitive_world`
attack_dummy + attack_dummy_ascii (2.1/2.5 → 2.3 ± 7px), `stats_demo` attack_dummy (2.2/2.55 →
2.4, -9/+7), `local_coop_demo` stat_widget_test (1.2/1.6 → 1.4 ± 29px). Arithmetic verified
correct in all three (delta_m × viewport_h/(2·d·tan(fov/2)), signs & original order preserved).
`EntityLabelDef`/`WorldLabelDef` have NO `screen_offset` — you cannot pixel-stack an entity
`label:` against a stat widget.

**The px/metre conversion in RON comments CONTRADICTS the docs.** `docs/20_data_formats.md`
("Picking `screen_offset` values for a new creature") explicitly says *do not* convert metres to
pixels — clear the number by roughly its own `font_size` and tune by eye (~21px rule of thumb).
The migration comments in all three projects teach the opposite. The conversion is only valid as a
*migration* device (reproduce the old look at one distance); flag it whenever a comment presents it
as the authoring method for a new prefab.

**A `reference_distance` above the camera's `max_radius` makes the block inert.** Orbit defaults are
`min_radius: 2.0`/`max_radius: 20.0`. `primitive_world` (ref 25) and `stats_demo` (ref 20) set no
camera block at all, so depth scaling never engages there — their `screen_offset` gaps are constant,
and the px/metre figure was derived at a distance the camera can never reach.

**Round-2 playtest confirmation (2026-08-16 screenshots, `3rd_person_game_demo`):** stacking works —
Spider name↔"75 / 75" stayed ~21px apart at default zoom AND zoomed in (was ~40px drift in round 1);
Hero name↔bar likewise. Two residuals seen on screen: (1) `min_scale` is a *relative* floor, so a
smaller base font bottoms out smaller — at `min_scale: 0.5` a 14px name floors at 7px (marginally
readable) while a 10px stat number floors at 5px (illegible). Recommend an absolute px floor, or a
higher per-widget `min_scale` for small-font stat labels. (2) enemy_snake's out-of-stack nameplate
(offset 1.4, see above) is *visibly* wrong on screen, not just theoretically — ~38px gap between
"Snake" and its bar while every other creature is ~20px.

See [[depth-scale-field-scope]] and [[nameplate-system]].
