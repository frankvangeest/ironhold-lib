# Feature: Player Stat Widgets (stat_label / world_stat_bar for players)

_Status: Done_
_Planned at: `6e38aa1` (2026-07-17)_

**Plan-review note (2026-07-17):** system-architect — Ready, fit confirmed against actual source
(Part A duplication verified line-accurate; caught that `spawn_scene_v2` is already at Bevy's
16-param `SystemParam` ceiling, so `DynamicStatUiQueue` must bundle into `SceneV2Params` rather
than add a bare param; caught a `depth_scale` type mismatch and a missing `{self}`-substitution
step — all three folded into Approach/Tasks above). ux-gamedesigner-reviewer — Needs more design
work → resolved: the plan's original RON example used the removed flat `world_stat_bar` schema
(`cells` at top level, now `style: Ascii(...)`, a `deny_unknown_fields` parse error — fixed in the
example above); added Part C (generic `{self}.<stat>`-with-no-matching-template warn/validate
check) and the global-vs-`{self}` co-op doc guidance per its findings.

**Code-review note (2026-07-17):** All 4 post-implementation reviews run in parallel.
alignment-reviewer — Aligned, no blocking issues (one non-blocking note logged: the primitive
player path re-reads `PrefabDef` directly rather than through `PlayerConfig`, a pre-existing
divergence-risk class already tracked by `player_model_source_unification.md`, not new here).
system-architect — no blockers, all 6 of its own earlier plan-review points confirmed correctly
implemented; caught that the acceptance criteria still claimed the `{self}` widget works on "GLB
or primitive path" when the primitive path can never resolve one (no runtime `StatMap`) — fixed
by scoping that criterion to GLB and documenting the known gap explicitly. debug-detective — found
and fixed a real doc-comment misattribution (a new function's doc block had been inserted above an
existing function, leaving both mis-documented); found and closed two real test gaps (the CLI-side
`missing_stat_widget_template` check had zero test coverage — added a new CLI fixture test; the
split-screen player-widget pipeline was only proven via an isolated helper call, not through the
real scene-load path — added an end-to-end split-screen test); logged one narrow, unreproducible-
today latent timing edge case (terrain + split-screen + a player widget) to `claude_suggestions.md`
rather than fixing, since no current project combines terrain with split-screen at all.
wasm-perf-reviewer — clean, no findings (spawn-time only, no per-frame cost, no binary-size impact).
Full `ironhold_core` test suite (191 tests) + `ironhold_cli` test suite (17 tests, including the
new fixture) green; `cargo check -p ironhold_cli` clean. Playtest confirmed by Frank in
`local_coop_demo` room3 — both players' Ascii mana bars visible and correctly duplicated across
both split viewports, no console errors.

## What
`PrefabDef.stat_label` (floating text tracking a stat) and `PrefabDef.world_stat_bar` (floating
ASCII/pixel bar) already work for any NPC or prop. This feature makes them work for players too —
a `tags: ["player"]` prefab authoring `stat_label:`/`world_stat_bar:` gets the same floating
overhead widget every other entity kind gets, instead of the field silently parsing and doing
nothing.

## Why
Logged as a real gap by alignment-reviewer during `per_player_stat_pools` (`planning/features/done/per_player_stat_pools.md`,
see `planning/claude_suggestions.md` line 81): the fields exist on `PrefabDef`, so authoring them
on a player prefab looks supported (schema is generic), but `PlayerConfig` has no equivalent
fields and `spawn_player_entity_core` never inserts the widget entities — same "looks supported,
silently no-ops" class as the nameplate `{self}.stat` gap already documented in
`docs/20_data_formats.md`. Concretely surfaced when `per_player_stat_pools`'s local_coop_demo
playtest aid wanted a `stat_label: ({self}.mana)` on the split-screen players to visually confirm
each player's independent mana pool, and had to fall back to the action-bar's own cooldown dim
instead.

Consulted system-architect before planning (see conversation 2026-07-17) on how to do this without
a player-specific reimplementation, given the project's standing rule that "every player gets X"
features must route through the same mechanism NPCs/props use, not a parallel one. This plan
folds in that consultation's recommendation.

## Approach
Two parts, both in one branch:

**Part A — de-duplicate the widget-spawn logic (pure refactor, no behavior change).**
`scene_loader.rs` currently has 3 near-identical NPC/prop "collector" sites (composite/GLB-child
~L387, single-primitive-mesh ~L592, single-GLB-actor ~L657) that each push
`(entity, resolved_key, def)` onto `pending_stat_labels`/`pending_world_bars`, 2 spawner loops that
drain those Vecs and build the actual `Text2d`/`Mesh2d` + marker entities (~L1023 stat labels,
~L1067 world bars — Ascii + Pixel sub-styles), and a 4th near-byte-for-byte copy in
`drain_dynamic_stat_ui_system` (~L2679) for `Action::Spawn`-created entities. Extract the actual
entity-spawning logic (not just the def→marker conversion) into two shared helpers in
`capabilities/stat_display.rs` — the layer this already conceptually belongs to, since
`resolve_stat`/the marker components/the per-frame update systems all live there and are already
fully entity-kind-agnostic:

```rust
pub struct StatWidgetSpawnCtx<'a> {
    pub meshes: &'a mut Assets<Mesh>,
    pub color_materials: Option<&'a mut Assets<ColorMaterial>>, // Pixel-style bars only
    pub depth_scale: Option<(f32, f32)>, // resolved caller-side via resolve_label_depth_scale
    pub is_split_screen: bool,
}

pub fn spawn_stat_label_widget(commands: &mut Commands, tracked: Entity, stat_key: &str, def: &StatLabelDef, ctx: &StatWidgetSpawnCtx);
pub fn spawn_world_stat_bar_widget(commands: &mut Commands, tracked: Entity, stat_key: &str, def: &WorldStatBarDef, ctx: &mut StatWidgetSpawnCtx);
```

Both Phase-B loops and `drain_dynamic_stat_ui_system` call these instead of their own inline
spawn code. Zero RON/schema change; existing NPC/prop/dynamic-spawn tests must still pass
unmodified — this step is a refactor, not a feature, and should be verifiably behavior-identical.
`resolve_label_depth_scale` stays private in `scene_loader.rs` — each caller resolves its own
`Option<(f32, f32)>` (from `scene.label_depth_scale` or `LoadedLabelDepthScale`, depending on the
call site) and passes the already-resolved value into `StatWidgetSpawnCtx`, rather than the helper
moving to `stat_display.rs` or becoming `pub`.

**Part B — wire players through the shared helpers via the existing dynamic-widget queue.**
The widget entities are independent top-level entities that track a target via
`tracked_entity: Some(entity)` (not children spawned atomically with the tracked entity) — so they
only need the player's `Entity` id to exist, not to be created inline during player construction.
For a GLB player the entity doesn't exist yet at what would be Part A's "collector" time (`assemble_player_config`
only builds a `PlayerConfig`; the entity is created later in `spawn_players_and_camera`) — so a
literal 4th collector site is not possible. Instead:

1. Add `stat_label: Option<StatLabelDef>` / `world_stat_bar: Option<WorldStatBarDef>` to
   `PlayerConfig` (`schema/player.rs`), forwarded in `assemble_player_config`
   (`entity_spawner.rs`) exactly like `material`/`stat_templates` — one edit, all GLB
   player-construction sites get it for free.
2. In `spawn_player_entity_core`, once the player entity exists, push a `DynamicStatUiEntry {
   entity, stat_label, world_stat_bar }` onto the existing `DynamicStatUiQueue` resource — with
   `{self}` already substituted against `player_config.spawn_id` (mirroring
   `drain_spawn_queue_system`'s `sl.stat_key.replace("{self}", &queued.spawn_id)`; the queue stores
   the *resolved* key, not the raw `{self}.mana` template — easy to miss since nothing else in
   `spawn_player_entity_core` does string substitution today). The existing
   `drain_dynamic_stat_ui_system` (now calling the Part A shared helpers) spawns the widgets a
   frame later — and gets the split-screen rank-duplication gate (`is_split_screen`,
   `WorldLabelRank`) for free, since `ActiveSplitScreen`/`DynamicSplitConfig` are already populated
   by then (verify this timing in the playtest step, not just by inspection — the values are
   written via deferred `commands.insert_resource` in `spawn_players_and_camera`, so the drain
   system must run after that command flush). This also means `SceneMaterialParams` (the
   `Assets<Mesh>`/`Assets<ColorMaterial>` the drain system already owns) doesn't need to be
   threaded down through `spawn_players_and_camera` → `spawn_player_entity_core`'s call chain.
   **`DynamicStatUiQueue` itself must be threaded through `spawn_player_entity_core`'s callers**:
   `spawn_player_entity`, `spawn_players_and_camera`, and `spawn_delayed_players_system` are
   ordinary functions/systems (easy to add the resource param), but `spawn_scene_v2`
   (`scene_loader.rs:881`, the immediate scene-load path) **already has exactly 16 top-level
   params** — Bevy's `SystemParam` derive tops out at 16-tuples, so a bare 17th
   `ResMut<DynamicStatUiQueue>` is a compile error, not a runtime one. Bundle it into the existing
   `SceneV2Params` `SystemParam` struct instead of adding it as a new top-level param.
3. For the primitive/capsule inline player spawn path (`scene_loader.rs`, does not route through
   `spawn_player_entity_core`), push the same `{self}`-resolved `DynamicStatUiEntry` inline once
   that entity exists.

No RON schema break: `PlayerConfig` is constructed in Rust from `PrefabDef` fields, never
deserialized directly from scene RON (confirmed by system-architect during `per_player_stat_pools`
and again in this feature's consultation) — adding fields to it is purely additive.

**RON authoring example** — no new syntax; this is the same `stat_label`/`world_stat_bar` block any
NPC/prop prefab already uses, now also legal (and honored) on a `tags: ["player"]` prefab:

```ron
(
    kind: "actor",
    tags: ["player"],
    model: "characters/hero.glb",
    stat_templates: [
        (key: "mana", base: 100.0, min: 0.0, max: 100.0, regen_rate: 5.0, regen_delay: 1.0),
    ],
    stat_label: (
        stat_key: "{self}.mana",
        offset: (0.0, 2.1, 0.0),
        show_max: true,
    ),
    world_stat_bar: (
        stat_key: "{self}.mana",
        offset: (0.0, 2.8, 0.0),
        style: Ascii( cells: 10 ),
    ),
)
```

`{self}` resolves to the player's `SpawnId` at spawn time, exactly like any other entity kind —
the bar tracks that specific player's own `StatMap` entry, independent of any other player's. Note
`world_stat_bar.style` is a required-shape enum (`Ascii(...)`/`Pixel(...)`, `deny_unknown_fields`)
— `cells`/`font_size` at the top level is the pre-style schema and is a parse error today (see
`docs/20_data_formats.md`'s "Migration from the pre-style schema" note); this plan's playtest aid
and doc updates must use the `style: Ascii(...)` form, and must use **Ascii** specifically (not
Pixel) since only `stat_label` and Ascii-style `world_stat_bar` duplicate across split-screen
viewports — a Pixel bar on a co-op player would only ever show in one viewport, which is the
opposite of what this feature is meant to demonstrate.

**Part C — close the sibling "authored `{self}.<stat>` with no matching `stat_templates` entry"
trap, generically (not player-specific).** ux-gamedesigner-reviewer flagged that once this feature
invites designers to put `stat_label`/`world_stat_bar` on players for the first time, the existing
silent-failure mode of `resolve_stat` — an entity-local key that doesn't resolve just renders an
empty label/bar with no warning — becomes much more likely to bite, since carrying over a `{self}.mana`
habit onto a player prefab that has no matching `stat_templates` entry looks identical to "it
worked" until you notice the widget is blank. This is **not new to this feature** — the same silent
gap already exists for any NPC/prop today — but per this project's warn-on-contradictory-intent
principle (the same reasoning behind `per_player_stat_pools`'s `missing_player_stat_template`
check), authoring a `{self}.<stat>` widget with no matching template is contradictory intent and
should warn, not silently no-op. Fix it generically at the same layer that resolves the key:
- Scene-load `warn!` (`scene_loader.rs`) when a prefab's `stat_label`/`world_stat_bar` `stat_key`
  is `{self}`-form and that prefab's `stat_templates` has no entry for the referenced stat name —
  applies to every entity kind, players included, so this is not a player-specific special case.
- `ironhold_cli validate` cross-file check, same condition, following the existing
  `missing_player_stat_template` check's shape (`validate.rs`).

## RON: global vs. `{self}` for co-op players
Worth calling out explicitly in the docs update (Part D below), since it's easy to get backwards:
a **global** key (`stat_key: "player_health"`, no `{self}`) reads the single shared `LoadedStats`
value — in split-screen, both players' widgets would show and move **identically**, which looks
like a bug, not two independent players. `{self}.<stat>` (paired with that player's own
`stat_templates` entry) is what gives each player their own independent readout. This mirrors the
same global-vs-per-player distinction `per_player_stat_pools` already introduced for `SlotCost`.

**Explicitly out of scope**: unifying the primitive-player and GLB-player body-construction paths
themselves (a `PlayerModelSource` enum collapsing the ~160-line inline primitive block in
`scene_loader.rs`). That's a larger, separate, higher-risk structural change (see
`planning/claude_suggestions.md`'s "single-path player construction" note, added alongside this
plan) — this feature only makes the primitive path's *existing* inline spawn also push the
`DynamicStatUiEntry`, it does not collapse the two paths into one.

## Tasks
- [x] Extract `spawn_stat_label_widget`/`spawn_world_stat_bar_widget` into `capabilities/stat_display.rs`
- [x] Refactor `scene_loader.rs`'s two Phase-B spawner loops to call the shared helpers
- [x] Refactor `drain_dynamic_stat_ui_system` to call the same shared helpers
- [x] Add `stat_label`/`world_stat_bar` fields to `PlayerConfig`; forward in `assemble_player_config`
- [x] Thread `DynamicStatUiQueue` through `spawn_player_entity_core`'s callers (`spawn_player_entity`,
      `spawn_players_and_camera`, `spawn_delayed_players_system`); bundle it into the existing
      `SceneV2Params` `SystemParam` struct for `spawn_scene_v2` rather than adding a bare 17th
      top-level param (that system is already at Bevy's 16-param `SystemParam` ceiling — a compile
      error, not a runtime one, if added directly)
- [x] `spawn_player_entity_core`: push a `{self}`-resolved (against `player_config.spawn_id`)
      `DynamicStatUiEntry` when either field is set
- [x] Primitive/capsule inline player spawn: push the same `{self}`-resolved `DynamicStatUiEntry`
      when either field is set
- [x] `local_coop_demo`: add a `stat_label`/`world_stat_bar` (Ascii style, not Pixel — see the
      split-screen duplication caveat above) to a split-screen player prefab as a playtest aid,
      confirming per-player independent readouts (this is the widget `per_player_stat_pools`
      originally wanted and had to drop)
- [x] Scene-load `warn!` + `ironhold_cli validate` check (Part C): `{self}`-form `stat_label`/
      `world_stat_bar` `stat_key` with no matching `stat_templates` entry on that prefab — generic
      across all entity kinds, not player-specific
- [x] Tests — regression coverage that Part A's refactor is behavior-identical for NPCs/props/dynamic
      spawns; new coverage that a player-authored `stat_label`/`world_stat_bar` actually spawns
      and updates from that player's own `StatMap`; new coverage for the Part C warn/validate check
- [x] Docs — `docs/20_data_formats.md`'s `StatLabelDef`/`WorldStatBarDef` sections: note player
      prefab applicability (same pattern as the `stat_templates` doc update from `per_player_stat_pools`),
      cross-reference the existing nameplate `{self}.stat` footgun note for the new Part C warning,
      and add the global-vs-`{self}` co-op guidance above
- [x] `crates/ironhold_core/src/CLAUDE.md` — note in "The four player-construction sites" section
- [x] Remove the now-resolved `claude_suggestions.md` entry (line 81) once shipped

## Open questions
- None outstanding — the two questions ux-gamedesigner-reviewer raised (RON example correctness;
  warn-vs-silent for an unresolved `{self}.<stat>` key) are resolved above (Part C) and folded into
  the Tasks/Approach.

## Acceptance criteria
- ~~Given an existing NPC/prop project with `stat_label`/`world_stat_bar` prefabs, when the Part A
  refactor lands, then their visual output is pixel-identical to before (regression, not a new
  feature).~~ **Met** — confirmed by the pre-existing `test_stat_widgets_duplicate_ranks_when_scene_is_split_screen`/`test_stat_widgets_stay_single_instance_*` tests passing unchanged after the refactor.
- ~~Given a **GLB** player prefab with a `stat_label`/`world_stat_bar` block referencing
  `"{self}.<stat>"`, when that player spawns, then the floating widget appears and tracks that
  player's own `StatMap` value, independently of any other player's.~~ **Met** — confirmed by
  `test_player_stat_widget_spawns_and_resolves_against_that_players_own_stat_map` and playtest.
  **Known gap (documented in `crates/ironhold_core/src/CLAUDE.md`, not fixed here):** a
  *primitive*-bodied player's `{self}.<stat>` widget still spawns (the queue push is generic) but
  always renders empty, since the primitive spawn path never builds a runtime `StatMap` regardless
  of `stat_templates` — that gap belongs to `player_model_source_unification.md`, not this feature.
  A **global**-key widget (no `{self}`) works correctly on either body type today, since it only
  reads `LoadedStats`.
- ~~Given a split-screen scene with 2+ players each having their own **Ascii-style**
  `stat_label`/`world_stat_bar`, when both are visible in 2+ active viewports simultaneously, then
  each viewport's rank-duplicated copy shows the correct owning player's value (reusing
  `WorldLabelRank`, not a new mechanism).~~ **Met** — confirmed by
  `test_player_stat_widget_duplicates_ranks_when_scene_is_split_screen` (added during code review,
  debug-detective finding) and playtest in `local_coop_demo` room3.
- ~~Given a player prefab with no `stat_label`/`world_stat_bar`, when that player spawns, then no
  widget entity is created and no warning is logged (this is the ordinary/majority case).~~ **Met**
  — confirmed by the same end-to-end test (player 2 authors neither field, gets no widget).
- ~~Given a prefab (player or NPC) whose `stat_label`/`world_stat_bar` `stat_key` is `{self}.<stat>`
  where `<stat>` has no matching `stat_templates` entry on that prefab, when the scene loads, then
  a `warn!` is logged and `ironhold_cli validate` reports it as an error — the widget still renders
  empty (unchanged runtime behavior) but the misconfiguration is no longer silent.~~ **Met** —
  confirmed by the new `missing_stat_widget_template_exits_1` CLI fixture test (added during code
  review, debug-detective finding: this check had zero test coverage until then).
