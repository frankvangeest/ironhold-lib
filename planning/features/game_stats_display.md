# Feature: Stat Display — Health Bars and Stat Spreads

_Status: Draft_
_Planned at: `1f63f4d` (2026-05-04)_

_Depends on: `game_stats_core.md` (Phase 1) must be complete. Phase 2 buffs are optional but useful for showing effective vs base values._

## What

Game designers can add health bars, mana bars, and stat spread panels to any scene's UI entirely through RON. A stat bar reads a named stat from `LoadedStats` and renders a filled rectangle that updates automatically. A stat spread lists multiple stats as labelled rows or a radar chart. Display style (colours, size, position, label format) is fully configurable in the scene's UI definition.

## Why

Without display primitives, the only way to show stat state is through static `Label` text updated by action-driven variable writes — fragile, verbose, and visually limited. A proper stat display component gives game designers self-updating HUD elements that require no event wiring, matching how health bars work in every commercial game.

## Approach

### Design constraints

- Display components must be **read-only** — they observe `LoadedStats` and render, they do not mutate stats.
- They must work with the existing UI scene format (`GameSceneV2`) — no new scene file type.
- They must be WASM-compatible (no platform-specific rendering).

### New UI node types in `GameSceneV2`

The scene's `ui` section gains two new node variants alongside existing `Rect`, `Label`, `Button`:

#### `StatBar`

A horizontal (or vertical) bar that fills proportionally to `current / max` of a named stat.

```ron
StatBar((
    id: "health_bar",
    stat_key: "health",
    orientation: Horizontal,           // Horizontal | Vertical
    position: (20.0, 20.0),
    size: (200.0, 20.0),
    fill_color:       (0.85, 0.15, 0.15, 1.0),  // red
    background_color: (0.25, 0.10, 0.10, 1.0),  // dark red
    border_color:     (1.0,  1.0,  1.0,  0.4),
    border_width: 1.0,
    show_label: false,
    show_value: false,                 // "75 / 100" text overlay
    use_effective_value: true,         // Phase 2: show effective value (with buffs)
))
```

Threshold colour bands (optional): the bar automatically shifts fill colour when the stat's effective value crosses designer-specified percentages — no event wiring required:

```ron
color_bands: [
    ( above_percent: 0.5,  color: (0.85, 0.15, 0.15, 1.0) ),  // red (normal)
    ( above_percent: 0.25, color: (1.0,  0.55, 0.0,  1.0) ),  // orange (low)
    ( above_percent: 0.0,  color: (0.6,  0.0,  0.0,  1.0) ),  // dark red (critical)
],
```

#### `StatSpread`

A panel listing multiple stats as labelled rows, useful for RPG character screens or debug overlays.

```ron
StatSpread((
    id: "stat_panel",
    stats: ["health", "mana", "stamina", "strength", "agility"],
    layout: Rows,              // Rows | Columns | Radar (future)
    position: (16.0, 60.0),
    label_width: 80.0,
    bar_width: 120.0,
    row_height: 22.0,
    row_gap: 4.0,
    label_color: (1.0, 1.0, 1.0, 0.8),
    bar_fill_color: (0.3, 0.6, 1.0, 1.0),
    bar_background_color: (0.1, 0.1, 0.25, 1.0),
    show_values: true,
))
```

### Schema types (`schema/scene_v2.rs`)

```rust
pub enum UiNodeDef {
    Button(ButtonDef),        // existing
    Label(LabelDef),          // existing
    Rect(RectDef),            // existing
    StatBar(StatBarDef),      // new
    StatSpread(StatSpreadDef), // new
}

pub struct StatBarDef {
    pub stat_key: String,
    pub orientation: BarOrientation,
    pub width: f32,
    pub height: f32,
    pub fill_color: RonColor,
    pub background_color: RonColor,
    pub border_color: RonColor,
    pub border_width: f32,
    pub show_label: bool,
    pub show_value: bool,
    pub use_effective_value: bool,
    pub color_bands: Vec<ColorBand>,
}

pub struct ColorBand {
    pub above_percent: f32,
    pub color: RonColor,
}

pub struct StatSpreadDef {
    pub stats: Vec<String>,
    pub layout: StatSpreadLayout,
    pub label_width: f32,
    pub bar_width: f32,
    pub row_height: f32,
    pub row_gap: f32,
    pub label_color: RonColor,
    pub bar_fill_color: RonColor,
    pub bar_background_color: RonColor,
    pub show_values: bool,
}
```

### Bevy components and systems

- **`StatBarComponent`** — marker component carrying `stat_key` and the `StatBarDef`; spawned alongside the Bevy UI node at scene load.
- **`StatSpreadComponent`** — same pattern for spreads.
- **`stat_bar_update_system`** — runs each frame; reads `LoadedStats`; updates `Style::width` (for horizontal bars) or `BackgroundColor` (for colour bands) on the Bevy UI node. Uses `Changed<LoadedStats>` to skip frames where stats haven't changed.
- Scene loader gains new branches in the UI spawning code for `StatBar` and `StatSpread` node kinds.

### No event wiring required

A `StatBar` does not listen to events — it polls `LoadedStats` directly each frame (efficiently, via change detection). Designers do not need to write any event rules to keep the display in sync.

### Radar chart (future, not in scope)

`StatSpread` with `layout: Radar` would render an SVG-style polygon. Out of scope for this feature; the `Radar` variant is reserved in the enum to avoid a future breaking schema change.

## Tasks

- [ ] `StatBarDef`, `StatSpreadDef`, `ColorBand`, `BarOrientation`, `StatSpreadLayout` schema types
- [ ] Add `StatBar` and `StatSpread` variants to `UiNodeDef`
- [ ] Scene loader: spawn `StatBarComponent` and `StatSpreadComponent` nodes
- [ ] `stat_bar_update_system` — change-detection poll, width/colour update
- [ ] `stat_spread_update_system` — rebuild row values on change
- [ ] Colour band selection logic (sorted by threshold, pick highest matching)
- [ ] Integration test: `StatBar` reflects correct fill ratio after `ModifyStat`
- [ ] Integration test: colour band switches when stat crosses threshold
- [ ] Integration test: `StatSpread` renders correct rows for listed stats
- [ ] RON validation: `StatBar` and `StatSpread` round-trip through serde
- [ ] Add a `stats_display_demo` scene to an existing project (or `primitive_world`) showing both components
- [ ] Docs: update `20_data_formats.md` with new UI node kind examples

## Open questions

- **Stat not found**: if `stat_key` references a stat that doesn't exist in `LoadedStats`, should the bar render empty (silent) or log a warning? Prefer a visible warning in debug builds, silent fallback in release.
- **Animated transitions**: should the fill width animate smoothly toward the target value, or snap immediately? Snap for now (simpler, avoids needing a lerp speed parameter); animation can be a follow-up.
- **Per-entity bars**: when stats become per-entity (Phase 2+ extension), the `StatBar` will need to know which entity to track. Defer; assume single global stat pool for this feature.
- **Radar chart**: reserved in the enum but not implemented. Should the variant exist in the schema now (returning an error/unsupported message) or be added later? Adding now prevents a future schema version bump.

## Acceptance criteria

- Given a scene with a `StatBar` for `"health"`, when `ModifyStat(key: "health", delta: -50.0)` is executed, then the bar visually fills to 50% on the next frame without any event rule.
- Given a `StatBar` with two colour bands (above 50% = red, above 0% = dark red), when health drops below 50%, then the fill colour changes to dark red automatically.
- Given a `StatSpread` listing `["health", "mana"]`, the panel renders two labelled rows each showing the correct current / max values.
- Given a `stat_key` that does not exist in `LoadedStats`, the bar renders as empty and a warning is logged in debug builds; no panic occurs.
- All display components are WASM-compatible and do not degrade frame rate noticeably on the existing demo scenes.
