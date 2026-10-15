---
name: schedule-test-positional-vs-edge
description: D3 schedule_order_tests positional asserts (toposort index) don't prove an edge exists; only conflicting_systems() is edge-sensitive. Plus the cross-chain drag from target_hud_update.after(target_auto_clear)
metadata:
  type: project
---

Order asserts that compare positions in `schedule.systems()` (the single-threaded toposort) show where a system landed in
the order. They do not show that an edge exists: if the edge is removed, the toposort can still put the pair in the same
order by chance. Only `graph().conflicting_systems()` changes when an edge is removed, and only for pairs that share data.
In D3 (feature/pipeline_system_sets, reviewed 2026-10-14) the conflict filter covered just Messages<Ui/Game/SceneEvent>,
ActionQueue and SimClock. That left two guarantees protected only by positional asserts: Scene -> Ui (LoadedKeyBindings /
LoadedGamepadBindings) and Targeting -> ActionBar (PlayerTarget/CurrentTarget).

**Why:** the review found these two edges could be removed without any D3 test failing.

**How to apply:** when reviewing a schedule test, look for each declared edge's shared resource in the conflict filter.
If it is missing, the edge is only "tested" by position. Also: a by-name cross-chain edge into a set member pulls that
set's upstream syncs into the other chain. `target_hud_update_system.after(target_auto_clear_system)` makes the
camera-chain tail wait for the EmitSet::Scene sync while the chain head stays unconstrained, so on a load frame the
chain can straddle the teardown flush. Related: [[project_bevy_auto_sync_distance_semantics]].
