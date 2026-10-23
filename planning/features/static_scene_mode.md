# Feature: Static Scene Mode

_Status: Ready (plan review 2026-10-23: system-architect + ux-gamedesigner-reviewer; the spike in Task 1 gates the rest)_
_Planned at: `9ff431b` (2026-06-18); re-planned at `e2a5578` (2026-10-23)_

## What

A harness mode, `?static=1` on the page URL, that holds the simulation **from the first frame**: no `FixedUpdate` tick (physics,
player, NPC AI, motion, trigger zones), no particle simulation or emission, no delayed events or timers, and a shader clock that
stays at 0. UI, the interpreter/executor, rendering, asset loading and audio keep running, so the scene still loads and draws.
Opt-in for automated tooling only; designers never need it.

## Why

`test_web.py` takes one baseline screenshot per scene. By the time the shot fires, characters are mid-animation, particles are
mid-flight and shader effects (`custom_pulse`, `custom_water_stylized`) sit at whatever moment the load happened to end, so
baselines flake between runs and machines (the 4% diff threshold hides some of it). Static mode gives one deterministic picture.

## Design (after plan review)

**Hold from boot, no freeze system.** In `start_app`, when the run mode is `Static`, insert `SimClock` with
`PauseHold::Static` already held. The first real-time Bevy update has zero delta, so virtual elapsed time stays exactly 0, and
`apply_sim_clock_system` pauses `Time<Virtual>` on the first frame (its `resource_changed` condition is true then). This removes
every earlier idea of a system that reacts to `SceneEvent::Ready`:
- shader `globals.time` is the virtual elapsed time (bevy_render `globals.rs`), so a Ready-time freeze would have frozen it at a
  value that differs every run and defeated pixel identity; frame-0 gives exactly 0;
- `SceneEvent::Ready` is written in the same `Update` that queues the scene's spawn commands and the fixed loop runs before
  `Update`, so a scene gets zero fixed ticks anyway and bodies stay at their authored transforms ("frozen mid-fall" does not happen);
- terrain, entity spawn, pipeline warmup and the spawn queue run in `Update` and count frames, not time, so loading completes;
  message buffers keep flowing while held (`keep_messages_flowing_while_held`);
- no new system means no `schedule_order_tests` change.

**Dropped from the old plan:** seeking `AnimationPlayer`s to t=0 (no player exists at Ready, it would overwrite authored freeze
poses such as a corpse's death pose, and with delta 0 a clip never leaves t=0 while `animate_targets` still samples it) and
resetting NPC transforms (nothing drifts, and papering over drift is wrong).

**`game.paused` / `game.resumed` contract (the one designer-visible change).** `apply_sim_clock_system` emits these when
`Time<Virtual>` flips paused, so a Static hold would open every pause-menu rule (`first_person_starter` maps `game.paused` to its
pause overlay) in each screenshot, and, with Static already holding, a later `PauseGame` would emit nothing at all. Fix: separate
"the clock is paused" (any hold) from "the game is paused, as a designer sees it":
- `PauseHold::announces(self) -> bool`: `Menu` true, `Static` false (a future `FocusLoss`/`Session` chooses explicitly);
- `apply_sim_clock_system` keeps a `Local<bool>` of the last announced state and emits `game.paused` / `game.resumed` when the
  announcing holds appear/disappear, still the only emitter and the only caller of `pause()`/`unpause()`;
- the designer sentence in `docs/30_runtime_events_and_logic.md` (~123, ~281) and the `PauseGame` row in `docs/20_data_formats.md`
  (~4020) changes from "when the clock really stops" to "when the game becomes paused by `PauseGame`/`TogglePause`"
  (identical with or without `?static=1`).

**Signal path.** `AppOptions { project, scene, run_mode }` and `enum RunMode { Live, Static }` (a resource; Replay/Lockstep can
follow without another signature break). `start_app(project, scene)` can stay as a thin wrapper over `start_app_with(AppOptions)`.
Web parses `?static=1` next to `?project=`/`?scene=`; native passes `Live`. **`?static=1` is honoured only together with
`testing=1`** (otherwise ignored with one console warning), so a shared link can never leave a player in a frozen-looking game;
when active, log one console info line. `RunMode` needs a reader: add `"run_mode"` to the `sync_debug_state_to_dom` JSON
(`lib.rs` ~896) and have `test_web.py` assert it, so a typo cannot silently produce unfrozen baselines.

**`test_web.py` scope.** Add `static_mode: bool = False` to `open_project()` and pass it only from the single-scene baseline path
(`test_screenshot_scene_baseline`). `pause_nav` and other multi-scene flows stay live (a start_menu to character_select to main
flow under a hold from boot would freeze the player at spawn and change its baselines for unrelated reasons). Keep
`SCREENSHOT_SETTLE_FRAMES = 120` (async GLB/texture arrival is still the main flake source, and freezing the sim does not make it
earlier); a readiness signal in the debug JSON (no pending spawns, warmup 0, no `awaiting_reveal`) is a later improvement.

**Projects where a frozen frame defeats the baseline.** A `STATIC_EXCLUDED` set next to `NON_DETERMINISTIC_SCENES` keeps these on
live capture: `particles_demo`, `effect_mayhem_demo` (empty emitters would check nothing about particle rendering),
`dynamic_animation_control` (its subject is seek/freeze). `custom_materials`/`foliage_demo` and water scenes should be
**included** (a shader clock at 0 is exactly what makes them stable) unless the spike shows otherwise. `/new-project` and
`docs/dev/browser_tests.md` say how to add a project to the set.

## Known behaviour differences while held (document in `docs/dev/browser_tests.md`)

Frozen: all `FixedUpdate`, particles (nothing emits), delayed events and timers (a splash scene's `EmitEventAfterDelay` never
fires, so its baseline is the first frame), despawn timers, dialogue timers, camera shake/blend, animation transitions (frozen
mid-fade), shader time. A `Follow` camera with `smoothing > 0` stays at its spawn position (`t = 1 - exp(0) = 0`). The 5 s
`REVEAL_FAILSAFE_SECS` never fires, so a model with a broken animation policy stays invisible. Rapier never steps (no
colliders for queries; the spike must confirm this logs no `error!`, because `open_project` fails on console errors). Still
running: UI, interpreter/executor, rendering, audio, button clicks (so `LoadScene` works), the debug frame counter.

## Tasks

- [ ] **Spike first** (about 30 min, `3rd_person_game_demo` + `custom_materials`): hold `Static` from boot (a temporary `start_app` tweak), screenshot twice. Record: pixel identity between runs (target 0.00%), console errors from the empty Rapier world, where the Follow camera and players sit, that nothing stays invisible (`awaiting_reveal`), and the particle scenes. If authored poses look wrong for some scene, the deterministic fallback is to step N ticks with `Time<Fixed>::accumulate_overstep` while held (one tick per frame), then re-freeze; defer until needed
- [ ] `PauseHold::announces` + announcement tracking in `apply_sim_clock_system`; `pause_tests` cases: Static from boot emits no `game.paused`; Static held then `PauseGame` emits `game.paused`; `ResumeGame` emits `game.resumed` and the clock stays paused; update `only_the_menu_hold_is_released_by_resume_and_by_a_replace_load` as needed
- [ ] `AppOptions` + `RunMode`; `start_app` seeds `SimClock` with `Static` when `RunMode::Static`; web parse (`static=1` requires `testing=1`); native passes `Live`
- [ ] `run_mode` in the debug-state JSON; `test_web.py`: `open_project(static_mode=...)`, `&static=1` for single-scene baselines only, assert `run_mode`, `STATIC_EXCLUDED`
- [ ] Re-capture the affected baselines in their own commit (`--real-gpu`, only the changed ones); gallery images show spawn-height poses
- [ ] Docs: `docs/dev/browser_tests.md` (flag, scope, exclusions, what stays frozen), the `game.paused` wording in `docs/30` and `docs/20`, a pause paragraph in `crates/ironhold_core/src/CLAUDE.md` ("`Static` does not announce"); `/new-project` mentions the exclusion set
- [ ] Verify: two consecutive `--real-gpu` runs diff 0.00% for each static baseline; all baselines pass the existing threshold

## Acceptance criteria

- With `?static=1&testing=1`, two consecutive `--real-gpu` runs on the same machine diff at **0.00%** for every included scene;
  every static baseline passes the 4% threshold.
- Entities are at their authored poses (frame 0 of each entity's initial clip) and authored transforms; the spike recorded no
  visible drift and nothing stays invisible.
- A `first_person_starter` static screenshot shows no pause overlay, and pause menus still open and close normally in live play.
- With Static held, `PauseGame` still emits `game.paused` and `ResumeGame` still emits `game.resumed`, and the clock stays held.
- `pause_nav` and other multi-scene flows pass unchanged; excluded projects still capture live.
- `?static=1` without `testing=1` is ignored with a console warning; native ignores static mode; no console errors in static runs.
- Without the parameter the engine behaves exactly as before (no new system, no change to Live mode behaviour).

## Open questions (resolved by the plan review unless noted)

- Options struct vs bool: `AppOptions`; run-mode enum vs bool: `RunMode` (architect, 2026-06 and 2026-10).
- NPC transform reset: dropped. Warmup particles at y=-100: off-screen, nothing frozen of interest.
- Open for the spike only: whether a held Rapier world logs errors; whether a `Follow` camera at its spawn position matches what
  players see closely enough to keep `camera_modes` in the set.

## Relation to networking determinism

Static mode and networking determinism are largely orthogonal: static mode *stops* the sim for a stable frame, determinism
requires the sim to *reproduce* identically across machines. The one shared seam is clock control, hence `RunMode`. The hard
blocker for lockstep networking is that Rapier3D is not cross-platform deterministic (native vs WASM diverge); state-replication
networking does not need full determinism. If networking becomes a concrete goal, open
`planning/investigations/rapier_cross_platform_determinism.md` first.
