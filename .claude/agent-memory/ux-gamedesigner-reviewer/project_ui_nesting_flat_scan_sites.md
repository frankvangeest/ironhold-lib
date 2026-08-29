---
name: ui-nesting-flat-scan-sites
description: Nesting UiNodeDef children (ui_flex_container) silently defeats 11 flat `scene.ui` scans — StatRadar material pre-pass, 4 scene-load warns, 6 CLI validate checks — plus the Container/ContainerPanel naming collision and the auto-size+SpaceBetween no-op
metadata:
  type: project
---

Any feature that lets a `UiNodeDef` hold **children** (i.e. `ui:` stops being a flat list) must
account for every place that iterates `scene.ui` non-recursively, or a nested widget silently loses
validation/setup with no error. As of 2026-08-28 there are **11** such sites:

- `scene_loader.rs` ~1166 — `radar_handles` `HashMap` pre-pass. A nested `StatRadar` gets **no
  `RadarMaterial` handle at all** → broken/invisible radar, no warning. This is the worst one: it's
  setup, not just diagnostics.
- `scene_loader.rs` — 4 flat warn fns: `warn_cross_bar_duplicate_keys` (~1346),
  `warn_same_player_gamepad_duplicate_slots` (~1387), `warn_missing_player_stat_templates` (~1495),
  `warn_gamepad_key_without_gamepad_index` (~1527).
- `ironhold_cli/src/commands/validate.rs` — 6 flat `for node in &scene.ui` loops (~492, 541, 569,
  613, 649 + the `invalid_font_size` pass). Covers `cross_bar_duplicate_key`, per-bar dup keys,
  gamepad dup slots, `missing_player_stat_template`, `invalid_font_size`.
- `ironhold_cli/src/commands/query.rs` ~382 — `ui_count: scene.ui.len()` becomes misleading (a
  container of 12 elements counts as 1).

**How to apply:** any plan that adds nesting to `ui:` must list "make these recursive" as an
explicit task. A plan that says "no CLI impact, just spot-check validate still runs clean" is wrong
— nesting *disables* existing checks rather than breaking the build.

## Two more traps specific to a flexbox container node

**"Container" is an already-taken domain word.** `UiNodeDef::ContainerPanel`, `Action::OpenContainer`/
`CloseContainer`, `container.opened`/`container.closed` events, and `initial_items` are the loot-chest
system. A layout box named `Container` sits adjacent to `ContainerPanel` in `docs/20_data_formats.md`
(§`ContainerPanel((...))` ~1366). Prefer `Group`/`Stack`/`Layout` for a layout node.

**Auto-size + `SpaceBetween`/`SpaceAround`/`SpaceEvenly` is a silent no-op.** A shrink-to-content
container has zero free main-axis space, so every `justify_content` spread value renders identically
to `Start`. Any example or acceptance criterion pairing `size: None` with `SpaceBetween` is wrong.

**Pixel-only sizing cannot deliver screen-edge anchoring.** The logged "no `anchor:`/percentage
positioning" gap is NOT solved by flex `justify_content` unless the container itself can span the
window — which needs a percentage/fill size, not `Option<(f32,f32)>` pixels.

## `ui_panel:` vs a new container — default drift a designer will hit

`UiPanelDef` defaults: `padding` 20.0, `gap` 12.0, `background_color` (0.1,0.1,0.1,0.95), and its
Panel node **always** `Overflow::clip()`s. A flexbox container following plain `#[serde(default)]`
gets 0.0/0.0/None/no-clip. Docs need a side-by-side default table, not just a field table.

Related: [[ui-label-button-font-and-clip]], [[container-events-undocumented]],
[[warn-vs-silent-fallback-principle]], [[schema-bool-toggle-house-style]].
