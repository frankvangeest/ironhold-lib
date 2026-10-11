---
name: runtime-validate-is-advisory
description: ProjectConfig/catalog validate() at runtime only error!s and continues; runtime consumers must be total over invalid config (f32::clamp panics on min>max/NaN)
metadata:
  type: project
---

`project_loader.rs` calls `ProjectConfig::validate()` (and catalog validates) once at project load and only
`error!`s on failure; the invalid config stays in `Assets<ProjectConfig>` and every system keeps reading it. It is
also not re-run on a hot-reload. So "validate() rejects X" protects only `ironhold validate` users, never the runtime.

**Why:** found 2026-10-11 reviewing responsive_ui Phase 2: `compute_ui_scale` used `needed.clamp(min_scale,
max_scale)`; `f32::clamp` panics if min > max or either is NaN, so a `min_scale > max_scale` project (which validate
"rejects") would panic every frame at runtime (WASM = dead tab).

**How to apply:** when reviewing any code that consumes a validated schema field per-frame, check it is total over
the invalid values validate() names (use `.max(a).min(b)` not `clamp`, guard divisions, or fall back to the
"off" value when `cfg.validate().is_err()`). Related: [[schema-field-defaults-ssot]] (Duration ctor panics),
[[cli-runtime-mirror-check-pairs]].
