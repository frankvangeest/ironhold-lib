# Feature: Nestable flexbox `Group` UI node

_Status: Draft (revised after 2 parallel plan reviews)_
_Planned at: `101bf03` (2026-08-28)_

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
  `justify_content: FlexEnd`/`SpaceBetween` gives real edge/spread anchoring, without inventing a
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

### Critical fix #1 — nesting must not blind the 11 existing flat `scene.ui` scans

The current code scans `scene.ui: Vec<UiNodeDef>` **flat** in 11 places across 3 files, and one of
them is a functional dependency, not a diagnostic:

- `scene_loader.rs` — the `radar_handles` pre-pass (builds the `HashMap` a `StatRadar` arm looks
  its material up in; a `StatRadar` nested inside a `Group` would silently get no material and
  render nothing), plus four `warn_*` diagnostics (`warn_cross_bar_duplicate_keys`,
  `warn_same_player_gamepad_duplicate_slots`, `warn_missing_player_stat_templates`,
  `warn_gamepad_key_without_gamepad_index`).
- `ironhold_cli/src/commands/validate.rs` — the five CLI mirrors of those checks, plus the
  `invalid_font_size` check (`ui_label_font_size.md`).
- `ironhold_cli/src/commands/query.rs` — `ui_count: scene.ui.len()`.

**Fix, mandatory for this feature, not deferred:** add one shared pre-order walker to
`schema/scene_v2.rs`:

```rust
pub fn walk_ui_nodes(nodes: &[UiNodeDef]) -> impl Iterator<Item = (usize, &UiNodeDef)> {
    // pre-order: a node before its children, index is a stable pre-order position
    // used identically by both scene_loader.rs's and validate.rs's same-bar-vs-
    // cross-bar collision checks (they currently key by positional index).
}
```

Convert all 11 sites to it in this same change — a `StatRadar`/`ActionBar` nested in a `Group`
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

- `JustifyContentDef`/`AlignItemsDef` expose **`Start`/`End`**, not Bevy's separate
  `FlexStart`/`FlexEnd` — Bevy 0.18 has both, and they genuinely differ under
  `RowReverse`/`ColumnReverse` (`Flex*` follow the reversal, plain `Start`/`End` are physical).
  Document this explicitly rather than leaving it implicit, since `Group` exposes reverse
  directions.
- **`AlignItemsDef` has no `Stretch` in v1** — every leaf `UiNodeDef` always has a definite
  `Val::Px` cross-axis size (from its own `size:` field), and `Stretch` only affects children with
  an *indefinite* cross-axis size. It would be silently inert on every leaf child, and only do
  anything on a nested `Group` with an `Auto` cross-axis size — a v2-adjacent case, not a v1 one.
  Revisit if `flex_grow`/auto-sizing children land later.
- Explicit defaults (not left to `#[derive(Default)]`'s "first variant" default, which would
  silently be whatever's declared first): `FlexDirection::Row`, `JustifyContent::Start`,
  `AlignItems::Start`, `FlexWrap::NoWrap`. `Start` for `justify_content` is deliberate, not
  arbitrary — it's the only value that does something sensible on the common case of an
  auto-sized `Group` (see Critical fix #2).

### `UiNodeDef` trait methods

`Group` slots into the existing polymorphic `id()`/`size()`/`position()`/`absolute()`/`align()`
methods like every other variant. `size()` returns `(0.0, 0.0)` when both axes are `Auto` (the
caller-built wrapping `Node` uses `Val::Auto`/`Val::Percent` in that case, not the returned tuple
— see below) and `align()` returns `UiTextAlign::Center` (unused for groups, kept only so the
shared match stays exhaustive without a separate trait).

### Spawn logic — recursive, via a shared spawn-context struct (not positional params)

`spawn_ui_element_node` already threads ~9 parameters through for the composite-widget arms
(`radar_handles`, `asset_server`, `atlas_layouts`, `asset_catalog`, `item_catalog`,
`inventory_ui`, `container_ui`). Recursing into a `Group`'s children with all 9 threaded
positionally would be error-prone and unreadable. This codebase already has the answer for this
exact shape — `ChildSpawnCtx<'a>` (`spawn_primitive_children`, documented in
`crates/ironhold_core/src/CLAUDE.md`) bundles equivalent per-recursion-frame state. Introduce a
parallel `UiSpawnCtx<'a>` bundling the same 9 fields, threaded as `&mut UiSpawnCtx` everywhere
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
shared `fn build_child_node(el: &UiNodeDef, container_align: UiTextAlign) -> Node` used by all
three sites (the two existing loops, plus `Group`'s own new children loop).

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

    let mut ec = parent.spawn((Name::new(format!("Group: {}", g.id)), group_node));
    if let Some((r, g_, b, a)) = g.background_color {
        ec.insert(BackgroundColor(Color::srgba(r, g_, b, a)));
    }
    ec.with_children(|parent| {
        for (_, child) in walk_ui_nodes(&g.children).filter(|(_, n)| /* direct children only, depth-limited by caller */ true) {
            let child_node = build_child_node(child, child.align());
            spawn_ui_element_node(parent, child, child_node, ctx);
        }
    });
}
```

(The sketch above shows the shape; the actual per-child loop just iterates `&g.children` directly
— `walk_ui_nodes` is for the 11 *flat-scan* sites elsewhere, not for spawning, which is naturally
recursive already.)

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
- Recursion depth: capped at 16 (double the nested-prefab cap of 8, per
  `crates/ironhold_core/src/CLAUDE.md` precedent), warn-and-truncate past that. Unlike nested
  prefabs there's no *cycle* risk (`children:` is a plain inline tree, not a catalog reference),
  but a cap is still worth ~5 lines: a pathologically deep authored tree would stack-overflow at
  RON *parse* time (recursive `Vec<UiNodeDef>` deserialization, uncapped), before any engine code
  runs — and the WASM main-thread stack is smaller than native's. Trusted local content, so this
  is a cheap defensive cap, not a security boundary.
- Panel singletons are unaffected by nesting: `InventoryPanel`/`ContainerPanel`'s arms still clear/
  store a single `Option<Entity>` regardless of what `Group` (if any) they're nested inside —
  `Group` does not enable two of the same singleton panel in one scene. Worth restating in docs.

## Tasks
- [ ] Add `walk_ui_nodes(nodes: &[UiNodeDef]) -> impl Iterator<Item = (usize, &UiNodeDef)>` to
      `schema/scene_v2.rs` and convert all 11 existing flat `scene.ui` scans to it: `scene_loader.rs`'s
      `radar_handles` pre-pass + 4 `warn_*` diagnostics, `validate.rs`'s 5 CLI mirrors +
      `invalid_font_size`, `query.rs`'s `ui_count`. This is a standalone, testable-against-current-
      behavior refactor with zero new schema — do this **first**, before `GroupDef` exists.
- [ ] Schema: `UiSizeDef` (`Auto`/`Px`/`Percent`) + `GroupDef` + `FlexDirectionDef`/
      `JustifyContentDef`/`AlignItemsDef`/`FlexWrapDef` enums, explicit defaults on all four
      (`schema/scene_v2.rs`)
- [ ] Add `Group` to `UiNodeDef` + its `id()`/`size()`/`position()`/`absolute()`/`align()` arms
- [ ] `scene_loader.rs`: bundle the 9 recursive spawn params into `UiSpawnCtx<'a>` (mirrors
      `ChildSpawnCtx<'a>`); factor `build_child_node` out of the two existing per-child-`Node`-
      building call sites; add the recursive `Group` arm to `spawn_ui_element_node`; recursion
      depth cap (16, warn-and-truncate)
- [ ] Scene-load `warn!` (+ matching `ironhold_cli validate` checks): auto-sized `Group` whose
      children are all `absolute: true` (collapses to zero, per `UiPanelDef.height`'s existing
      precedent); `clip: true` paired with both axes `Auto` (inert)
- [ ] Tests — nested `Group`s parse and spawn with correct `Node` flex properties (including that
      `gap` sets both `row_gap`/`column_gap`); a `Group` child with `absolute: true` still escapes
      the flex flow exactly like `ui_panel:` today; `width`/`height: Auto` produce `Val::Auto`,
      `Percent(n)`/`Px(n)` produce the matching `Val`; a nested `StatRadar` still gets a material
      handle and renders; a nested duplicate-keyed `ActionBar` is still flagged by both the
      scene-load `warn!` and `ironhold_cli validate`
- [ ] Docs (`docs/20_data_formats.md`) — new `Group((...))` section: full field table, a short
      flexbox primer (link MDN rather than re-teach CSS), and five specific callouts the reviews
      identified as real designer traps:
      1. A decision table for "which UI mechanism do I reach for" (`ui_panel:` vs `Group` vs plain
         `position:` vs a direct always-absolute HUD widget).
      2. `SpaceBetween`/`SpaceAround`/`SpaceEvenly` need a definite (`Px`/`Percent`) main-axis size
         to do anything — inert on an `Auto`-sized `Group`.
      3. A nested child's `position:` is relative to its `Group`'s box, not the screen; don't nest
         `ActionBar`/`DialoguePanel`/`InventoryPanel`/`ShopPanel`/`ContainerPanel` (they always
         position absolutely against whatever they're placed in — nesting only changes what their
         coordinates are measured from, which is rarely what a designer wants).
      4. `ui_panel:` → `Group` default drift if a designer migrates a layout by hand: `ui_panel:`'s
         own defaults (padding, gap, near-black background, always-on clip) do NOT carry over —
         `Group`'s defaults are all off/zero/transparent.
      5. `Visibility::Hidden` children still occupy layout space (only `Display::None` doesn't).
- [ ] `crates/ironhold_core/src/CLAUDE.md` — add "`scene.ui` must be walked via `walk_ui_nodes`,
      never iterated flat" next to the existing "`spawn_primitive_children` is the only child-
      spawn path" rule — same class of invariant, same reason (a new consumer that iterates
      `scene.ui` flat silently under-covers nested content).
- [ ] Worked example — retrofit `3rd_person_game_demo/scenes/options.scene.ron` (a hand-computed
      vertical stack of labels/toggles at y=30..445, plus a hand-computed horizontal row of 4
      volume buttons at x=100/200/300/400 with an implied 10px gap) to use nested `Group`s. Concrete,
      absolute-mode (no `ui_panel:` today), and small enough to diff before/after in review.
      Regenerate only this project's baseline: `python test_web.py --project 3rd_person_game_demo
      --update-baseline 3rd_person_game_demo_options --skip-build` — do NOT run a blanket
      `--update-baselines` (this project's `camera_modes`/`local_coop_demo` scenes have ~34
      deliberately-overflowing `Label`/`Button` defs unrelated to this change that must not shift).

## Open questions
- Exact enum variant lists for `JustifyContentDef` beyond the 6 named above — expand later if a
  real layout needs a Bevy/taffy value not yet exposed.
- Should `Group`'s `background_color`/`clip`/fixed-size fields make it a de facto replacement for
  hand-rolled `Rect`-behind-a-column patterns designers use today? Probably yes, organically — not
  a compatibility concern since `Rect` remains untouched and still works standalone.
- `walk_ui_nodes`'s pre-order index must stay **identical** between `scene_loader.rs`'s and
  `validate.rs`'s same-bar-vs-cross-bar collision checks (both currently key by positional node
  index) — implementation must keep both call sites using the exact same walk, not two similar but
  independently-written ones.

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
