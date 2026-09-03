# Feature: Static Scene Mode

_Status: Queued_
_Planned at: `9ff431b` (2026-06-18)_

## What

A harness mode that loads a scene with all time-driven systems frozen: animations stay in their
initial pose (seek to t=0), NPC AI does not tick, motion (rotate/bob) is stopped, and particles
do not emit. Activated via a `?static=1` URL query parameter — invisible to normal players,
opt-in for automated tooling.

## Why

The browser screenshot test (`test_web.py`) takes baseline images of each scene. Characters are
mid-animation and NPCs have drifted from their spawn positions by the time the screenshot fires,
causing flaky baselines that differ between runs and machines. Static mode gives a fully
deterministic, pose-consistent freeze that makes baselines stable.

## Approach

**Signal: URL param → Bevy Resource**

`crates/ironhold_web/src/lib.rs` already parses `?project=` from `window.location`. Add `?static=1`
parsing in the same block, pass the flag into `start_app` as a new argument (or an options struct),
and insert a `StaticMode(bool)` resource before the app runs.

`ironhold_native` passes `StaticMode(false)` unconditionally (desktop never freezes).

**Mechanism: pause `Time<Virtual>`**

`time.pause()` is the primary lever. Bevy's own `AnimationPlugin` drives `AnimationPlayer` off the
virtual clock, so one pause call freezes all animations, NPC tick deltas, motion integration, and
particle emission simultaneously. No per-system surgery needed for the majority of systems.

**Freeze point: `SceneEvent::Ready` (after warmup)**

Insert a system in `ironhold_core` that runs when `StaticMode(true)` and observes
`SceneEvent::Ready`. On that event:
1. Call `time.pause()` to halt the virtual clock.
2. Seek all `AnimationPlayer` components to `t = 0.0` — this gives a canonical first-frame pose
   rather than whatever frame they happened to land on during async load.
3. Optionally reset NPC transforms to their spawn-point translations (guards against async-load
   drift before the clock was paused).

Warmup `SpawnEffect` calls in `scene.ready` rules still fire (they are action-executor driven, not
clock-driven) but particles freeze immediately after spawning because their lifetime integration
reads `Time`.

**`start_app` signature change**

The signature change touches all three crates in one commit:
- `ironhold_core::start_app(project: Option<String>, static_mode: bool)` — or wrap into an options struct if more flags are anticipated.
- `ironhold_native` passes `false`.
- `ironhold_web` parses the URL and passes the bool.

**test_web.py**

Append `&static=1` to the URL when navigating to a scene for screenshot purposes. The existing
`SCREENSHOT_SETTLE_FRAMES` wait can be reduced or removed for static sessions since there is
nothing to settle.

## Tasks

- [ ] Parse `?static=1` in `crates/ironhold_web/src/lib.rs`
- [ ] Update `start_app` signature (options struct preferred over growing positional args); update `ironhold_native` and `ironhold_web` call sites
- [ ] Insert `StaticMode(bool)` resource in `ironhold_core::start_app`
- [ ] Add freeze system: on `SceneEvent::Ready` + `StaticMode(true)` → pause virtual clock + seek all `AnimationPlayer`s to 0.0
- [ ] Append `&static=1` to screenshot URLs in `test_web.py`; optionally reduce `SCREENSHOT_SETTLE_FRAMES`
- [ ] Verify all baseline tests pass consistently across two consecutive runs with no diff
- [ ] Docs: note `?static=1` in `docs/browser_tests.md`

## Open questions

- **Options struct vs extra arg**: if more harness flags arrive (e.g. `?headless=1`, `?fixed_seed=42`),
  an `AppOptions` struct scales better than positional booleans. Worth doing now?
- **Run-mode enum vs bare bool**: model the run mode as `enum RunMode { Live, Static }` (and later
  `Replay`, `Lockstep`) rather than a bare `StaticMode(bool)`. Costs nothing extra now, avoids a second
  cross-crate signature break when networking arrives. Architect-recommended.
- **NPC transform reset**: is seeking the clock to 0 enough, or do NPCs drift during the async-load
  window before `SceneEvent::Ready`? Needs a test run to confirm.
- **Particle warmup effects**: the `SpawnEffect` calls on `scene.ready` fire after freeze.
  Do frozen particles at y=-100 cause any visual artefact in the screenshot? Probably not (off-screen),
  but worth checking.

## Relation to networking determinism

Static mode and networking determinism are largely orthogonal. Static mode *stops* the sim for a
stable frame; determinism requires the sim to *reproduce* identically across machines. The one
shared seam is clock control — hence the run-mode enum recommendation above.

The hard blocker for lockstep networking is that **Rapier3D is not cross-platform deterministic**
(native vs WASM diverge). State-replication / server-authoritative networking doesn't require full
determinism and is the realistic first target. If networking becomes a concrete goal, open
`planning/investigations/rapier_cross_platform_determinism.md` before committing to an approach.

## Acceptance criteria

- Given `?static=1` in the URL, when `main.scene.ron` loads in `3rd_person_game_demo`, then the
  player character is in its rest pose (t=0) and all NPCs are at their scene-defined spawn positions.
- Given two consecutive screenshot runs with `?static=1`, the resulting PNGs are pixel-identical.
- Given normal play (no `?static` param), the engine behaves exactly as before — no regression.
