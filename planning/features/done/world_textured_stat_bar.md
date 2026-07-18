# Feature: World-space Textured Stat Bar (`WorldStatBarStyle::Textured`)

_Status: Ready (schema/demo corrected 2026-07-18 against the actual supplied art)_
_Planned at: `7168ccc` (2026-07-17)_

## What
A fourth `world_stat_bar` style — `Textured` — rendering a stat as a **continuous** textured fill
bar built from designer-authored art: a rounded-corner (or any-shape) "empty" track sprite with a
"full" fill sprite drawn on top, whose visible width is driven continuously by the stat ratio. The
caps and border are part of the texture art and stay undistorted at every fill width via Bevy's
9-slice (`SpriteImageMode::Sliced`) — so a designer gets a shippable, art-directed health bar
(begin-cap → stretching middle → end-cap, empty and full states) without any of the flat-colour
look of `Pixel` or the discrete per-cell look of `Icon`.

This is the "textured continuous bar" concept in Frank's own framing: *"a rounded corner health
bar, begin sprite, n-middle sprites and end sprite, for empty and full health"* — deliberately
**not** the hearts/pips interpretation (that is `Icon`, see `world_icon_stat_bar.md`), and **not**
a flat solid fill (that is `Pixel`). Architecturally it is much closer to `Pixel` than to `Icon`:
same continuous-fill update mechanism, just a 9-sliced textured `Sprite` in place of `Pixel`'s flat
`ColorMaterial` `Mesh2d` quad.

## Why
`world_stat_bar` now has three production-relevant looks in flight: `Pixel` (flat solid fill,
production-quality, split-screen-complete), `Icon` (discrete pips/hearts, shipped `672d003`), and `Ascii`
(prototyping-only, slated for eventual retirement). None of them let a designer ship an
**art-directed** continuous bar — the single most common shippable HUD element in the genre this
engine targets (action/RPG floating enemy health bars, boss bars, player overhead health). Today a
designer who wants a rounded, bordered, textured health bar has no data-driven path to it; they'd
have to fall back to `Pixel`'s flat rectangle. `Textured` closes that gap using art the designer
already authors, with zero new render plumbing beyond Bevy's built-in 9-slice sprite support.

It also rounds out the style taxonomy cleanly: **flat** (`Pixel`), **discrete** (`Icon`),
**textured-continuous** (`Textured`) — three orthogonal production looks, plus `Ascii` as the
retiring debug default. Like `Icon`, this plan builds full split-screen duplication in from day one
rather than repeating `Pixel`'s original single-viewport-only mistake.

**Purely additive — no schema break.** A new `WorldStatBarStyle` enum variant; every existing
`Ascii`/`Pixel` bar and every `style`-less bar (defaulting to `Ascii`) is byte-for-byte unaffected.

## Approach

### Rendering — a 9-sliced `Sprite`, continuous fill via `custom_size.x` (the textured analog of `Pixel`)
Bevy 0.18's `bevy_sprite` ships full 9-slice support (verified against the Cargo-locked
`bevy_sprite-0.18.0` source): `Sprite.image_mode: SpriteImageMode::Sliced(TextureSlicer)`, where
`TextureSlicer { border: BorderRect, center_scale_mode, sides_scale_mode, max_corner_scale }` and
`BorderRect { min_inset, max_inset }` define the four cap-inset lines in **texture pixels**. On
resize the four corners stay fixed-size, the two horizontal sides + center stretch — exactly the
begin-cap / stretch-middle / end-cap decomposition Frank described, produced internally by
`TextureSlicer::compute_slices` with no manual 3-entity composition needed.

The continuous fill is driven by **`Sprite.custom_size.x`**, the direct textured analog of how
`Pixel` already drives `Transform.scale.x`:

| Mechanism | Cap art at varying fill | New render pipeline | Verdict |
|---|---|---|---|
| **Sliced `Sprite` + `custom_size.x`** (this plan) | Correct — 9-slice keeps caps undistorted; re-slices on every size change | Standard 2D sprite pipeline (see below) | **Chosen** |
| `Transform.scale.x` on a textured `Mesh2d`/quad (what `Pixel` does, but textured) | **Distorts** the caps horizontally — rounded ends smear | none new | Rejected — the exact artifact 9-slice exists to prevent |
| `Sprite.rect` crop (reveal a sub-rect of a full texture) | **Clips** the right cap flat — no rounded receding edge | none new | Rejected for capped art (viable only for flat/patterned fills) |
| Custom UV-mask shader | Correct, but reinvents 9-slice in WGSL + a new material/pipeline | new pipeline | Rejected — unjustified complexity vs. built-in slicing |

**Correction vs. the original draft (2026-07-18, after Frank supplied the actual art)** — Frank
added `assets/shared/ui/rounded-healthbar-texture-sheet.png`: a single 48×48 **sheet**, not two
separate fill/empty PNGs as first drafted. Inspecting it (`PIL`, per-row alpha profile) shows two
frames stacked vertically in the one file: rows 0–16 (~38px wide, cols 5–42) are a **solid filled
pill** (opaque middle); rows 17–31 are a **hollow outline pill** (only the border stroke is
opaque, alpha≈4–9 in the middle — a see-through track ring); rows 32–47 are unused padding. Both
frames are flat mid-grey (`R==G==B`, varying only in edge-antialiasing alpha) — colorless, exactly
so a single sheet can be tinted red for health, blue for mana, etc. (Frank's own framing). Verified
against the Cargo-locked `bevy_sprite_render-0.18.0` source
(`texture_slice/computed_slices.rs::compute_sprite_slices`) that 9-slicing composes correctly with
a plain `Sprite.rect` crop of a larger image — `rect` (or the atlas rect, if a `TextureAtlas` were
used instead) is passed straight through as `compute_slices`' `texture_rect` argument, so slicing
happens **within** the cropped sub-rect's own coordinate space, not the full sheet. No
`TextureAtlasLayout` is needed for this (unlike `Icon`, which genuinely needs grid-index swapping)
— a static `Sprite.rect` crop is simpler and sufficient here since each layer only ever draws one
fixed sub-rect for its whole lifetime.

This replaces the schema's `fill_texture`/`empty_texture` two-catalog-key design with **one**
`texture_sheet` catalog key plus two designer-authored sub-rects (`fill_rect`, `empty_rect`, in
texture pixels) — see Schema below. It also means both `Sprite` layers share **one**
`Handle<Image>` (one asset load, not two), differing only in their static `Sprite.rect` crop —
strictly cheaper than the original two-handle design, not just different.

**Two `Sprite` layers per bar**, both referencing the same `texture_sheet` image handle, cropped
via `Sprite.rect` to their own sub-rect:
- **Empty/track layer** — sliced `Sprite`, `rect = empty_rect`, `custom_size = (width, height)`,
  **static** (never updated), lower z. Tinted by `bg_color` (see Schema below — flipped from the
  original draft's "bg_color has no equivalent here", now that the art is a colorless outline
  meant to be recolored).
- **Fill layer** — sliced `Sprite`, `rect = fill_rect`, `custom_size = (ratio * width, height)`,
  updated per frame, higher z, **left-aligned by mirroring `Pixel`'s proven translation math**
  (`translation.x = -width/2 + fill_width/2`) so the fill grows from the left edge and its rounded
  right end recedes as the stat drops. (Using the translation shift rather than an `Anchor`
  component sidesteps any anchor-API specifics and reuses a pattern already shipped in
  `world_pixel_bar_update_system`.) Tinted by `fill_color`/`color_bands`, unchanged from the
  original draft.

**Low-fill behavior (document, don't fight):** when `custom_size.x` shrinks below the summed cap
widths, `TextureSlicer`'s `min_coef = coef.x.min(coef.y).min(max_corner_scale)` scales the corner
slices *down* proportionally rather than overlapping/clipping them — so near-empty the rounded caps
get tighter (smaller radius) instead of glitching. This is benign and arguably desirable; note it
in the docs so a designer isn't surprised the corners "un-round" at very low health.

**Pipeline / WASM.** A **sliced** sprite renders on the **same** standard 2D sprite pipeline as a
plain `Sprite` — `compute_slices` just emits more quads into that one pipeline; there is no
distinct "sliced" pipeline. So the WASM cost is exactly one standard sprite pipeline, lazily
compiled and warmed at scene-load like every other render path. **This is the same
first-use-of-`Sprite` risk `world_icon_stat_bar.md` already flags** (the engine currently uses
`Mesh2d`/`ColorMaterial` and Bevy UI `ImageNode`, never `Sprite`). Two consequences:
- **Sequence after `Icon` if both are in flight** (soft, organizational — not a hard dependency):
  whichever of `Icon`/`Textured` lands first pays the one-time sprite-pipeline introduction cost
  and proves it under `python test_web.py`; the second inherits a proven path. Neither blocks the
  other technically.
- **Fallback is weaker than `Icon`'s** (be honest): if the sprite pipeline stalls unrecoverably on
  WASM, there is no clean *9-sliced* fallback — a `Mesh2d` + `ColorMaterial { texture: Some(..) }`
  quad can show the texture but only under `scale.x`, which reintroduces cap distortion. So the
  real mitigation is the sequencing above (prove the sprite pipeline via `Icon` first) plus the
  `test_web.py` gate, not a drop-in degraded mode. If the pipeline is fine (expected — sliced
  sprites are a mainstream Bevy path), no fallback is needed.

### Schema — a separate `Textured` variant, reusing the shared colour fields as a tint
```rust
/// Textured continuous fill bar: a 9-sliced "empty" track sprite with a 9-sliced "full" fill
/// sprite drawn on top, both cropped from one shared sheet, fill width driven continuously by
/// the stat ratio. Caps/border are part of the art and stay undistorted at every width via
/// Bevy's `SpriteImageMode::Sliced`.
Textured {
    /// Catalog key into `AssetCatalog.textures` — ONE sheet containing both the fill and empty
    /// frames (see `fill_rect`/`empty_rect`). Same catalog convention as `EffectDef.sprite`,
    /// `Icon.icon_sheet`, and every other texture reference in the engine (key → path →
    /// `asset_server.load`). Both Sprite layers share this single `Handle<Image>`.
    texture_sheet: String,
    /// Sub-rect `(x, y, w, h)` in **TEXTURE pixels** — the FULL/fill frame within `texture_sheet`
    /// (9-sliced, drawn on top, width = ratio * size.0). Cropped via `Sprite.rect`.
    fill_rect: (f32, f32, f32, f32),
    /// Sub-rect `(x, y, w, h)` in **TEXTURE pixels** — the EMPTY/track frame within
    /// `texture_sheet` (9-sliced, static, full width, drawn underneath). Cropped via `Sprite.rect`.
    empty_rect: (f32, f32, f32, f32),
    /// Bar dimensions in **screen pixels** `(width, height)` — same coordinate space as
    /// `Pixel.size`/`Icon.size` (Camera2d, 1 unit = 1 px; constant at all camera distances, no
    /// depth scaling in v1). Clamped to a min of `(1.0, 1.0)`. Default: `(64.0, 12.0)`.
    #[serde(default = "default_textured_bar_size")]
    size: (f32, f32),
    /// 9-slice cap insets in **TEXTURE pixels** `(left, right, top, bottom)`, relative to each
    /// rect's own origin — the fixed corner/cap regions of the source art that must NOT stretch.
    /// Maps directly to `TextureSlicer.border` (`BorderRect { min_inset: (left, top), max_inset:
    /// (right, bottom) }`). Author this to match the actual cap radius of your fill/empty art
    /// (for a stadium/pill shape, cap width ≈ frame height / 2). Applied identically to both the
    /// fill and empty layers (both frames share one cap geometry in every sheet seen so far — a
    /// per-layer override can be added later if a sheet ever needs asymmetric caps). Default:
    /// `(6.0, 6.0, 6.0, 6.0)`. (Distinct unit from `size`: `size` is on-screen pixels,
    /// `slice_border` is source-image pixels — spelled out in the docs so it isn't mistaken for
    /// screen space.)
    #[serde(default = "default_textured_bar_slice")]
    slice_border: (f32, f32, f32, f32),
},
```

**No new colour/tint fields — reuse the shared `WorldStatBarDef.fill_color`/`bg_color` +
`color_bands`.** Both frames in the reference sheet are flat, colorless mid-grey — authored
specifically to be recolored by multiply-tint (Frank: *"this texture can be multiplied with a
color, e.g. red for health, blue for mana"*). The fill `Sprite.color` multiplies `fill_rect`'s
pixels exactly like it drives `Pixel`'s `ColorMaterial.color` today, and — **correction vs. the
original draft** — the empty/track `Sprite.color` is now multiplied by the existing shared
`bg_color` field the same way, rather than being left untinted. This is a straight reuse of a
field that already exists on `WorldStatBarDef` and is already used this way for `Pixel`'s
background quad (`stat_display.rs:437`) — no `Textured`-only field needed, one colour convention
across all three production styles. **Caveat to document:** the shared `fill_color` default is
bright green `(0.15, 0.85, 0.15, 0.95)` and `bg_color`'s default is dark red
`(0.25, 0.08, 0.08, 0.75)` — both would tint an already-coloured sheet. A designer using
pre-coloured art should set both to white/neutral; a designer using the shipped greyscale sheet
leans on `fill_color`/`color_bands` (state tint) and `bg_color` (track tint) exactly as intended.
If a designer supplies a *pre-colored, opaque* empty-track frame and wants zero tint on it, they
set `bg_color: (1.0, 1.0, 1.0, 1.0)`.

**`deny_unknown_fields` note (correction vs. the `Icon` plan).** The existing `Ascii` and `Pixel`
enum variants do **not** carry `#[serde(deny_unknown_fields)]` (only the outer `WorldStatBarDef`
struct does). `Textured` should **match the existing variants** (no per-variant attribute) for
consistency — do *not* add `#[serde(deny_unknown_fields)]` to just this one variant. `world_icon_
stat_bar.md`'s schema section claims to add it to `Icon`; that would introduce an inconsistency and
should be corrected there too (logged in Documentation & Planning below).

```ron
// The shipped reference sheet — greyscale fill art, coloured by state bands, dark track tint.
world_stat_bar: (
  stat_key: "player_health",
  offset: (0.0, 2.3, 0.0),
  bg_color: (0.15, 0.15, 0.15, 0.6),  // dim neutral track tint
  color_bands: [
    (0.0, (0.85, 0.12, 0.12, 1.0)),  // < 30% → red
    (0.3, (0.95, 0.75, 0.10, 1.0)),  // ≥ 30% → yellow
    (0.6, (0.15, 0.85, 0.15, 1.0)),  // ≥ 60% → green
  ],
  style: Textured(
    texture_sheet: "healthbar_sheet",
    fill_rect:  (0.0, 0.0, 48.0, 17.0),   // solid pill frame, top of the sheet
    empty_rect: (0.0, 17.0, 48.0, 15.0),  // hollow outline frame, below it
    size: (72.0, 14.0),
    slice_border: (8.0, 8.0, 8.0, 8.0),
  ),
),

// Pre-coloured, opaque art — disable both tints by setting fill_color/bg_color to white.
world_stat_bar: (
  stat_key: "{self}.health",
  fill_color: (1.0, 1.0, 1.0, 1.0),
  bg_color: (1.0, 1.0, 1.0, 1.0),
  style: Textured(
    texture_sheet: "boss_hp_sheet",
    fill_rect: (0.0, 0.0, 64.0, 20.0),
    empty_rect: (0.0, 20.0, 64.0, 20.0),
  ),
),
```

### Runtime — one new marker + one update system, mirroring `Pixel`
- New anchor-child fill marker `WorldTexturedBarFillMarker { stat_key, full_width, fill_color,
  color_bands }` (structurally identical to `WorldPixelBarFillMarker`, minus the mesh-specific bits).
- New `world_textured_bar_update_system` — near-identical to `world_pixel_bar_update_system`, but
  writes `Sprite.custom_size.x` + `Sprite.color` instead of `Transform.scale.x` +
  `ColorMaterial.color`. Same clamp-to-ratio maths, same `color_bands`/`fill_color` selection, same
  left-align translation update, **same change-detection guarding** (only write when the value
  meaningfully differs — the `>= 0.5 px` / colour-inequality guards, per `crates/ironhold_core/
  src/CLAUDE.md`'s change-detection discipline). Register it alongside `world_pixel_bar_update_system`.
- Extend `spawn_world_stat_bar_widget`'s `match def.style` with a `Textured` arm.

**`StatWidgetSpawnCtx` already has what this needs — no further extension required.** `world_icon_
stat_bar.md` (shipped, `672d003`) already added `asset_server: Option<&'a AssetServer>` and
`asset_catalog: Option<&'a AssetCatalog>` to `StatWidgetSpawnCtx` for exactly this purpose (the
"coordinate with Icon, do it once" note in the original draft is now resolved — `Icon` landed
first and paid that cost). `Textured` resolves **one** catalog key (`texture_sheet` →
`catalog.textures.get(key)` → path → `asset_server.load::<Image>()`) and clones the resulting
`Handle<Image>` for both the fill and empty `Sprite` layers — it needs no `TextureAtlasLayout`
(unlike `Icon`), since a static `Sprite.rect` crop is sufficient (verified above). Missing-key
handling: `warn!` once and skip the bar (never fabricate a `shared/...` path — per the
shader/asset-fallback rule in `crates/ironhold_core/src/CLAUDE.md`).

### Split-screen duplication — built in from day one
Identical to `pixel_world_stat_bar_split_screen_duplication.md` (shipped, `0257c83`) and planned
for `Icon`: the `Textured` arm wraps its construction in the same `for rank in 0..ranks` loop
(`ranks` already computed from `ctx.is_split_screen`). Per rank: one anchor (`WorldLabel` +
`WorldLabelRank(rank)` + `Visibility::Hidden` for rank > 0) with two `Sprite` children (empty +
fill), children inheriting the anchor's visibility via Bevy's `InheritedVisibility` cascade exactly
like `Pixel`'s `Mesh2d` children. **The `TextureSlicer` and the two `Handle<Image>` are built once
per bar instance and cloned across ranks** (identical geometry/art per rank — same optimisation
`Pixel` makes for its border/bg mesh+material). Only the per-rank fill `Sprite`'s `custom_size`/
`color` differ frame-to-frame, and even those are identical across ranks of the same bar (all ranks
track the same entity's same stat). `LevelEntity` on every rank's children so scene-change cleanup
frees all ranks. Soft-sequenced after the shipped `Pixel` duplication to reuse a landed pattern and
avoid a merge collision on `spawn_world_stat_bar_widget` — not a hard technical dependency
(`WorldLabelRank` + hierarchy visibility is already proven at rank-0 in shipped code).

## Explicitly out of scope
- **Non-horizontal / non-rectangular bars** — vertical fill, radial/circular arcs, diagonal bars.
  v1 is a horizontal left-to-right fill only. A different fill axis is a separate feature.
- **Animated fill effects** — shine sweeps, pulse-on-damage, easing/lerp of the fill toward the
  target ratio, "ghost"/delayed-damage trailing bars. v1 snaps to the current ratio each frame,
  same as every other style today. Animated fill is a materially bigger feature (needs per-bar
  animation state), deferred.
- **Exposing `SliceScaleMode::Tile`** for the middle/sides — v1 hardcodes `Stretch` (the standard
  health-bar look). A `tile:` option for patterned/repeating middles is a trivial future addition
  (one field, one enum) but no current need has surfaced; don't add speculative surface area now.
- **Hard-edge crop mode** (`Sprite.rect`-based reveal instead of 9-slice `custom_size`) — a
  different visual (flat receding edge, clipped caps); if a designer ever wants that look it's a
  separate style/field, not bundled here.
- **Rounded corners beyond what the art + 9-slice provide** — the engine does not synthesise
  rounded geometry; roundness lives entirely in the designer's texture. No `corner_radius` field.
- **Depth scaling** — same pre-existing limitation `Pixel` has (fixed screen-pixel size at all
  camera distances); the anchor's `depth_scale` stays `None`. Not solved here.
- **A generic "any-shape health bar" system** — this is one concrete style (horizontal 9-sliced
  continuous fill), not a shape/geometry authoring framework.

## Tasks
- [ ] Schema — `WorldStatBarStyle::Textured { texture_sheet, fill_rect, empty_rect, size,
      slice_border }` struct variant in `catalog.rs` (inline fields matching `Ascii`/`Pixel`; **no**
      per-variant `deny_unknown_fields`, matching the existing variants), with
      `default_textured_bar_size` / `default_textured_bar_slice` default fns and full doc comments
      (esp. the two coordinate spaces: `size` = screen px, `fill_rect`/`empty_rect`/`slice_border` =
      texture px; and that `fill_color` tints the fill while `bg_color` now tints the empty/track
      layer — both reused, no `Textured`-only colour field).
- [ ] `capabilities/stat_display.rs` — `WorldTexturedBarFillMarker` component;
      `world_textured_bar_update_system` (writes `Sprite.custom_size.x` + `Sprite.color`, guarded
      for change-detection, left-align translation, `color_bands`/`fill_color` selection identical
      to the Pixel system); `Textured` arm in `spawn_world_stat_bar_widget`, rank-duplicated, with
      **one** `Handle<Image>` (`texture_sheet`) resolved once and cloned across both layers and every
      rank, two `Sprite` children (empty static + fill marked) per rank — each with its own static
      `Sprite.rect` crop (`empty_rect`/`fill_rect`) and `SpriteImageMode::Sliced(TextureSlicer)`
      built from `slice_border` — `LevelEntity` on every child.
- [ ] `StatWidgetSpawnCtx` — **no extension needed.** `asset_server`/`asset_catalog` already exist
      on the ctx (added by `world_icon_stat_bar.md`, shipped `672d003`); the `Textured` arm reuses
      them as-is. Missing catalog key → `warn!` once + skip (no fabricated paths).
- [ ] Register `world_textured_bar_update_system` alongside `world_pixel_bar_update_system` in
      `lib.rs`.
- [ ] Tests — parse/defaults tests for the `Textured` variant in `ron_validation.rs` (matching the
      `Ascii`/`Pixel`/`Icon` set: minimal RON parses, defaults resolve, a full RON parses);
      spawn-behavior test (a `Textured` bar spawns exactly one anchor + two `Sprite` children in a
      non-split scene; the fill child carries `WorldTexturedBarFillMarker`; both children share one
      cloned `Handle<Image>` but differ in `Sprite.rect`); split-screen rank-duplication test
      (`Sprite`/marker counts scale to `MAX_SPLIT_PLAYERS`, image handle cloned not re-loaded),
      mirroring the Pixel/Icon duplication tests.
- [ ] CLI — `cargo check -p ironhold_cli` (new enum variant must not break `query.rs`); run
      `cargo run -p ironhold_cli -- query <project>` on the demo to confirm the new style surfaces
      and nothing crashes.
- [ ] Demo — **replace** `3rd_person_game_demo`'s `player_male`/`player_female` `world_stat_bar`
      (currently `Icon`, the hearts bar shipped in `world_icon_stat_bar.md`) with this `Textured`
      style, tracking the same global `player_health` key, using the already-supplied
      `assets/shared/ui/rounded-healthbar-texture-sheet.png`. **Scope change vs. the original
      draft** (2026-07-18, Frank's explicit instruction) — the original draft placed `Textured` on
      an NPC specifically to avoid colliding with the not-yet-built `Icon` player demo; `Icon` has
      since shipped and is now the thing being replaced, so that avoidance no longer applies.
      Register `texture_sheet` in `assets.ron` `textures:` (already placed at the shared path, no
      new art to produce/source this time). Measured sub-rects for the shipped sheet:
      `fill_rect: (0,0,48,17)`, `empty_rect: (0,17,48,15)`, `slice_border: (8,8,8,8)` (see the
      per-row alpha-profile measurement in Approach above). Run
      `python tools/asset_checker/check.py` after editing `assets.ron`.
- [ ] Docs — new `WorldStatBarStyle::Textured` section in `docs/20_data_formats.md`: fields table
      (the two coordinate spaces called out explicitly; `texture_sheet`/`fill_rect`/`empty_rect`
      anchored to the catalog `textures:` convention and the single-sheet-two-frames layout;
      the `fill_color`/`bg_color`/`color_bands`-tint note incl. the "set to white for pre-coloured
      art" guidance); RON example; the low-fill cap-scaling behavior note; split-screen behavior
      (duplicates from day one). Update the two summary tables (lines ~1724 and the style-list
      intro at ~3280) to list `Textured` as a fourth style (alongside the now-shipped `Icon`).
      Update `crates/ironhold_core/src/CLAUDE.md`'s stat-widget / split-screen notes to mention the
      `Textured` style + its update system, and to note the player `world_stat_bar` swapped from
      `Icon` to `Textured` in `3rd_person_game_demo`.
- [ ] WASM dev build + `python test_web.py` — confirm the standard 2D sprite pipeline compiles/
      warms without a first-draw stall on the player's textured bar in `3rd_person_game_demo`.
      `Icon` already shipped and proved this same sprite pipeline in the same project (`672d003`),
      so this is a low-risk confirmation, not first-use.

## Open questions
- **None blocking.** Rendering (sliced `Sprite` + `custom_size.x` against a cropped `Sprite.rect`,
  verified against `bevy_sprite_render-0.18.0` source), schema (separate `Textured` variant reusing
  shared colour fields, corrected to a single-sheet `texture_sheet`/`fill_rect`/`empty_rect` design
  once the actual art was supplied), split-screen (day-one rank duplication), and scope are all
  resolved above.
- **WASM sprite-pipeline risk** — was shared with `Icon`; now resolved, since `Icon` shipped first
  and already proved the sprite pipeline in `3rd_person_game_demo` itself. The `test_web.py` gate
  remains as ordinary regression coverage, not a first-use risk mitigation.
- **`SliceScaleMode::Tile` for patterned middles** — deliberately deferred (see Out of scope); a
  one-field future addition, not an open question blocking v1.
- **Shared scaffolding across `Pixel`/`Icon`/`Textured`** — all three now share the anchor + rank
  loop in `spawn_world_stat_bar_widget`; their child primitives (Mesh2d vs Sprite-atlas vs
  Sprite-sliced) diverge enough that further abstraction is premature. Revisit only once all three
  have shipped and can be compared directly (same conclusion `world_icon_stat_bar.md` reached).

## Acceptance criteria
- Given `world_stat_bar: (stat_key: "...", style: Textured(texture_sheet: "...", fill_rect: ...,
  empty_rect: ...))`, when the tracked stat is at 60% of its range, then the fill sprite's visible
  width is ~60% of the bar width and its rounded caps render undistorted (9-slice), over a static
  full-width empty track sprite.
- Given the same bar as the stat changes (`ModifyStat`/`SetStat`), when the next frame runs, then
  the fill width updates smoothly to the new ratio, with the fill left-aligned (grows/recedes from
  the right end) and change-detection guarding every write.
- Given `color_bands` set, when the ratio crosses a threshold, then the fill sprite's `color` tint
  updates to the matching band colour (highest `above_ratio` ≤ ratio wins), identically to `Pixel`.
- Given a split-screen scene with a `Textured` bar on any entity visible in 2+ active viewports,
  when it renders, then it appears correctly in every one of them — **from this feature's first
  release**, with the `TextureSlicer` + image handles shared (cloned, not reloaded) across ranks.
- Given a non-split scene, when a `Textured` bar spawns, then exactly one anchor + two `Sprite`
  children are created (regression parity with `Pixel`'s single-instance behavior).
- Given an existing project with no `Textured` bars, when this feature ships, then all existing
  `Ascii`/`Pixel`/`Icon`/`style`-less bars are byte-for-byte unaffected (purely additive enum
  variant).
- RON validation: parse/defaults tests pass for the `Textured` variant, matching existing
  `Ascii`/`Pixel`/`Icon` coverage; `cargo check -p ironhold_cli` stays green.
- The `3rd_person_game_demo` player's `world_stat_bar` renders as the rounded textured health bar
  (replacing the `Icon` hearts bar), using the real `rounded-healthbar-texture-sheet.png` art —
  not a placeholder or a stretched non-sliced texture.
