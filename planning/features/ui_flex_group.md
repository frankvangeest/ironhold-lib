# Feature: Nestable flexbox `Group` UI node

_Status: In Progress (v1 Done 2026-10-06, v2 Queued) — v1 shipped on `feature/ui_flex_group`; v2 (portrait / touch-first) is queued in `planning/backlog.md`_
_Planned at: `8baeac7` (2026-08-28)_
_Drift refreshed at: `f34dd16` (2026-10-05)_

> **Drift refresh (2026-10-05).** `git log 8baeac7..HEAD` on the touched files shows heavy `validate.rs`
> growth. Corrections vs. the original text below, verified by grep at `f34dd16`:
> - `scene_loader.rs` lives at `crates/ironhold_core/src/runtime/scene_manager/scene_loader.rs`
>   (not `runtime/scene_loader.rs`). Its flat-scan sites are unchanged: the `radar_handles`
>   pre-pass plus 4 `warn_*` fns (`warn_cross_bar_duplicate_keys`,
>   `warn_same_player_gamepad_duplicate_slots`, `warn_missing_player_stat_templates`,
>   `warn_gamepad_key_without_gamepad_index`).
> - `ironhold_cli/src/commands/validate.rs` now has **9** `for ... in &scene.ui` scans (was 5 + the
>   font-size check), plus `query.rs`'s `ui_count`. **Re-enumerate with `grep -n "scene\.ui"` in both
>   crates when the walker task starts** instead of trusting any count in this file.
> - `scene_loader.rs:98` (`scene.ui.len()` in an info log) counts top-level nodes only; decide
>   whether it should count nested nodes via the walker.
> - `scene_loader.rs` also carries `// det: lookup-only` annotations on `HashMap`s (D2 determinism
>   guard, `determinism_lint`) — any new `HashMap`/`HashSet` this feature adds needs one.
> - `rules.ron` was consolidated into `state_machine.ron` and `query rules` renamed `query logic`;
>   no effect on this design.

## Plan-review resolutions (2026-10-05)

Decisions made by Frank (D) and findings folded in from the review (F). Where the body below
still says something different, **this section wins** and the body has been edited to match.

| # | Decision / fix | Source |
|---|---|---|
| R1 | `options.scene.ron` retrofit is an **intentional even rhythm** (e.g. item `gap` 8, section gap 24), not pixel-identical; a baseline diff for that one scene is expected. A spacer (`Group((height: Px(20.0), children: []))`) is documented as a known idiom. | D (UX #2) |
| R2 | `Group.id` stays optional. `GameSceneV2::validate()` exempts `Group` from the non-empty-id check, skips `""` in duplicate detection, and walks nested nodes so ids are unique **per scene across all nodes that have one**. Leaf nodes still require ids. | D (architect blocking) |
| R3 | `ironhold validate` calls `GameSceneV2::validate()` (precedent: `validate.rs` already calls the four catalogs' own `.validate()`). **Gate:** run it over every `assets/projects/*` first; if any shipped project fails, split this task into its own backlog item instead of fixing projects inside this feature (CI's `assets_schema_version_regression.rs` already runs it on every shipped scene, so failures are not expected). **Scope accepted (R19):** this also reports duplicate/empty entity and world_label ids and scene `schema_version`, not only UI ids; each error gets the R9 path since `validate()` is fail-fast (one error per scene). | D |
| R4 | **v1 is landscape-only.** Portrait is v2 with different backlog dependencies — see `## Phases`. | D (UX mobile) |
| R5 | The `Percent`/`SpaceBetween` showcase example lives in a new minimal **`ui_demo`** project created by this feature (Group stations only). The backlog's broader `ui_demo` item (buttons, data-bound labels, overlays, stat widgets) stays Queued and extends it. | D (UX #3) |
| R6 | `query.rs` keeps `ui_count` = top-level count (JSON consumers unaffected) and adds `ui_node_count` = all nodes; the load log prints both. `walk_ui_nodes` applies the same depth cap (16) as the spawner so diagnostics cover exactly the nodes that spawn. | D (architect) |
| R7 | **Corrected in pass 2.** A `Group` never carries `Interaction` or `FocusPolicy::Block`, so clicks on its empty area reach the world. This is the same as `ui_panel:` (which has neither); it differs from the inventory/shop/container panel roots and the overlay backdrop, which block deliberately (`ui_panel_blocker.rs`). Setting `FocusPolicy::Pass` explicitly is a no-op (it is `Node`'s default) and is NOT added. Mechanism: `click_select_system` only skips the world when some `Interaction == Pressed` (`targeting.rs`); camera orbit/strafe drags have no UI guard at all. Test: spawn a full-screen `Group`, assert it has no `Interaction`/`Block`, and that a click on it changes the target. | F (UX #1, architect pass 2) |
| R8 | Edge anchoring recipe is documented as the intended pattern: full-screen root `Group` (`width/height: Percent(100.0)`, `flex_direction: Column`, `justify_content: SpaceBetween`) holding a top row and a bottom row; `End` for right-aligned. No separate anchor system in v1. | F (UX #1, architect #10) |
| R9 | Every new/converted diagnostic names the node by **path** (`ui[2].children[0] (Group)`, plus `id` when set), never a bare empty id or index. | F (UX #5) |
| R10 | Extra `ironhold validate` warnings: `SpaceBetween/Around/Evenly` on a `Group` whose main-axis size is `Auto`; `Percent` on a nested `Group` whose parent is `Auto` on that axis; any of `ActionBar`/`DialoguePanel`/`InventoryPanel`/`ShopPanel`/`ContainerPanel` nested in a `Group`; negative or non-finite `gap`/`padding`/`Px`/`Percent`. | F (UX) |
| R11 | Docs: update the six leaf `position` rows, the `absolute` rows and the UI Panel intro in `docs/20_data_formats.md` ("ignored in panel mode" becomes "inside `ui_panel:` or a `Group`"). Document top-level `Group` behaviour, `width: Px(..)` vs `ui_panel:`'s bare number, and `width`/`height` vs leaf `size:`. | F (UX #4) |
| R12 | `walk_ui_nodes` yields nodes (callers `.enumerate()` if they need a per-bar tag). The earlier requirement that core and CLI use an identical index was wrong: core's `warn_cross_bar_duplicate_keys` already keys by its own ActionBar counter and the CLI by `enumerate()`; both only need a unique-per-bar tag. | F (architect #3) |
| R13 | `GameSceneV2::validate()` (`schema/scene_v2.rs`, flat `for elem in &self.ui`) is an additional flat-scan site to convert. | F (architect blocking) |
| R14 | `JustifyContentDef`/`AlignItemsDef` keep the names `Start`/`End` but map to Bevy's **`FlexStart`/`FlexEnd`** (CSS semantics: they follow `RowReverse`/`ColumnReverse`, and for `align_items` follow `WrapReverse`). Bevy's physical `Start`/`End` are not exposed. Default `justify_content` is therefore FlexStart: identical to the earlier `Start` on auto-sized boxes. Reverse directions stay in v1. Corrects the earlier claim that Start/FlexStart differ for `align_items` under Row/ColumnReverse (they differ only under `WrapReverse`). | D (architect pass 2) |
| R15 | `Group` **children** get `flex_shrink: 0.0` (set in the Group children loop only, not the shared `ui_panel:` path) so `size:` means exactly that many px; Bevy's default of 1.0 would shrink leaves and silently disable `SpaceBetween` in a Group narrower than its content. | D (architect pass 2) |
| R16 | A `Group` with `background_color` still lets clicks through (callout in docs); no `block_clicks` field in v1. Revisit if a dialog-box use case appears. | D (UX pass 2) |
| R17 | `mobile_ui_demo` v1 = discrete on-screen buttons only (menu, pause, tapped ActionBar/ability slots). Movement controls (virtual stick/d-pad, held or analog input from UI) are a separate backlog item and part of v2. | D (UX pass 2) |
| R18 | Walker depth: pin ONE convention - a top-level node is depth 1, nodes at depth 17+ are neither spawned nor walked - shared by spawner and walker; test 16 (kept) vs 17 (dropped) on both sides. `ironhold validate` reports a diagnostic naming the path of any truncated subtree (silent truncation is the same blindness this feature removes). Define `UiPath` (a `Vec<usize>` of child indices with a `Display` impl producing `ui[2].children[0]`). | F (architect pass 2) |
| R19 | See R3 (CLI scope widened to entity/world_label ids and schema_version). | D |
| R20 | Extra validate rule: a `Percent` `Group` placed directly in a `ui_panel:` with no `width`/`height` warns (same trap as Percent under an Auto parent). The nested-panel warning ends with "move it to top-level `ui:`". | F (UX + architect pass 2) |
| R21 | Docs/registration additions: `docs/15_authoring_tools.md` (the canonical list of validate checks) lists every new/wired check; README "Example projects" table gains `ui_demo`; `crates/ironhold_cli/tests/validate_projects.rs` gains a `ui_demo` test (hand-maintained list); `ui_demo` is a single full-screen R8 root (top HUD bar with `SpaceBetween`, middle nested-row/column + spacer stations, bottom right-aligned row) so stations don't overlap at (0,0), with `ui.button_pressed:` bindings in `logic/state_machine.ron` (e.g. `SetVariable` shown in a bound Label) so `validate` does not report `unreachable_trigger`; minimum file set: project.ron, scene, assets.ron, prefabs, state_machine (use the `/new-project` skill / blank_project template). | F (UX + architect pass 2) |
| R22 | Doc callouts added: "sizes to content" means the leaf `size:` boxes, not their text (long text still spills into a Row sibling - the Why section's claim to solve the font footgun is narrowed accordingly); `box_sizing` is BorderBox so `padding` eats into a `Px` width; absolute children's insets resolve against the Group's padding box (Group `padding` does not offset them); `Percent(n)` under an Auto parent is resolved cyclically against the parent's final content-derived size (not simply "like auto"); `Percent` on a top-level Group is measured against the window (R8 depends on this) - split-screen behaviour unverified, add to playtest checklist (`camera.rs:801`: RON UI roots rely on the default UI camera); spacer idiom needs `width: Px(n)` in a Row; link `ui_demo` and the retrofitted `options.scene.ron` as the examples to copy. | F (UX + architect pass 2) |

## What

Adds `Group(GroupDef)` to `UiNodeDef` — a UI node that holds its own `children: Vec<UiNodeDef>`
plus RON-authorable flexbox properties (`flex_direction`, `justify_content`, `align_items`,
`flex_wrap`, `gap`, `padding`, per-axis `width`/`height`). Groups nest arbitrarily, so a designer
can compose real layouts in RON the way you'd nest `<div>`s with CSS flexbox — a row of buttons
inside a column inside another row — instead of hand-placing every element at an absolute pixel
coordinate.

```ron
ui: [
  Group((
    flex_direction: Row,
    justify_content: SpaceBetween,
    width: Percent(100.0),   // SpaceBetween needs free space to distribute — see "Sizing" below
    padding: 16.0,
    children: [
      Label((id: "title", text: "Inventory")),
      Group((
        flex_direction: Row,
        gap: 8.0,
        children: [
          Button((id: "sort_btn", text: "Sort", action: "ui.sort")),
          Button((id: "close_btn", text: "X", action: "ui.close")),
        ],
      )),
    ],
  )),
]
```

## Why

Every `ui:` element today is either placed with a manual pixel `position:` (no layout at all), or
— if the scene sets `ui_panel:` — flows into exactly *one* scene-wide box hardcoded to a single
flex column (`FlexDirection::Column`, `AlignItems::Center`, `JustifyContent::Center`; only
`padding`/`gap`/`width`/`height` are authorable). There is no nesting anywhere: a designer gets one
flat list of children in at most one container, ever.

This is the same root cause behind two problems already logged from recent features:
- The recurring "hand-compute pixel positions against a proportional font" footgun
  (`ui_label_font_size.md`, `planning/claude_suggestions.md` ▸ UI) — a real flex layout would
  right-align/space-between elements instead of requiring a designer to calculate where box N+1
  needs to start.
- The logged "`UiNodeDef` has no `anchor:`/percentage positioning" gap
  (`dynamic_animation_control.md`'s UI review) — a `Group` with `width: Percent(100.0)` +
  `justify_content: End`/`SpaceBetween` gives real edge/spread anchoring, without inventing a
  parallel percentage-position system on every leaf node.
- The logged "an `Auto`-sized box would be a better long-term answer than `font_size`/`clip`"
  suggestion (`ui_label_font_size.md`'s post-implementation review, system-architect) — this
  plan's `Group` sizes to content by default, scoped to the one node type where "grow to fit
  children" is unambiguous, rather than every leaf node type.

Internally, Bevy's real flexbox is already used everywhere in this engine — every composite
widget (`StatBar`, `StatSpread`, `ActionBar`, the panel types) builds its own `flex_direction`/
`justify_content`/`align_items` layout in Rust, entirely hardcoded, never exposed to RON. This
feature doesn't add a new capability to the engine's rendering — it exposes machinery that
already exists and is already trusted in production, to the RON layer.

**Named `Group`, not `Container`** — the obvious name collides head-on with the existing
loot-container domain (`UiNodeDef::ContainerPanel`, `Action::OpenContainer`/`CloseContainer`,
`container.opened`/`container.closed` events, `ContainerPanelDef.initial_items`). A designer
searching docs or RON for "container" must not land on two unrelated concepts.

## Approach

**Revised after parallel plan reviews from system-architect and ux-gamedesigner-reviewer, both
verified against the actual current code and (for system-architect) vendored Bevy 0.18/taffy
0.9.2 source rather than assumption.** Both independently found the same two critical gaps below;
this section is written against their fixes, not the original draft.

### Critical fix #1 — nesting must not blind the existing flat `scene.ui` scans

The current code scans `scene.ui: Vec<UiNodeDef>` **flat** in ~17 places across 4 files (re-grep
`scene\.ui` in `ironhold_core` and `ironhold_cli` when the walker task starts; do not trust any
count in this file), and one of them is a functional dependency, not a diagnostic. Add
`schema/scene_v2.rs`'s own `GameSceneV2::validate()` to the list (R13):

- `scene_loader.rs` (under `runtime/scene_manager/`) — the `radar_handles` pre-pass (builds the `HashMap` a `StatRadar` arm looks
  its material up in; a `StatRadar` nested inside a `Group` would silently get no material and
  render nothing), plus four `warn_*` diagnostics (`warn_cross_bar_duplicate_keys`,
  `warn_same_player_gamepad_duplicate_slots`, `warn_missing_player_stat_templates`,
  `warn_gamepad_key_without_gamepad_index`).
- `ironhold_cli/src/commands/validate.rs` — the CLI mirrors of those checks, plus the
  `invalid_font_size` check (`ui_label_font_size.md`) and others added since (9 scans at
  `f34dd16`; see the drift note at the top).
- `ironhold_cli/src/commands/query.rs` — `ui_count: scene.ui.len()`.

**Fix, mandatory for this feature, not deferred:** add one shared pre-order walker to
`schema/scene_v2.rs`:

```rust
pub const MAX_UI_DEPTH: usize = 16;

/// Pre-order, depth-capped at MAX_UI_DEPTH: yields exactly the nodes the spawner will spawn.
/// Implemented with an explicit Vec stack (deterministic order, no recursion).
pub fn walk_ui_nodes(nodes: &[UiNodeDef]) -> impl Iterator<Item = &UiNodeDef> { /* ... */ }

/// Same traversal, also yielding a human-readable path for diagnostics (R9),
/// e.g. `ui[2].children[0]`.
pub fn walk_ui_nodes_pathed(nodes: &[UiNodeDef]) -> impl Iterator<Item = (UiPath, &UiNodeDef)> { /* ... */ }
```

Callers that need a unique-per-bar tag `.enumerate()` the iterator themselves (R12).

Convert every site to it in this same change — a `StatRadar`/`ActionBar` nested in a `Group`
must be exactly as diagnosed and functional as one at the top level.

### Critical fix #2 — sizing needs `Percent`, not just `Px`/auto

The feature's own motivating claim ("real edge/spread anchoring... without inventing a parallel
percentage-position system") is undeliverable with pixels-only sizing: `justify_content:
SpaceBetween`/`SpaceAround`/`SpaceEvenly` distribute *free space*, and a size-to-content box has
none by definition — so a designer wanting a right-aligned or spread-out HUD row must hardcode
`width: 1280.0` to fill the screen, reintroducing the exact "hardcode against the window size"
footgun this feature exists to delete, and breaking on any other resolution.

Fix: a small per-axis sizing enum, not a single `size: Option<(f32,f32)>` tuple (also matches
`UiPanelDef.width`/`.height` being two independent `Option<f32>` fields, not one tuple — a
designer very commonly wants "fixed width, height fits content" and a tuple can't express that):

```rust
#[derive(Deserialize, Debug, Clone, Default)]
pub enum UiSizeDef {
    #[default]
    Auto,
    Px(f32),
    Percent(f32),
}
```

`GroupDef.width`/`.height: UiSizeDef` (default `Auto` on both axes — sizes to content, the natural
default for a layout wrapper). `Val::Auto`/`Val::Px`/`Val::Percent` map directly; no measurement
subtlety (system-architect verified: every leaf `UiNodeDef` already has a definite `Val::Px` box,
so an auto-sized `Group`'s content size is trivially resolvable in one pass — no text
`MeasureFunc` involved at the `Group` level).

### Schema

```rust
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct GroupDef {
    /// Pure layout wrappers ("this row exists only to group two buttons") are common — don't
    /// force a designer to invent a meaningless id for one. Only feeds `Name::new` for debugging.
    #[serde(default)]
    pub id: String,
    /// Children laid out according to this group's flex properties. Each child's own
    /// `absolute: true` still escapes the flex flow entirely (identical mechanism to `ui_panel:`
    /// today, verified correct even when nested — see "Absolute children" below) — positioned via
    /// its own `position:`, relative to THIS group's box, not the screen.
    pub children: Vec<UiNodeDef>,
    #[serde(default)]
    pub flex_direction: FlexDirectionDef,   // Row (default) | Column | RowReverse | ColumnReverse
    #[serde(default)]
    pub justify_content: JustifyContentDef, // Start (default) | Center | End | SpaceBetween | SpaceAround | SpaceEvenly
    #[serde(default)]
    pub align_items: AlignItemsDef,         // Start (default) | Center | End — no Stretch, see below
    #[serde(default)]
    pub flex_wrap: FlexWrapDef,             // NoWrap (default) | Wrap | WrapReverse
    /// Gap between children, in pixels — sets BOTH Bevy's `row_gap` and `column_gap`, matching
    /// CSS's own `gap` shorthand exactly (which also sets both). A single-axis mapping was
    /// considered and rejected: it left the *other* axis's gutter at zero, so a wrapped row's
    /// lines touched with no way to add space between them.
    #[serde(default)]
    pub gap: f32,
    /// Inner padding on all four sides, in pixels.
    #[serde(default)]
    pub padding: f32,
    #[serde(default)]
    pub width: UiSizeDef,
    #[serde(default)]
    pub height: UiSizeDef,
    /// Background colour as sRGB RGBA (0.0-1.0) — same convention as every other color field in
    /// this schema (`crates/ironhold_core/src/CLAUDE.md`'s Color field convention). `None` = no
    /// background, for a pure layout wrapper.
    #[serde(default)]
    pub background_color: Option<(f32, f32, f32, f32)>,
    /// Same opt-in clip convention as `Label`/`Button` (`ui_label_font_size.md`) — off by
    /// default. Only meaningful when `width`/`height` are NOT both `Auto` (verified: taffy's
    /// `Overflow::Clip` does not shrink an item's automatic minimum size, so clipping an
    /// auto-sized box is harmless but inert — `ironhold_cli validate` warns on this combination).
    #[serde(default)]
    pub clip: bool,
    #[serde(default)]
    pub position: (f32, f32),
    #[serde(default)]
    pub absolute: bool,
}
```

Four new small enums (`FlexDirectionDef`, `JustifyContentDef`, `AlignItemsDef`, `FlexWrapDef`)
mirror Bevy's own names 1:1 (verified against precedent: `AlphaModeDef` mirrors Bevy's
`AlphaMode` verbatim, `EaseKind` mirrors CSS easing names) rather than inventing house-specific
vocabulary — there is no clean one-word alternative for `SpaceEvenly`, and mirroring lets a
designer reuse any CSS/Bevy flexbox tutorial directly.

- `JustifyContentDef`/`AlignItemsDef` expose **`Start`/`End`** but map them to Bevy's
  **`FlexStart`/`FlexEnd`** (R14): these follow `RowReverse`/`ColumnReverse` (justify) and
  `WrapReverse` (align) exactly like CSS `flex-start`/`flex-end`, so CSS tutorials apply. Bevy's
  physical `Start`/`End` (which ignore the reversal) are not exposed. Document the mapping.
- **`AlignItemsDef` has no `Stretch` in v1** — every leaf `UiNodeDef` always has a definite
  `Val::Px` cross-axis size (from its own `size:` field), and `Stretch` only affects children with
  an *indefinite* cross-axis size. It would be silently inert on every leaf child, and only do
  anything on a nested `Group` with an `Auto` cross-axis size — a v2-adjacent case, not a v1 one.
  Revisit if `flex_grow`/auto-sizing children land later.
- Explicit defaults (not left to `#[derive(Default)]`'s "first variant" default, which would
  silently be whatever's declared first): `FlexDirection::Row`, `JustifyContent::Start`,
  `AlignItems::Start`, `FlexWrap::NoWrap` (`Start` = Bevy `FlexStart`, per R14). `Start` for
  `justify_content` is deliberate, not
  arbitrary — it's the only value that does something sensible on the common case of an
  auto-sized `Group` (see Critical fix #2).

### `UiNodeDef` trait methods

`Group` slots into the existing polymorphic `id()`/`size()`/`position()`/`absolute()`/`align()`
methods like every other variant. `size()` returns `(0.0, 0.0)` when both axes are `Auto` (the
caller-built wrapping `Node` uses `Val::Auto`/`Val::Percent` in that case, not the returned tuple
— see below) and `align()` returns `UiTextAlign::Center` (unused for groups, kept only so the
shared match stays exhaustive without a separate trait).

### Spawn logic — recursive, via a shared spawn-context struct (not positional params)

`spawn_ui_element_node` already threads ~10 parameters through (7 of them shared state) for the composite-widget arms
(`radar_handles`, `asset_server`, `atlas_layouts`, `asset_catalog`, `item_catalog`,
`inventory_ui`, `container_ui`). Recursing into a `Group`'s children with all 7 shared-state fields threaded
positionally would be error-prone and unreadable. This codebase already has the answer for this
exact shape — `ChildSpawnCtx<'a>` (`spawn_primitive_children`, documented in
`crates/ironhold_core/src/CLAUDE.md`) bundles equivalent per-recursion-frame state. Introduce a
parallel `UiSpawnCtx<'a>` bundling the 7 shared-state fields (`radar_handles` — keeping its `// det: lookup-only` marker, as `ChildSpawnCtx` does — `asset_server`, `atlas_layouts`, `asset_catalog`, `item_catalog`, `inventory_ui`, `container_ui`), threaded as `&mut UiSpawnCtx` everywhere
`spawn_ui_element_node` is called (including its own recursive `Group` arm) — collapses each
~200-character call site to one short one and means the next resource a panel arm needs is a
one-line struct field, not an edit to three call sites.

One real Bevy 0.18 borrow detail: `atlas_layouts: Option<&mut Assets<TextureAtlasLayout>>` is not
`Copy` — each loop iteration (including the new recursive one) must reborrow via
`atlas_layouts.as_deref_mut()`, exactly as both existing call sites already do. Same for
`inventory_ui`/`container_ui`. `UiSpawnCtx` doesn't remove this need, just gives it one place to
happen correctly instead of three.

Both existing loops (`ui_panel:`'s children loop, and the flat absolute-mode loop) build a
per-child `Node` from `el.size()`/`.position()`/`.absolute()`/`.align()`. Factor that into one
shared `fn build_child_node(el: &UiNodeDef, force_absolute: bool) -> Node` used by all
three sites. (Alignment comes from `el.align()` itself; the real difference between the two
existing loops is that panel mode honours `el.absolute()` while absolute mode always forces
`PositionType::Absolute`. `Group`'s children loop behaves like panel mode: `force_absolute: false`.)

`spawn_ui_element_node`'s new `UiNodeDef::Group(g)` arm mutates the incoming `node` before
spawning — the exact pattern `Label`/`Button`'s `clip` field already established this session:

```rust
UiNodeDef::Group(g) => {
    let mut group_node = node;
    group_node.flex_direction = g.flex_direction.into();
    group_node.justify_content = g.justify_content.into();
    group_node.align_items = g.align_items.into();
    group_node.flex_wrap = g.flex_wrap.into();
    group_node.column_gap = Val::Px(g.gap);
    group_node.row_gap = Val::Px(g.gap);
    group_node.padding = UiRect::all(Val::Px(g.padding));
    group_node.width = g.width.into();   // UiSizeDef -> Val
    group_node.height = g.height.into();
    if g.clip { group_node.overflow = Overflow::clip(); }

    let mut ec = parent.spawn((Name::new(format!("Group: {}", ctx.path_or_id(&g.id))), group_node));
    if let Some((r, g_, b, a)) = g.background_color {
        ec.insert(BackgroundColor(Color::srgba(r, g_, b, a)));
    }
    ec.with_children(|parent| {
        for child in &g.children {
            let mut child_node = build_child_node(child, false);
            child_node.flex_shrink = 0.0; // R15: size: means exactly that many px
            spawn_ui_element_node(parent, child, child_node, ctx);
        }
    });
}
```

(`walk_ui_nodes` is for the flat-scan sites elsewhere, not for spawning, which is naturally
recursive. Spawning also stops at `MAX_UI_DEPTH`, with one `warn!`.)

### Absolute children — verified correct, including nested

`ActionBar`/`DialoguePanel`/`InventoryPanel`/`ShopPanel`/`ContainerPanel` are hardcoded
`absolute() == true` — they already opt out of `ui_panel:`'s flex flow today. system-architect
verified against Bevy 0.18 source that this generalizes correctly to nesting: `PositionType::
Absolute`'s own doc is *"independent of all other nodes, but relative to its **parent** node"* —
taffy does not implement CSS's nearest-positioned-ancestor chain, so an absolute child always
resolves against its direct parent regardless of that parent's own position type. No
special-casing needed.

**Two consequences that must be documented, not just true-by-construction:**
- A nested panel's `position:` silently changes meaning from screen coordinates to
  group-box-relative coordinates. Every shipped panel today is authored in screen coordinates —
  not a regression (new content only), but the single most likely "why is my inventory panel
  off-screen" question this feature will generate.
- An auto-sized `Group` (`width`/`height` both `Auto`) whose children are **all** `absolute: true`
  collapses to a zero-size box (verified: taffy's `generate_anonymous_flex_items` filters out
  absolutely-positioned items from content-size calculation entirely) — so that panel then
  positions against a box with no meaningful size. This is exactly the case
  `UiPanelDef.height`'s own existing doc comment already warns about ("Set this when the panel
  contains absolutely-positioned children... so the panel has a known size to contain them") —
  cite that precedent. Add a scene-load `warn!` (and matching `ironhold_cli validate` check) for
  an auto-sized `Group` whose children are all `absolute: true`.

Also worth an explicit doc callout, not a code change: `Visibility::Hidden` does not remove a node
from layout (only `Display::None` does) — a hidden child inside an auto-sized `Group` still
reserves its space.

### Click pass-through (R7)

A `Group` never blocks pointer input (corrected in pass 2, see R7): it carries no `Interaction`
and no `FocusPolicy::Block`, exactly like `ui_panel:` today. Setting `FocusPolicy::Pass`
explicitly would be a no-op (`Node`'s default) and is not done. It differs from the
inventory/shop/container panel roots and the overlay backdrop, which block deliberately
(`ui_panel_blocker.rs`). Interactive children (`Button` etc.) still capture their own clicks. This
holds even with `background_color` set (R16): a coloured Group is decoration, not a dialog box.
Required test: a full-screen `Group` has no `Interaction`/`Block`, and a click on it changes the
target.

### Edge anchoring recipe (R8)

A top-level node's `position:` is always a top-left pixel offset; v1 has no `right:`/`bottom:`
anchors. The documented pattern for bottom/right-anchored UI is a full-screen root `Group`
(`width: Percent(100.0), height: Percent(100.0), flex_direction: Column,
justify_content: SpaceBetween`) containing a top row and a bottom row; a row's own
`justify_content: End` right-aligns. Showcased in `ui_demo`. `mobile_ui_demo` must reuse this, not
invent a parallel anchoring mechanism.

### `ui_panel:` — kept as-is, not touched

`GameSceneV2.ui_panel: Option<UiPanelDef>` and its dedicated spawn path are left completely
unchanged. `Group` is a strictly additive, independent mechanism — no existing scene's behavior
changes, and the two mechanisms remain genuinely separate (not one reimplemented in terms of the
other) for this plan. (Optional future cleanup, explicitly **out of scope**: `ui_panel:` could
eventually be reimplemented as sugar for "wrap `ui:` in an implicit root `Group`" — a refactor
with its own risk/reward, not a dependency of this feature — logged to
`planning/claude_suggestions.md`.)

### Scope boundary

- Does not touch `IconButton`, `Rect`, `Label`, `Button`, `StatBar`/`StatSpread`/`StatRadar`, or
  any panel type's own internals — they become **children** of a `Group` unchanged, with no new
  fields of their own.
- Does not add flex-grow/shrink/basis or per-child margin in v1 — `gap`/`padding` cover the common
  "space things out" cases the two motivating incidents actually needed; every other `UiNodeDef`
  variant would need three new fields to participate meaningfully in a parent's stretch behavior,
  which is real schema surface not yet justified by a concrete use case. Logged as a natural v2
  extension in `planning/claude_suggestions.md`.
- Does not add CSS Grid — flexbox only, matching what Bevy/`taffy` already expose and what every
  existing hardcoded composite-widget layout in this codebase already uses.
- No RON-reachable way to toggle a `Group`'s visibility at runtime in v1 — `Action::SetEntityVisible`
  resolves against `SpawnRegistry` (world entities), not UI node ids, and this plan doesn't add a
  UI-id-based equivalent. Likely the first real follow-up ask ("hide this whole group") — logged
  to `planning/claude_suggestions.md` as a known v2 gap rather than a surprise.
- Recursion depth: capped at 16 (`MAX_UI_DEPTH`; double the nested-prefab cap of 8, per
  `crates/ironhold_core/src/CLAUDE.md` precedent), warn-and-truncate past that, applied identically
  by the spawner and by `walk_ui_nodes` (R6). Unlike nested prefabs there's no *cycle* risk
  (`children:` is a plain inline tree). Note: RON parsing is already bounded (ron's default
  `recursion_limit` is 128), so a pathologically deep tree fails to parse cleanly rather than
  overflowing the stack; the 16 cap exists so spawn and diagnostics agree and layouts stay sane,
  not as a stack-safety guard.
- Panel singletons are unaffected by nesting: `InventoryPanel`/`ContainerPanel`'s arms still clear/
  store a single `Option<Entity>` regardless of what `Group` (if any) they're nested inside —
  `Group` does not enable two of the same singleton panel in one scene. Worth restating in docs.

## Phases

| Phase | Scope | Status | Backlog dependencies |
|---|---|---|---|
| **v1 - landscape** | Everything in this file: `Group`, `UiSizeDef` (`Auto`/`Px`/`Percent`), walker, diagnostics, `ui_demo` showcase, `options.scene.ron` retrofit | Done - Completed: `b7fbf1a` (2026-10-06) | none (touch input is NOT required: a landscape layout demo can be keyboard/mouse) |
| **v2 - portrait / touch-first** | `Percent` (or `max_width`) on leaf `size:`, runtime show/hide of a `Group` by UI id (touch-only controls), safe-area insets, optionally `flex_grow` + margin | Queued (backlog: "Flex `Group` v2") | v1; a UI-id-based visibility action (today `Action::SetEntityVisible` resolves `SpawnRegistry` entities only); confirmed touch input in the web build (`action_bar_mouse_click.md` declines to promise it) |

`mobile_ui_demo` is split accordingly in `planning/backlog.md`: **v1 (landscape)** is blocked only
on this feature's v1; **v2 (portrait/touch)** is blocked on the v2 items above.

## Tasks
- [x] _(done on `feature/ui_flex_group`; the nested-fixture unit test lands with `GroupDef`)_ Add `walk_ui_nodes` + `walk_ui_nodes_pathed` + `MAX_UI_DEPTH` to `schema/scene_v2.rs` and convert
      every existing flat `scene.ui` scan (re-grep both crates; `scene_loader.rs`'s `radar_handles`
      pre-pass + 4 `warn_*`, `validate.rs`'s 9 scans, `query.rs`, and `GameSceneV2::validate()`).
      Standalone, behaviour-preserving refactor with zero new schema -- do this **first**, before
      `GroupDef` exists. Its unit test gets a nested fixture as soon as `GroupDef` lands.
- [x] `GameSceneV2::validate()`: exempt `Group` from the empty-id check, skip `""` in duplicate
      detection, detect duplicates across all nested nodes (R2).
- [x] Schema: `UiSizeDef` (`Auto`/`Px`/`Percent`) + `GroupDef` + `FlexDirectionDef`/
      `JustifyContentDef`/`AlignItemsDef`/`FlexWrapDef` enums, explicit defaults on all four
- [x] Add `Group` to `UiNodeDef` + its `id()`/`size()`/`position()`/`absolute()`/`align()` arms
- [x] _(done; a Group's `Name` is its id or plain "Group", not its path - paths are for diagnostics only)_ `scene_loader.rs` (`runtime/scene_manager/`): `UiSpawnCtx<'a>` (7 fields); factor
      `build_child_node(el, force_absolute)`; recursive `Group` arm (children get `flex_shrink: 0.0`, R15); depth
      cap (16, one `warn!`)
- [x] Load log prints top-level + total node counts; `query.rs` adds `ui_node_count` (R6)
- [x] _(one shared `ui_layout_diagnostics` in `schema/scene_v2.rs`; errors in `cross_file_checks`, inert/surprising settings are `--strict` warnings, the engine `warn!`s all of them at scene load)_ Diagnostics, all naming nodes by path (R9), with matching `ironhold validate` checks: auto-sized
      `Group` whose children are all `absolute: true`; `clip: true` with both axes `Auto`; the
      R10 list
- [x] _(gate passed: all 15 shipped projects validate clean)_ `ironhold validate` calls `GameSceneV2::validate()` (R3) -- first run it across all
      `assets/projects/*`; if any shipped project fails, split into its own backlog item
- [x] _(done except a literal end-to-end click test: R7 is asserted as 'no Interaction / no FocusPolicy::Block' - see claude_suggestions.md)_ Tests: nested `Group`s parse and spawn with correct `Node` flex properties (incl. `gap` setting
      both `row_gap`/`column_gap`); `absolute: true` child escapes the flow like `ui_panel:`;
      `Auto`/`Px`/`Percent` map to the matching `Val`; nested `StatRadar` still gets a material;
      nested duplicate-keyed `ActionBar` still flagged by `warn!` and by `validate`; empty-id
      `Group` passes `validate()` while an empty-id leaf still fails; nested duplicate ids fail; a
      click on the empty area of a full-screen `Group` reaches the world (R7); walker stops at depth 16
- [x] _(project, registration in test_web.py/README/index.html/validate_projects.rs done and validated; the baseline screenshot `screenshot_baselines/scenes/ui_demo_main.png` is generated in the WASM dev-build step, so the index.html card thumbnail is missing until then)_ New minimal `ui_demo` project (R5) with labeled stations: nested rows/columns, a
      `Percent(100.0)` + `SpaceBetween` HUD bar (title left, buttons right), the full-screen
      bottom-anchored root recipe (R8), the spacer idiom. Register it: `test_web.py` `PROJECTS`,
      baseline screenshot, `index.html` card (see root `CLAUDE.md` "Adding a new asset project").
      Use plain ASCII hyphens in comments/on-screen text (the engine font has no em-dash glyph).
- [x] _(scene retrofitted and validated; its baseline screenshot is regenerated in the WASM dev-build step)_ Retrofit `3rd_person_game_demo/scenes/options.scene.ron` to nested `Group`s with the even
      rhythm from R1 (item gap 8, section gap 24; the 4-button volume row maps cleanly at gap 10).
      Add a top-of-file comment pointing at the `docs/20_data_formats.md` Group section. Regenerate
      only this scene's baseline: `python test_web.py --project 3rd_person_game_demo
      --update-baseline 3rd_person_game_demo_options --skip-build` -- NOT a blanket
      `--update-baselines` (other projects' scenes, e.g. `camera_modes`, have ~34
      deliberately-overflowing `Label`/`Button` defs that must not shift; within
      `3rd_person_game_demo` the other scenes are main, start_menu, character_select and pause). The options baseline diff
      is expected (R1).
- [x] Docs (`docs/20_data_formats.md`): new `Group((...))` section with a full field table, a short
      flexbox primer (link MDN), and these callouts:
      1. "Which UI mechanism do I reach for" table (`ui_panel:` vs `Group` vs plain `position:` vs an
         always-absolute HUD widget).
      2. `SpaceBetween`/`SpaceAround`/`SpaceEvenly` need a definite (`Px`/`Percent`) main-axis size.
      3. A nested child's `position:` is relative to its `Group`'s box, not the screen; don't nest
         `ActionBar`/`DialoguePanel`/`InventoryPanel`/`ShopPanel`/`ContainerPanel`.
      4. `ui_panel:` -> `Group` default drift (padding, gap, background, clip do not carry over);
         `width: Px(380.0)` vs `ui_panel:`'s bare `380.0` and the parse error you get; `width`/`height`
         not `size:`.
      5. `Visibility::Hidden` children still occupy layout space.
      6. Anchoring to screen edges (R8) and click pass-through (R7).
      7. `Percent(n)` under an `Auto`-sized parent resolves cyclically against the parent's final
         content-derived size; a `Percent` group inside `ui_panel:` (auto box unless
         `width`/`height` set) will surprise designers (R20).
      8. Spacer idiom `Group((height: Px(20.0), children: []))` in a Column, `width: Px(n)` in a Row.
      9. Top-level `Group` without `ui_panel:` is positioned at `position:` from the screen, and
         `Percent` there is measured against the window; inside `ui_panel:` it flows like any child.
      10. The R22 callouts (sizes-to-content vs text, BorderBox padding, absolute-child insets,
          coloured Group lets clicks through, FlexStart/FlexEnd mapping).
      11. Link `ui_demo` and the retrofitted `options.scene.ron` as the examples to copy.
      Also update the six leaf `position` rows, the six `absolute` rows and the UI Panel intro (R11), `docs/15_authoring_tools.md` (validate checks table) and README's example-project table (R21).
- [x] `crates/ironhold_core/src/CLAUDE.md`: "`scene.ui` must be walked via `walk_ui_nodes`, never
      iterated flat" next to the `spawn_primitive_children` rule.
- [x] Backlog bookkeeping: `mobile_ui_demo` split into v1/v2 (done at plan time); v2 entries added when v1 shipped
      ("Flex `Group` v2", plus the volume-readout, canvas-fit and baseline-regeneration follow-ups).

## Open questions
- Exact enum variant lists for `JustifyContentDef` beyond the 6 named above — expand later if a
  real layout needs a Bevy/taffy value not yet exposed.
- Should `Group`'s `background_color`/`clip`/fixed-size fields make it a de facto replacement for
  hand-rolled `Rect`-behind-a-column patterns designers use today? Probably yes, organically — not
  a compatibility concern since `Rect` remains untouched and still works standalone.

## Acceptance criteria
- Given a `Group` with `flex_direction: Row, justify_content: SpaceBetween, width: Percent(100.0)`,
  when the scene loads, then its children are laid out in a row with even spacing across the full
  window width — no hand-computed pixel `position:` needed on any of them.
- Given a `Group` nested inside another `Group`, when the scene loads, then both flex layouts apply
  correctly (a "row of buttons inside a column" composes exactly as it would in CSS flexbox).
- Given a `Group` child with `absolute: true`, when the scene loads, then that child escapes the
  flex flow and positions via its own `position:`, relative to the group's box — matching
  `ui_panel:`'s existing `absolute: true` behavior exactly, including when nested.
- Given a `Group` with `width`/`height` both `Auto` (the default), when the scene loads, then the
  group's box sizes to fit its children's natural size, not a fixed or window-relative box.
- Given a `StatRadar` or `ActionBar` nested inside a `Group`, when the scene loads or is validated,
  then it still gets its material handle / is still covered by every existing `warn_*`/
  `ironhold_cli validate` diagnostic — nesting must not silently disable existing coverage.
- Given an existing scene using `ui_panel:` and no `Group`, when the scene loads, then nothing
  about its layout changes — this feature is purely additive.
- Given a full-screen `Group` with no background, when the player clicks its empty area, then the
  click reaches the world (clicks pass through a `Group`).
- Given a `Group` with no `id`, when `ironhold validate` runs, then it passes; given a leaf node with
  no `id`, or two nodes anywhere in the tree sharing an `id`, then it fails.
- Given a diagnostic about a `Group`, when it is printed, then it names the node by path
  (`ui[2].children[0]`), not by an empty id.
- Given a `Percent(100.0)` `Group` with `SpaceBetween` on a window narrower than its children's
  combined `size:` widths, when the scene loads, then leaves keep their authored `size:` (no
  shrink, `flex_shrink: 0`) rather than silently collapsing the spacing.
- Given a `Group` subtree deeper than 16 levels, when `ironhold validate` runs, then it reports the
  path of the truncated subtree.

## Notes moved from `crates/ironhold_core/src/CLAUDE.md` (2026-10-08, core CLAUDE.md split)

These paragraphs were in the crate `CLAUDE.md` and are kept here verbatim; that file now carries only the condensed current-state rule. Wording such as "above"/"below" refers to the old file.

### b:108: `scene.ui` must be walked via `walk_ui_nodes`
<!-- moved-from-claude-md: b:108 -->

- **`scene.ui` must be walked via `schema::scene_v2::walk_ui_nodes` (or `walk_ui_nodes_pathed` for diagnostics), never iterated flat.** Same class of invariant as the rule above: a consumer that loops over the top-level `Vec` silently under-covers any nested `ui:` node (e.g. a `StatRadar` inside a future `Group` would get no material, an `ActionBar` would escape every duplicate-key warning). The walker is pre-order, depth-capped at `MAX_UI_DEPTH` (16, a top-level node is depth 1) so diagnostics cover exactly the nodes that spawn, and yields nodes only -- `.enumerate()` it if you need a per-node tag. The one deliberate exception is the spawn loops in `scene_loader.rs`, which recurse structurally. See `planning/features/ui_flex_group.md`.
