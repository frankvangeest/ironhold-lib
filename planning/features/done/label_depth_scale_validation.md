# Feature: `ironhold_cli validate` coverage for `label_depth_scale`

_Status: Done_
_Planned at: `aae875a` (2026-08-16)_
_Plan-reviewed at: `68ae660` (2026-08-30) — system-architect + ux-gamedesigner-reviewer, both addressed below_
_Completed: `034b40c` (2026-08-31) — implementation + 5-agent post-implementation review round, all
findings fixed in the same commit; playtest confirmed by Frank. See `planning/backlog.md`'s Done
entry for the full summary._

## What
Two silent-failure modes in `GameSceneV2.label_depth_scale` (`LabelDepthScaleDef`) get design-time
*and* runtime signal, mirroring the existing `start_at_fraction`/`jump_cannot_clear_ground_sensor`
precedents (design-time check + a matching runtime `warn!`, not CLI-only):

1. **`min_scale` out of `[0.0, 1.0]` range.**
   - `ironhold_cli validate` — hard error (`CrossFileError`, always-on, not `--strict`-gated).
   - Runtime — `depth_scale_factor_from` (`lib.rs:536-538`) already computes
     `(ref_dist / dist).min(1.0).max(min_floor)`: a `min_floor > 1.0` genuinely **pins every
     depth-scaled widget in the scene at that factor forever**; a **negative** `min_floor` is
     **inert** (the `.max()` never binds against an already-non-negative ratio) — a silent no-op,
     not a pin. Add a scene-load clamp + `warn!` (mirrors `animation_resolver.rs:139-150`'s
     `start_at_fraction` clamp-and-warn) so a WASM-only designer (no `ironhold_cli` access) also
     gets a signal, not just an exit-1 they'll never see.
2. **`reference_distance` far outside the scene's reachable player-camera radius range.** This is
   the exact misconfiguration that shipped in `3rd_person_game_demo` before the "Nameplate/
   health-bar spacing looks wrong at the zoom extremes" bug fix — scaling never engages at any
   zoom level.
   - `ironhold_cli validate --strict` — `StrictWarning`, following the `jump_cannot_clear_ground_
     sensor` precedent (`validate.rs`'s `strict_checks()`, ~line 1001-1050) — **not** a
     `CrossFileError**. `CrossFileError` (`validate.rs:29`) is unconditionally a hard error;
     `StrictWarning` (`:35`) is the CLI's only actual warning tier, and it only prints under
     `--strict`.
   - Runtime — a scene-load `warn!` in `scene_loader.rs`, called alongside `warn_missing_player_
     stat_templates` (~line 808, where `player_configs` — with each player's resolved `camera`/
     `camera_mode` — is already assembled), mirroring `warn_jump_cannot_clear_ground_sensor`'s
     precedent (`scene_loader.rs:2866`) so the WASM-only designer gets this signal too.

## Why
Both silent-failure modes had zero design-time signal before this feature — a designer only
discovered either one by actually zooming a camera in-browser and noticing nothing shrinks (or
that everything is stuck oversized). Plan-review (system-architect + ux-gamedesigner-reviewer)
surfaced that a CLI-only fix would still miss the audience that matters most: a designer working
from a prebuilt WASM build with no `ironhold_cli` access ever sees a CLI warning at all — so this
plan now pairs each check with the runtime `warn!` counterpart the codebase already uses for
exactly this audience (`docs/20_data_formats.md:1986`). Promoted from `planning/claude_
suggestions.md` (Nameplate System section, 2026-08-15/16).

## Approach

### Schema default: `reference_distance`'s fallback changes `50.0` → `20.0`
`default_label_ref_distance()` (`schema/scene_v2.rs:792`) currently returns `50.0`. Checked every
shipped project with a `label_depth_scale` block (12 scene files, 8 projects) — **all** set
`reference_distance` explicitly; **none** rely on the schema default, and since `label_depth_scale`
is `Option`, the default is never read at all for a scene that omits the block entirely. Changing
the default is therefore a zero-impact change for every shipped project today.

`20.0` is chosen to match `entity_spawner.rs::default_camera_config()`'s `max_radius: 20.0`
(lines 1657-1664) — the engine's own fallback `CameraConfig`, used whenever a player prefab omits
`camera`/`camera_mode` entirely (already referenced by comments in `primitive_world`/`stats_demo`
as "the engine's default Orbit range"). A new project that authors `label_depth_scale: ()` with no
custom camera config now lands exactly at the edge of that default camera's zoom-out range
(`20.0 / 20.0 = 1.0` — scaling just starts to engage at max zoom-out) instead of `50.0`, which sits
entirely outside every typical camera range and never engages at all — closing off the very
misconfiguration class this feature exists to catch, right at the schema level, with no need for a
"skip check when at default" special case in the validator itself.

**Also fix the second, independent hardcoded `50.0`** in `resolve_label_depth_scale`
(`runtime/scene_manager/mod.rs:501`, the `None`-scene fallback reached when a per-label
`depth_scale: true` override is set with no scene-level `label_depth_scale:` block at all) — it
does not read `default_label_ref_distance()`, it duplicates the literal. Update it to `20.0` too
(or better, call `default_label_ref_distance()` directly) so the two fallbacks can't drift apart
again the next time either one is tuned.

### `min_scale` range check + runtime clamp
- **CLI**: for every scene with a `label_depth_scale` block, if `min_scale` is `Some(v)` and
  `v < 0.0 || v > 1.0`, push a `CrossFileError` (`error_type:
  "label_depth_scale_min_scale_out_of_range"`). Message must name the scene, the value, and the
  concrete consequence — e.g. `"min_scale 1.5 in <scene> is outside [0.0, 1.0] — this would pin
  every nameplate/stat label/bar in this scene at 150% size forever (values <0.0 are silently inert
  instead)."`
- **Runtime**: clamp `min_scale` to `[0.0, 1.0]` at scene load (wherever `LabelDepthScaleDef` is
  read into `LoadedLabelDepthScale`, mirroring `apply_seek_and_freeze`'s clamp-and-warn shape) and
  `warn!` with the same message content as the CLI error. The CLI check is normally what a designer
  sees first; the runtime clamp is the safety net for anyone playtesting a build the CLI never
  validated.

### `reference_distance` vs. camera range
This engine has **two** camera-config surfaces on a player prefab: `components.camera:
Option<CameraConfig>` (`schema/player.rs:104` — legacy, used with `camera.split`/`camera.party`)
and `components.camera_mode: Option<CameraModeDef>` (`schema/camera.rs:26` — v2 registry enum).
`CameraModeDef` variants:
- `Orbit(CameraConfig)` / `Party(PartyCameraDef)` — radius-bearing (`min_radius`/`max_radius`).
- `Fixed` / `FirstPerson` / `Flycam` — no radius concept; skip.
- `Follow(FollowCameraDef)` — constant `offset: (f32, f32, f32)` (`schema/camera.rs:112`), a
  well-defined **fixed** camera-to-target distance. Contributes `min = max = offset.length()` to
  the union rather than being skipped — it still narrows/widens the acceptable band meaningfully.

A scene can also define additional `Orbit`/`Party` presets in its `camera_modes:` registry
(reachable only via `Action::SetCameraMode` at runtime).

1. Collect every **radius-bearing** camera config reachable from the scene:
   - each player-tagged prefab instantiated in this scene's `entities:` (via `prefab.components.
     tags` containing `"player"`) — read from whichever of `camera`/`camera_mode` is present;
     `Orbit`/`Party` contribute `min_radius`/`max_radius`, `Follow` contributes
     `offset.length()` as both bounds, `Fixed`/`FirstPerson`/`Flycam` are skipped.
   - each `Orbit`/`Party` entry in the scene's own `camera_modes:` registry.
2. If this collection is **empty** (every reachable camera is `Fixed`/`FirstPerson`/`Flycam`, or
   the scene has no player prefabs at all), skip the check entirely.
3. Otherwise take the union: `overall_min = min(all mins)`, `overall_max = max(all maxes)`. Warn
   if `reference_distance < overall_min * 0.5 || reference_distance > overall_max * 2.0` (a
   generous band — this bug's own root cause showed actual camera-to-widget distance can exceed
   the raw radius range, e.g. `3rd_person_game_demo`: `Orbit` `3.0`-`18.0` but real NPC-to-camera
   distances landed `16`-`26`). Message must name the scene, the configured `reference_distance`,
   the camera range compared against, **and a concrete suggested value** — e.g. `"reference_
   distance 50.0 in <scene> is outside this scene's typical camera zoom range (3.0-18.0) — try
   ~10.5 (the range midpoint), then confirm in-browser."`

This mirrors the `camera_modes:` registry checks already in `validate.rs` (~lines 870-941) for
structure, but the check itself lives in `strict_checks()` (~line 1001-1050, beside
`jump_cannot_clear_ground_sensor`), not alongside those registry checks.

## Tasks
- [ ] Change `default_label_ref_distance()` (`schema/scene_v2.rs:792`) from `50.0` to `20.0`.
- [ ] Fix the duplicate hardcoded `50.0` fallback in `resolve_label_depth_scale`
      (`runtime/scene_manager/mod.rs:501`) to match (call `default_label_ref_distance()` or update
      the literal to `20.0`).
- [ ] Add `min_scale` range hard error (`CrossFileError`) in `crates/ironhold_cli/src/commands/
      validate.rs`, with a message stating the concrete consequence (pin vs. inert-no-op).
- [ ] Add a scene-load clamp + `warn!` for out-of-range `min_scale` (wherever `LabelDepthScaleDef`
      is read into `LoadedLabelDepthScale`), mirroring `animation_resolver.rs`'s `start_at_
      fraction` clamp-and-warn pattern.
- [ ] Add the radius-collection helper (player prefabs' `camera`/`camera_mode`, including the
      `Follow` fixed-distance case, plus the scene's own `camera_modes:` registry) and the
      `reference_distance` `StrictWarning` in `strict_checks()`, with a message including a
      concrete suggested value.
- [ ] Add a matching scene-load `warn!` for `reference_distance` outside the reachable camera
      range, called alongside `warn_missing_player_stat_templates` (`scene_loader.rs`, ~line 808).
- [ ] Unit/fixture tests in `ironhold_cli`'s cross-file test suite — at minimum: `min_scale` out of
      range (both directions), `reference_distance` inside range (no warning), `reference_distance`
      outside range with an `Orbit` camera, a `Follow`-only scene (fixed distance narrows the
      band), a scene with only `Fixed`/`Flycam` cameras (no warning, no crash), a scene with no
      player prefabs at all (no warning, no crash), and a **split-screen fixture** (multiple
      player-tagged prefabs with different camera ranges — confirms the union approach).
- [ ] Verify against real projects: run `cargo run -p ironhold_cli -- validate --strict` on every
      example project — `3rd_person_game_demo` (`reference_distance: 12.0` vs. `Orbit` `3.0`-
      `18.0`), `stats_demo`/`primitive_world` (`10.0` vs. the engine default `2.0`-`20.0`), and
      **`local_coop_demo` rooms 3/9/10** (`6.0` vs. split-screen `Orbit` `4.5`-`9.0`) should NOT
      warn. Deliberately test a reverted `(8.0, 0.25)` tuning locally to confirm it also doesn't
      false-positive (that retune was about *legibility*, not *engagement range* — `8.0` is still
      inside `3.0`-`18.0`).
- [ ] Docs: `docs/20_data_formats.md`'s "Label depth scaling" section — note both the CLI checks
      (naming which is a hard error vs. `--strict`-only) and their runtime `warn!` counterparts
      (mirroring the `start_at_fraction` docs wording), and cross-link from the existing "no
      per-widget override" paragraph. Also add the two checks to `docs/60_contributing.md`'s
      `validate <project_dir>` check list.

## Resolved during plan review (2026-08-30)
- **Runtime `warn!` counterparts added to scope** (system-architect confirmed this stays CLI-crate
  + a small `ironhold_core` scene-load addition, no runtime-hot-path/WASM-perf/determinism
  surface — the added `warn!` calls are scene-load-only, not per-frame).
- **`reference_distance`'s schema default changes `50.0` → `20.0`**, matching the engine's own
  fallback camera's `max_radius` — see "Schema default" above. Zero shipped-project impact
  (verified: no project relies on the default).
- **`CameraModeDef::Follow` contributes a fixed distance** (`min = max = offset.length()`) to the
  radius union, rather than being skipped like `Fixed`/`FirstPerson`/`Flycam`.
- **Severity tiers corrected**: `min_scale` is a `CrossFileError` (always-on hard error);
  `reference_distance` is a `StrictWarning` (`--strict`-gated), mirroring `jump_cannot_clear_
  ground_sensor` — there is no generic "warning" tier in `ironhold_cli validate` outside
  `--strict`.
- **Negative `min_scale` wording corrected**: it's an inert no-op (never binds against an
  already-non-negative ratio), not "pins forever" like `> 1.0` — still worth a hard error since
  it's a documented-range violation with unexpected (silently-doing-nothing) behavior, just not
  the same failure mode as too-high a value.

## Acceptance criteria
- Given a scene with `label_depth_scale.min_scale` outside `[0.0, 1.0]`, `ironhold_cli validate`
  reports a hard error naming the scene, the out-of-range value, and the concrete consequence; the
  runtime clamps the value and logs a matching `warn!` at scene load.
- Given a scene with `label_depth_scale.reference_distance` far outside every reachable
  radius-bearing camera's range, `ironhold_cli validate --strict` reports a warning naming the
  scene, the configured `reference_distance`, the camera range it was compared against, and a
  concrete suggested value; the runtime logs a matching `warn!` at scene load.
- Given a scene whose only cameras are `Fixed`/`FirstPerson`/`Flycam` (no radius concept), or a
  scene with no player prefabs, neither the CLI check nor the runtime warn fires or crashes.
- Given a `Follow`-mode camera, its `offset.length()` is treated as a fixed point in the union,
  narrowing or widening the acceptable `reference_distance` band accordingly.
- Given `3rd_person_game_demo`'s current tuning (`reference_distance: 12.0`), `stats_demo`'s/
  `primitive_world`'s (`10.0`), and `local_coop_demo` rooms 3/9/10's (`6.0`), none produce a new
  warning.
