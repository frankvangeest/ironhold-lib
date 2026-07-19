---
name: idempotent-despawn-untestable-by-state
description: Double-despawn bugs are invisible to ECS-state assertions; only log capture detects them — a regression test asserting entity count/no-panic passes with the bug still present
metadata:
  type: project
---

A "same entity queued for despawn twice in one frame" bug (e.g. [[project_target_indicator_double_despawn]] in `target_indicator_system`) produces **no observable ECS-state difference**. Bevy's deferred `commands.entity(e).despawn()` is idempotent: the second despawn of an already-removed entity only emits a `warn!` ("Entity ... is invalid" / "generation N"), it does not panic and does not change final entity count/components.

**Why:** the only symptom is the log line. A regression test whose assertions are limited to `app.update()` not panicking + final ring/entity count + surviving owner will pass **identically whether or not the dedup fix is present** — it exercises the code path but cannot detect reintroduction of the bug.

**How to apply:** when reviewing or writing a regression test for a double-despawn / double-command bug, insist the test capture logs (e.g. a tracing layer / `LogPlugin` capture, or a custom counter) and assert the warning is NOT emitted — OR restructure so the duplicate command has an observable effect. If neither is feasible, label the test honestly as a scenario smoke-test, not a guard against reintroduction. `test_target_indicator_ring_not_double_despawned_when_target_dies_and_owner_retargets_same_frame` (action_tests.rs) is the latter: correct scenario, inert as a regression guard.

Related same-class sites that only misfire across multiple ActionQueue actions in one frame (lower severity, backlog-worthy): `action_executor.rs` UnloadOverlay+ToggleOverlay, duplicate `Action::Despawn(same_id)`, StopMusic+PlayMusicLoop — each despawns from a query snapshot that a sibling action already emptied.
