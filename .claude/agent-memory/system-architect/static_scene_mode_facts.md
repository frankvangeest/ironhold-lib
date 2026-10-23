---
name: static_scene_mode_facts
description: Verified facts from static_scene_mode.md plan review (e2a5578, 2026-10-23) — freeze-at-Ready vs hold-from-boot, globals.time trap, game.paused contract, what a frozen clock leaves un-frozen
metadata:
  type: project
---

Plan review of `planning/features/static_scene_mode.md` at `e2a5578` (2026-10-23), verdict Needs more design work.

- **`SceneEvent::Ready` is written in the SAME frame as `spawn_scene_v2`'s spawn commands** (scene_loader.rs ~1353, non-overlay only). RunFixedMainLoop precedes Update, so a hold taken that frame (before `apply_sim_clock_system`) means the scene gets **zero** fixed ticks: bodies sit at authored transforms, never "mid-fall". A hold taken one frame late gives a variable 0/1/2 ticks = nondeterminism.
- **Bevy shader `globals.time` = `Res<Time>` (Virtual) `elapsed_secs_wrapped`** (bevy_render 0.18 globals.rs:36/72). `custom_pulse.wgsl` and `custom_water_stylized.wgsl` use it. Freezing at Ready freezes it at the *load duration* (varies per run) → not pixel-identical. Holding from app build (SimClock pre-seeded with `Static`) freezes it at exactly 0 (first Real update has zero delta).
- Hold-from-boot needs no new system (insert `SimClock` with the hold in `start_app`), so no `schedule_order_tests` impact. No load-path system reads virtual time or needs FixedUpdate.
- `apply_sim_clock_system` emits `game.paused`/`resumed` on the Time<Virtual> transition, and early-returns if Time is already paused — so with Static held, a Menu pause would emit NOTHING (first_person_starter's pause menu opens on `game.paused`). Fix = emit on the designer-visible-hold transition (Menu; later FocusLoss/Session), tracked separately from the clock. Designer docs (docs/30 line ~123, docs/20 PauseGame row) say "fires when the clock really stops" → contract wording changes.
- Frozen-clock leftovers: Follow camera with smoothing>0 never moves (lerp t = 0); `REVEAL_FAILSAFE_SECS` (animation.rs, virtual time) never fires → broken-policy models stay hidden; particle emitters emit nothing (particles_demo/effect_mayhem baselines go empty); transitions freeze mid-fade; dialogue/typewriter, delayed events, despawn timers never fire.
- The plan's "seek all AnimationPlayers to t=0" step would clobber durable `ActiveOverride.seek_fraction`/`frozen` (corpse death pose). Unneeded: with delta 0 every clip starts and stays at t=0.
- Deterministic settle primitive if ever needed: while held, `Time<Fixed>::accumulate_overstep(period)` gives exactly one fixed tick/frame at dt=1/64 (see [[fixed_tick_pipeline_facts]]).

**Post-implementation review (2026-10-23, feature/static_scene_mode, 4d4983e):** shipped as planned. `apply_sim_clock_system` now has two independent edges: clock (vs `Time<Virtual>::is_paused`) and announcement (`Local<bool>` vs `SimClock::announces_pause`, i.e. Menu only). `SimClock::with_hold(Static)` is inserted in `start_app_with` BEFORE `GamePlugin`, whose `init_resource` is then a no-op; the `resource_changed` condition is true on a system's first run (Bevy initializes last_run far in the past), so frame 1 pauses. `?static=1` parse lives in `ironhold_web` (gated on `testing=1`); `RunMode` is a core resource read only by the debug JSON. Gap found: `ironhold_web` calls `web_sys::console::warn_1` without declaring web-sys feature `"console"` (compiles only via feature unification from a dependency).

Related: [[real_pause_review_facts]], [[time_and_animation_advance]], [[animation_seek_freeze_constraints]], [[demo_project_baseline_determinism]].
