---
name: max-fixed-delta-secs
description: ProjectConfig.max_frame_delta_secs (renamed from max_fixed_delta_secs) — slow-motion floor is NOW validated (2 ticks..60s); doc surfaces it lives on
metadata:
  type: project
---

Field was renamed `max_fixed_delta_secs` -> `max_frame_delta_secs` (verified 2026-10-07). It caps
`Time<Virtual>::max_delta` for the whole clock (Update too, not just FixedUpdate). Default 0.25s.

**Old footgun now closed:** validate (`invalid_project_config`) rejects values outside
`MIN_MAX_FRAME_DELTA_SECS` (= 2/64 s, ~0.03) .. `MAX_MAX_FRAME_DELTA_SECS` (60 s), `schema/project.rs`
~267-273. Values below one real frame interval used to give permanent silent slow-motion.

**Why:** the measurement behind the knob lives only in `planning/investigations/fixed_timestep_max_delta.md`,
which docs/20 ~4716 still cites (designer-unreachable path, see [[project_ron_comments_cite_dev_paths]]).

**How to apply:** if the field or its bounds change, check docs/20 field table (~116), docs/20
`## max_frame_delta_secs` (~4693), docs/15 "Players, items and dialogue" table (`invalid_project_config` row),
docs/dev/70 and docs/dev/40 mentions. Older memory names (`max_fixed_delta_secs`) in other agents' memory are stale.
