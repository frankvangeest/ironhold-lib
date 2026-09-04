---
name: local-coop-tests-flaky-targeting
description: local_coop_tests fails a different targeting test on roughly 2 of 3 runs (equidistant enemy_a/enemy_b tie-break); passes in isolation — not a regression signal
metadata:
  type: project
---

`cargo test -p ironhold_core --test local_coop_tests` is **flaky**: observed 2026-09-04 on
`feature/ui_trigger_reachability_check` failing `test_tab_targeting_each_player_cycles_independently`
on one run, `test_legacy_target_vars_populate_when_single_player` on another, and passing clean on
a third. Every failing test passes when run alone by name.

Likely mechanism (not yet root-caused): the helper spawns `test_targetable_at("enemy_a", (2,0,0))`
and `("enemy_b", (-2,0,0))` — **exactly equidistant** from the player at the origin. Nearest-target
selection has no stable tie-break, so Bevy query/archetype iteration order decides which one wins,
and the cycle order flips run to run. The test file has no statics/env mutation, so it is not
cross-test global state.

**Why:** this failure shows up in step-4/step-11 full-suite runs and looks like a regression from
whatever branch is under review. It is not.

**How to apply:** on a full-suite failure in `local_coop_tests`, re-run the single test by name and
re-run the binary 2-3 times before attributing it to the branch. Check whether the branch touches
`ironhold_core` at all first. A real fix would be a deterministic tie-break (spawn id / entity
index) in target selection, or moving the fixture enemies to unequal distances.
