# Feature: `ironhold_cli validate` coverage for `label_depth_scale`

_Status: Draft_
_Planned at: `aae875a` (2026-08-16)_

## What
Two new `ironhold_cli validate` checks for `GameSceneV2.label_depth_scale` (`LabelDepthScaleDef`),
mirroring the load-time-silent-failure pattern this checker already catches elsewhere
(`missing_stat_widget_template`, `reserved_camera_mode_key`, etc.):

1. **Hard error** — `min_scale` outside the documented `[0.0, 1.0]` range. `min_scale: 1.5` (or
   negative) is accepted silently today and pins every depth-scaled widget in the scene at that
   factor forever, since `resolve_label_depth_scale`/`depth_scale_factor_from` never clamp the
   *input*, only the ratio computed from it.
2. **Warning** — `reference_distance` that falls entirely outside the scene's own player camera(s)'
   radius range. This is the exact misconfiguration that shipped in `3rd_person_game_demo` before
   the "Nameplate/health-bar spacing looks wrong at the zoom extremes" bug fix — a
   `reference_distance` far outside the camera's real zoom range means `(reference_distance /
   distance)` never drops below `1.0`, so depth scaling silently never engages at any zoom level.

## Why
Both silent-failure modes have zero design-time signal today — a designer only discovers either
one by actually zooming a camera in-browser and noticing nothing shrinks (or that everything is
stuck oversized). This is precisely the misconfiguration class the whole `label_depth_scale`
feature and its recent nameplate fix exist to prevent; without CLI coverage, the next project that
adds `show_nameplates`/`stat_label`/`world_stat_bar` can trivially reintroduce the same bug with no
warning at all. Promoted from `planning/claude_suggestions.md` (Nameplate System section,
2026-08-15/16) as part of a push to make the nameplate feature actually polished project-wide, not
just in `3rd_person_game_demo`.

## Approach

### `min_scale` range check (straightforward)
For every scene with a `label_depth_scale` block, if `min_scale` is `Some(v)` and `v < 0.0 || v >
1.0`, push a `CrossFileError` (`error_type: "label_depth_scale_min_scale_out_of_range"`). No
ambiguity here — `min_scale`'s own doc comment already states the valid range.

### `reference_distance` vs. camera range (the part that needs a decision)
This engine has **two** camera-config surfaces on a player prefab (`schema/catalog.rs`):
`components.camera: Option<CameraConfig>` (legacy `min_radius`/`max_radius`, used with
`camera.split`/`camera.party`) and `components.camera_mode: Option<CameraModeDef>` (v2 registry
enum — `Orbit`/`Party` variants carry their own `min_radius`/`max_radius`; `Fixed`/`FirstPerson`/
`Flycam` have no radius concept at all). A scene can also define additional `Orbit`/`Party` presets
in its `camera_modes:` registry (reachable only via `Action::SetCameraMode` at runtime, not
necessarily the camera a player starts in).

Proposed resolution, mirroring how `resolve_label_depth_scale` itself stays permissive rather than
strict:

1. Collect every **radius-bearing** camera config reachable from the scene:
   - each player-tagged prefab instantiated in this scene's `entities:` (via
     `prefab.components.tags` containing `"player"`, the established lookup pattern in
     `validate.rs`) — read `min_radius`/`max_radius` from whichever of `camera`/`camera_mode`
     (`Orbit`/`Party` variant) is present; skip a player prefab whose `camera_mode` is
     `Fixed`/`FirstPerson`/`Flycam` (no radius) instead of treating it as 0..0.
   - each `Orbit`/`Party` entry in the scene's own `camera_modes:` registry (reachable at runtime
     via `SetCameraMode`, so a scene could zoom into a range no player *starts* in).
2. If this collection is **empty** (every reachable camera is `Fixed`/`FirstPerson`/`Flycam`, or
   the scene has no player prefabs at all), skip the check entirely — there is no meaningful
   "camera range" to compare against, and a false warning here would be worse than no check.
3. Otherwise take the **union**: `overall_min = min(all min_radius)`, `overall_max = max(all
   max_radius)`. Warn (not error — this is inherently a heuristic, see Open Questions) if
   `reference_distance` falls entirely outside `[overall_min, overall_max]` by some margin (e.g.
   `reference_distance < overall_min * 0.5 || reference_distance > overall_max * 2.0` — a
   generous band, not an exact-range check, since this bug's own root cause was partly that
   *actual* camera-to-widget distance also depends on how far entities sit from the camera's own
   target, which the CLI has no way to know statically; see the real numbers in
   `3rd_person_game_demo`: `Orbit` radius `3.0`–`18.0`, but actual NPC-to-camera distances landed
   around `16`–`26`). Message should say "outside this scene's typical camera zoom range" rather
   than assert scaling definitely never engages, since the CLI can't prove the negative.

This mirrors the `camera_modes:` registry checks already in `validate.rs` (lines ~652–700) for
structure — same `for (scene_path, scene) in scenes` loop shape, same `CrossFileError` push.

## Tasks
- [ ] Add `min_scale` range hard error in `crates/ironhold_cli/src/commands/validate.rs`.
- [ ] Add the radius-collection helper (player prefabs' `camera`/`camera_mode`, plus the scene's
      own `camera_modes:` registry) and the `reference_distance` range warning.
- [ ] Unit/fixture tests in `ironhold_cli`'s existing cross-file test suite — at minimum: `min_scale`
      out of range (both directions), `reference_distance` inside range (no warning),
      `reference_distance` outside range with an `Orbit` camera, a scene with only `Fixed`/`Flycam`
      cameras (no warning, no crash), a scene with no player prefabs at all (no warning, no crash).
- [ ] Verify against real projects: run `cargo run -p ironhold_cli -- validate` on every example
      project — `3rd_person_game_demo`'s current tuning (`reference_distance: 12.0` against `Orbit`
      `3.0`–`18.0`) should NOT warn; deliberately revert to the old `(8.0, 0.25)` locally and
      confirm it also doesn't false-positive-warn (that tuning was about *legibility*, not
      *engagement range* — `8.0` is still inside `3.0`–`18.0`). Also spot-check `primitive_world`/
      `stats_demo` (already have `label_depth_scale` blocks) don't newly warn.
- [ ] Docs: add a short note to `docs/20_data_formats.md`'s "Label depth scaling" section pointing
      at the new validate checks, and to `docs/20_data_formats.md`'s CLI/validate reference if one
      exists.

## Open questions
- **Warning vs. hard error for `reference_distance`?** Proposed as a warning since it's a
  heuristic band, not a provable misconfiguration — but if false positives turn out to be rare in
  practice across real projects, Frank may want to tighten it to an error later. Decide after
  seeing real output across all example projects (see the verification task above).
- **Union vs. per-player-worst-case for split-screen?** The union approach above means a scene
  where player 1 has a tight `Orbit` range and player 2 has a much wider one won't warn even if
  `reference_distance` is badly wrong for player 1 specifically, as long as it's fine for player 2.
  Tightening to "must be in-range for every player's own camera individually" is more precise but
  more likely to false-positive on scenes that intentionally give players different zoom ranges.
  Leaning toward union (fewer false positives) unless real projects show this hides real bugs.
- **Should the check also account for `camera_modes:` presets never actually reached by any
  `SetCameraMode` action?** Probably not worth the complexity — the union approach already errs
  toward inclusion (more cameras counted → wider acceptable band → fewer false positives), so an
  unreachable preset only makes the check more permissive, never wrongly strict.

## Acceptance criteria
- Given a scene with `label_depth_scale.min_scale` outside `[0.0, 1.0]`, `ironhold_cli validate`
  reports a hard error naming the scene and the out-of-range value.
- Given a scene with `label_depth_scale.reference_distance` far outside every reachable
  radius-bearing camera's range, `ironhold_cli validate` reports a warning naming the scene, the
  configured `reference_distance`, and the camera range it was compared against.
- Given a scene whose only cameras are `Fixed`/`FirstPerson`/`Flycam` (no radius concept), or a
  scene with no player prefabs, `ironhold_cli validate` neither warns nor crashes.
- Given `3rd_person_game_demo`'s current tuning (`reference_distance: 12.0`), `stats_demo`'s
  (`20.0`), and `primitive_world`'s (`25.0`), none produce a new warning.
