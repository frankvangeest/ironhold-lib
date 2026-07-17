# Feature: World-space Textured Stat Bar (`WorldStatBarStyle::Textured`)

_Status: Ready_
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
production-quality, split-screen-complete), `Icon` (discrete pips/hearts, planned), and `Ascii`
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

**Two `Sprite` layers per bar** (mirroring `Pixel`'s bg + fill split, minus the border/bg flat
quads — the texture art supplies its own frame and background):
- **Empty/track layer** — sliced `Sprite` using `empty_texture`, `custom_size = (width, height)`,
  **static** (never updated), lower z.
- **Fill layer** — sliced `Sprite` using `fill_texture`, `custom_size = (ratio * width, height)`,
  updated per frame, higher z, **left-aligned by mirroring `Pixel`'s proven translation math**
  (`translation.x = -width/2 + fill_width/2`) so the fill grows from the left edge and its rounded
  right end recedes as the stat drops. (Using the translation shift rather than an `Anchor`
  component sidesteps any anchor-API specifics and reuses a pattern already shipped in
  `world_pixel_bar_update_system`.)

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
/// sprite drawn on top, fill width driven continuously by the stat ratio. Caps/border are part
/// of the art and stay undistorted at every width via Bevy's `SpriteImageMode::Sliced`.
Textured {
    /// Catalog key into `AssetCatalog.textures` — the FULL/fill bar art (9-sliced, drawn on top,
    /// width = ratio * size.0). Same catalog convention as `EffectDef.sprite`, `Icon.icon_sheet`,
    /// and every other texture reference in the engine (key → path → `asset_server.load`).
    fill_texture: String,
    /// Catalog key into `AssetCatalog.textures` — the EMPTY/track art (9-sliced, static, full width,
    /// drawn underneath). The art supplies its own border/background, so `bg_color`/`border` from
    /// other styles have no equivalent here.
    empty_texture: String,
    /// Bar dimensions in **screen pixels** `(width, height)` — same coordinate space as
    /// `Pixel.size`/`Icon.size` (Camera2d, 1 unit = 1 px; constant at all camera distances, no
    /// depth scaling in v1). Clamped to a min of `(1.0, 1.0)`. Default: `(64.0, 12.0)`.
    #[serde(default = "default_textured_bar_size")]
    size: (f32, f32),
    /// 9-slice cap insets in **TEXTURE pixels** `(left, right, top, bottom)` — the fixed
    /// corner/cap regions of the source art that must NOT stretch. Maps directly to
    /// `TextureSlicer.border` (`BorderRect { min_inset: (left, top), max_inset: (right, bottom) }`).
    /// Author this to match the actual cap width/height of your fill/empty art. Default:
    /// `(6.0, 6.0, 6.0, 6.0)`. (Distinct unit from `size`: `size` is on-screen pixels, `slice_border`
    /// is source-image pixels — spelled out in the docs so it isn't mistaken for screen space.)
    #[serde(default = "default_textured_bar_slice")]
    slice_border: (f32, f32, f32, f32),
},
```

**No new colour/tint fields — reuse the shared `WorldStatBarDef.fill_color` + `color_bands`.**
The fill `Sprite.color` multiplies its texture, so the existing shared `fill_color` (and
threshold-based `color_bands`) drive a state tint on the fill sprite exactly like they drive
`Pixel`'s `ColorMaterial.color` today — one colour convention across all styles, no `Textured`-only
field. **Caveat to document:** the shared `fill_color` default is bright green
`(0.15, 0.85, 0.15, 0.95)`, which would *tint* an already-coloured fill texture. A designer using
pre-coloured art should set `fill_color: (1.0, 1.0, 1.0, 1.0)` (white = no tint); a designer using
greyscale/white art can lean on `fill_color`/`color_bands` for green→yellow→red state colouring.
`empty_texture` is never tinted (its own art is authoritative); `bg_color` is ignored by this style.

**`deny_unknown_fields` note (correction vs. the `Icon` plan).** The existing `Ascii` and `Pixel`
enum variants do **not** carry `#[serde(deny_unknown_fields)]` (only the outer `WorldStatBarDef`
struct does). `Textured` should **match the existing variants** (no per-variant attribute) for
consistency — do *not* add `#[serde(deny_unknown_fields)]` to just this one variant. `world_icon_
stat_bar.md`'s schema section claims to add it to `Icon`; that would introduce an inconsistency and
should be corrected there too (logged in Documentation & Planning below).

```ron
// A textured rounded health bar on an enemy — greyscale fill art, coloured by state bands.
world_stat_bar: (
  stat_key: "{self}.health",
  offset: (0.0, 2.6, 0.0),
  color_bands: [
    (0.0, (0.85, 0.12, 0.12, 1.0)),  // < 30% → red
    (0.3, (0.95, 0.75, 0.10, 1.0)),  // ≥ 30% → yellow
    (0.6, (0.15, 0.85, 0.15, 1.0)),  // ≥ 60% → green
  ],
  style: Textured(
    fill_texture:  "hpbar_fill",
    empty_texture: "hpbar_empty",
    size: (72.0, 14.0),
    slice_border: (8.0, 8.0, 6.0, 6.0),
  ),
),

// Pre-coloured art — disable the tint by setting fill_color to white.
world_stat_bar: (
  stat_key: "{self}.health",
  fill_color: (1.0, 1.0, 1.0, 1.0),
  style: Textured( fill_texture: "boss_hp_full", empty_texture: "boss_hp_empty" ),
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

**`StatWidgetSpawnCtx` needs image handles.** The current ctx carries `meshes` + `color_materials`
but no `AssetServer`/`AssetCatalog` — the `Textured` arm needs to resolve two catalog keys to
`Handle<Image>` (`catalog.textures.get(key)` → path → `asset_server.load`, the exact pattern in
`capabilities/particle.rs`). Add the resolved `Handle<Image>` pair (or an `&AssetServer` +
`&AssetCatalog`) to `StatWidgetSpawnCtx`. **Coordinate with `Icon`** — that plan needs the same
extension (sheet image + `TextureAtlasLayout`); do it once. Both scene-load and
`drain_dynamic_stat_ui_system` call sites already have `AssetServer`/catalog in scope. Missing-key
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
- [ ] Schema — `WorldStatBarStyle::Textured { fill_texture, empty_texture, size, slice_border }`
      struct variant in `catalog.rs` (inline fields matching `Ascii`/`Pixel`; **no** per-variant
      `deny_unknown_fields`, matching the existing variants), with `default_textured_bar_size` /
      `default_textured_bar_slice` default fns and full doc comments (esp. the two coordinate
      spaces: `size` = screen px, `slice_border` = texture px; and that `fill_color`/`color_bands`
      tint the fill while `bg_color`/`border` don't apply).
- [ ] `capabilities/stat_display.rs` — `WorldTexturedBarFillMarker` component;
      `world_textured_bar_update_system` (writes `Sprite.custom_size.x` + `Sprite.color`, guarded
      for change-detection, left-align translation, `color_bands`/`fill_color` selection identical
      to the Pixel system); `Textured` arm in `spawn_world_stat_bar_widget`, rank-duplicated, with
      the `TextureSlicer` + both `Handle<Image>` built once and cloned per rank, two `Sprite`
      children (empty static + fill marked) per rank, `LevelEntity` on every child.
- [ ] `StatWidgetSpawnCtx` — add the image-handle resolution path (`&AssetServer` + `&AssetCatalog`,
      or pre-resolved handle pair); wire both call sites (`scene_loader.rs` Phase B loops +
      `drain_dynamic_stat_ui_system`). **Coordinate with `world_icon_stat_bar.md`** — same ctx
      extension; land it once. Missing catalog key → `warn!` once + skip (no fabricated paths).
- [ ] Register `world_textured_bar_update_system` alongside `world_pixel_bar_update_system` in
      `lib.rs`.
- [ ] Tests — parse/defaults tests for the `Textured` variant in `ron_validation.rs` (matching the
      `Ascii`/`Pixel` set: minimal RON parses, defaults resolve, a full RON parses); spawn-behavior
      test (a `Textured` bar spawns exactly one anchor + two `Sprite` children in a non-split scene;
      the fill child carries `WorldTexturedBarFillMarker`); split-screen rank-duplication test
      (`Sprite`/marker counts scale to `MAX_SPLIT_PLAYERS`, and the shared image handles are cloned
      not re-loaded), mirroring the Pixel duplication feature's tests.
- [ ] CLI — `cargo check -p ironhold_cli` (new enum variant must not break `query.rs`); run
      `cargo run -p ironhold_cli -- query <project>` on the demo to confirm the new style surfaces
      and nothing crashes.
- [ ] Demo — add a `Textured` bar to `3rd_person_game_demo`, on an **NPC/enemy** (e.g. the training
      dummy or an orc) tracking `{self}.health`, **not** the player and **not** `local_coop_demo`'s
      per-player bars — the player overhead slot is where `world_icon_stat_bar.md`'s hearts demo
      goes and `local_coop_demo`'s bars are `pixel_world_stat_bar_split_screen_duplication.md`'s;
      using a separate entity avoids colliding with either. **Includes producing/sourcing the actual
      fill + empty bar art** (two 9-sliceable PNGs with clear cap regions), added to `assets.ron`
      `textures:`, following `assets/CLAUDE.md`'s art direction — no shipped texture is a
      9-sliceable bar today. Run `python tools/asset_checker/check.py` after editing `assets.ron`.
- [ ] Docs — new `WorldStatBarStyle::Textured` section in `docs/20_data_formats.md`: fields table
      (the two coordinate spaces called out explicitly; `fill_texture`/`empty_texture` anchored to
      the catalog `textures:` convention; the `fill_color`/`color_bands`-tint note incl. the
      "set to white for pre-coloured art" guidance; `bg_color`/`border` do-not-apply note); RON
      example; the low-fill cap-scaling behavior note; split-screen behavior (duplicates from day
      one). Update the two summary tables (lines ~1724 and the style-list intro at ~3280) to list
      `Textured` as a third style. Update `crates/ironhold_core/src/CLAUDE.md`'s stat-widget /
      split-screen notes to mention the `Textured` style + its update system.
- [ ] WASM dev build + `python test_web.py` — confirm the standard 2D sprite pipeline (first
      `Sprite` use in the engine unless `Icon` landed first) compiles/warms without a first-draw
      stall on the demo NPC's textured bar. If it stalls, see the sequencing mitigation in Approach
      (land `Icon` first to prove the pipeline) — there is no clean 9-sliced degraded fallback.

## Open questions
- **None blocking.** Rendering (sliced `Sprite` + `custom_size.x`, verified against
  `bevy_sprite-0.18.0` source), schema (separate `Textured` variant reusing shared colour fields),
  split-screen (day-one rank duplication), and scope are all resolved above.
- **WASM sprite-pipeline risk** — shared with `Icon`, mitigated by sequencing (`Icon` first proves
  the pipeline) + the `test_web.py` gate; the fallback is weaker than `Icon`'s (no clean 9-sliced
  degraded mode), so if Frank wants zero risk, hold `Textured` until `Icon` has shipped and warmed
  the sprite path. Not a design gap — an implementation-ordering preference for Frank to confirm.
- **`SliceScaleMode::Tile` for patterned middles** — deliberately deferred (see Out of scope); a
  one-field future addition, not an open question blocking v1.
- **Shared scaffolding across `Pixel`/`Icon`/`Textured`** — all three now share the anchor + rank
  loop in `spawn_world_stat_bar_widget`; their child primitives (Mesh2d vs Sprite-atlas vs
  Sprite-sliced) diverge enough that further abstraction is premature. Revisit only once all three
  have shipped and can be compared directly (same conclusion `world_icon_stat_bar.md` reached).

## Acceptance criteria
- Given `world_stat_bar: (stat_key: "{self}.health", style: Textured(fill_texture: "...",
  empty_texture: "..."))`, when the tracked stat is at 60% of its range, then the fill sprite's
  visible width is ~60% of the bar width and its rounded caps render undistorted (9-slice), over a
  static full-width empty track sprite.
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
  `Ascii`/`Pixel`/`style`-less bars are byte-for-byte unaffected (purely additive enum variant).
- RON validation: parse/defaults tests pass for the `Textured` variant, matching existing
  `Ascii`/`Pixel` coverage; `cargo check -p ironhold_cli` stays green.
- The `3rd_person_game_demo` NPC textured bar renders with real, purpose-made 9-sliceable fill +
  empty art — not a placeholder or a stretched non-sliced texture.
