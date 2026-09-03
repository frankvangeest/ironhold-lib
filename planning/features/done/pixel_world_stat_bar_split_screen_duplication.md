# Feature: Pixel-style `world_stat_bar` Split-Screen Duplication

_Status: Done_
_Planned at: `039ee00` (2026-07-17)_

**Code review note (2026-07-17):** alignment-reviewer, system-architect, debug-detective, and
ux-gamedesigner-reviewer all ran in parallel post-implementation. No blocking findings from any of
the four. alignment-reviewer confirmed zero new RON schema and full designer-reachability, and
caught one stale doc comment (`lib.rs`'s `world_label_screen_pos_system` doc still listed Pixel
among rank-0-only consumers) — fixed. system-architect confirmed the mesh/material sharing is safe
and `LevelEntity` is preserved on every rank, and logged one non-blocking observation (the fill
mesh/material could technically be shared across ranks too, since neither is ever mutated in
place) to `planning/claude_suggestions.md`. debug-detective independently verified the same safety
properties by tracing `world_pixel_bar_update_system` directly and confirmed the revised test
assertions test the real invariant (rank lives on the anchor, not on border/bg/fill children — an
assumption the first draft of the new tests got wrong, caught by two failing test runs before this
review). ux-gamedesigner-reviewer caught a real doc defect: the initial docs draft named `Icon` as
an already-available production style — `Icon` does not exist yet (see `world_icon_stat_bar.md`,
a separate not-yet-implemented plan) — fixed in both `docs/20_data_formats.md` and the
`local_coop_demo` prefab comment. Full `ironhold_core` test suite (all 16 test files) + `cargo
check -p ironhold_cli` green. WASM dev build succeeded; playtest confirmed by Frank in
`local_coop_demo` room3 (both split-screen players' mana bars render as Pixel style, correctly
duplicated in both viewports), no console errors.

**Plan-review note (2026-07-17):** system-architect — Ready. All three technical claims verified
directly against source (single Pixel-arm call site confirmed; `InheritedVisibility` cascade
confirmed already proven at rank-0 in shipped code, not an untested mechanism; mesh/material
sharing confirmed safe against `world_pixel_bar_update_system`'s actual query shape). Two minor
notes folded in below: preserve `LevelEntity` on every rank's children (easy to lose in the loop
refactor), and the fill *mesh* geometry could technically be shared too (kept per-rank anyway —
correct and simpler, not worth the added complexity to optimize away 3 trivial allocations).
ux-gamedesigner-reviewer — confirmed the RON swap for `local_coop_demo`'s `player_p1_split`/
`player_p2_split` is a clean drop-in (all three shared fields — `stat_key`, `offset`, `fill_color`
— remain valid under `Pixel`; no other field needs touching) but found the doc task badly
under-scoped: the existing "Split-screen visibility" callout has two more stale sentences beyond
the one the plan named, and the inline prefab comment explaining *why* Ascii was chosen becomes
actively wrong once the swap happens. Both folded into the Tasks below. Also flagged a cross-cutting
gap (with `world_icon_stat_bar.md`): no existing doc signals that Ascii is a prototyping-only style
slated for eventual retirement — added as a task here since this feature is likely to land first.

## What
Today, in a split-screen scene, a `stat_label` or **Ascii**-style `world_stat_bar` correctly shows
in every simultaneously-visible viewport (via `WorldLabelRank` sibling duplication — Phase 4 of
`split_screen_camera_followups.md`), but a **Pixel**-style `world_stat_bar` shows in **at most
one** viewport — a documented, accepted limitation since that feature shipped, deferred at the
time because it needed to duplicate a whole anchor+children mesh hierarchy, not a single entity.
This feature closes that gap: a Pixel bar duplicates across split-screen viewports exactly like an
Ascii bar already does, for **any entity kind** — NPC, prop, or player.

## Why
`player_stat_widgets.md` just shipped giving players first-class floating stat widgets, and its
own playtest aid (`local_coop_demo`'s `player_p1_split`/`player_p2_split`) had to use **Ascii**
specifically, *not* Pixel, purely because of this limitation — despite Pixel being the intended
production-quality style (the original `world_pixel_stat_bar.md` design doc frames Ascii itself as
"recognisable as a debug artefact," a stopgap for prototyping, not a shippable look). That is now a
concrete, current, reproducing case of the gap blocking real usage, not a hypothetical
"if a real project need surfaces" — the bar the note in the Ascii-only demo prefabs already flags.

Frank is also considering removing the Ascii style from the engine entirely once Pixel has full
feature parity (see `world_icon_stat_bar.md`, planned alongside this feature) — this is the
concrete blocker to that decision: Pixel cannot become the default/only production style until it
works correctly in split-screen, since removing Ascii today would leave *no* style that both looks
production-ready and works in split-screen.

## Approach

### The consolidation that makes this tractable now
When `split_screen_camera_followups.md` deferred this (2026-07-XX, before `player_stat_widgets`
existed), the Pixel spawn logic lived in **two** separate, duplicated call sites — `scene_loader.rs`'s
own spawn loop and `drain_dynamic_stat_ui_system`'s near-byte-for-byte copy — meaning "duplicate
the child hierarchy" would have meant fixing the same non-trivial logic twice. `player_stat_widgets`'s
Part A refactor already extracted **both** into one shared function,
`spawn_world_stat_bar_widget` (`capabilities/stat_display.rs`), which both call sites now invoke
identically. **This feature is therefore a single-site fix**, not two — a smaller, lower-risk change
than it would have been when first deferred.

### The fix
`spawn_world_stat_bar_widget`'s `WorldStatBarStyle::Pixel` arm currently spawns exactly one
anchor entity (carrying `WorldLabel`, no rank) with up to 3 `Mesh2d` children (border — conditional
on `border > 0.0`, background, fill — carrying `WorldPixelBarFillMarker`), parented via
`commands.entity(anchor).add_child(...)`. Wrap this construction in the same
`for rank in 0..ranks` loop the Ascii arm already uses (`ranks` is already computed from
`ctx.is_split_screen` at the top of the function — this is not a new gate, just applying an
existing one to the Pixel arm too):

```rust
for rank in 0..ranks {
    let mut anchor_cmds = commands.spawn((
        Name::new(format!("PixelBarAnchor: {} (rank {})", stat_key, rank)),
        Transform::default(),
        Visibility::default(),
        WorldLabel { /* unchanged fields */ },
    ));
    if rank > 0 {
        anchor_cmds.insert((WorldLabelRank(rank as u8), Visibility::Hidden));
    }
    let anchor = anchor_cmds.id();
    // border/bg/fill children spawn exactly as today, parented to this rank's anchor
}
```

**Children need no `Visibility`/`WorldLabelRank` component of their own** — Bevy's hierarchy
visibility propagation (`InheritedVisibility`) already cascades a parent's `Visibility::Hidden`
down to its children automatically, since the anchor already carries an explicit `Visibility`
component and the border/bg/fill children are spawned without one (default `Visibility::Inherited`
behavior). This is confirmed by how `world_label_screen_pos_system`/the visibility-toggling systems
already treat the anchor as the single source of truth for the whole bar's on/off state today (for
the single, rank-0-only case) — extending that same mechanism to N ranks needs no new propagation
logic, only N anchors instead of 1.

**Mesh/material handles should be shared (cloned), not re-registered, across ranks.** The border/
background quads are geometrically and visually identical across every rank of the same bar
instance (same `size`, `border`, `border_color`, `bg_color` — only the *fill* differs frame-to-frame,
and even then all ranks always show the identical value since they track the same entity's same
stat). Call `ctx.meshes.add(...)`/`color_mats.add(...)` **once** per bar instance (outside the rank
loop) and clone the resulting `Handle<Mesh>`/`Handle<ColorMaterial>` into each rank's border/bg
spawn — avoiding N× growth of `Assets<Mesh>`/`Assets<ColorMaterial>` for geometry that never
changes between ranks. The **fill** mesh/material must still be created fresh per rank (each rank's
fill entity needs its own `Transform.scale`/`ColorMaterial.color`, updated independently by
`world_pixel_bar_update_system` — already true for Ascii's N independent fill entities today, so
this isn't new complexity, just applied to Pixel too).

**No changes needed to `world_pixel_bar_update_system`.** It already queries
`Query<(&WorldPixelBarFillMarker, &mut Transform, &MeshMaterial2d<ColorMaterial>)>` with no
entity-count assumption — N independent fill entities (one per rank) already update correctly and
independently, exactly like `world_stat_bar_update_system` already handles N Ascii fill siblings.

### Scope discipline — what this does NOT touch
- **Damage popups and nameplate anchors remain single-instance**, exactly as `split_screen_camera_followups.md`
  left them. Those are unrelated entity kinds (not `world_stat_bar`) with their own separate spawn
  sites; duplicating them is a distinct, separately-scoped follow-up if ever needed, not bundled here.
- **No schema change.** `WorldStatBarStyle::Pixel`'s fields are unchanged; this is purely a runtime
  spawn-behavior fix. A project with existing Pixel bars gets the fix automatically on next load —
  no RON migration required.
- **Depth scaling for Pixel bars remains out of scope** (separate, pre-existing, documented
  limitation — `docs/20_data_formats.md`'s "Pixel bar depth scaling" note — unrelated to this
  feature and not touched by it).

## Tasks
- [x] `capabilities/stat_display.rs`: wrap `spawn_world_stat_bar_widget`'s `Pixel` arm in a
      `for rank in 0..ranks` loop; hoist the shared (border/background) mesh+material `.add()`
      calls outside the loop and clone handles per rank; keep the fill mesh/material creation
      inside the loop (one fresh instance per rank, as today). Ensure every rank's border/bg/fill
      child keeps its `LevelEntity` tag (present today) so scene-change cleanup frees all ranks,
      not just rank 0 — easy to lose when restructuring the spawn block into a loop
      (system-architect finding).
- [x] Tests — a new split-screen Pixel-bar duplication test (mirroring
      `test_stat_widgets_duplicate_ranks_when_scene_is_split_screen`, but asserting
      `WorldPixelBarFillMarker` count == `MAX_SPLIT_PLAYERS` and that non-split scenes still spawn
      exactly 1); a regression test that mesh/material asset counts don't grow 4x for the
      shared (border/bg) geometry specifically — only the fill entities should scale with rank count
- [x] `local_coop_demo`: switch `player_p1_split`/`player_p2_split`'s `world_stat_bar` from
      `style: Ascii(cells: 10, font_size: 14.0)` to an explicit `style: Pixel(size: (60.0, 6.0))`
      (not a bare `Pixel()` default — ux-gamedesigner-reviewer's suggestion, so the demo bars read
      as deliberately tuned for the showcase, not defaulted) as the playtest aid proving this
      feature. Confirmed a clean drop-in: `stat_key`/`offset`/`fill_color` are shared top-level
      `WorldStatBarDef` fields valid under either style; neither prefab sets `color_bands`, so
      there's no Ascii-vs-Pixel band-fallback difference to reconcile.
- [x] **Update the inline comment block above `player_p1_split`/`player_p2_split`**
      (`local_coop_demo/prefabs/prefabs.ron`) that currently explains *why* Ascii (not Pixel) was
      chosen — that reasoning becomes actively wrong once this feature ships and the swap above
      lands; rewrite it to state the opposite (Pixel now duplicates correctly; these prefabs use
      Pixel deliberately to showcase it) (ux-gamedesigner-reviewer finding — the plan's original
      task list only mentioned the `style:` swap itself, not this adjacent designer-facing comment).
- [x] Docs — `docs/20_data_formats.md`'s "Split-screen visibility" callout needs a full rewrite,
      not a one-word removal (ux-gamedesigner-reviewer finding: the existing paragraph has **three**
      claims that go stale, not one) — (1) remove Pixel from the "do not duplicate" list (now only
      damage popups and nameplates); (2) delete the "a combined bar will show its Ascii half in
      every viewport but its Pixel half in only one" sentence entirely (becomes false — both halves
      now duplicate); (3) **flip** the co-op guidance that currently tells designers to prefer
      Ascii over Pixel on split-screen players — it is now backwards and would actively steer
      designers to the wrong choice; state that any style works correctly on a co-op player.
- [x] `crates/ironhold_core/src/CLAUDE.md`'s "Pixel-style world stat bars ... remain single-instance"
      note: remove Pixel from that list (only damage popups/nameplates remain single-instance) —
      broader than the plan's original wording, this line lists Pixel by name and must change, not
      just get a pointer added (system-architect finding).
- [x] `planning/features/done/split_screen_camera_followups.md`: add a one-line pointer to this
      feature as the resolution of its historical "Not in scope" note (the done-feature doc itself
      is not reopened/rewritten, per convention).
- [x] Docs — add one soft-deprecation sentence for the Ascii style to `docs/20_data_formats.md`
      (cross-cutting finding, ux-gamedesigner-reviewer): Ascii is the silent default when `style` is
      omitted, so every project not explicitly setting `style` is building on the one style Frank is
      considering eventually retiring, with zero signal today. A single sentence — "`Ascii` is a
      prototyping/debug style; `Pixel` and `Icon` (see `world_icon_stat_bar.md`) are the
      production-quality styles; `Ascii` may be retired in a future version" — costs nothing now and
      makes an eventual removal far less disruptive. No hard warning or migration pass; just honest
      signposting. Whichever of this feature or `world_icon_stat_bar.md` lands first should add it.
      **Landed wording differs slightly** (ux-gamedesigner-reviewer caught during code review that
      `Icon` doesn't exist yet and would mislead designers into trying `style: Icon(...)`): shipped
      as "`Ascii` is a prototyping/debug style; `Pixel` is the production-quality choice," with no
      `Icon` mention — `world_icon_stat_bar.md` can add its own note once that style actually ships.

## Open questions
- None — the mechanism to reuse (`WorldLabelRank` + hierarchy visibility propagation) is already
  proven by the Ascii case; this applies it to one additional spawn shape (anchor+children instead
  of a bare entity), in one already-consolidated function.

## Acceptance criteria
- [x] Given a split-screen scene with a Pixel-style `world_stat_bar` on any entity (NPC, prop, or
  player), when that entity is visible in 2+ active split viewports simultaneously, then the bar
  renders correctly in every one of them (browser-observable). **Met** — confirmed by
  `test_pixel_world_stat_bar_duplicates_ranks_when_scene_is_split_screen` (real `spawn_scene_v2`
  pipeline) and browser playtest.
- [x] Given a non-split-screen scene with a Pixel-style `world_stat_bar`, when it spawns, then exactly
  one anchor+children set is created (regression — pixel-identical to before this feature). **Met**
  — `test_pixel_world_stat_bar_stays_single_instance_in_non_split_scene` and the pre-existing
  `test_spawn_world_stat_bar_widget_pixel_style_spawns_anchor_and_children_without_duplication`.
- [x] Given a Pixel bar's border/background geometry, when it spawns in a split-screen scene, then the
  mesh/material assets for the shared (unchanging) parts are created once and reused across ranks,
  not re-registered per rank. **Met** — `test_spawn_world_stat_bar_widget_pixel_style_duplicates_ranks_when_split_screen`
  asserts `Assets<Mesh>`/`Assets<ColorMaterial>` counts stay at `2 + MAX_SPLIT_PLAYERS`, not `4x`.
- [x] Given `local_coop_demo`'s `player_p1_split`/`player_p2_split` switched to Pixel style, when both
  players play split-screen, then both bars render correctly in both viewports with a
  production-quality (non-Ascii) look. **Met** — playtest confirmed by Frank.
