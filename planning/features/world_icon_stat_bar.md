# Feature: World-space Icon Stat Bar (`WorldStatBarStyle::Icon`)

_Status: Ready_
_Planned at: `d80e73b` (2026-07-17)_

**Plan-review note (2026-07-17):** system-architect — Needs minor design work → resolved: found a
materially better answer to this plan's own flagged open question (`Sprite` + `TextureAtlas`,
reusing the exact `TextureAtlas { layout, index }` mechanism already used by
`ItemDef`/`ActionBarDef`, instead of either mesh-based option this plan originally proposed —
folded in below, replacing the whole Rendering section). Also caught two concrete schema bugs
before coding: the `Icon(IconBarStyle)` tuple-variant shape didn't match the plan's own
struct-variant RON example (fixed — `Icon` is now a struct variant inline, like `Ascii`/`Pixel`),
and `size`/`spacing` were documented as world-space metres when they're actually screen pixels
(same coordinate space as `Pixel.size` — fixed). Redesigned the update system to resolve the stat
once per anchor (not once per cell) for a real perf reason: `resolve_stat`'s dotted-key lookup is
O(entities-with-`StatMap`), and per-cell resolution would have paid that cost up to 20x per bar
per frame instead of once. Confirmed the split-screen sequencing dependency on
`pixel_world_stat_bar_split_screen_duplication.md` is soft/organizational (avoiding a merge
collision on the same function), not technical — the `WorldLabelRank`+hierarchy-visibility
mechanism is already proven at rank-0 in shipped Pixel code, so this feature isn't blocked if that
one slips. ux-gamedesigner-reviewer — Needs more design work → resolved: decided round-vs-ceil for
the fill-count computation (documented below, with the 5%/95% edge cases spelled out — a genuine
game-feel choice, not left silent); made the demo task concrete (one project, one prefab, one stat,
plus an explicit sub-task to produce the missing filled/empty pip art asset, since no shipped icon
sheet has one); fixed a sentence that inaccurately compared this feature's whole-cell rounding to
"Pixel's continuous width" (Pixel is continuous, not cell-quantized — the comparison was wrong,
not just imprecise); the Ascii soft-deprecation doc task was added to
`pixel_world_stat_bar_split_screen_duplication.md` instead (whichever feature lands first should
add it — not duplicated here).

## What
A third `world_stat_bar` style — `Icon` — rendering a stat as a row of per-cell icons (hearts,
shields, or any catalog sprite) instead of an Ascii character bar or a solid Pixel fill bar. Each
cell shows either a "filled" or "empty" variant of the designer's chosen icon; the number of
filled cells reflects the stat's current ratio, rounded to the nearest whole cell (no partial-icon
rendering in v1 — see Open questions).

This was flagged as a planned follow-up in the original `world_pixel_stat_bar.md` design doc
("Icon style (v3?) ... Add as an Icebox item after Pixel ships") and has sat in
`planning/backlog.md`'s Icebox as **World-space icon stat bar** since then. Promoted now because
Frank is considering consolidating `world_stat_bar` down to just Pixel (solid fill) + Icon (per-
cell sprites) once both are production-quality, retiring Ascii (see Why).

## Why
Ascii-style bars are explicitly framed, in the engine's own prior design doc, as a prototyping
stopgap ("recognisable as a debug artefact... games beyond the prototype phase need a pixel bar
they can ship without replacing their entire UI stack") — never intended as the long-term look.
Frank's direction: once Pixel has full feature parity with Ascii (split-screen duplication — see
`pixel_world_stat_bar_split_screen_duplication.md`, planned alongside this feature), Ascii becomes
genuinely redundant, and the two production-quality styles a designer would actually want are
Pixel (a solid bar) and Icon (a discrete per-cell display — hearts, ability charges, shield pips).
This plan designs Icon so it launches with full split-screen support from day one, rather than
repeating Pixel's original mistake of shipping single-viewport-only and needing a follow-up fix.

**Removing Ascii itself is explicitly out of scope for this plan** — that's a separate migration
decision (every existing project's `world_stat_bar` RON that omits `style` defaults to Ascii; a
removal needs its own scoped feature with a RON migration pass across every example project, not
bundled into adding a new style). This plan only adds Icon as a third option alongside the existing
two.

## Approach

### Schema — reuse the existing icon-atlas convention, don't invent a new one
The engine already has a consistent icon-referencing pattern used by `ItemDef`, `ActionSlotDef`,
`ActionBarDef`, `InventoryPanelDef`, and `ContainerPanelDef`: a sheet key into
`AssetCatalog.textures`, a grid shape (`icon_cols`/`icon_rows`/`icon_cell_size`), and a flat
row-major `icon_index` (`col + row * icon_cols`). `Icon` style reuses this exactly, rather than a
bespoke `filled_icon`/`empty_icon` pair of separate textures — one designer-facing convention for
"how do I reference an icon" across the whole engine, not two. `Icon` is a **struct variant with
inline fields**, matching `Ascii { cells, font_size }`/`Pixel { size, border, border_color }`
exactly (a tuple variant wrapping a separate struct would not match this plan's own RON example —
system-architect finding):

```rust
Icon {
    /// Catalog key into `AssetCatalog.textures` — the sprite sheet both icon variants come from.
    icon_sheet: String,
    /// Atlas grid shape — same convention as `ActionBarDef`/`ItemDef`.
    #[serde(default = "default_icon_cols")]
    icon_cols: u32,
    #[serde(default = "default_icon_rows")]
    icon_rows: u32,
    #[serde(default = "default_icon_cell_size")]
    icon_cell_size: u32,
    /// Row-major index of the "filled" cell variant (e.g. a solid heart). Same indexing as
    /// `icon_index` elsewhere in the engine (`col + row * icon_cols`) — you author two cells here
    /// instead of one, since a fill bar needs both states (ux-gamedesigner-reviewer: anchor this
    /// naming to the established convention explicitly in the docs, not just in this comment).
    filled_index: u32,
    /// Row-major index of the "empty" cell variant (e.g. a hollow heart outline).
    empty_index: u32,
    /// Total number of cells (pips) the bar represents. Practical range 1–20.
    #[serde(default = "default_icon_cells")]
    cells: u8,
    /// Spacing between cell centres, in **screen pixels** — same coordinate space as
    /// `Pixel.size` (Camera2d is 1 unit = 1 px; anchor offsets/sizes are constant at all camera
    /// distances). Default: `20.0`. (Corrected from an earlier draft that called this world-space
    /// metres — system-architect finding: `world_label_screen_pos_system` positions the anchor in
    /// Camera2d viewport coordinates, so it is pixels, not metres, matching Pixel exactly.)
    #[serde(default = "default_icon_spacing")]
    spacing: f32,
    /// Per-cell size (width, height) in **screen pixels**. Default: `(24.0, 24.0)`.
    #[serde(default = "default_icon_size")]
    size: (f32, f32),
},
```

**Texture dimensions — power-of-2 is recommended, not required.** WebGPU (and WebGL2, and
Vulkan/Metal/D3D12) don't require power-of-2 (POT) texture dimensions for sampling — that was a
WebGL1/OpenGL ES 2.0-era constraint (mipmapping, `REPEAT` wrap mode on old hardware), not a
WebGPU/WebGL2 one. This feature introduces no new atlas-building code — `Icon` reuses the exact
same `TextureAtlasLayout::from_grid` call path `ActionBarDef`/`ItemDef` icon sheets already use in
production, so it inherits whatever dimension behavior those already have (which today is: works
fine at any dimensions, POT or not — confirmed, no POT check exists anywhere in the engine or
`tools/asset_checker/check.py`). The three shipped icon sheets happen to be POT (512×512,
512×512, 256×256) by hand-authored coincidence, not validation. Docs task below should note this
so a designer producing the new filled/empty pip art (see the Demo task) isn't left guessing:
non-POT sheets work, but keeping the sheet POT (matching the other shipped sheets) is recommended
for mip-mapping/compression parity with the rest of the engine's icon atlases.

```ron
// A 5-pip heart bar using a shared item/UI icon sheet.
world_stat_bar: (
    stat_key: "{self}.health",
    style: Icon(
        icon_sheet: "ui_icons",
        icon_cols: 8, icon_rows: 8, icon_cell_size: 64,
        filled_index: 12,  // solid heart cell
        empty_index: 13,   // hollow heart cell
        cells: 5,
    ),
)
```

### Rendering — `Sprite` + `TextureAtlas`, not a custom mesh/material
An earlier draft of this plan proposed either a custom UV-cropped `Mesh2d` or a bespoke UV-offset
`Material2d` — both reinvent something Bevy 0.18 already provides. **`Sprite` carries a
`texture_atlas: Option<TextureAtlas>` field** — the exact same `TextureAtlas { layout, index }`
type the engine's existing UI icon rendering already uses (`ActionSlotDef`/inventory/shop slots),
built from the same `TextureAtlasLayout::from_grid` already used and cached there. `Sprite` renders
on the same Camera2d as the existing Ascii (`Text2d`) and Pixel (`Mesh2d`) world-stat-bar children,
so it drops directly into the same `WorldLabel` anchor + children hierarchy with no new rendering
plumbing (system-architect finding — this is a materially better answer than either option this
plan originally proposed):

| Approach | New pipeline (WASM) | Fill/empty swap | Convention reuse |
|---|---|---|---|
| **`Sprite` + `TextureAtlas`** (v1, this plan) | One standard 2D sprite pipeline (lazy-compiled, warms at scene-load like everything else) | `sprite.texture_atlas.index = ...` (an integer swap) | Exact 1:1 with the existing atlas convention |
| Custom cropped-UV mesh + `ColorMaterial` (fallback, see below) | Zero new pipeline (reuses the already-warm `ColorMaterial` pipeline Pixel bars use) | Swap a per-cell `Mesh2d` handle | Partial — custom UV math, not the atlas API |

Ship `Sprite`+`TextureAtlas` for v1. **Fallback plan, not a blocker:** if `python test_web.py`
reveals an unwarmable sprite-pipeline stall on first Icon-bar draw (this codebase's WASM build has
hit exactly this class of issue before with other new pipelines), fall back to the cropped-UV-mesh
approach — the schema and per-cell update logic below are unaffected either way, only the concrete
child-entity type changes.

**Resolve the stat once per bar (per rank-anchor), not once per cell.** `resolve_stat`'s
entity-local key lookup is O(entities-with-`StatMap`) — paying that cost per cell (up to 20x per
bar per frame) instead of once would multiply an already-linear scan for no reason. Put a single
component on the **anchor**, not a marker on every cell:

```rust
/// On the anchor entity of an Icon-style world_stat_bar. `world_icon_bar_update_system` resolves
/// the stat ONCE per anchor, then walks its children (in spawn order) to set each Sprite's atlas
/// index — not once per cell, since resolve_stat's dotted-key lookup is O(entities-with-StatMap).
#[derive(Component, Clone)]
pub struct WorldIconBar {
    pub stat_key: String,
    pub cells: u8,
    pub filled_index: u32,
    pub empty_index: u32,
}
```

`world_icon_bar_update_system` (structurally parallel to `world_pixel_bar_update_system`): queries
`(&WorldIconBar, &Children)`, resolves the ratio once, computes `filled_count` (see rounding
decision below), then iterates the anchor's `Children` in spawn order (cell 0..cells, spawned in
that order) setting each `Sprite`'s `texture_atlas.as_mut().unwrap().index` to `filled_index` or
`empty_index` — guarded for change-detection exactly like the other two styles' update systems.

**Rounding decision: `ceil`, not `round`, when the ratio is above zero** — a genuine game-feel
choice this plan resolves explicitly rather than leaving implicit (ux-gamedesigner-reviewer
finding). With plain `.round()` on a 5-cell bar, 5% health rounds to `round(0.25) = 0` filled cells
— the player reads as dead while still alive. The idiomatic convention for pip/heart displays
(matching how most action games handle low-health hearts) is: `filled = 0` only at exactly `ratio
== 0.0`; otherwise `filled = max(1, (ratio * cells).ceil())`. Concretely, on a 5-cell bar: 1% health
→ 1 filled cell (never reads as dead while alive); 21% → 2 filled cells; 95% → 5 filled cells (a
sliver of damage doesn't visually round up to "full" either, since `ceil` only rounds *up* from a
fraction, and 0.95 * 5 = 4.75 → ceil = 5 — **note this means a bar can show "full" at anything
above 80% on a 5-cell bar, which is expected/idiomatic for this style, not a bug**). Document this
rounding rule explicitly in the docs table, with the 1%/95% edge cases spelled out, so designers
aren't surprised by either end.

### Split-screen duplication — built in from day one
Icon's spawn logic goes into the **same** `spawn_world_stat_bar_widget` function
(`capabilities/stat_display.rs`) from its first implementation, using the identical `for rank in
0..ranks` anchor-duplication pattern `pixel_world_stat_bar_split_screen_duplication.md`
establishes — each rank gets its own anchor (carrying `WorldIconBar` + `WorldLabelRank(rank)` +
`Visibility::Hidden` for rank > 0) and its own row of `Sprite` children, which inherit the anchor's
visibility via Bevy's existing hierarchy propagation exactly like Pixel's `Mesh2d` children already
do. **Sequenced after the Pixel duplication fix as a matter of avoiding a merge collision on the
same function and reusing a landed, reviewed pattern — not a hard technical dependency**
(system-architect finding: the `WorldLabelRank` + hierarchy-visibility mechanism is already proven
at rank-0 in shipped Pixel code today, so this feature is not blocked if the Pixel fix slips; it
would just derive the same rank loop independently instead of copying a merged one). Shared
(never-changing) `TextureAtlasLayout`/image handles should be built once per bar instance and
cloned across ranks; only the per-cell `Sprite.texture_atlas.index` differs by fill state, and even
that is identical across all ranks of the same bar (all ranks track the same entity's same stat).

### Explicitly out of scope
- **Partial-cell / half-icon rendering** (e.g. a half-filled heart for a ratio that lands mid-cell)
  — v1 rounds to the nearest whole cell per the rule above, matching every existing bar style's
  discrete-unit convention. A future phase could add this via UV-masking or a second overlay icon,
  but it's a materially bigger feature (partial-shape rendering, not just index-swapping) and no
  current project need has surfaced for it.
- **Removing the Ascii style** — separate decision, separate feature, not bundled here (see Why).
- **Depth scaling** — same pre-existing limitation Pixel bars already have (both are fixed
  screen-pixel size, constant at all camera distances); not solved by this feature either.

## Tasks
- [ ] Schema — `WorldStatBarStyle::Icon { .. }` struct variant in `catalog.rs` (inline fields, not
      a wrapped separate struct — matches `Ascii`/`Pixel`), with `#[serde(deny_unknown_fields)]`
      and full doc comments including the pixel-not-metres unit clarification
- [ ] `capabilities/stat_display.rs` — `WorldIconBar` anchor-level component (stat_key, cells,
      filled_index, empty_index); `world_icon_bar_update_system` (resolves once per anchor via
      `&Children` iteration, `ceil`-based whole-cell rounding per the documented rule,
      change-detection guarded); extend `spawn_world_stat_bar_widget`'s `match wb.style` with the
      `Icon` arm, spawning a rank-duplicated anchor (per rank: `WorldIconBar` + one `Sprite` child
      per cell with `texture_atlas: Some(TextureAtlas { layout, index: empty_index })` as the
      initial state) using the pattern established by the Pixel duplication fix
- [ ] Register `world_icon_bar_update_system` alongside the other stat-widget update systems
      (wherever `world_pixel_bar_update_system` is registered)
- [ ] Tests — parse/defaults/unknown-field-rejection (`ron_validation.rs`, matching the existing
      Ascii/Pixel test set); spawn-behavior tests (correct filled/empty cell count for a given
      ratio, including the 1%-shows-≥1-cell and 95%-shows-full edge cases from the rounding rule);
      split-screen rank duplication test, mirroring the Pixel duplication feature's own test
- [ ] Demo — add an Icon-style bar to `3rd_person_game_demo`'s player, as a "lives"/hearts overhead
      display tracking `{self}.health` (**not** `local_coop_demo`'s per-player mana bars — those
      are the exact prefabs `pixel_world_stat_bar_split_screen_duplication.md` converts to Pixel;
      reusing them here would collide with that feature). Includes producing (or sourcing) the
      actual filled/empty pip art asset this demo needs — **no shipped icon sheet has one today**
      (`iconsheet-status-effects-01.json`/`iconsheet-item-01.json` only have single buff/item
      icons, not a designed adjacent filled-heart/hollow-heart pair) — following the stylized
      hand-painted direction in `assets/CLAUDE.md`. Without this sub-task the demo would silently
      reuse mismatched icons and undercut the feature it's meant to showcase
      (ux-gamedesigner-reviewer finding).
- [ ] Docs — new `WorldStatBarStyle::Icon` section in `docs/20_data_formats.md`: fields table
      (explicitly anchoring `filled_index`/`empty_index` to the established `icon_index` row-major
      convention used elsewhere, and stating `size`/`spacing` are screen pixels like `Pixel.size`,
      not metres); RON example; the `ceil`-based rounding rule with the 1%/95% edge cases spelled
      out; split-screen behavior (correctly documented as duplicating from day one, unlike Pixel's
      historical caveat); a one-line texture-dimensions note (power-of-2 not required by
      WebGPU/WebGL2, but recommended for parity with the other shipped icon sheets — see Schema
      section above) so whoever produces the new pip art isn't left guessing
- [ ] WASM rebuild + `python test_web.py` — confirm the `Sprite` pipeline (this engine's first use
      of it — everything else uses `Mesh2d`/`ColorMaterial` or Bevy UI `ImageNode`) compiles/warms
      without a stall on first Icon-bar draw; if it does stall, fall back to the cropped-UV-mesh
      approach noted in Approach (schema and update-system logic are unaffected by which rendering
      backend is used)

## Open questions
- None blocking — the rendering approach (`Sprite`+`TextureAtlas`, with a documented mesh-based
  fallback), the rounding rule (`ceil`, documented with edge cases), and the sequencing dependency
  (soft, not technical) are all resolved above.
- **Partial-cell rendering** — deferred to a future phase (see Explicitly out of scope), not an
  open question blocking this plan, just a known future extension point.
- Should the two production styles (`Pixel`, `Icon`) eventually share more code (e.g. a common
  "world-space billboard row" helper), or is the overlap small enough that duplication is fine?
  Deliberately not resolved now — `system-architect` recommends revisiting only once Icon has
  actually shipped and the two implementations can be compared directly (Icon's `Sprite` children
  vs. Pixel's `Mesh2d` children diverge enough at the child-primitive level that the only real
  shared scaffolding, the anchor + rank loop, already lives in `spawn_world_stat_bar_widget` —
  don't over-abstract further pre-emptively).

## Acceptance criteria
- Given `world_stat_bar: (stat_key: "{self}.health", style: Icon(...))` with `cells: 5`, when the
  tracked stat is at 60% of its range, then 3 of 5 cells show the filled icon and 2 show the empty
  icon (`ceil(0.6 * 5) = 3`).
- Given the same bar at 1% health (not zero), when it renders, then at least 1 cell shows filled —
  never reads as fully empty/dead while the entity is still alive (the `ceil`-above-zero rule).
- Given the same bar at 95% health, when it renders, then all 5 cells show filled (`ceil(0.95 * 5)
  = 5`) — documented as expected/idiomatic, not a bug.
- Given a change to the tracked stat (`ModifyStat`), when the next frame runs, then the correct
  cells update to reflect the new ratio, with the stat resolved once per bar (not once per cell)
  and change-detection guarding all writes.
- Given a split-screen scene with an Icon-style bar on any entity, when it's visible in 2+ active
  viewports simultaneously, then it renders correctly in every one of them — **from this feature's
  first release**, no follow-up phase required (unlike Pixel's history).
- Given an existing project with no Icon-style bars, when this feature ships, then nothing about
  existing Ascii/Pixel bars changes (purely additive — new enum variant, no schema break).
- RON validation: parse/defaults/unknown-field-rejection tests pass for the `Icon` variant,
  matching the existing Ascii/Pixel test coverage shape.
- The `3rd_person_game_demo` hearts demo renders with a real, purpose-made filled/empty pip icon
  pair — not a placeholder or mismatched existing icon.
