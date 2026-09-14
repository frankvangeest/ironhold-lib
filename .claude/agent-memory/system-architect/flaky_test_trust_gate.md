---
name: flaky-test-trust-gate
description: Stakeholder item #4 (test-suite trust) — the TargetingPlugin race fix closed the one known flake but also removed its canary; ambiguity detection is still unlogged
metadata:
  type: project
---

The only recurring flake in `ironhold_core`'s suite was the `TargetingPlugin` scheduling race (`entity_logic_tests.rs`, `local_coop_tests.rs`; 3 confirmed manifestations, ~15% rate under default parallelism, 100% under single-core affinity). `feature/targeting_race_fix` (2026-09-14) closed it by `.chain()`ing the three targeting systems and adding `.before(action_bar_input_system)`, plus registering the test fixtures' target entities in `SpawnRegistry`.

**Why this matters beyond the one bug:** this is the first half of my stakeholder item #4 (see [[stakeholder-wishlist-tracking]]). "Trust in the merge gate" means a red suite is always a real regression — one tolerated flake trains everyone to re-run instead of investigate, which is exactly what happened for three sessions.

**How to apply:**
- **The fix removed the canary.** Registering the fixtures in `SpawnRegistry` makes those two tests pass *regardless* of system order, so nothing in the suite now fails if a future refactor drops the `.chain()`/`.before()`. If a new ordering-sensitive bug appears in targeting, don't assume the suite would have caught it.
- **Two guards are still unbuilt and worth proposing:** (1) a schedule-graph assertion test (`app.get_schedule(Update).unwrap().graph()`) that asserts the targeting→action_bar edge exists — deterministic, unlike a behavioral test which would just be flaky again; (2) Bevy `ScheduleBuildSettings { ambiguity_detection: LogLevel::Warn }` on `Update` in debug/test builds. The backlog entry for the race says the ambiguity-detection hardening was "logged separately" — **it never was** (verified 2026-09-14: no `ambiguity` item exists in `planning/backlog.md` or `claude_suggestions.md`). Re-raise it rather than assuming it's tracked.
- Item #4's *second* half — no "assert a warning was logged" infrastructure — remains entirely open and is the reason a lot of `warn!`-only fixes ship untested.
