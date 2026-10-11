---
name: f32-clamp-panics-on-authored-bounds
description: f32::clamp(min,max) with RON-authored bounds is a release-mode panic, because validate() errors are non-fatal and clamp asserts min<=max (and non-NaN) unconditionally
metadata:
  type: project
---

`f32::clamp` uses an unconditional `assert!(min <= max)` (fails for NaN bounds too), not a debug_assert. So
any runtime `x.clamp(cfg.min, cfg.max)` on designer-authored bounds panics on WASM and native whenever the
config is invalid. A `validate()` that rejects `min > max` does NOT protect it: project_loader only `error!`s
and keeps going (see [[schema-validate-errors-are-nonfatal]]), and a per-frame system reading
`Assets<ProjectConfig>` directly may even run before that error is logged.

Found 2026-10-11 in `feature/responsive_ui` Phase 2 (`compute_ui_scale`, ui_scale.rs): `ui_scale: (reference:
(1280.0, 720.0), min_scale: 3.0)` (default max_scale 2.0) panics every frame.

**Why:** a tidy-looking validate + clamp pair reads as safe in review; the panic only shows on an invalid file.
**How to apply:** in any diff, grep new `.clamp(` calls whose bounds come from schema fields; require
`x.max(min).min(max)` (never panics; NaN bounds are ignored) or a runtime guard. Also check the
"floored/limited" diagnostic compares the RAW value against the bound, not a snapped/rounded one.
