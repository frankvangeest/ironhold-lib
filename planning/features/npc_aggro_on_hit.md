# Feature: NPC aggro-on-hit (chase when attacked from range)

_Status: Active_
_Planned at: `6bb3f2f` (2026-06-19)_

## What

When the player damages an enemy from outside its `detection_radius`, the enemy should
start moving toward the player to retaliate — just as it would if the player walked into
its detection zone. Currently enemies stand still while being attacked from range.

## Why

The detection-radius model works for close-range encounters but breaks the feel of ranged
combat: the player can kite an enemy indefinitely by staying just outside detection range
and repeatedly casting skills. Aggro-on-hit makes ranged skills feel impactful and forces
the player to either kite properly or hold their ground.

## Approach

### Signal — RON emits the trigger

In `state_machine.ron`, each attack skill's `action_bar.activated:N` handler already runs
`{target}` substitution before pushing actions. Adding `EmitEvent("entity.attacked:{target}")`
to slots 1–4 (the attack skills) costs nothing at the schema layer — `EmitEvent` already exists.
Slot 5 (self-heal) is intentionally excluded.

### Reaction — NPC listens for the event

Add `pub aggroed: bool` (default `false`) to `NpcAgent`. A new system
`npc_aggro_on_hit_system` reads `EventReader<GameEvent>`, matches events of the form
`"entity.attacked:{id}"`, and sets `aggroed = true` on any hostile `NpcAgent` whose
`npc_id` matches and whose current state is `Patrol`, `Idle`, or `Return`.

In `npc_behavior_system`, the existing distance-based detection condition:

```rust
// Before:
if in_detect { transition_to(Alerted) }
```

becomes:

```rust
// After:
if in_detect || npc.aggroed { transition_to(Alerted); npc.aggroed = false; }
```

The flag is cleared on transition so it does not interfere with normal distance-based
detection on subsequent frames. Once in `Chase`, the NPC follows standard `chase_radius`
rules — the player can still disengage by running far enough away.

### No new schema fields

`aggroed` is a transient runtime flag on `NpcAgent` (not serialized). `NpcDef` in the
schema is unchanged. Designers trigger the mechanic by emitting `entity.attacked:{target}`
from any RON rule; no new action types or schema fields are required.

### System ordering

`npc_aggro_on_hit_system` reads the same `GameEvent` bus as the interpreter systems. It
must run **after** the interpreter pushes the `EmitEvent` action and **before**
`npc_behavior_system` reads `NpcAgent.aggroed`. Ordering: after `action_executor_system`,
before `npc_behavior_system` (both in `FixedUpdate`).

## Tasks

- [ ] Add `aggroed: bool` to `NpcAgent` (initialize `false` in `entity_spawner.rs`)
- [ ] Add `npc_aggro_on_hit_system` to `npc.rs` — reads `GameEvent`, sets flag on matching hostile NPC
- [ ] Update Patrol / Idle / Return detection branch in `npc_behavior_system` to check `|| npc.aggroed`; clear the flag on transition
- [ ] System ordering: register `npc_aggro_on_hit_system` in `FixedUpdate` after `action_executor_system`, before `npc_behavior_system`
- [ ] RON: add `EmitEvent("entity.attacked:{target}")` to `action_bar.activated:1–4` in `state_machine.ron`
- [ ] Tests: add integration test for aggro-on-hit (emit event → NpcAgent.aggroed becomes true → state transitions to Alerted)
- [ ] Docs: note `entity.attacked:{id}` event in `docs/30_runtime_events_and_logic.md`

## Open questions

- Should `NpcFaction::Neutral` NPCs aggro on hit (e.g. alpaking attacked by mistake)? Initial
  recommendation: yes — only `Flee` NPCs should not aggro. Architect to confirm.
- Should re-hitting an already-Chasing NPC reset anything? Probably not — the NPC is already
  heading toward the player.
- Is the FixedUpdate ordering guarantee solid given that `GameEvent` is a Bevy `Event` cleared
  each frame? Need architect to confirm the event is still present when `npc_aggro_on_hit_system`
  runs in the same `FixedUpdate` tick.

## Acceptance criteria

- Snake patrolling outside skill range: player casts Attack (slot 1), snake immediately enters
  Alerted then Chase and closes to melee range.
- Spider outside detection radius: same behaviour.
- Orc/zombie: same behaviour.
- Alpaking (neutral): attacked → starts chasing (unless architect rules Neutral NPCs should not).
- Self-heal (slot 5) does NOT cause any NPC aggro.
- Normal proximity detection still works — no regression.
- Player kites beyond `chase_radius`: NPC gives up and returns to patrol (no change to disengage logic).
