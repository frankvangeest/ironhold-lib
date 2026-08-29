---
name: ui-flat-scan-and-taffy-facts
description: scene.ui is scanned FLAT in 11 places (6 core + 5 CLI) plus a hard-dependency radar pre-pass — any nestable UI node silently blinds all of them; plus verified Bevy 0.18/taffy 0.9.2 flex facts
metadata:
  type: project
---

**`GameSceneV2.ui` is treated as a flat `Vec` by every scene-wide consumer.** Introducing any
nestable UI node (`Container`-style, `children: Vec<UiNodeDef>`) makes all of these silently
under-cover nested nodes — no compile error, since each is a `let ... else { continue }` or a
`match` with `_ => continue`:

- `scene_loader.rs` ~1166 — **`radar_handles` pre-pass. This one is a functional dependency, not a
  diagnostic**: the `StatRadar` arm warns "no pre-created material handle — skipping spawn" and
  renders nothing if its id isn't in that map. A nested `StatRadar` would silently not exist.
- `scene_loader.rs` — `warn_cross_bar_duplicate_keys`, `warn_same_player_gamepad_duplicate_slots`,
  `warn_missing_player_stat_templates`, `warn_gamepad_key_without_gamepad_index` (all
  `for el in &scene.ui` + `let UiNodeDef::ActionBar(bar) = el else { continue }`).
- `ironhold_cli/src/commands/validate.rs` ~492/541/569/613/649 — the CLI mirrors of the four above
  plus `invalid_font_size` (Label/Button). Note 492 and its core twin key same-bar-vs-cross-bar by
  **positional node index**, so a flattening walk must produce a stable deterministic pre-order
  index shared by both sides.
- `query.rs` ~382 — `ui_count: scene.ui.len()` becomes a top-level-only count.

**Why:** "additive new enum variant, no CLI impact" is the wrong instinct for `UiNodeDef` — the
compiler cannot catch under-coverage here, and the radar case is a silent render failure, not just
a missing warning.

**How to apply:** any feature adding nesting to `scene.ui` must ship one shared
flattening/pre-order iterator (`fn walk_ui_nodes(&[UiNodeDef]) -> impl Iterator<Item = (usize, &UiNodeDef)>`
or similar) in `schema/scene_v2.rs` and convert all 11 sites to it, in the same change.

## Verified Bevy 0.18 / taffy 0.9.2 UI facts (checked against registry source, 2026-08-28)

- **`PositionType::Absolute` is "relative to its parent node"** — taffy does *not* implement CSS's
  nearest-positioned-ancestor containing-block chain. An `absolute: true` child nests correctly
  under any parent, but its `position:` becomes parent-box-relative, not screen-relative.
- **Absolute children are excluded from flex content sizing** (`generate_anonymous_flex_items`
  filters `position() != Absolute`). So an auto-sized (`Val::Auto`) parent containing only
  absolutely-positioned children collapses to ~zero. This is the same footgun `UiPanelDef.height`'s
  doc comment already warns about ("set this when the panel contains absolutely-positioned
  children").
- **`Overflow::clip()` → `OverflowAxis::Clip` → taffy `Overflow::Clip`, which is NOT a scroll
  container** (`is_scroll_container()` is true only for `Hidden`/`Scroll`). So clip does *not*
  zero a flex item's automatic minimum size — clipping on an auto-sized box is inert, not
  destructive. `Hidden`/`Scroll` would be the dangerous ones.
- **`row_gap` = gutters in a *vertical* (Column) flexbox; `column_gap` = gutters in a *horizontal*
  (Row) flexbox.** A "single `gap` field" mapped Row→`column_gap` / Column→`row_gap` is correct for
  the main axis — but leaves the **cross-axis (between wrapped lines) gap at zero** when
  `flex_wrap` is `Wrap`/`WrapReverse`. `Val::Auto` in either gap is treated as zero.
- **`JustifyContent`/`AlignItems` each expose both `Start`/`End` and `FlexStart`/`FlexEnd`** —
  they differ under `RowReverse`/`ColumnReverse` (Flex* follow the reversal, plain Start/End are
  physical). Exposing reverse directions but only `Start`/`End` is a real semantic choice to
  document, not equivalent.
- **`justify_content: SpaceBetween`/`SpaceAround`/`SpaceEvenly` distribute *free space*** — they are
  no-ops on a content-sized (`Val::Auto`) container by definition. Same for `align_items: Stretch`
  against children with a definite cross-axis size (which every current `UiNodeDef` leaf has:
  always `Val::Px`, never `Auto`).
- **`update_clipping_system` and `ui_stack_system` recurse the whole UI tree every frame, ungated by
  change detection.** Per-frame UI cost scales with total node count, so grouping wrappers are not
  free — but they're cheap; the count that matters is nodes, not depth.
- `EntityCommands::with_children` takes `impl FnOnce(&mut ChildSpawnerCommands)`, so moving `&mut`
  params into the closure for a recursive spawn helper is fine. `Option<&mut Assets<_>>` params must
  be reborrowed per loop iteration (`.as_deref_mut()`); they aren't `Copy`.

See also [[ui-label-box-overflow-reliance]], [[ui-hover-and-tooltip]], [[panel-input-blocking]].
