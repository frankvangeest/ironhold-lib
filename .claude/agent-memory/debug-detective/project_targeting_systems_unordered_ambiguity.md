---
name: targeting-systems-unordered-ambiguity
description: TargetingPlugin's 3 PlayerTarget writers are registered unchained and unordered vs action_bar_input_system/message_interpreter_system — a real Bevy scheduling ambiguity that makes 2 entity_logic_tests flake ~15-20%
metadata:
  type: project
---

`TargetingPlugin` (`capabilities/targeting.rs`, `add_systems(Update, (click_select_system,
tab_targeting_system, target_auto_clear_system, debug_selectables_system))`) registers its three
`&mut PlayerTarget` writers as a **plain tuple — no `.chain()`, no `.before()`/`.after()` anywhere**,
including none relative to `action_bar_input_system` (which reads `&PlayerTarget`) or
`message_interpreter_system` (which reads `Res<CurrentTarget>`, also written by all three). They
conflict, so the executor serializes them — in an order that varies with thread availability.

**Diagnosing this class:** single-core process affinity is the decisive probe. A flaky test that
fails 40/40 when pinned to one core (`$p.ProcessorAffinity = 1` in PowerShell) but ~15% multi-core
is executor-ordering nondeterminism, not data-dependence — one core collapses the executor to
deterministic index order. Verified on `entity_logic_tests::test_both_players_bars_firing_same_
frame_neither_press_dropped`.

**Why it bites tests specifically:** `target_auto_clear_system` clears any `PlayerTarget` whose id
is absent from `SpawnRegistry` (`registry.entities.get(&id)` → `None` ⇒ clear). A test that spawns
`PlayerTarget(Some("enemy_a"))` **without** `spawn_registry.entities.insert("enemy_a", e)` is racing
that clear every single frame. The discriminator is exact: in `entity_logic_tests.rs` the two tests
with unregistered ids flake 6/40 and 8/40; the sibling that registers its target is 0/40. Every
`action_tests.rs` use is paired with a registry insert, so that file is immune.

**Why the symptom is `Some("")` and not "the slot didn't fire":** `action_needs_target`
(`action_bar.rs`) has no `Action::SetVariable` arm, so the `no_target` gate never engages for it and
`target_id = player_target.0.as_deref().unwrap_or("")` writes an empty string silently.

**Note the intent conflict before "fixing" by ordering:** `target_auto_clear_system`'s own doc
comment says it exists to "prevent the action bar from firing at invisible/nonexistent enemies" —
i.e. the *documented* intended order is auto-clear **first**, which is exactly the order that makes
these two tests fail. Declaring the ordering therefore requires fixing the tests (register the ids)
in the same change, or they go from flaky to deterministically red.

Related: [[local-coop-tests-flaky-targeting]] is a *different* targeting flake (equidistant
tie-break), not this one.
