---
name: delayed-events-uncancellable
description: EmitEventAfterDelay entries live in a flat DelayedEventQueue with no cancel action, so an FSM state change never disarms timers the old state armed — stale fires match by event name only
metadata:
  type: project
---

`DelayedEventQueue(pub Vec<(f32, String)>)` (`runtime/scene_manager/mod.rs`) is a flat list ticked by
`tick_delayed_events_system` (`lib.rs`) with `retain_mut`. There is **no** `CancelDelayedEvent` action
and no owner/state tag on an entry — once armed, a timer always fires.

Consequences for entity-behavior FSMs:
- Leaving a state does **not** disarm the `EmitEventAfterDelay`s its `entry_actions` armed.
- A stale fire is usually harmless *because* `entity_fsm_interpreter_system` scopes `on:` bindings and
  transitions to the current state (verified: in-state `on:` is looked up by `fsm_state.current`;
  transitions filter on `t.from`). So a stale event whose name matches nothing in the *new* state is
  a silent no-op.
- It bites when the entity **re-enters** the state that does match. Two deaths inside one death's
  respawn window means the first death's respawn timer fires against the second corpse and revives it
  early. Any behavior file where a state can be re-entered faster than its own longest armed timer has
  this hazard.

**How to apply:** when reviewing a behavior-file diff, list the timers each state arms and the
shortest possible path back into that state. If (shortest re-entry) < (longest armed delay), the FSM
has a stale-timer cross-fire. Shortening one path's respawn/decay delay is the classic way to newly
expose it. Related: [[test-harness-message-buffers-never-rotate]] (the other "events don't get cleaned
up when you'd expect" trap).
