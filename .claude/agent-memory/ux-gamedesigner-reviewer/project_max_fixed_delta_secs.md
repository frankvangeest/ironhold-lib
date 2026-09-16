---
name: max-fixed-delta-secs
description: ProjectConfig.max_fixed_delta_secs — the "too small = permanent slow-motion" floor that validate does not check, and the doc surfaces it landed on
metadata:
  type: project
---

`ProjectConfig.max_fixed_delta_secs` (`Option<f32>`, added on `feature/fixed_timestep_max_delta`,
reviewed 2026-09-16) caps Bevy's `Time<Virtual>::max_delta` — how much simulated time one rendered
frame's `FixedUpdate` catch-up may cover. Default 0.25s ≈ 16 ticks at the engine's 64Hz
`FIXED_TICK_RATE`.

**The load-bearing footgun:** `max_delta` clamps *every* frame's virtual advance, not only
post-stall recovery frames. Any value at or below the real frame interval makes the game run
permanently slower than real time even at a perfect framerate (0.0156 = 1/64 already yields ~93%
speed on a 60Hz display; 0.01 yields ~60%). `ProjectConfig::validate()` only rejects `<= 0.0` and
non-finite, so every value in the permanently-slow band validates clean with no warning anywhere.
Practical floor to recommend: >= 0.033 (one 30fps frame), realistically >= 0.05.

**Why:** the field was added as a perf-safety knob from
`planning/investigations/fixed_timestep_max_delta.md`; the measurement and any safe range live only
in `planning/`, which designers cannot read.

**How to apply:** when this field next changes (a validate warn band, a rename, an example), check
all of these surfaces — as of the review only the first three existed:
- `crates/ironhold_core/src/schema/project.rs` (field doc comment — developer-only, designers never see it)
- `docs/20_data_formats.md` ProjectConfig field table (~line 118)
- `docs/60_contributing.md` "Checks performed" (`invalid_project_config`, ~line 263)
- MISSING: both `ProjectConfig` example RON blocks in docs/20 (~lines 122-157) omit it
- MISSING: no shipped `assets/projects/*/*.project.ron` sets it — only the CLI fixture
  `crates/ironhold_cli/tests/fixtures/bad_max_fixed_delta_secs/`
- MISSING: `docs/70_profiling.md` (the only designer-facing perf doc) and `docs/STATUS.md` never
  mention it; `docs/40_determinism_and_networking.md` covers the 64Hz tick but not this knob

Related: [[project_ron_comments_cite_dev_paths]], [[project_validate_coverage_gaps]],
[[project_warn_vs_silent_fallback_principle]].
