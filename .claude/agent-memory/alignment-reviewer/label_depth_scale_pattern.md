---
name: label-depth-scale-pattern
description: How label_depth_scale reaches each WorldLabel consumer — the TextFont-vs-anchor branch in world_label_screen_pos_system, which 3 anchor styles are still hardcoded to None, and the hardcoded .min(1.0)/no-max_scale gap
metadata:
  type: project
---

`GameSceneV2.label_depth_scale: Option<LabelDepthScaleDef>` (`schema/scene_v2.rs` ~745;
`reference_distance` default **20.0** since `feature/label-depth-scale-validation` — matches
`entity_spawner::default_camera_config()`'s `max_radius`, which is now `pub` specifically so
`ironhold_cli` can reuse the constants; `min_scale: Option<f32>`) is the ONE scene-level knob for
distance-based label shrinking. `resolve_label_depth_scale(scene, per_label) -> Option<(f32,f32)>`
(`scene_loader.rs` ~2737, `pub(crate)` since `feature/nameplate-zoom-spacing`) is the single
precedence resolver; `LoadedLabelDepthScale(Option<LabelDepthScaleDef>)` (`scene_manager/mod.rs`
~286, `init_resource`'d `lib.rs` ~170, inserted unconditionally per scene load at
`scene_loader.rs` ~743 beside `ActiveTonemapping`) is how non-scene-load systems reach it.

**Two application mechanisms, branched on `Option<&TextFont>` in `world_label_screen_pos_system`
(`lib.rs` ~623, added by `feature/nameplate-zoom-spacing` 2026-08):**
- **Has `TextFont`** → rewrite `TextFont.font_size` (glyph stays crisp). 0.5 epsilon guard.
- **No `TextFont` (anchor-style)** → write the anchor's own `Transform.scale = Vec3::splat(s)`,
  which propagates to the whole Text2d/Mesh2d/Sprite child subtree. 0.005 epsilon guard.

**Exactly 4 anchor-style (no-TextFont) `WorldLabel` spawn sites exist** — verify this list before
assuming a change is scoped: `nameplate.rs` ~131 (NameplateAnchor) and `stat_display.rs` ~585 /
~677 / ~766 (Pixel / Icon / Textured bar anchors). Every other `WorldLabel` site (scene
`world_labels:`, entity `label:`, stat_label, Ascii bar bg+fill, all 4 damage-popup/floating-text
sites in `action_executor.rs`) carries `TextFont` and takes the font-size branch.

**Per-widget override matrix (all `deny_unknown_fields` — a designer cannot invent a field):**
| Consumer | `depth_scale: Option<bool>` override? |
|---|---|
| `WorldLabelDef` (`world_labels:`) | YES |
| `EntityLabelDef` (`label:`) | YES |
| `StatLabelDef` | NO — inherits scene only |
| `WorldStatBarDef` | NO — inherits scene only |
| `NameplateOptionsDef` | NO — inherits scene only (as of the fix) |

Consequence: a scene wanting depth scaling for `world_labels:` but NOT for nameplates/stat widgets
has no clean escape hatch — the only workaround (drop the scene block, set `depth_scale: true`
per-label) forces the hardcoded `(50.0, 0.0)` fallback in `resolve_label_depth_scale`, so
`reference_distance` becomes unauthorable. Adding `depth_scale: Option<bool>` to
`NameplateOptionsDef`/`StatLabelDef`/`WorldStatBarDef` + passing it as `per_label` is the
2-line, fully-backward-compatible fix if this ever comes up.

**CORRECTED 2026-08-16 — the Pixel/Icon/Textured anchors are NO LONGER hardcoded to `None`.** All
SIX `WorldLabel` construction sites in `stat_display.rs` (stat_label :465, Ascii bg :526 + fill
:547, Pixel anchor :592, Icon anchor :684, Textured anchor :774) now pass `depth_scale:
ctx.depth_scale`. An earlier version of this memory said otherwise — that was true only mid-way
through `feature/nameplate-zoom-spacing`'s first round. All four `world_stat_bar` styles scale.
Only `nameplate.rs`'s anchor + the 4 damage-popup/floating-text sites in `action_executor.rs` and
the two scene-loader label loops differ (nameplate resolves it; popups deliberately stay `None`).

**Two hardcoded numbers in the formula, neither RON-exposed:**
- `.min(1.0)` upper clamp — labels never GROW when closer than `reference_distance`. There is no
  `max_scale` field on `LabelDepthScaleDef`. **This is NOT the cause of the "spread too far apart
  when zoomed in" bug** — a 2026-08-16 playtest screenshot proved the real cause was three
  independent `WorldLabel`s with slightly different world `offset`s, fixed via `screen_offset` (see
  [[world-label-screen-offset-pattern]]). The `.min(1.0)` clamp is very likely fine as designed.
- The `None`-scene fallback `(50.0, 0.0)` in `resolve_label_depth_scale`.

**`3rd_person_game_demo`'s block was retuned `(8.0, 0.25)` → `(12.0, 0.5)`** after a real-hardware
playtest showed 14px name text rendering at ~3.5px (illegible) at max zoom-out. Treat `min_scale`
below ~0.4 as a legibility smell when the scene's smallest `font_size` is in the 13–16px range.

**CORRECTED 2026-08-31 — `ironhold_cli validate` now covers both** (`feature/label-depth-scale-
validation`, `planning/features/label_depth_scale_validation.md`):
- `label_depth_scale_min_scale_out_of_range` — `CrossFileError`, always-on. Twin runtime
  `warn_label_depth_scale_min_scale_out_of_range` (`scene_loader.rs` ~1464) + a silent
  `.clamp(0.0, 1.0)` (NaN→0.0) now inside `resolve_label_depth_scale` (`mod.rs` ~509). This IS a
  behavior change for `min_scale > 1.0` (was pinned at 2×, now 1×) — verified harmless, all 10
  shipped scenes author 0.25–0.5.
- `label_depth_scale_reference_distance_outside_camera_range` — `StrictWarning` (`--strict` only),
  band `[overall_min*0.5, overall_max*2.0]` over the union of reachable radius-bearing cameras.
  Twin runtime `warn_label_depth_scale_reference_distance` (`scene_loader.rs` ~1518).

**Asymmetry to remember: the CLI check also scans project-wide `Action::Spawn` prefabs for
player tags (`validate.rs` ~1067); the runtime twin only sees scene-load `player_configs`.** So
`3rd_person_game_demo`'s main scene (player spawned from `state_machine.ron`, never in
`scene.entities`, no `camera_modes:` registry) gets the CLI warning but NO runtime warning — the
exact scene the feature was written for. Any future runtime-side camera-range check inherits this
hole.

**`camera_mode_radius_range` exists TWICE** — private fn in `scene_loader.rs` ~1496 and a copy in
`validate.rs` ~164. Classifies only schema types (`Orbit`/`Party` → radii, `Follow` →
`offset.length()` as both bounds, `Fixed`/`FirstPerson`/`Flycam` → `None`), so it belongs on
`impl CameraModeDef` in `schema/camera.rs` per the `PrefabDef::is_flycam()` precedent (see
[[diagnostic-only-feature-pattern]]). Both matches are exhaustive, so a *new* variant breaks
compile in both; a *reclassification* drifts silently.

**Which shipped scenes author a `label_depth_scale` block** (check this before calling a change
to the resolver a compat break): `stats_demo`, `3rd_person_game_demo` (added by the fix),
`effect_mayhem_demo`, `custom_materials`, `particles_demo` (main + particles2), `primitive_world`.
Only `3rd_person_game_demo` combines it with `show_nameplates: true` — `local_coop_demo`'s
room3/room9/room10 have nameplates but no depth-scale block, so they were unaffected.
