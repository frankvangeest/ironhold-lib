# Feature: Split-Screen — Remaining Single-Camera Assumption Sites

_Status: Draft_
_Planned at: `4848727` (2026-07-11)_

## Phases

| Phase | Backlog item | Status | Completed |
|---|---|---|---|
| v1 | `nameplate_visibility_system` distance-culling | Queued | — |
| v2 | `WorldLabelRank` extended to stat labels / world stat bars | Queued | — |
| v3 | `particle_renderer.rs` billboard orientation | Queued | — |
| v4 | `targeting.rs` viewport-aware click-to-select | Queued | — |

## What

Four systems still assume at most one `Camera3d` exists, predicted in
`planning/claude_suggestions.md` ▸ Camera back in 2026-07-05 (before any split-screen code was
written) and explicitly left "not touched" by
`planning/features/done/world_label_split_screen_positioning.md`, which fixed the fifth (portal
room-name / entity labels) via `world_label_screen_pos_system`:

1. `nameplate_visibility_system` (`nameplate.rs:212`) — distance-culling, `camera_q.single()`.
2. `WorldLabelRank`'s multi-viewport duplication only covers `scene_loader.rs`'s `world_labels:`
   and `label:` (`EntityLabelDef`) spawn loops. Stat labels, world stat bars (Ascii + Pixel),
   damage popups, and nameplate anchors are still single-instance (implicit rank 0), so they don't
   duplicate across two simultaneously-visible split viewports the way room-name labels now do.
3. `particle_renderer.rs:303` (`rebuild_pool_meshes_system`) — billboard orientation basis vectors.
4. `targeting.rs:122` (`click_select_system`) — click-to-select nearest-entity search.

## Why

Every split-screen scene (`local_coop_demo`, Stage 3+) has 2+ `Camera3d` entities alive
simultaneously — often even when only one is currently `is_active` (a merged dynamic split still
keeps the inactive sibling camera entity around). Each of the four sites above either silently
no-ops, falls back to an arbitrary/incorrect default, or picks a non-viewport-aware camera whenever
that's true. None of them currently break anything visibly in `local_coop_demo` today only because
nameplates aren't enabled there and click-targeting/particles aren't exercised across simultaneous
split viewports yet — but any future project combining split-screen with these systems will hit the
same class of bug the room-name labels did.

## Research findings

- **`nameplate_visibility_system`'s query has no `is_active` filter at all**
  (`Query<&GlobalTransform, With<Camera3d>>`), so `.single()` fails whenever 2+ `Camera3d` entities
  exist *at all* — not just when 2+ are simultaneously active. In a merged dynamic split (1 active
  camera, 1 inactive sibling still alive) this still silently no-ops every frame. This is a
  stricter/wider bug than "which of two active cameras" — it fails even in the *non-split* common
  case once any split-screen project's cameras are all spawned at scene load.
- **`particle_renderer.rs:303` has the identical defect** — same query shape, same missing
  `is_active` filter, same `.single()` call — except its failure mode is a *silent visual
  fallback* (`Vec3::X, Vec3::Y` axis-aligned billboarding) rather than an early return. This means
  every particle billboard in any split-screen project is billboarding against world axes, not the
  camera, at all times — a more severe practical bug than the other three, since it's unconditional
  rather than gated on an actual simultaneous-camera edge case.
- **`targeting.rs:122` already filters on `camera.is_active`**
  (`cameras.iter().find(|(c, _)| c.is_active)`) — it does not share the other three's "any 2+
  cameras alive" defect. Its remaining gap is narrower: when 2 cameras are *simultaneously* active
  (a fixed split, not a merged dynamic one), `.find()` returns whichever camera iterates first,
  regardless of which viewport the cursor is actually over — so a click in player 2's viewport can
  be evaluated against player 1's camera, silently selecting the wrong entity or missing a
  should-be-in-range one.
- **`world_label_screen_pos_system`'s selection primitive is directly reusable** for the
  viewport-aware half of this work: `camera.world_to_viewport(cam_gt, point)` +
  `camera.logical_viewport_rect().is_some_and(|r| r.contains(point))`, iterated over
  `camera.is_active` cameras in `SplitViewportSlot`-deterministic order. `targeting.rs` needs this
  same test applied to the *cursor position* (find the active camera whose viewport contains the
  cursor) rather than a projected world point.
- **`WorldLabelRank`'s duplication pattern is directly reusable** for v2: spawn `MAX_SPLIT_PLAYERS`
  ranked sibling entities instead of 1, `rank > 0` starts `Visibility::Hidden`,
  `world_label_screen_pos_system already resolves `.nth(rank)` generically — no changes needed to
  that system itself, only to the spawn sites. The complication is that nameplate anchors and Pixel
  stat bars aren't flat `Text2d` entities — they have child hierarchies (border/fill mesh children
  parented to an anchor). Duplicating those means duplicating the whole child subtree per rank, not
  just one entity, which is a larger and more error-prone change than the original `world_labels:`
  fix. Scoping v2 to stat labels + Ascii world stat bars first (flat `Text2d`, same shape as the
  already-fixed labels) and treating nameplate anchors + Pixel bars + damage popups as a follow-up
  keeps each phase's diff small and independently reviewable, matching how the original fix was
  staged (scene-level `world_labels:` first, then corrected to also cover `label:`).

## Approach

**v1 — `nameplate_visibility_system`:** change `camera_q` to
`Query<(&Camera, &GlobalTransform), With<Camera3d>>`, drop `.single()`, and for each nameplate
resolve distance against the *same* camera `world_label_screen_pos_system` will use to position
that entity's anchor this frame (same `SplitViewportSlot`-deterministic "first qualifying active
camera" order) — not an arbitrary/first active camera. This keeps the two systems in agreement
about which camera is authoritative for a given anchor, avoiding flicker where one system hides
based on camera A's distance while the other positions based on camera B's viewport. Entities
out of range of every active camera hide, matching today's contract.

**v2 — stat label / world stat bar duplication:** add `WorldLabelRank(rank as u8)` +
`Visibility::Hidden` (for `rank > 0`) to the `pending_stat_labels` and `WorldStatBarStyle::Ascii`
spawn loops in `scene_loader.rs`, spawning `MAX_SPLIT_PLAYERS` siblings exactly like the
`world_labels:`/`label:` fix. `WorldStatBarStyle::Pixel` and nameplate anchors are explicitly
deferred (see Open questions) since their child-hierarchy duplication is a materially bigger change.

**v3 — particle billboard orientation:** change `camera_q` in `rebuild_pool_meshes_system` to
`Query<(&Camera, &GlobalTransform), With<Camera3d>>`, filter `is_active`, and pick the first
qualifying camera in `SplitViewportSlot`-deterministic order (same tie-break as v1/the label fix)
instead of falling back to world axes whenever 2+ `Camera3d` entities merely exist. Document as a
known, accepted limitation that with 2 *simultaneously* active split cameras at different angles,
particles will only billboard correctly toward the picked camera — true per-viewport-correct
billboarding would require duplicating particle meshes per viewport, out of scope here (see Not in
scope).

**v4 — `targeting.rs` viewport-aware click-to-select:** before the existing nearest-entity search,
resolve which active camera's `logical_viewport_rect()` contains the cursor position (reusing the
`world_label_screen_pos_system` selection primitive), and use only that camera for the
`world_to_viewport` distance math — replacing `cameras.iter().find(|(c, _)| c.is_active)`'s
arbitrary first-match.

### Not in scope

- **Duplicating nameplate anchors, Pixel-style world stat bars, and damage popups across
  simultaneously-visible split viewports** — deferred out of v2 due to child-hierarchy duplication
  complexity (see Research findings). Track as a v5 if a real project need surfaces; the pattern to
  reuse is identical, just applied to a subtree instead of a single entity.
- **Per-viewport-correct particle billboarding** (v3) — duplicating particle meshes per active
  camera. The single-shared-mesh particle pool architecture makes this a much larger change than
  picking a better camera; not attempted here.
- **Extracting a shared "pick the active camera whose viewport contains point P" helper** — v1, v3,
  and v4 all reimplement a shape of this selection independently again, same call as the original
  fix's deferral. Worth revisiting once 3-4 call sites share the exact same query shape; still
  premature until this phase's implementations reveal whether the shapes actually converge.

## Tasks

- [ ] v1: `nameplate_visibility_system` — drop `.single()`, per-entity active-camera selection
      matching `world_label_screen_pos_system`'s order; tests for merged vs. split states
- [ ] v2: `WorldLabelRank` + `Visibility::Hidden` on `pending_stat_labels` and
      `WorldStatBarStyle::Ascii` spawn loops; tests mirroring
      `test_entity_label_ranks_spawn_for_tracked_entity_labels_not_just_world_labels`
- [ ] v3: `rebuild_pool_meshes_system` — `is_active`-filtered, deterministically-ordered camera
      selection for billboard basis vectors; regression test for the non-split single-camera case
- [ ] v4: `click_select_system` — viewport-aware active-camera selection by cursor position; test
      that a click in each viewport of a 2-way fixed split resolves against that viewport's own
      camera
- [ ] Docs: update `crates/ironhold_core/src/CLAUDE.md`'s known-limitations note and
      `planning/claude_suggestions.md` ▸ Camera as each phase ships
- [ ] Tests (per phase, see above) + full suite green
- [ ] WASM dev build + playtest per phase per the standard ship workflow

## Open questions

- Should v2 eventually cover nameplate anchors and damage popups, or is that only worth doing if a
  real project actually enables nameplates in a split-screen scene? No current project does today.
- Is v3's "pick one camera, document the limitation" an acceptable permanent answer, or does a
  future project need visually-correct billboarding from every simultaneously active split camera?
- Order of phases: v1-v2-v3-v4 as listed follows severity (v3's unconditional-fallback bug is
  arguably worse than v1/v4's edge-case gating) — worth reordering to fix v3 first?

## Acceptance criteria

- Given a split-screen scene with 2+ `Camera3d` entities (any split state, merged or active),
  when `nameplate_visibility_system` runs, then distance-culling evaluates against the same camera
  that positions that nameplate's anchor, not a silent no-op.
- Given a stat label or Ascii world stat bar simultaneously visible in 2 active split viewports,
  when the frame updates, then both viewports render their own correctly positioned copy (same
  contract as the shipped room-name-label fix).
- Given any split-screen project, when particles render, then billboard orientation faces an actual
  active camera's basis vectors, never the unconditional world-axis fallback.
- Given a 2-way fixed split screen, when the player clicks inside one viewport, then click-to-select
  evaluates against that viewport's own camera, not an arbitrary other active camera.
- Given any existing single-camera (non-split) scene, when any of the four systems run, then
  behavior is unchanged from today (regression guard).
