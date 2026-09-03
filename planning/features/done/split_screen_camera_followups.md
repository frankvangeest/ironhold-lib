# Feature: Split-Screen — Remaining Single-Camera Assumption Sites

_Status: Done — all 4 phases shipped._
_Planned at: `7cb222a` (2026-07-11)_
_Plan review (2026-07-11): system-architect + ux-gamedesigner-reviewer, verdict Ready. Findings
below are incorporated; Frank resolved the two open decisions (Phase 4 perf gating, Phase 4
dual-spawn-site scope) the same day._
_Phase 4 re-review (2026-07-12): alignment-reviewer (ALIGNED), system-architect (merge-ready),
debug-detective (one gating bug found and fixed — see Approach below), wasm-perf-reviewer (OK,
minor non-blocking notes logged to `planning/claude_suggestions.md`)._

## Phases

Ordered by build sequence (bug fixes first, the perf-sensitive duplication feature last), per
system-architect's recommendation — this differs from the topic order the sites were first listed
in.

| Phase | Backlog item | Status | Completed |
|---|---|---|---|
| 1 | `particle_renderer.rs` billboard orientation | Done | `4eb5295` (2026-07-12) |
| 2 | `targeting.rs` viewport-aware click-to-select | Done | `940dbf8` (2026-07-12) |
| 3 | `nameplate_visibility_system` distance-culling | Done | `42441f6` (2026-07-12) |
| 4 | `WorldLabelRank` extended to stat labels / world stat bars | Done | `db63402` (2026-07-13) |

## What

Four systems still assume at most one `Camera3d` exists, predicted in
`planning/claude_suggestions.md` ▸ Camera back in 2026-07-05 (before any split-screen code was
written) and explicitly left "not touched" by
`planning/features/done/world_label_split_screen_positioning.md`, which fixed the fifth (portal
room-name / entity labels) via `world_label_screen_pos_system`:

1. ~~`particle_renderer.rs:303` (`rebuild_pool_meshes_system`) — billboard orientation basis
   vectors.~~ **Fixed in Phase 1 (`4eb5295`).**
2. ~~`targeting.rs:122` (`click_select_system`) — click-to-select nearest-entity search.~~
   **Fixed in Phase 2 (`940dbf8`).**
3. ~~`nameplate_visibility_system` (`nameplate.rs:212`) — distance-culling, `camera_q.single()`.~~
   **Fixed in Phase 3 (`42441f6`).**
4. ~~`WorldLabelRank`'s multi-viewport duplication only covers `scene_loader.rs`'s `world_labels:`
   and `label:` (`EntityLabelDef`) spawn loops. Stat labels, world stat bars (Ascii + Pixel),
   damage popups, and nameplate anchors are still single-instance (implicit rank 0), so they don't
   duplicate across two simultaneously-visible split viewports the way room-name labels now do.~~
   **Fixed for `stat_label`/`Ascii`-style `world_stat_bar` in Phase 4 (`db63402`).** Pixel-style
   bars, damage popups, and nameplate anchors remain single-instance (deferred, see Not in scope).

## Why

Every split-screen scene (`local_coop_demo`, Stage 3+) has 2+ `Camera3d` entities alive
simultaneously — often even when only one is currently `is_active` (a merged dynamic split still
keeps the inactive sibling camera entity around). Each of the four sites above either silently
no-ops, falls back to an arbitrary/incorrect default, or picks a non-viewport-aware camera whenever
that's true. None of them currently break anything visibly in `local_coop_demo` today only because
nameplates aren't enabled there and stat-widgets aren't exercised across simultaneous split
viewports yet — but any future project combining split-screen with these systems will hit the same
class of bug the room-name labels did. See "Playtest setup" below: today's `local_coop_demo`
project has *nothing* authored that exercises any of these four systems, so each phase needs a
demo-project addition before it can be play-tested at all.

## Research findings

- **`particle_renderer.rs:303` has the same missing-`is_active`-filter defect as
  `nameplate_visibility_system`** (below) — except its failure mode is a *silent visual fallback*
  (`Vec3::X, Vec3::Y` axis-aligned billboarding) rather than an early return. This means every
  particle billboard in any split-screen project is billboarding against world axes, not the
  camera, at all times — an unconditional bug, not gated on a simultaneous-camera edge case, which
  is why it's fixed first.
- **`targeting.rs:122` already filters on `camera.is_active`**
  (`cameras.iter().find(|(c, _)| c.is_active)`). Its remaining gap is narrower: when 2 cameras are
  *simultaneously* active (a fixed split, not a merged dynamic one), `.find()` returns whichever
  camera iterates first, regardless of which viewport the cursor is actually over — so a click in
  player 2's viewport can be evaluated against player 1's camera, silently selecting the wrong
  entity or missing a should-be-in-range one.
- **`nameplate_visibility_system`'s query has no `is_active` filter at all**
  (`Query<&GlobalTransform, With<Camera3d>>`), so `.single()` fails whenever 2+ `Camera3d` entities
  exist *at all* — not just when 2+ are simultaneously active. In a merged dynamic split (1 active
  camera, 1 inactive sibling still alive) this still silently no-ops every frame.
  **(Plan-review finding, system-architect):** the nameplate anchor is itself a rank-0 `WorldLabel`
  (confirmed: `nameplate_cleanup_system` queries `&WorldLabel` on anchors), already positioned every
  frame by `world_label_screen_pos_system`'s own camera selection. For distance-culling to agree
  with wherever the anchor is actually drawn, this system cannot just pick "first active camera in
  sorted order" independently — that can disagree with `world_label_screen_pos_system`'s
  containment-tested choice and reintroduce exactly the flicker this fix is meant to prevent.
  Recomputing the full containment test a second time is possible but risks the two
  implementations drifting apart with no compile-time signal. **Resolved approach: store-and-read**
  (see Phase 3 Approach below) — `world_label_screen_pos_system` stashes the camera entity/distance
  it actually chose for the anchor, and `nameplate_visibility_system` reads that instead of
  reselecting.
- **`world_label_screen_pos_system`'s selection logic is not one reusable primitive across all
  four sites — it's three different shapes sharing one comparator** (plan-review finding,
  system-architect): Phase 3 projects a *world* point and tests containment (same shape as
  `world_label_screen_pos_system` itself — hence store-and-read is possible). Phase 1 has no point
  to project at all; a shared particle mesh can't test "is point P visible in this viewport," it can
  only pick the highest-priority active camera. Phase 2 starts from the *cursor* (already
  screen-space) and tests viewport containment directly, no `world_to_viewport` projection needed.
  The one genuinely shared, correctness-critical piece across all three is the **sort comparator**
  (`SplitViewportSlot.map_or(u32::MAX)` then `Entity` as tiebreak) — this gets extracted as a small
  shared fn in Phase 1 (the first site that needs deterministic ordering) so Phase 2 and Phase 3
  consume it instead of re-typing it, rather than fully generalizing the selection helper (still
  premature per the original fix's own deferral — the three call shapes genuinely differ).
  **Landed as `capabilities::camera::camera_priority_key(entity, slot)` in Phase 1** —
  `world_label_screen_pos_system` was refactored (behavior-preserving) to call it too, and Phase 2
  consumed it directly (no re-typing). Phase 3 has it available already too.
- **`WorldLabelRank`'s duplication pattern is directly reusable** for Phase 4: spawn
  `MAX_SPLIT_PLAYERS` ranked sibling entities instead of 1, `rank > 0` starts `Visibility::Hidden`,
  `world_label_screen_pos_system` already resolves `.nth(rank)` generically — no changes needed to
  that system itself, only to the spawn sites. Two problems found in plan review that the original
  `world_labels:` precedent's reasoning didn't surface, because static labels don't share this
  shape:
  - **(system-architect) Per-frame cost, not a one-time cost.** Unlike static `world_labels:` text,
    stat labels and world stat bars are rewritten every frame by `stat_label_update_system` /
    `world_stat_bar_update_system`, which iterate by marker component regardless of current
    `Visibility`. Unconditionally spawning 4 ranked siblings means 4x the per-frame Text2d
    re-layout work in *every* scene, including ordinary single-player ones with no split-screen at
    all — ranks 1-3 would sit permanently `Hidden` there, pure overhead. **Resolved (Frank,
    2026-07-11): gate rank-spawning on the scene actually being split-screen** — only spawn the
    extra ranked siblings when the loading scene has split-screen configured at all (see Phase 4
    Approach below); ordinary projects get exactly 1 entity per stat label/bar, identical to today.
  - **(system-architect) A second spawn site was missing from scope.** Stat labels and Ascii world
    stat bars spawn from *two* independent code paths: the scene-loader loops
    (`pending_stat_labels` / `pending_world_bars`, scene-load time) **and**
    `drain_dynamic_stat_ui_system` (`Action::Spawn`/wave-spawn path, runtime). The original Phase 4
    scope named only the scene-loader loop — leaving wave-spawned enemies' bars single-instance
    while scene-placed ones duplicate, the exact spawn-site divergence class
    `should_insert_nameplate`/`tag_spawned_entity` centralization exists elsewhere in this codebase
    to prevent. **Resolved (Frank, 2026-07-11): both spawn sites are in scope for Phase 4.**
  - The complication that stays: nameplate anchors and Pixel stat bars aren't flat `Text2d`
    entities — they have child hierarchies (border/fill mesh children parented to an anchor).
    Duplicating those means duplicating the whole child subtree per rank, not just one entity,
    which is a larger and more error-prone change than either spawn site above. Scoping Phase 4 to
    stat labels + Ascii world stat bars only (flat `Text2d`, same shape as the already-fixed labels)
    and treating nameplate anchors + Pixel bars + damage popups as a follow-up keeps this phase's
    diff reviewable.
- **(Phase 2 playtest finding) Split-screen's single shared mouse limits what click-to-select can
  actually deliver, but doesn't invalidate the fix.** `click_select_system`'s bug was that a click
  resolved against an *arbitrary* camera regardless of cursor position — not that two players
  couldn't click simultaneously (impossible with one physical mouse regardless of this fix).
  Confirmed by playtest (2026-07-12): clicking either viewport's test sphere correctly resolves
  against that viewport's own camera, one click at a time — the fix is real and testable
  independent of any multi-player-simultaneous concern. A separate, deeper gap surfaced during that
  same playtest: `CurrentTarget` is one shared global resource and `tab_targeting_system` hardcodes
  `controllers.iter().next()` (always the first `CharacterController`, never player 2's), so
  genuine per-player-independent targeting doesn't exist regardless of this fix. Logged as its own
  backlog item ("Per-player targeting for split-screen"), not folded into this feature.

## Approach

**Phase 1 — particle billboard orientation. DONE (`4eb5295`).** Changed `camera_q` in
`rebuild_pool_meshes_system` to `Query<(Entity, &Camera, &GlobalTransform, Option<&SplitViewportSlot>),
With<Camera3d>>`, filtered `camera.is_active`, and picked the highest-priority active camera via
`.min_by_key(camera_priority_key)` (the new shared comparator, added to `capabilities/camera.rs` and
also consumed by `world_label_screen_pos_system`) instead of falling back to world axes whenever 2+
`Camera3d` entities merely existed. Falls back to `(Vec3::X, Vec3::Y)` only when zero cameras are
active (unchanged edge case). **Known, accepted limitation** (documented in
`crates/ironhold_core/src/CLAUDE.md`): with 2 simultaneously active split cameras at different
angles, particles still only billboard correctly toward the one picked camera — true
per-viewport-correct billboarding would require duplicating particle meshes per viewport, out of
scope (see Not in scope).

**Phase 2 — `targeting.rs` viewport-aware click-to-select. DONE (`940dbf8`).** Changed
`click_select_system`'s `cameras` query to `Query<(Entity, &Camera, &GlobalTransform,
Option<&SplitViewportSlot>), With<Camera3d>>`; before the nearest-entity search, filters to
`is_active` cameras whose `logical_viewport_rect()` contains the cursor position, then picks via
`.min_by_key(camera_priority_key)` (reusing Phase 1's comparator directly, no re-typing) to break
ties deterministically (cursor exactly on a shared viewport boundary) — replacing
`cameras.iter().find(|(c, _)| c.is_active)`'s arbitrary first-match. **Known, minor behavior
change** (documented in `crates/ironhold_core/src/CLAUDE.md`): a click in a screen region no active
camera's viewport covers (e.g. a dead grid quadrant) now does nothing, where the old arbitrary pick
used to fall through to "clicked empty space" and clear `CurrentTarget`; invisible in ordinary
single-camera scenes.

**Phase 3 — `nameplate_visibility_system` (store-and-read). DONE (`42441f6`).** Added
`NameplateCameraDistance(Option<f32>)`, a new component attached to every nameplate anchor at spawn
time. `world_label_screen_pos_system` (which already selects one active, containment-tested camera
per `WorldLabel` each frame) now also stashes that camera's distance onto this component whenever
the label carries it — `None` on every early-return path (tracked entity gone/hidden, no qualifying
camera this frame), `Some(distance)` on the success path, reusing the same distance value already
computed for the pre-existing `depth_scale` font-size calculation. `nameplate_visibility_system` no
longer queries cameras at all (`camera_q.single()` removed entirely); it reads the anchor's stashed
distance instead, guaranteeing the two systems always agree on which camera is authoritative for a
given anchor's position and its visibility, with no drift possible between two independent
implementations. An anchor with no stashed distance is treated as out-of-range (hidden), matching
the prior no-op contract for "no qualifying camera." Added a `warn_once!` diagnostic (debug-detective
finding) if an anchor is ever found with the component entirely missing (a bug) rather than present
with `None` (a legitimate off-viewport frame), so a future anchor-spawn path that forgets the
component fails loudly instead of producing a silently-invisible nameplate. **Known, minor behavior
change** (debug-detective finding, documented in `crates/ironhold_core/src/CLAUDE.md`): the culling
distance is now measured from the anchor's actual position (tracked entity origin +
`NameplateOptionsDef.offset`) against the viewport-selected camera, rather than the old
`.single()` path's entity-origin-to-only-camera distance — more correct, sub-metre difference at
normal `max_distance` scales. Acceptance criteria updated to state explicitly: because nameplate
anchors remain single-instance (Phase 4 does not extend to them), an entity's nameplate shows in
**at most one** viewport in split-screen — a real, accepted limitation, not a bug.

**Phase 4 — stat label / world stat bar duplication. DONE (`db63402`).** Added `WorldLabelRank(rank
as u8)` + `Visibility::Hidden` (for `rank > 0`) to **both** the scene-loader's `pending_stat_labels` /
`WorldStatBarStyle::Ascii` spawn loops **and** `drain_dynamic_stat_ui_system`'s equivalent spawns,
spawning `MAX_SPLIT_PLAYERS` siblings exactly like the `world_labels:`/`label:` fix — but **only
when the loading scene is configured for split-screen**. The two spawn sites derive that gate
differently, by necessity: the scene-loader loops compute `player_configs.len() >= 2 &&
player_configs.first().camera.split.is_some()` directly (captured before `player_configs` is
potentially moved into `PendingPlayerConfig` for terrain-delayed scenes — reading
`ActiveSplitScreen`/`DynamicSplitConfig` there would see stale values for those scenes), while
`drain_dynamic_stat_ui_system` reads `ActiveSplitScreen.0.is_some() || DynamicSplitConfig.0.is_some()`
(the OR is required because a dynamic split that starts/is merged reports `ActiveSplitScreen(None)`
even though 2 real per-player cameras exist). Ordinary single-camera scenes get exactly 1 entity per
widget, zero behavior/perf change. `WorldStatBarStyle::Pixel` and nameplate anchors are explicitly
deferred (see Not in scope) since their child-hierarchy duplication is a materially bigger change.
Docs: added the designer-facing note to `docs/20_data_formats.md` beside the existing
Pixel-depth-scaling limitation note, stating that in split-screen scenes, stat labels and Ascii
world stat bars duplicate correctly across simultaneously-visible viewports while Pixel-style bars,
damage popups, and nameplates do not.

**Debug-detective review caught a real gating bug**, fixed in the same commit: the scene-loader gate
initially checked only `camera.split.is_some()`, omitting the `>= 2 players` half of
`spawn_players_and_camera`'s own activation condition — a lone player whose prefab happened to
carry a `camera.split` block (e.g. copy-pasted from a co-op prefab) would trigger 4-way rank
duplication even though the engine renders a single full-window camera for it, and would disagree
with `drain_dynamic_stat_ui_system`'s resource-based gate (which correctly evaluated `false` for
that same scene). Fixed by adding the `player_configs.len() >= 2` check; regression test
`test_stat_widgets_stay_single_instance_with_one_player_carrying_split_config` locks it in. Two
narrower, non-blocking findings were logged to `planning/claude_suggestions.md` rather than fixed
now: a terrain-delayed-scene edge case where `drain_dynamic_stat_ui_system` can briefly
under-duplicate before `spawn_players_and_camera` runs, and the pre-existing
`stat_label_update_system`/`world_stat_bar_update_system` pattern of allocating a `format!` string
before the change-detection guard, now 4x'd in split scenes (wasm-perf-reviewer finding).

### Not in scope

- **Duplicating nameplate anchors, Pixel-style world stat bars, and damage popups across
  simultaneously-visible split viewports** — deferred out of Phase 4 due to child-hierarchy
  duplication complexity (see Research findings). Track as a follow-up phase if a real project need
  surfaces; the pattern to reuse is identical, just applied to a subtree instead of a single entity.
  **Pixel-style `world_stat_bar` duplication resolved** — see
  `planning/features/pixel_world_stat_bar_split_screen_duplication.md`. Nameplate anchors and
  damage popups remain deferred.
- **Per-viewport-correct particle billboarding** (Phase 1) — duplicating particle meshes per active
  camera. The single-shared-mesh particle pool architecture makes this a much larger change than
  picking a better camera; not attempted here.
- **A fully general "pick the active camera for point/cursor P" helper** — Phase 1/2/3 share only
  the sort comparator (extracted), not the full selection shape, since each site's input (no point,
  world point, cursor point) genuinely differs. Worth revisiting only if a fourth consumer needs
  the exact same shape as one of these three.
- **Per-player targeting (per-player `CurrentTarget`, `tab_targeting_system` iterating all players)**
  — surfaced during Phase 2's playtest, logged as its own backlog item ("Per-player targeting for
  split-screen"), deliberately not folded into this feature. Phase 2 fixes *which camera* a click
  resolves against; it does not (and was never meant to) give two players independent simultaneous
  target state.

## Playtest setup — `local_coop_demo` changes needed

Confirmed by inspecting `assets/projects/local_coop_demo/`: it currently authors **no nameplates,
no stat labels/world stat bars, no particle effects, and no `ClickSelectable` entities anywhere** —
none of the four systems have anything to observe today. Each phase needs a small demo-project
addition before it can be dev-build play-tested, in addition to the standard ship workflow steps:

- **Phase 1 — DONE.** No `assets/shared/effects/` library exists yet (confirmed empty/absent), so
  a new `"billboard_test_spark"` `EffectDef` was added directly to
  `assets/projects/local_coop_demo/assets.ron` instead of reusing a shared one — stationary (speed
  0), long-lived (30s), non-fading (`color_start == color_end`) sparks, fired via a `SpawnEffect`
  action on `scene.ready:room3` (`logic/rules.ron`) at `(0.0, 1.5, 0.0)`, the midpoint between
  room3's two spawn points. **Playtest confirmed by Frank (2026-07-12): both split viewports render
  correctly camera-facing particles**, with no manual orbiting needed — each player's fixed default
  camera angle already differs enough (spawn points 8m apart in x) that the old world-axis bug would
  have shown visibly wrong orientation in at least one viewport. (Manual camera orbiting turned out
  to be impossible to use for this playtest in split-screen at all — see the new backlog item
  "Per-player keyboard camera pivot for split-screen," logged separately, not part of this feature.)
  Left in place per the "documents the fix" precedent below.
- **Phase 2 — DONE.** New `"click_target_test"` prefab (stationary yellow sphere,
  `click_selectable: true`) added to `assets/projects/local_coop_demo/prefabs/prefabs.ron`, placed
  once per viewport in room3 (`click_target_left`/`click_target_right`, one near each spawn point).
  **Playtest confirmed by Frank (2026-07-12): clicking either sphere correctly triggers the
  `target.clicked:click_target_test` console message; clicking the ground clears the target; no
  console errors.** Left in place per the same precedent as Phase 1.
- **Phase 3 — DONE.** `room3.scene.ron` now enables `show_nameplates: true` with `faction_filter:
  All`; the existing `"click_target_test"` prop (Phase 2's playtest aid) got `nameplate: true` +
  `display_name: "Click Target"` so it force-shows regardless of faction filtering. **Playtest
  confirmed by Frank (2026-07-12): both nameplates showed up correctly, no console errors.** Left
  in place per the same precedent as Phases 1-2.
- **Phase 4 — DONE.** New `"stat_widget_test"` prefab (purple sphere with a `stat_templates`
  health stat, `stat_label`, and Ascii `world_stat_bar`) added to
  `assets/projects/local_coop_demo/prefabs/prefabs.ron`. One instance is scene-placed on room3's
  centerline (`stat_widget_scene_placed`, visible from both fixed split viewports at once — mirrors
  the portal-label bug's original repro condition); a second is dynamically spawned via
  `Action::Spawn` on `scene.ready:room3` (`stat_widget_dynamic`), exercising
  `drain_dynamic_stat_ui_system`'s duplication path per the dual-spawn-site scope above. **Playtest
  confirmed by Frank (2026-07-12): both spheres' stat label and Ascii bar render correctly in both
  split viewports simultaneously, no console errors.** Left in place per the same precedent as
  Phases 1-3.

These additions are scoped to `local_coop_demo` only (the existing split-screen demo project) —
not a new project — and were left in place per Frank's preference once each phase's playtest was
confirmed (documents the fix for future reference, similar to how Stage 6 left "Room N" labels on
every portal).

## Tasks

- [x] Phase 1: `rebuild_pool_meshes_system` — `is_active`-filtered, deterministically-ordered camera
      selection for billboard basis vectors (extract shared sort comparator here); regression test
      for the non-split single-camera case; `local_coop_demo` particle-effect playtest addition —
      `4eb5295`. 3 new tests in `particle_tests.rs` (2-camera split priority, single-camera
      regression, zero-camera world-axis fallback), all passing; full `ironhold_core` test suite
      (16 binaries) + `cargo check -p ironhold_cli` green; alignment-reviewer (ALIGNED),
      system-architect (ready to merge), debug-detective (no bugs found), wasm-perf-reviewer (OK,
      negligible cost) all clean. WASM dev build clean. Playtest confirmed by Frank.
- [x] Phase 2: `click_select_system` — viewport-aware active-camera selection by cursor position
      (reused Phase 1's comparator directly); `local_coop_demo` `click_target_test` playtest
      addition — `940dbf8`. 3 new tests in `local_coop_tests.rs` (left-viewport click resolves
      correctly even when the right camera spawns first — proving it's not iteration-order
      dependent, right-viewport click resolves correctly, single-camera regression), all passing;
      full `ironhold_core` test suite (16 binaries, including a `ron_lint` fix for a `Some(...)`
      style violation left over from Phase 1's own playtest RON) + `cargo check -p ironhold_cli`
      green; alignment-reviewer (ALIGNED), system-architect (ready to merge), debug-detective
      (correct and safe, no blocking bugs), wasm-perf-reviewer (OK, negligible cost) all clean.
      WASM dev build clean, no console errors. Playtest confirmed by Frank.
- [x] Phase 3: `world_label_screen_pos_system` stashes its selected camera's distance onto a new
      `NameplateCameraDistance` component for anchor `WorldLabel`s; `nameplate_visibility_system`
      reads it instead of reselecting; dropped `.single()` entirely; `local_coop_demo` nameplate-
      enable playtest addition (room3 `show_nameplates`/`faction_filter: All` +
      `click_target_test`'s `nameplate: true` override) — `42441f6`. 2 new full-pipeline tests in
      `local_coop_tests.rs` (split-camera agreement — proves the stashed distance matches the
      LEFT camera's pick, not the right camera's, when only the left camera's viewport actually
      shows the point; off-viewport → hidden regardless of raw distance) plus all 8 existing
      `nameplate_tests.rs` distance/faction/override tests updated to inject
      `NameplateCameraDistance` directly (isolating the visibility-logic unit tests from the
      full-pipeline camera-selection tests); full `ironhold_core` test suite (16 binaries) +
      `cargo check -p ironhold_cli` green; alignment-reviewer (ALIGNED), system-architect
      (mergeable — logged a forward-looking `Option<Entity>`-generalization suggestion for a
      future 4th `WorldLabel` consumer), debug-detective (no high-severity bugs — one MEDIUM
      finding fixed with a `warn_once!` diagnostic, low-severity numeric/doc notes addressed),
      wasm-perf-reviewer (OK, net-neutral-to-cheaper) all clean/addressed. WASM dev build clean,
      no console errors. Playtest confirmed by Frank.
- [x] Phase 4: `WorldLabelRank` + `Visibility::Hidden` on `pending_stat_labels` and
      `WorldStatBarStyle::Ascii` spawn loops, gated on the scene being split-screen (and on
      `player_configs.len() >= 2`, per the debug-detective fix); identical treatment for
      `drain_dynamic_stat_ui_system`'s stat-label/Ascii-bar spawns; `local_coop_demo` stat widget
      playtest addition (scene-placed + dynamically-spawned) — `db63402`. 5 new tests in
      `local_coop_tests.rs` (split-screen duplication via full scene load, non-split regression,
      single-player-with-split-config regression) and `spawn_tests.rs` (dynamic-path duplication
      via `ActiveSplitScreen`, dynamic-path duplication via `DynamicSplitConfig` alone while
      merged), all passing; full `ironhold_core` test suite (16 binaries, `ron_lint`/
      `ron_validation` re-run after the late `local_coop_demo` RON additions) +
      `cargo check -p ironhold_cli` green; alignment-reviewer (ALIGNED, 3 stale doc comments fixed),
      system-architect (merge-ready, one non-blocking predicate-extraction suggestion logged),
      debug-detective (one real gating bug found and fixed, one narrow edge case logged),
      wasm-perf-reviewer (OK, non-blocking `format!`-allocation note logged) all
      clean/addressed. WASM dev build clean, no console errors. Playtest confirmed by Frank.
- [x] Docs: update `crates/ironhold_core/src/CLAUDE.md`'s known-limitations/consumer-duplication
      list as each phase ships (Phase 4 specifically: update in the same commit, per system-architect
      — list which `WorldLabel` consumers duplicate and which don't, to prevent the partial-coverage
      enumeration from drifting out of date) and `planning/claude_suggestions.md` ▸ Camera —
      done for all 4 phases
- [x] Tests (per phase, see above) + full suite green — done for all 4 phases
- [x] WASM dev build + updated playtest checklist (per phase, using the `local_coop_demo` additions
      above) per the standard ship workflow — done for all 4 phases

## Open questions

- Should Phase 4 eventually cover nameplate anchors, Pixel-style world stat bars, and damage
  popups, or is that only worth doing if a real project actually needs simultaneous multi-viewport
  visibility for those? No current project does today.
- Is Phase 1's "pick one camera, document the limitation" an acceptable permanent answer, or does a
  future project need visually-correct billboarding from every simultaneously active split camera?
- Should the `local_coop_demo` playtest additions (particle effect, `ClickSelectable` prop,
  nameplate toggle, stat widget) stay in the project permanently after each phase ships, or be
  removed once confirmed? **Resolved for all 4 phases: kept in place** (documents the fix,
  consistent with Stage 6 leaving "Room N" labels in place).

## Acceptance criteria

- ~~Given any split-screen project, when particles render, then billboard orientation faces an
  actual active camera's basis vectors, never the unconditional world-axis fallback.~~ **Met —
  Phase 1, confirmed by playtest.**
- ~~Given a 2-way fixed split screen, when the player clicks inside one viewport, then
  click-to-select evaluates against that viewport's own camera, not an arbitrary other active
  camera.~~ **Met — Phase 2, confirmed by playtest.**
- ~~Given a split-screen scene with 2+ `Camera3d` entities (any split state, merged or active), when
  `nameplate_visibility_system` runs, then distance-culling evaluates against the exact same camera
  that positions that nameplate's anchor (via the stored selection), not an independently-reselected
  or no-op'd one.~~ **Met — Phase 3, confirmed by playtest + `test_nameplate_visibility_agrees_with_world_label_selected_camera_in_split_screen`.**
- ~~Given nameplate anchors remain single-instance after this feature, when an entity's nameplate is
  simultaneously on-screen in 2+ active split viewports, then it renders in **at most one** of them
  (accepted limitation, not a regression target).~~ **Met — Phase 3 (unchanged, accepted limitation
  confirmed still in effect).**
- ~~Given a stat label or Ascii world stat bar simultaneously visible in 2 active split viewports
  (scene-placed OR dynamically spawned via `Action::Spawn`), when the frame updates, then both
  viewports render their own correctly positioned copy (same contract as the shipped
  room-name-label fix).~~ **Met — Phase 4, confirmed by playtest +
  `test_stat_widgets_duplicate_ranks_when_scene_is_split_screen` /
  `test_dynamic_stat_widgets_duplicate_ranks_when_split_screen_active`.**
- ~~Given an ordinary single-camera (non-split) scene, when a stat label or world stat bar spawns,
  then exactly 1 entity is created per widget — no rank-duplication overhead, pixel-identical to
  today's behavior (regression + perf guard).~~ **Met — Phase 4, confirmed by
  `test_stat_widgets_stay_single_instance_in_non_split_scene` and the
  single-player-with-split-config regression test.**
- ~~Given any existing single-camera (non-split) scene, when any of the four systems run, then
  behavior is unchanged from today (regression guard).~~ **Met for all 4 phases** (regression tests
  pass).
