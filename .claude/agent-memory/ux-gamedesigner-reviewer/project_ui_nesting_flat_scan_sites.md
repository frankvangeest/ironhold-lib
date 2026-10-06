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

## 2026-10-05 re-review of ui_flex_group.md (Group, UiSizeDef Auto/Px/Percent)

- Bottom/right edge anchoring is only reachable via a **full-screen root Group** (width+height
  Percent(100), Column, SpaceBetween) — the plan never names this recipe, and never says what
  FocusPolicy a Group gets. ui_panel/overlay backdrops use FocusPolicy::Block
  (`ui_panel_blocker.rs`); a full-screen Group must Pass or it eats world clicks/orbit.
- options.scene.ron vertical rhythm is irregular (gaps 5,5,5,5,25,35,15,10,20) — no margin means
  the retrofit can't be pixel-identical without spacer Groups (`Group((height: Px(n), children: []))`).
- Leaves stay fixed-px `size:` — blocks portrait phones (~360 logical px; options labels are 400px).
- `ui_demo` project cited by the mobile_ui_demo backlog entry does not exist.
- Optional `id` on Group means diagnostics need a `ui[2].children[0]`-style path, not a pre-order index.

## 2026-10-05 second re-review (at a487a56) — verdict Ready with doc fixes
All first-pass items resolved (R1-R13). Remaining traps worth re-checking at code review:
- Group "sizes to content" = sizes to leaf `size:` boxes, NOT text; long text still spills into the
  next sibling in a Row. Plan's "Why" oversells this vs the font footgun.
- FocusPolicy::Pass on a Group WITH background_color looks like a panel but leaks clicks to world.
- New validate checks need listing in docs/60 "Checks performed", not only docs/20.
- mobile_ui_demo v1 "on-screen controls driving player" is unbuildable: Buttons are discrete taps;
  no held/analog UI->movement input exists. Only menus/ActionBar taps are v1-feasible.
- camera_modes is its own PROJECT, not a 3rd_person_game_demo scene (plan text mislabels it).
- README "Example projects" table lists only 5 of 14 projects.

## 2026-10-06 post-implementation review (feature/ui_flex_group) — Ship with doc fixes
- Shipped: Group section docs/20 ~1438-1504, 11 traps; docs/60 ~263-264 check list; ui_demo
  project (3 stations + top/bottom anchored bars); options.scene.ron retrofit; README + index.html.
- Open at review time: trap 1 lumps StatBar into "position themselves" but StatBar/StatSpread/
  StatRadar have `absolute: bool` and DO flow in a Group (only ActionBar + 4 panels are
  always-absolute); trap 10 uses Bevy jargon (Visibility::Hidden / Display::None) with no RON path;
  percent_under_auto message says "Px/Percent width" even when the parent is ui_panel: (bare f32);
  scene-load `UI layout [...]` warn omits scene name; validate summary counts layout warnings as
  "unused definitions"; ui_demo never shows flex_wrap/Percent<100/Reverse/clip/absolute.
- Group heading sits 600 lines below the "UI Elements" intro with no pointer from it.

Related: [[ui-label-button-font-and-clip]], [[container-events-undocumented]],
[[warn-vs-silent-fallback-principle]], [[schema-bool-toggle-house-style]].
