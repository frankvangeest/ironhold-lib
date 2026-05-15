# Feature: World-space stat bar — Pixel style

_Status: Ready_
_Planned at: `2015a1e` (2026-05-15)_

## What

Add a `Pixel` rendering mode to the existing `world_stat_bar` field on `PrefabDef`. Designers
pick a style via the `style` field; the same shared properties (`stat_key`, `offset`,
`fill_color`, `bg_color`, `color_bands`) apply to all styles. The field name stays
`world_stat_bar` — nothing new to discover.

```ron
// Minimal ASCII — unchanged, style defaults to Ascii
world_stat_bar: ( stat_key: "{self}.health" )

// ASCII with explicit cells
world_stat_bar: (
    stat_key: "{self}.health",
    style: Ascii( cells: 10, font_size: 14.0 ),
)

// Pixel bar — polished, fixed screen-pixel size
world_stat_bar: (
    stat_key: "{self}.health",
    fill_color: (0.15, 0.85, 0.15, 1.0),
    bg_color:   (0.20, 0.05, 0.05, 0.85),
    style: Pixel(
        size:         (64.0, 8.0),   // screen pixels (width, height)
        border:       1.5,           // screen pixels; 0.0 = no border sprite
        border_color: (0.05, 0.05, 0.05, 1.0),
    ),
)

// Pixel bar with colour bands
world_stat_bar: (
    stat_key: "{self}.health",
    // Each entry: (min_ratio_for_this_color, rgba).
    // Highest min_ratio ≤ current fill ratio wins.
    color_bands: [
        (0.0, (0.85, 0.12, 0.12, 1.0)),  // ratio >= 0.0 (overridden above 0.3)
        (0.3, (0.95, 0.75, 0.10, 1.0)),  // ratio >= 0.3 (overridden above 0.6)
        (0.6, (0.15, 0.85, 0.15, 1.0)),  // ratio >= 0.6
    ],
    style: Pixel( size: (64.0, 8.0) ),
)
```

**Note:** Pixel bar size is in screen pixels and stays constant regardless of camera distance.
For best results use with a roughly fixed-distance camera. Depth-scaled pixel bars are planned
for a future release (see Open questions).

## Why

The ASCII bar is recognisable as a debug artefact. Games beyond the prototype phase need a
pixel bar they can ship without replacing their entire UI stack. Unifying under one field with
a `style` discriminator also sets up `Icon` mode (row of per-cell sprites) cleanly with no
additional field proliferation.

## Approach

### Schema refactor (`catalog.rs`)

`WorldStatBarDef` is restructured. Shared fields stay at the top level; style-specific fields
move into the `WorldStatBarStyle` enum variants. Existing RON that doesn't set `cells` or
`font_size` explicitly requires **no changes** — `style` defaults to `Ascii` with its own
defaults.

```rust
/// World-space stat bar above an entity. Visual mode is chosen via `style`.
/// `{self}` in `stat_key` is resolved at scene load.
/// Shared fields (`fill_color`, `bg_color`, `color_bands`) apply to all styles.
/// Single bar per entity — see open question for multi-bar use cases.
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct WorldStatBarDef {
    /// Stat key — e.g. `"{self}.health"` (entity-local) or `"global_mana"` (global).
    pub stat_key: String,
    /// World-space offset from the entity's origin in metres. Default: `(0.0, 2.8, 0.0)`.
    #[serde(default = "default_world_bar_offset")]
    pub offset: (f32, f32, f32),
    /// Fill base colour (RGBA linear). Used when `color_bands` is absent or no band matches.
    /// Default: bright green `(0.15, 0.85, 0.15, 0.95)`.
    #[serde(default = "default_world_bar_fill_color")]
    pub fill_color: (f32, f32, f32, f32),
    /// Background / track colour (RGBA linear). Default: dark red-brown `(0.25, 0.08, 0.08, 0.75)`.
    #[serde(default = "default_world_bar_bg_color")]
    pub bg_color: (f32, f32, f32, f32),
    /// Threshold-based fill colour overrides. Each entry: `(min_ratio, rgba)`.
    /// The entry with the highest `min_ratio` ≤ current fill ratio is selected.
    /// Example: `[(0.0, red), (0.3, yellow), (0.6, green)]`
    #[serde(default)]
    pub color_bands: Vec<(f32, (f32, f32, f32, f32))>,
    /// Visual rendering mode. Default: `Ascii` — existing bars require no `style` field.
    #[serde(default)]
    pub style: WorldStatBarStyle,
}

/// Visual mode for `WorldStatBarDef`.
#[derive(Deserialize, Debug, Clone, Default)]
pub enum WorldStatBarStyle {
    /// ASCII character bar (`=` fill on space track). Default mode.
    #[default]
    Ascii(AsciiBarStyle),
    /// Pixel-rendered sprite-quad bar. Fixed screen-pixel size at all camera distances.
    Pixel(PixelBarStyle),
    // Icon(IconBarStyle),  // planned — row of per-cell sprites
}

/// Style parameters for `WorldStatBarStyle::Ascii`.
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct AsciiBarStyle {
    /// Total character cells. Practical range 1–32. Default: 10.
    #[serde(default = "default_world_bar_cells")]
    pub cells: u8,
    /// Font size in screen pixels. Default: 14.
    #[serde(default = "default_world_bar_font_size")]
    pub font_size: f32,
}

impl Default for AsciiBarStyle {
    fn default() -> Self { Self { cells: default_world_bar_cells(), font_size: default_world_bar_font_size() } }
}

/// Style parameters for `WorldStatBarStyle::Pixel`.
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct PixelBarStyle {
    /// Bar dimensions in screen pixels `(width, height)`. Clamped to minimum `(1.0, 1.0)`.
    /// Size is constant at all camera distances — no depth scaling in v1.
    /// Default: `(64.0, 8.0)`.
    #[serde(default = "default_pixel_bar_size")]
    pub size: (f32, f32),
    /// Border thickness in screen pixels. `0.0` disables the border sprite.
    /// Clamped to `[0.0, height / 2.0]`. Default: `1.5`.
    #[serde(default = "default_pixel_bar_border")]
    pub border: f32,
    /// Border quad colour (RGBA linear). Default: near-black `(0.05, 0.05, 0.05, 1.0)`.
    #[serde(default = "default_pixel_bar_border_color")]
    pub border_color: (f32, f32, f32, f32),
}

impl Default for PixelBarStyle {
    fn default() -> Self {
        Self {
            size: default_pixel_bar_size(),
            border: default_pixel_bar_border(),
            border_color: default_pixel_bar_border_color(),
        }
    }
}
```

Remove the now-redundant top-level `cells`, `font_size` fields from `WorldStatBarDef`.
Keep `default_world_bar_cells` and `default_world_bar_font_size` helpers — they move to
`AsciiBarStyle`. Add `default_pixel_bar_*` helpers.

### Rendering strategy (Pixel mode)

Reuse `WorldLabel + Sprite` layer. Three `Sprite` quads share the same `WorldLabel`
(`tracked_entity`, `offset`, `depth_scale: None`). `world_label_screen_pos_system` repositions
any `WorldLabel` entity each frame regardless of whether it carries `Text2d` or `Sprite`.

Border and Background are static after spawn; only Fill width and color change per frame.

| Z | Sprite | Size | Color | Mutable? |
|---|--------|------|-------|---------|
| 1.0 | Border | `(w + 2b, h + 2b)` | `border_color` | No |
| 2.0 | Background | `(w, h)` | `bg_color` | No |
| 3.0 | Fill | `(ratio×w, h)` | fill / band | Yes |

Fill uses `Anchor::CenterLeft` positioned at `x = -(w / 2.0)` so it grows rightward.
Border sprite is omitted entirely when `border <= 0.0`.

### Runtime components (`stat_display.rs`)

Existing `WorldStatBarFillMarker` is unchanged — still drives ASCII mode.

New marker for Pixel mode:

```rust
/// Marker on the fill sprite of a Pixel-mode `WorldStatBarDef`.
/// `world_pixel_bar_update_system` reads the stat and updates `Sprite.custom_size.x` + color.
#[derive(Component, Clone)]
pub struct WorldPixelBarFillMarker {
    pub stat_key: String,
    pub full_width: f32,
    pub fill_color: (f32, f32, f32, f32),
    pub color_bands: Vec<(f32, (f32, f32, f32, f32))>,
}
```

New update system `world_pixel_bar_update_system`:
- Queries `(&WorldPixelBarFillMarker, &mut Sprite)`.
- Same `resolve_stat` call as `world_stat_bar_update_system`.
- Guards `sprite.custom_size.x` and `sprite.color` writes for change-detection.
- Unresolved `stat_key` → zero fill + `warn!` guarded by `cfg!(debug_assertions)`.

### Scene loader (`scene_loader.rs`)

The existing `pending_world_bars` collection continues unchanged. After collecting, the spawn
block matches on `wb.style`:

```
Ascii => existing two-Text2d-entity path (unchanged)
Pixel => new three-Sprite-entity path:
    - clamp size.x, size.y to ≥ 1.0
    - clamp border to [0.0, size.y / 2.0]
    - spawn Border sprite (skip when border ≤ 0.0) at Z=1
    - spawn Background sprite at Z=2
    - spawn Fill sprite (Anchor::CenterLeft, x = -w/2) at Z=3 + WorldPixelBarFillMarker
```

All three pixel sprites share the same `WorldLabel` with `depth_scale: None`.

### Registration (`lib.rs`)

Add `world_pixel_bar_update_system` to the same `.add_systems(Update, (...))` call that has
`world_stat_bar_update_system`.

Export `WorldPixelBarFillMarker` and `world_pixel_bar_update_system` from `capabilities/mod.rs`.

### Migration of existing ASCII bars (`primitive_world`)

Existing `world_stat_bar` entries that specify top-level `cells` or `font_size` must move
those fields into `style: Ascii(...)`. Run `rg "world_stat_bar" assets/` to find all
instances before coding.

The `attack_dummy` prefab is being upgraded to Pixel mode in this PR, so its ASCII entry is
replaced entirely. Any remaining ASCII bars (e.g. on `goblin_guard`) only need updating if
they set `cells` or `font_size` explicitly — otherwise they continue to work with no changes.

### Demo (`primitive_world`)

Replace `attack_dummy`'s `world_stat_bar` ASCII entry with Pixel:

```ron
// Bar is fixed screen-pixel size — constant at all camera distances.
world_stat_bar: (
    stat_key:   "{self}.health",
    fill_color: (0.15, 0.85, 0.15, 0.95),
    bg_color:   (0.20, 0.05, 0.05, 0.85),
    color_bands: [
        (0.0, (0.85, 0.12, 0.12, 1.0)),
        (0.3, (0.95, 0.75, 0.10, 1.0)),
        (0.6, (0.15, 0.85, 0.15, 1.0)),
    ],
    style: Pixel( size: (48.0, 6.0) ),
),
```

## Tasks

- [ ] Schema — refactor `WorldStatBarDef` in `catalog.rs`: move `cells`/`font_size` into `AsciiBarStyle`; add `WorldStatBarStyle` enum, `AsciiBarStyle`, `PixelBarStyle` (each with `#[serde(deny_unknown_fields)]`, full `///` doc comments, `Default` impls, and `default_*` helpers); add `style: WorldStatBarStyle` field to `WorldStatBarDef`
- [ ] Migrate existing RON — run `rg "world_stat_bar" assets/` and update any entry that specifies top-level `cells`/`font_size` to `style: Ascii( cells: ..., font_size: ... )`
- [ ] Runtime marker — add `WorldPixelBarFillMarker` component to `stat_display.rs`
- [ ] Update system — add `world_pixel_bar_update_system` to `stat_display.rs` (zero-fill + debug warn on unresolved key; change-detection guards on width and color writes)
- [ ] Scene loader — extend the `pending_world_bars` spawn block with a `match wb.style` arm: Ascii → existing path; Pixel → 3-sprite path with size/border clamps
- [ ] Export — add `WorldPixelBarFillMarker` and `world_pixel_bar_update_system` to `capabilities/mod.rs`; register in `lib.rs`
- [ ] Demo — replace `attack_dummy` ASCII `world_stat_bar` with Pixel style in `primitive_world/prefabs/prefabs.ron`
- [ ] Tests — `test_world_stat_bar_pixel_style_parses`, `test_world_stat_bar_pixel_style_defaults`, `test_world_stat_bar_ascii_style_unchanged`, `test_world_stat_bar_rejects_unknown_fields` in `ron_validation.rs`
- [ ] Docs — update `WorldStatBarDef` section in `docs/20_data_formats.md`: add `style` field row, add Pixel style sub-table (size/border/border_color units in screen px), add minimal and full Pixel examples, add v1 depth-scaling limitation callout
- [ ] WASM rebuild — `wasm-pack build` and verify no WebGPU errors in `python test_web.py`
- [ ] Screenshot — regenerate `screenshot_baselines/scenes/primitive_world_main.png` via `python test_web.py --update-baseline primitive_world` and visually confirm the pixel bar renders above the attack dummy

## Open questions

- **Depth scaling (v2?)** `WorldLabel.depth_scale` drives font size, not sprite size. Supporting
  depth-scaled pixel bars needs a `base_size: Vec2` on the marker + a separate scaling path in
  `world_label_screen_pos_system`. Defer unless needed for first release.

- **Multiple bars on one entity?** A single `Option<WorldStatBarDef>` cannot stack health + mana
  bars. `Vec<WorldStatBarDef>` is the obvious fix but adds spawner complexity. Leave as `Option`
  for v1; document in the field's doc-comment; add an icebox backlog entry.

- **Fill sprite Z vs. damage popups.** Pixel bar sprites claim Z = 1.0 / 2.0 / 3.0. Damage
  popup `WorldLabel + Text2d` entities use Z = 10.0 (`action_executor.rs`). Confirm during
  manual testing that popups render in front of, not behind, bar sprites.

- **Icon style (v3?)** `WorldStatBarStyle::Icon(IconBarStyle)` — row of per-cell sprites
  (hearts, shields, custom catalog icons). Requires design for asset reference format and
  partial-cell handling. Add as an Icebox item after Pixel ships.

- **`visible_when` condition?** Most games want to hide a full-health bar. Could be
  `visible_when: Option<String>` with a simple predicate (`"ratio < 1.0"`). Icebox for now.

## Acceptance criteria

- Given `world_stat_bar: ( stat_key: "{self}.health" )` (no `style` field), the bar renders as
  ASCII with default cells/font — existing behavior is unchanged.
- Given `world_stat_bar: ( stat_key: "{self}.health", style: Pixel( size: (64.0, 8.0) ) )`, a
  filled rectangle bar appears above the entity.
- Fill width correctly reflects the current stat ratio and updates on the next frame when
  `ModifyStat` fires.
- When `color_bands` are set, fill colour changes at the correct thresholds.
- When `border <= 0.0`, no border sprite is spawned.
- When `stat_key` cannot be resolved, the bar renders at zero fill and a `warn!` appears in
  debug builds.
- `size.x / size.y = 0.0` and `border > size.y / 2.0` are clamped — no panic.
- Existing RON files with no explicit `style` field continue to parse and render correctly.
- RON validation: parse, defaults, ASCII-unchanged, and unknown-field-rejection tests pass.
- `python test_web.py` passes with no new WebGPU errors.
- Screenshot baseline regenerated and pixel bar is visible in `primitive_world_main.png`.
