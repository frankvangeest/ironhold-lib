---
name: targeting-systems-unordered-ambiguity
description: TargetingPlugin's 3 PlayerTarget writers are now .chain()ed .before(action_bar_input_system) (feature/targeting_race_fix) — but target_indicator_system/target_hud_update_system still read PlayerTarget unordered, and click_select_system lacks the With<CharacterController> filter the other two have
metadata:
  type: project
---

**Status: the core ambiguity is FIXED** on `feature/targeting_race_fix` — `TargetingPlugin`
(`capabilities/targeting.rs`) now registers `(click_select_system, tab_targeting_system,
target_auto_clear_system).chain().before(action_bar_input_system)`. `debug_selectables_system`
stays a sibling (read-only gizmos, deliberately unordered). No cycle: the resulting frame order is
targeting-chain → `cooldown_tick`/`action_bar_input`/`action_bar_visual` (their own chain) →
`message_interpreter` → `action_executor` → `drain_spawn_queue`.

**Diagnosing this class:** single-core process affinity is the decisive probe. A flaky test that
fails 40/40 when pinned to one core (`$p.ProcessorAffinity = 1` in PowerShell) but ~15% multi-core
is executor-ordering nondeterminism, not data-dependence — one core collapses the executor to
deterministic index order.

**Why it bit tests:** `target_auto_clear_system` clears any `PlayerTarget` whose id is absent from
`SpawnRegistry` (`registry.entities.get(&id)` → `None` ⇒ clear). A fixture that sets
`PlayerTarget(Some("enemy_a"))` or spawns a `Targetable` **without**
`SpawnRegistry.entities.insert(...)` is now cleared 100% of the time, not ~15%. Fixed fixtures:
`entity_logic_tests.rs`'s two flaky tests, and `local_coop_tests.rs`'s new `spawn_targetable_at`
helper (bundle + registry insert; the bare `test_targetable_at` bundle still exists and still
does NOT register).

**The accidental safety net that keeps the 4 click-select tests green:** `click_select_system`'s
player query is `Query<(Entity, &mut PlayerTarget, Option<&PlayerIndex>)>` with **no**
`With<CharacterController>` filter, while `tab_targeting_system` and `target_auto_clear_system`
both filter on it. Those tests spawn players as bare `PlayerTarget::default()` (no controller), so
auto-clear can't see them and their unregistered `ClickSelectable` entities survive. Adding a
`CharacterController` to any of those fixtures breaks all four deterministically. The same
asymmetry is a live production inconsistency: a controller-less player can be click-targeted but
can never be tab-cycled or auto-cleared.

**Still unordered after the fix** (the new `crates/ironhold_core/src/CLAUDE.md` rule "order any
fourth PlayerTarget reader explicitly" is already violated by existing code): `target_indicator_
system` (`TargetIndicatorPlugin`, bare `add_systems(Update, ...)`, uses `Changed<PlayerTarget>`)
and `target_hud_update_system` (inside lib.rs's big camera `.chain()`). Change-detection ticks
aren't lost, so the effect is a nondeterministic one-frame ring/HUD latency, not a dropped update.

**New deterministic loss the ordering introduces:** `target_auto_clear_system` is now
transitively guaranteed to run *before* `drain_spawn_queue_system` (`SPAWNS_PER_FRAME = 2`). A
rule that queues 3+ `Action::Spawn`s and `SetTarget`s one that hasn't drained yet used to keep the
target ~50% of runs (whenever auto-clear happened to run after the drain); now it is cleared every
time. Auto-clear has no "queued but not yet spawned" grace.

**Why the old symptom was `Some("")`:** `action_needs_target` (`action_bar.rs`) has no
`Action::SetVariable` arm, so the `no_target` gate never engaged and
`target_id = player_target.0.as_deref().unwrap_or("")` wrote an empty string silently.

Related: [[local-coop-tests-flaky-targeting]] is a *different* targeting flake (equidistant
tie-break), not this one. [[target-indicator-double-despawn]] is the same system's other hazard.
