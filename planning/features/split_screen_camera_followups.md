# Feature: Split-Screen — Remaining Single-Camera Assumption Sites

_Status: Ready_
_Planned at: `4848727` (2026-07-11)_
_Plan review (2026-07-11): system-architect + ux-gamedesigner-reviewer, verdict Ready. Findings
below are incorporated; Frank resolved the two open decisions (Phase 4 perf gating, Phase 4
dual-spawn-site scope) the same day. Re-review recommended before merging Phase 4 specifically,
since its shape changed materially._

## Phases

Ordered by build sequence (bug fixes first, the perf-sensitive duplication feature last), per
system-architect's recommendation — this differs from the topic order the sites were first listed
in.

| Phase | Backlog item | Status | Completed |
|---|---|---|---|
| 1 | `particle_renderer.rs` billboard orientation | Queued | — |
| 2 | `targeting.rs` viewport-aware click-to-select | Queued | — |
| 3 | `nameplate_visibility_system` distance-culling | Queued | — |
| 4 | `WorldLabelRank` extended to stat labels / world stat bars | Queued | — |

## What

Four systems still assume at most one `Camera3d` exists, predicted in
`planning/claude_suggestions.md` ▸ Camera back in 2026-07-05 (before any split-screen code was
written) and explicitly left "not touched" by
`planning/features/done/world_label_split_screen_positioning.md`, which fixed the fifth (portal
room-name / entity labels) via `world_label_screen_pos_system`:

1. `particle_renderer.rs:303` (`rebuild_pool_meshes_system`) — billboard orientation basis vectors.
2. `targeting.rs:122` (`click_select_system`) — click-to-select nearest-entity search.
3. `nameplate_visibility_system` (`nameplate.rs:212`) — distance-culling, `camera_q.single()`.
4. `WorldLabelRank`'s multi-viewport duplication only covers `scene_loader.rs`'s `world_labels:`
   and `label:` (`EntityLabelDef`) spawn loops. Stat labels, world stat bars (Ascii + Pixel),
   damage popups, and nameplate anchors are still single-instance (implicit rank 0), so they don't
   duplicate across two simultaneously-visible split viewports the way room-name labels now do.

## Why

Every split-screen scene (`local_coop_demo`, Stage 3+) has 2+ `Camera3d` entities alive
simultaneously — often even when only one is currently `is_active` (a merged dynamic split still
keeps the inactive sibling camera entity around). Each of the four sites above either silently
no-ops, falls back to an arbitrary/incorrect default, or picks a non-viewport-aware camera whenever
that's true. None of them currently break anything visibly in `local_coop_demo` today only because
nameplates aren't enabled there and click-targeting/particles/stat-widgets aren't exercised across
simultaneous split viewports yet — but any future project combining split-screen with these systems
will hit the same class of bug the room-name labels did. See "Playtest setup" below: today's
`local_coop_demo` project has *nothing* authored that exercises any of these four systems, so each
phase needs a demo-project addition before it can be play-tested at all.

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

## Approach

**Phase 1 — particle billboard orientation:** change `camera_q` in `rebuild_pool_meshes_system` to
`Query<(&Camera, &GlobalTransform), With<Camera3d>>`, filter `is_active`, and pick the first
qualifying camera using a new shared comparator fn (`SplitViewportSlot`-then-`Entity` order,
extracted here for Phase 2/3 to reuse) instead of falling back to world axes whenever 2+ `Camera3d`
entities merely exist. Document as a known, accepted limitation that with 2 *simultaneously* active
split cameras at different angles, particles will only billboard correctly toward the picked
camera — true per-viewport-correct billboarding would require duplicating particle meshes per
viewport, out of scope here (see Not in scope).

**Phase 2 — `targeting.rs` viewport-aware click-to-select:** before the existing nearest-entity
search, resolve which active camera's `logical_viewport_rect()` contains the cursor position (using
the Phase 1 comparator to break ties deterministically if the cursor is somehow in 2 viewports'
rects, which shouldn't normally happen for non-overlapping split layouts but is a defined
tiebreak regardless), and use only that camera for the `world_to_viewport` distance math —
replacing `cameras.iter().find(|(c, _)| c.is_active)`'s arbitrary first-match.

**Phase 3 — `nameplate_visibility_system` (store-and-read):** `world_label_screen_pos_system`
already selects one active camera per `WorldLabel` each frame (containment-tested, deterministic
order) and computes that camera's distance for `depth_scale`. Extend it to also stash the selected
camera's distance onto the `WorldLabel` component itself (or a small new field/component read-only
to `nameplate_visibility_system`) for anchors that have a `NameplateAnchor` back-reference.
`nameplate_visibility_system` reads that stored distance instead of independently querying cameras
and recomputing selection — guaranteeing the two systems always agree on which camera is
authoritative for a given anchor's position and its visibility, with no drift possible between two
independent implementations. Entities whose anchor found no qualifying camera this frame (fully
off every active viewport) are treated as out-of-range (hidden), matching today's contract.
Acceptance criteria updated to state explicitly: because nameplate anchors remain single-instance
(Phase 4 does not extend to them), an entity's nameplate shows in **at most one** viewport in
split-screen — a real, accepted limitation, not a bug.

**Phase 4 — stat label / world stat bar duplication (revised scope):** add `WorldLabelRank(rank as
u8)` + `Visibility::Hidden` (for `rank > 0`) to **both** the scene-loader's `pending_stat_labels` /
`WorldStatBarStyle::Ascii` spawn loops **and** `drain_dynamic_stat_ui_system`'s equivalent spawns,
spawning `MAX_SPLIT_PLAYERS` siblings exactly like the `world_labels:`/`label:` fix — but **only
when the loading scene is configured for split-screen** (reuse whatever scene-level flag/resource
`ActiveSplitScreen`/`SplitViewportSlot` setup already gates on; ordinary single-camera scenes get
exactly 1 entity per widget, zero behavior/perf change). `WorldStatBarStyle::Pixel` and nameplate
anchors are explicitly deferred (see Open questions) since their child-hierarchy duplication is a
materially bigger change. Docs: add a designer-facing note to `docs/20_data_formats.md` (~line
3083, beside the existing Pixel-depth-scaling limitation note) stating that in split-screen scenes,
stat labels and Ascii world stat bars duplicate correctly across simultaneously-visible viewports
while Pixel-style bars and damage popups do not — since the docs already recommend combining Ascii
+ Pixel bars on one prefab (~line 3120), this asymmetry is designer-reachable and must not ship
undocumented.

### Not in scope

- **Duplicating nameplate anchors, Pixel-style world stat bars, and damage popups across
  simultaneously-visible split viewports** — deferred out of Phase 4 due to child-hierarchy
  duplication complexity (see Research findings). Track as a follow-up phase if a real project need
  surfaces; the pattern to reuse is identical, just applied to a subtree instead of a single entity.
- **Per-viewport-correct particle billboarding** (Phase 1) — duplicating particle meshes per active
  camera. The single-shared-mesh particle pool architecture makes this a much larger change than
  picking a better camera; not attempted here.
- **A fully general "pick the active camera for point/cursor P" helper** — Phase 1/2/3 share only
  the sort comparator (extracted), not the full selection shape, since each site's input (no point,
  world point, cursor point) genuinely differs. Worth revisiting only if a fourth consumer needs
  the exact same shape as one of these three.

## Playtest setup — `local_coop_demo` changes needed

Confirmed by inspecting `assets/projects/local_coop_demo/`: it currently authors **no nameplates,
no stat labels/world stat bars, no particle effects, and no `ClickSelectable` entities anywhere** —
none of the four systems have anything to observe today. Each phase needs a small demo-project
addition before it can be dev-build play-tested, in addition to the standard ship workflow steps:

- **Phase 1**: place or trigger a particle effect (e.g. reuse an existing shared effect from
  `assets/shared/effects/` if one fits) visible from a split-screen room, so billboard orientation
  is visually checkable from both viewports.
- **Phase 2**: add a `ClickSelectable`-tagged prop or NPC to a room with a **fixed** (not dynamic)
  2-way split, so a click in each viewport can be tested independently against that viewport's own
  camera.
- **Phase 3**: enable `show_nameplates: true` (or a per-prefab override) on at least one entity in a
  split-screen room, to exercise distance-culling across 2+ active cameras.
- **Phase 4**: add a `stat_label` and/or Ascii `world_stat_bar` to a prefab placed where 2 fixed
  split viewports can simultaneously see it (mirrors the portal-label bug's original repro
  condition), to visually confirm duplication; also verify a dynamically-spawned instance (via
  `Action::Spawn`, e.g. a wave-spawned NPC) duplicates identically, per the dual-spawn-site scope
  above.

These additions are scoped to `local_coop_demo` only (the existing split-screen demo project) —
not a new project — and should be removed or left in place per Frank's preference once each phase's
playtest is confirmed (leaving them in place documents the fix for future reference, similar to how
Stage 6 left "Room N" labels on every portal).

## Tasks

- [ ] Phase 1: `rebuild_pool_meshes_system` — `is_active`-filtered, deterministically-ordered camera
      selection for billboard basis vectors (extract shared sort comparator here); regression test
      for the non-split single-camera case; `local_coop_demo` particle-effect playtest addition
- [ ] Phase 2: `click_select_system` — viewport-aware active-camera selection by cursor position
      (reuse Phase 1's comparator); test that a click in each viewport of a 2-way fixed split
      resolves against that viewport's own camera; `local_coop_demo` `ClickSelectable` playtest
      addition
- [ ] Phase 3: `world_label_screen_pos_system` stashes its selected camera/distance for anchor
      `WorldLabel`s; `nameplate_visibility_system` reads it instead of reselecting; drop
      `.single()`; tests for merged vs. split states confirming agreement between the two systems;
      `local_coop_demo` nameplate-enable playtest addition
- [ ] Phase 4: `WorldLabelRank` + `Visibility::Hidden` on `pending_stat_labels` and
      `WorldStatBarStyle::Ascii` spawn loops, gated on the scene being split-screen; identical
      treatment for `drain_dynamic_stat_ui_system`'s stat-label/Ascii-bar spawns; tests mirroring
      `test_entity_label_ranks_spawn_for_tracked_entity_labels_not_just_world_labels` for both spawn
      sites, plus a regression test confirming a non-split scene spawns exactly 1 entity per widget
      (no rank siblings); `docs/20_data_formats.md` designer-facing note; `local_coop_demo` stat
      widget playtest addition (scene-placed + dynamically-spawned)
- [ ] Docs: update `crates/ironhold_core/src/CLAUDE.md`'s known-limitations/consumer-duplication
      list as each phase ships (Phase 4 specifically: update in the same commit, per system-architect
      — list which `WorldLabel` consumers duplicate and which don't, to prevent the partial-coverage
      enumeration from drifting out of date) and `planning/claude_suggestions.md` ▸ Camera
- [ ] Tests (per phase, see above) + full suite green
- [ ] WASM dev build + updated playtest checklist (per phase, using the `local_coop_demo` additions
      above) per the standard ship workflow

## Open questions

- Should Phase 4 eventually cover nameplate anchors, Pixel-style world stat bars, and damage
  popups, or is that only worth doing if a real project actually needs simultaneous multi-viewport
  visibility for those? No current project does today.
- Is Phase 1's "pick one camera, document the limitation" an acceptable permanent answer, or does a
  future project need visually-correct billboarding from every simultaneously active split camera?
- Should the `local_coop_demo` playtest additions (particle effect, `ClickSelectable` prop,
  nameplate toggle, stat widget) stay in the project permanently after each phase ships, or be
  removed once confirmed? Leaning toward keeping them (documents the fix, consistent with Stage 6
  leaving "Room N" labels in place) — Frank's call at playtest time.

## Acceptance criteria

- Given any split-screen project, when particles render, then billboard orientation faces an actual
  active camera's basis vectors, never the unconditional world-axis fallback.
- Given a 2-way fixed split screen, when the player clicks inside one viewport, then click-to-select
  evaluates against that viewport's own camera, not an arbitrary other active camera.
- Given a split-screen scene with 2+ `Camera3d` entities (any split state, merged or active), when
  `nameplate_visibility_system` runs, then distance-culling evaluates against the exact same camera
  that positions that nameplate's anchor (via the stored selection), not an independently-reselected
  or no-op'd one.
- Given nameplate anchors remain single-instance after this feature, when an entity's nameplate is
  simultaneously on-screen in 2+ active split viewports, then it renders in **at most one** of them
  (accepted limitation, not a regression target).
- Given a stat label or Ascii world stat bar simultaneously visible in 2 active split viewports
  (scene-placed OR dynamically spawned via `Action::Spawn`), when the frame updates, then both
  viewports render their own correctly positioned copy (same contract as the shipped
  room-name-label fix).
- Given an ordinary single-camera (non-split) scene, when a stat label or world stat bar spawns,
  then exactly 1 entity is created per widget — no rank-duplication overhead, pixel-identical to
  today's behavior (regression + perf guard).
- Given any existing single-camera (non-split) scene, when any of the four systems run, then
  behavior is unchanged from today (regression guard).
