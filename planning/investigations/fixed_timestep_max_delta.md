# Investigation: `Time<Virtual>::max_delta` catch-up burst — does it actually spiral?

See `planning/backlog.md` ▸ Active for the tracked item this investigation is scoped to
(promoted from `deterministic_fixed_timestep` v1's review, 2026-09-16). Branch:
`feature/fixed_timestep_max_delta`, branched from `integration` (not `main`) since it depends on
that feature's `TimestepMode::Fixed`/`FixedUpdate` physics work, not yet promoted to `main`.

## Symptoms / original hypothesis

`deterministic_fixed_timestep` v1 moved Rapier physics into `FixedUpdate`. Bevy's `Time<Virtual>`
default `max_delta` (250ms) means a real stall can trigger up to `250ms / 15.625ms ≈ 16`
`FixedUpdate` ticks (full gameplay chain + physics step each) in a single frame — on WASM,
already prone to shader-pipeline-compile stalls (`pipeline_warmup_system` exists for exactly
this). The original review framed this as a possible **"spiral of death"**: the frame recovering
from a stall gets *longer*, not shorter, potentially cascading.

## Method

Headless Playwright has no GPU adapter in this environment (confirmed: `test_web.py` itself also
times out headless here — pre-existing, see `planning/investigations/headless_webgpu_testing.md`,
unrelated to this investigation). Switched to **headed** Chromium (`headless=False`,
`--enable-unsafe-webgpu`, real GPU) — this worked cleanly with zero WebGPU device-creation errors,
which updates that older investigation's unresolved "does headed rendering actually work" question
(see note added there) — worth someone eventually reconciling why this session's Chromium didn't
need the `--use-webgpu-adapter=d3d11` workaround that investigation found necessary; not pursued
further here since it wasn't blocking.

Temporary instrumentation added to `lib.rs` (`FixedTickCounter` resource + `DebugState.
fixed_ticks_last_frame`/`max_fixed_ticks_observed`, exposed via the existing `#debug-state` DOM
element): counts `FixedUpdate` ticks per rendered frame, plus a running session-max. **Not part of
the final fix** — needs stripping (or deciding to keep as a permanent diagnostic) once this
investigation concludes.

Three experiments, `3rd_person_game_demo` (dev WASM build):
1. **Cold page load, no artificial stress** — just observe natural behavior.
2. **Synchronous main-thread busy-wait** (`while (performance.now() - start < 300) {}` via
   `page.evaluate`) to simulate a discrete ~300ms stall.
3. **Sustained CDP CPU throttling** (`Emulation.setCPUThrottlingRate(12)`) during actual gameplay
   (character-select scene, not just the start menu) — simulates a genuinely overloaded/slow
   machine rather than one instantaneous blip.

## Findings

1. **16-tick bursts are not a rare edge case — they happen on every page load.** Cold-load
   experiment: `max_fixed_ticks_observed` hit **16** (the theoretical max) by frame 13, before any
   artificial stress was applied — WASM instantiation + initial asset loading alone produces
   enough real elapsed time before the game loop starts ticking. The very next frames settled to
   1 tick/frame with no further disruption — **the one-time recovery was clean, no visible
   cascading hitch** on this (capable) dev machine.

2. **The synchronous busy-wait experiment was inconclusive** — post-stall samples showed only
   1-2 ticks, not the ~19 the injected 300ms delay would predict. Not fully root-caused (candidate
   explanation: `requestAnimationFrame`'s callback timestamp may reflect the *scheduled* vsync
   time rather than actual JS-resume time, making a `page.evaluate`-based main-thread block a poor
   proxy for a real stall) — **do not read anything into this experiment's numbers**; superseded
   by experiment 3, which used a more realistic sustained-load mechanism instead.

3. **Under sustained heavy load (12x CPU throttle), the game ran a full 16-tick burst on nearly
   every single frame, continuously, for the entire ~6s throttled window** (`fixed_ticks_last_frame`
   pinned at or near 16 across ~60 consecutive samples). This is the key finding: **frame times
   did not spiral upward** — they stayed roughly proportional to the throttle factor rather than
   compounding. `max_delta`'s clamp is already doing exactly what it's designed to do: instead of
   trying to fully catch up (which would make each frame progressively longer — the literal
   spiral the original review worried about), the game settles into **sustained slow-motion**
   ("bullet time") — it falls further behind real time each frame, silently, rather than the
   *recovery cost itself* compounding. This is Bevy's existing 250ms default already behaving
   correctly, not a gap this investigation needs to close.

## Corrected understanding

The original "spiral of death" framing (frame-time cascading unboundedly) **does not manifest empirically** — `max_delta`'s clamp already prevents that specific failure mode, at the current
default. What actually happens under sustained overload is graceful (if unglamorous) degradation
to slow-motion, which is the intended purpose of capping `max_delta` at all.

The real, narrower open question is a **pure gameplay-feel tradeoff**, not a correctness bug:
- **Leave the default (250ms / ~16 ticks)**: absorbs bigger stalls before visibly slowing down,
  but the worst-case single-frame catch-up cost is larger (16 ticks' worth of gameplay+physics
  work in one frame) — on a weak/overloaded machine this could still be the more *perceptible*
  hitch, even though it isn't a runaway spiral.
- **Tighten the cap (e.g. ~100ms / ~6 ticks)**: bounds the worst-case per-frame cost tighter, at
  the price of entering visible slow-motion sooner/more readily for smaller stalls.

Neither option is a "fix" for a bug — this is Frank's call to make, informed by the above, not
something to resolve unilaterally.

## Next steps

- Decide the tradeoff above with Frank.
- Strip or keep the temporary `FixedTickCounter`/`DebugState` instrumentation depending on the
  outcome — it's cheap and could be a genuinely useful permanent perf diagnostic (surfaces via the
  existing `#debug-state` DOM element with zero added UI), but wasn't scoped as part of the
  original ask; get an explicit call on this too rather than assuming either way.
- If a cap is chosen: implement, route through `wasm-perf-reviewer` (per-frame hot-path change,
  per the original triage's instruction), full review/test/playtest cycle per the standard
  workflow.

## Decision (2026-09-16)

Frank: don't hardcode either tradeoff option — expose it as a per-project RON setting instead
(`ProjectConfig.max_frame_delta_secs`, `schema/project.rs`), and leave the shipped default
untouched at Bevy's own 250ms/~16-tick default when the field is omitted. Verified this is safe to
make RON-authorable, unlike `FIXED_TICK_RATE` itself: it has no interaction with any tick-derived
gameplay constant (`jump_air_grace_ticks()`, `coyote_ticks()`) since `dt` per tick never changes —
only how many ticks a single frame may run to catch up — so per-project values cannot desync
gameplay feel the way a per-project tick *rate* would.

The temporary `FixedTickCounter`/`DebugState` instrumentation was stripped entirely (not kept as a
permanent diagnostic) — it was scoped only to this investigation's measurement, and `lib.rs` is
back to its pre-investigation state.

Implementation went straight to code (no separate feature-plan file — a single optional RON field
with no cross-capability surface) through the standard review cycle (`alignment-reviewer`, `system-architect`,
`debug-detective`, `ux-gamedesigner-reviewer`, `wasm-perf-reviewer`), which converged on two real
panic bugs in the naive `secs > 0.0 && secs.is_finite()` guard (`Duration::from_secs_f32` overflows
above ~1.8e19s, and separately rounds anything below ~5e-10s to `Duration::ZERO`, which Bevy's own
`set_max_delta` refuses) plus a missing floor: any positive value below ~2 fixed-tick periods
passes a naive check but runs the whole game in permanent, silent slow-motion. Both are now hard
`ProjectConfig::validate()` errors (`MIN_MAX_FRAME_DELTA_SECS`/`MAX_MAX_FRAME_DELTA_SECS` in
`schema/project.rs`), and the field was renamed `max_fixed_delta_secs` → `max_frame_delta_secs`
before any shipped project could reference it, since it clamps the whole virtual clock
(`Update`-schedule delta too), not just `FixedUpdate`.
