# Feature: Three-Channel Icon Masking

_Status: Draft_
_Planned at: `0f8560b` (2026-06-22)_

## What

Let an icon texture encode up to three independent visual regions in its R, G, and B
channels, each recolored by a designer-specified RGBA in RON. The channel value (0..1)
acts as the coverage/alpha of that region's color; the texture's own alpha stays as the
overall transparency mask. So a single grayscale-channel-packed icon can render, e.g., a
flame body in the R channel (colored orange) and a glow in the G channel (colored yellow),
with one shared sprite cell.

Two render paths must coexist:

1. **Plain tint mode (existing):** `icon_color: (r, g, b, a)` — one multiplicative tint over
   the icon. Stays exactly as today for white-on-transparent icons. No shader.
2. **Three-channel mask mode (new):** `icon_colors: [(r,g,b,a), (r,g,b,a), (r,g,b,a)]` —
   opt-in via the new field. When present, the slot's icon renders through a custom
   `UiMaterial` (WGSL) that does per-channel color replacement.

_Prerequisite: `planning/features/icon_washed_out_fix.md` (doc fix, ships first)._

## Why

The icon system currently only supports a single multiplicative tint (`ImageNode.color`).
That is fine for monochrome icons but cannot express multi-tone icons (a colored body + a
colored accent) from one sprite cell. Designers today must author one fully-colored cell per
variant. Channel packing lets one grayscale-packed cell drive many recolored variants from
RON — the data-driven philosophy applied to icon art. It also gives us a deliberate place to
fix the washed-out white-icon problem, which is a multiplicative-blend artifact, not a content
bug.

## Approach

### Schema changes

Add an **optional** `icon_colors` field alongside the existing `icon_color` on the three
icon-bearing defs. `icon_color` is retained verbatim; `icon_colors` is the opt-in trigger for
mask mode. Both being set is a validation warning (mask mode wins).

```rust
// schema/scene_v2.rs — ActionSlotDef
/// Three-channel mask mode. When set, the icon's R/G/B channels are treated as
/// independent coverage masks, each replaced by the corresponding color here.
/// `[r_color, g_color, b_color]`. Only `.rgb` of each entry is used for the blend;
/// the texture's own alpha controls transparency. Mutually exclusive with `icon_color`
/// (mask mode wins if both are set). Omit for the default single-tint path.
#[serde(default)]
pub icon_colors: Option<[(f32, f32, f32, f32); 3]>,
```

Identical field added to:
- `schema/scene_v2.rs::ActionSlotDef` (already has `icon_color`)
- `schema/items.rs::ItemDef` (already has `icon_color`)

**`ShopItemDef` does not exist** — shop stock is `MerchantDef.stock: Vec<ShopEntry>`, and each
`ShopEntry` references an `ItemDef` by `item_key`. Shop rows therefore inherit `icon_colors`
from the item automatically; **no shop-specific schema change is needed**. (The original task
named `ShopItemDef`; the actual type is `ShopEntry` + `ItemDef`.) The shop panel is currently
display-only and does not render per-row atlas icons with independent tints, so it only needs
to honor the item's mode if/when it gains icons.

Container slots render item icons via `ItemDef` too, so they inherit `icon_colors` for free
once `inventory.rs` (which serves both inventory and container panels) handles mask mode.

### The load-bearing risk: UiMaterial + texture atlas

**Bevy 0.18 `UiMaterial` (`MaterialNode<M>`) does NOT use `ImageNode`'s built-in
`TextureAtlas` UV-slicing.** `MaterialNode` and `ImageNode` are different UI render paths. A
`UiMaterial` receives the node's full `[0,1]` UV in `UiVertexOutput.uv` and binds its own
textures; it has no concept of `TextureAtlas { layout, index }`. This is the single biggest
implementation risk and it is real, not hypothetical.

**Consequence:** the atlas cell selection that `ImageNode` does for us today must be done
manually inside the icon shader. We must pass the cell's UV sub-rect as a uniform and remap the
incoming UV:

```
cell_uv = uv_rect.xy + uv * uv_rect.zw     // xy = cell origin, zw = cell size, in [0,1]
```

The cell rect is computed at spawn time from `icon_index`, `icon_cols`, `icon_rows`,
`icon_cell_size` (the same grid math `TextureAtlasLayout::from_grid` does), normalized against
the sheet's pixel dimensions. The sheet image is bound directly to the material as
`#[texture]` + `#[sampler]` rather than via a shared `TextureAtlasLayout`.

This means mask-mode slots **bypass the `TextureAtlas` path entirely** and own their UV math.
Plain-tint slots keep using `ImageNode` + `TextureAtlas` unchanged. The two paths diverge at
spawn time on whether `icon_colors` is `Some`.

### Custom UiMaterial: `IconMaskMaterial`

Follows the established embedded-UiMaterial pattern (`RadarMaterial` in `stat_radar.rs` is the
precedent — see the five-touchpoint pattern). 16-byte-aligned `vec4`-only uniform.

```rust
// capabilities/icon_mask.rs
pub const ICON_MASK_SHADER_HANDLE: Handle<Shader> =
    uuid_handle!("69636f6e-6d61-4736-b96b-000000000001"); // pick a fresh UUID

#[derive(ShaderType, Clone, Default, PartialEq)]
pub struct IconMaskUniforms {
    pub color_r:  Vec4,   // .rgb used; .a ignored in blend
    pub color_g:  Vec4,
    pub color_b:  Vec4,
    pub uv_rect:  Vec4,   // xy = cell origin (uv), zw = cell size (uv)
}

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct IconMaskMaterial {
    #[uniform(0)] pub uniforms: IconMaskUniforms,
    #[texture(1)] #[sampler(2)] pub sheet: Handle<Image>,
}

impl UiMaterial for IconMaskMaterial {
    fn fragment_shader() -> ShaderRef { ICON_MASK_SHADER_HANDLE.into() }
}
```

Shader embed + plugin exactly mirror `setup_stat_radar_shader` / `StatRadarPlugin`:
`include_str!("../../../../assets/shared/shaders/custom_icon_mask.wgsl")` registered at the
stable handle in a `Startup` system, `UiMaterialPlugin::<IconMaskMaterial>::default()` added in
the plugin, plugin registered in `lib.rs`.

### WGSL fragment shader (`assets/shared/shaders/custom_icon_mask.wgsl`)

```wgsl
#import bevy_ui::ui_vertex_output::UiVertexOutput

struct IconMaskUniforms {
    color_r: vec4<f32>,
    color_g: vec4<f32>,
    color_b: vec4<f32>,
    uv_rect: vec4<f32>,   // xy = cell origin, zw = cell size (both in [0,1] sheet UV)
};

@group(1) @binding(0) var<uniform> material: IconMaskUniforms;
@group(1) @binding(1) var sheet_tex: texture_2d<f32>;
@group(1) @binding(2) var sheet_sampler: sampler;

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    // Remap node UV [0,1] into this icon's atlas cell.
    let cell_uv = material.uv_rect.xy + in.uv * material.uv_rect.zw;
    let tex = textureSample(sheet_tex, sheet_sampler, cell_uv);

    // Each channel is an independent coverage mask, recolored by its color.rgb.
    // Additive composite of the three colored regions.
    let rgb = tex.r * material.color_r.rgb
            + tex.g * material.color_g.rgb
            + tex.b * material.color_b.rgb;

    // Texture alpha is the only transparency control.
    return vec4<f32>(rgb, tex.a);
}
```

Notes on the math:
- This is **additive** across channels. If two channels overlap on a pixel their colors sum
  (can exceed 1.0 → clips bright). That is the intended "glow over body" behavior. If overlap
  is undesirable, designers author non-overlapping channel masks.
- A pure white single-color icon `(1,1,1,a)` rendered in mask mode with
  `icon_colors: [(c,c,c,1),(0,0,0,1),(0,0,0,1)]` would NOT reproduce plain-tint behavior
  (all three channels are 1 for white, so R+G+B colors all sum). **This is why we keep the
  plain-tint `ImageNode` path for monochrome icons** — mask mode assumes channel-packed art,
  not white art. Do not route white-on-transparent icons through mask mode.

### Does the existing `icon_color` tint path move to the shader?

**No.** The plain-tint path stays on `ImageNode.color` + `TextureAtlas`. Rationale:

- It already works and uses Bevy's atlas slicing — moving it to the shader would force us to
  reimplement atlas UV math for the common case and add a material per slot, costing pipeline
  compiles on WASM for zero behavioral gain.
- Only `icon_colors`-bearing slots pay the `UiMaterial` cost. This keeps the blast radius
  small and the WASM pipeline-compile count proportional to actual mask-mode usage.

### Scene loader / inventory / shop spawn changes

- **`SceneMaterialParams`** (`runtime/scene_manager/mod.rs`): add
  `pub icon_mask: ResMut<'w, Assets<IconMaskMaterial>>` (it is a `SystemParam`, so no 16-param
  pressure — same as `RadarMaterial` was added).
- **ActionBar (`scene_loader.rs` ~1766–1816):** branch on `slot.icon_colors`. If `Some`,
  compute the cell `uv_rect` from `icon_index`/`icon_cols`/`icon_rows`/`icon_cell_size` +
  resolved sheet image dimensions, mint an `IconMaskMaterial` handle **before** the
  `with_children` closure, and spawn the icon child as `(Node, MaterialNode(handle))` instead
  of `(Node, ImageNode{texture_atlas,...})`. Else keep the current `ImageNode` spawn.
- **Inventory & container (`inventory.rs` update systems):** the icon node is spawned once and
  updated each frame as the slot contents change. Mask mode complicates this because the
  material (uv_rect + colors) depends on the item in the slot, which changes at runtime. Two
  options:
  - **(A) Per-slot dual nodes:** spawn both an `ImageNode` icon child (hidden) and a
    `MaterialNode<IconMaskMaterial>` icon child (hidden); the update system shows whichever
    matches the current item's mode and writes the appropriate fields (atlas index for tint
    mode; `uv_rect` + colors for mask mode). Change-guarded writes.
  - **(B) Swap components at runtime:** insert/remove `MaterialNode`/`ImageNode` on a single
    icon entity when the item's mode changes. Avoids a second node but churns archetypes.
  - **Recommendation: (A)** — archetype churn on inventory updates is worse for frame pacing
    than one extra hidden node per slot; and it keeps the update system's writes simple and
    guardable. Flag (A) vs (B) as an open question for play-test feedback.
- **Shop:** no change needed now (display-only, no atlas icons). When shop rows gain icons,
  reuse the inventory dual-node approach since rows are also `ItemDef`-driven.

### `LoadedAssetCatalog` discipline

The sheet image is still resolved through the catalog (`asset_catalog.textures.get(key)`) — no
hardcoded path. The shader is engine-owned and embedded via `include_str!` at a stable handle,
per the engine-shader rule in `crates/ironhold_core/src/CLAUDE.md` — it is **not** a
designer-authored `assets.ron` shader path.

## Tasks

- [ ] Add `icon_colors: Option<[(f32,f32,f32,f32); 3]>` to `ActionSlotDef` and `ItemDef`
      (`#[serde(default)]`, doc comments, mutual-exclusion note vs `icon_color`).
- [ ] Validation: warn (don't error) when both `icon_color` and `icon_colors` are set on the
      same slot/item; mask mode wins. Add to the relevant `validate()` paths and/or CLI lint.
- [ ] New capability `capabilities/icon_mask.rs`: `IconMaskMaterial` (+ `IconMaskUniforms`),
      `impl UiMaterial`, `ICON_MASK_SHADER_HANDLE`, `setup_icon_mask_shader` (Startup,
      `include_str!`), `IconMaskPlugin` (adds `UiMaterialPlugin::<IconMaskMaterial>`).
- [ ] WGSL shader `assets/shared/shaders/custom_icon_mask.wgsl` (per above). Bind group 1:
      uniform(0), texture(1), sampler(2).
- [ ] Add `icon_mask: ResMut<Assets<IconMaskMaterial>>` to `SceneMaterialParams`.
- [ ] ActionBar spawn: branch on `icon_colors`; compute `uv_rect`; spawn `MaterialNode` icon
      child for mask mode, `ImageNode` otherwise. Pre-create handles before `with_children`.
- [ ] Inventory/container: implement dual-node approach (A); update systems show the correct
      icon child and write mask uniforms (`uv_rect` + colors) with change-guards.
- [ ] Register `IconMaskPlugin` and any new update system in `lib.rs`.
- [ ] `cargo check -p ironhold_cli` and `query actions`/`query` spot-checks (schema changed).
- [ ] Tests: `ron_validation` (mask-mode RON parses), an integration test that a slot/item with
      `icon_colors` spawns a `MaterialNode<IconMaskMaterial>` child (mirror existing StatRadar
      `MaterialNode` test if present).
- [ ] Add a mask-mode example to `3rd_person_game_demo` (one skill slot + one item) so it is
      exercised by the browser baseline suite.
- [ ] Docs: `20_data_formats.md` (ActionSlotDef + ItemDef `icon_colors`), `25_custom_shaders.md`
      (new engine UiMaterial), and the icon-system note in `crates/ironhold_core/src/CLAUDE.md`.
- [ ] WASM dev build + size check; verify the new pipeline compiles on WebGPU (UiMaterial is
      strictly validated in web builds — test via `python test_web.py`).

## Out of scope

- Animated icons (flipbook / per-frame UV stepping on UI icons).
- More than three mask layers, or alpha-channel-as-fourth-mask (alpha stays transparency).
- Non-additive channel compositing modes (max/over/screen) — additive only in v1.
- 3D / world-space icon rendering (this is UI `UiMaterial` only).
- Moving the plain-tint `icon_color` path onto the shader.
- Shop-row icon rendering (shop is display-only today; revisit when it gains atlas icons).
- Per-channel independent alpha (the color `.a` is intentionally ignored in the blend).

## Open questions

- Inventory mask mode: dual hidden nodes (A) vs runtime component swap (B)? Default plan: (A).
  Confirm during play-test that the extra hidden node per slot is acceptable.
- Should both-fields-set be a hard validation error instead of a warning? Default: warning,
  mask wins — keeps existing RON forward-compatible.
- Do we want a CLI `query` surface for icon mode (which slots/items use mask mode)? Probably a
  `--strict` lint nicety, not v1.
- Color space: keep `icon_colors` linear (consistent with `icon_color`) — confirmed default. Do
  we ever want sRGB authoring? Only if the washed-out fix proves insufficient.

## Acceptance criteria

- **Given** an `ActionBar` slot whose sheet cell packs a flame body in R and a glow in G, and
  RON:
  ```ron
  ActionSlotDef(
      key: "1",
      icon_index: 3,
      icon_colors: Some([
          (1.0, 0.45, 0.05, 1.0),   // R channel → orange flame body
          (1.0, 0.9,  0.2,  1.0),   // G channel → yellow glow
          (0.0, 0.0,  0.0,  1.0),   // B channel → unused (black, contributes nothing)
      ]),
      do_actions: [ /* ... */ ],
  )
  ```
  **when** the action bar renders, **then** the slot shows an orange flame with a yellow glow
  from a single grayscale-packed sprite cell, the transparent background stays transparent
  (driven by the texture's alpha), and no Rust changes were needed.
- **Given** an `ItemDef` with **no** `icon_colors` (only `icon_index`, optionally
  `icon_color`), **when** it appears in an inventory or container slot, **then** it renders
  exactly as today via `ImageNode` + `TextureAtlas` (no regression, no `UiMaterial`).
- **Given** a slot/item with both `icon_color` and `icon_colors` set, **when** the project is
  validated, **then** a warning is emitted and mask mode is used at runtime.
- **Given** a WASM release build, **when** the mask-mode example scene loads in the browser,
  **then** the `IconMaskMaterial` pipeline compiles without WebGPU validation errors and the
  icon renders identically to native.
