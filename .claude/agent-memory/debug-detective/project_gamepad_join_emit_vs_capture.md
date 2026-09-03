---
name: gamepad-join-emit-vs-capture
description: unclaimed_gamepad_trigger_system emits one UiEvent per (pad, binding) match but captures only one pad into PendingJoinGamepad — "at most one per frame" is only true for the capture, not the join
metadata:
  type: project
---

`runtime/input.rs::unclaimed_gamepad_trigger_system` has **two independent per-frame budgets**
that are easy to conflate: it `write`s a `UiEvent::ButtonPressed(trigger)` for *every*
(unclaimed pad × bound button) match, but only captures the **first** pad into
`PendingJoinGamepad` (guarded by a local `captured` bool). Since `message_interpreter_system`
does no dedup (`match_rules` is called once per UiEvent message), N simultaneous pad presses on
a `"join"` binding produce N `Action::JoinPlayer` actions in one executor pass — only the first
gets a `gamepad_index`; the rest fall back to whatever the join prefab authored. The executor's
`.take()` correctly stops pad *reuse* but cannot stop the extra join.

**Why:** the plan (`planning/features/gamepad_hot_join.md`) and `docs/20_data_formats.md` both
state "only the lower one joins that frame" — that is a claim about the *emission* budget, which
the code does not implement. Hot-leave is out of scope, so a spurious join permanently burns a
co-op slot.

**How to apply:** when reviewing anything that pairs a broadcast message with a single-slot
side-channel resource, check that the *message* count is capped too, not just the resource write.
Tests that assert only the side-channel resource (e.g.
`test_two_gamepads_pressed_same_frame_captures_only_lowest_sorted_index`) pass while the
user-visible outcome is wrong — assert the outcome (player count / emitted message count), not the
carrier. The shared-`gamepad_index` double-fire hazard this used to link to
(`project_gamepad_index_routing.md`) is now closed: the positional `resolve_gamepad` lookup was
removed and replaced by `BoundGamepad`/`gamepad_bind_system`'s `claimed: HashSet<Entity>`
invariant (see `crates/ironhold_core/src/CLAUDE.md`'s "Gamepad routing" section), and that memory
file was deleted.
