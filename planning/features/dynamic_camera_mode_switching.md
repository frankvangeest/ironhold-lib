# Feature: Dynamic camera mode switching demo

_Status: Draft_
_Planned at: `59bd33c` (2026-08-31) — hash updated after the 2026-09-03 `pkg/` history purge; the original citation, `0ff54d5`, was a pkg-only rebuild commit fully pruned during that purge, so this points to its parent instead (same code state)_

## Phases

| Phase | Backlog item | Status | Completed |
|---|---|---|---|
| v1 | One `camera_modes` scene that live-switches between every *existing* mode (`Orbit`, `Follow`, `FirstPerson`, `Fixed`, `Flycam`, `Party`) via UI buttons/hotkeys, no scene reload | Queued | — |
| v2 | Extend the same scene to include `OverTheShoulder`/`LockOn` once those ship (see `planning/backlog.md` ▸ Camera) | Queued | — |

## What
A new scene in the existing `camera_modes` project where a player can cycle through *every* camera
mode live, in place, via `Action::SetCameraMode` — instead of today's structure, where trying a
different mode means walking onto a different pad and triggering a full scene `LoadScene`
transition to a dedicated single-mode test scene (`follow_test.scene.ron`,
`first_person_test.scene.ron`, etc.). This is a genuinely different demonstration: it shows off the
`camera_modes:` registry + `Action::SetCameraMode` v2 mechanism itself (an already-shipped engine
feature — see `planning/features/done/camera_modes.md` — that today has no single project actually
demonstrating a live in-scene switch between more than two modes), and gives a designer one place to
compare framing/feel across modes without the scene-transition context switch resetting their
mental model of "where am I."

## Why
`camera_modes` v2 shipped the `camera_modes:` scene registry and `Action::SetCameraMode` (with
`CameraBlendState` smooth transitions between modes) specifically so a scene could switch a camera
live — e.g. a cutscene cutting to a fixed shot and back. No shipped project actually exercises
switching between more than two registry entries in one running scene; this demo is the natural
"prove it, and let a designer feel the difference" companion to that engine feature, and doubles as
the natural home for OTS/Lock-on once those ship (v2 phase) since designers will want to compare
them against the existing modes side-by-side, not in isolation.

## Approach
- New scene, e.g. `assets/projects/camera_modes/scenes/live_switch.scene.ron`, reachable via a new
  portal from `main.scene.ron`'s hub (existing pattern — see its `portal_to_*` entities) rather than
  replacing any of the existing single-mode test scenes (keeps their baselines/tests untouched).
- Author a `camera_modes:` registry entry per mode on the scene (mirroring
  `camera_modes.md`'s "Named mode registry" resolution — see `docs/30_runtime_events_and_logic.md`),
  each pre-tuned so the switch is a clean comparison (consistent starting distance/height where the
  mode allows it).
- UI: an `ActionBar` (reusing the existing skill-bar mechanism, or a plain row of `Button`s) with one
  slot per mode, each firing `SetCameraMode(mode: "<name>")` — number-key hotkeys `1`-`6` (`1`-`8`
  once OTS/Lock-on land) double as the fast-iteration path for a designer comparing modes rapidly.
  `"default"` (the scene-authored starting mode) should also be reachable, matching the registry's
  reserved-key convention.
- Leans entirely on already-shipped mechanics — `CameraBlendState`'s existing transition smoothing
  means switches don't need to be instant cuts; author a `transition:` block per registry entry if a
  soft blend reads better than a hard cut for a given mode pair (e.g. `Orbit` → `Fixed` cutscene-
  style vs. `Orbit` → `Follow`, which might read better as a cut).
- `Flycam`/`Party` are worth including for completeness even though they're less directly comparable
  (`Flycam` has no player target; `Party` needs 2+ players) — decide during implementation whether
  `Party` is in scope for a single-player demo scene at all, or deferred to `local_coop_demo`'s own
  camera coverage instead.

## Tasks
- [ ] Author `live_switch.scene.ron` with a `camera_modes:` registry covering `Orbit`/`Follow`/
      `FirstPerson`/`Fixed`/`Flycam` (decide on `Party` per the open question above).
- [ ] UI/hotkey wiring for `SetCameraMode` per mode.
- [ ] Add a portal from `main.scene.ron`'s hub.
- [ ] `ironhold_cli validate` clean; register the new scene per `test_web.py`'s baseline convention.
- [ ] Docs: note the new scene in `docs/30_runtime_events_and_logic.md`'s camera_modes section as
      the "compare every mode live" reference example.
- [ ] v2 (once OTS/Lock-on ship): add both to the same registry + UI.

## Open questions
- Should `Party` be included in a single-player demo scene (would need a second, non-interactive
  player entity purely to give `Party` something to frame), or left out and covered by
  `local_coop_demo` instead?
- Button bar vs. plain number-key hotkeys vs. both — the existing `camera_modes` project uses plain
  `Label`/portal hints, not an `ActionBar`; decide whether introducing an `ActionBar` here is worth
  the extra RON, or whether a `Label` listing the hotkeys is sufficient for a demo project.
- Does OTS end up needing a new `CameraModeDef` variant at all (see the backlog item — it may just
  be a tuned `Follow`), which would change whether v2 needs any engine work beyond RON.

## Acceptance criteria
- Given the `live_switch` scene is loaded, pressing each mode's hotkey/button switches the active
  camera to that mode with no scene reload, and the previous mode's camera cleanly stops driving the
  transform (no fighting between two modes' systems in the same frame).
- Given `Fixed` or another mode with a `transition:` block, switching onto it blends rather than cuts.
- Given the demo is played through once for every mode, no console errors.
