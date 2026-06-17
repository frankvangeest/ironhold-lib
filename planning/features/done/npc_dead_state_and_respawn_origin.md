# Feature: NPC dead-state fix + configurable respawn origin

_Status: Active_
_Planned at: `be229b7` (2026-06-17)_

## What

Two related fixes shipped together:

1. **Bug fix** — `npc_behavior_system` processes all `NpcAgent` entities regardless of visibility.
   When `SetEntityVisible(false)` hides a dead enemy, the AI loop keeps running: state machine,
   target detection, and `velocity.linvel` writes. The invisible Rapier capsule chases the player,
   producing a "ghost hitbox that follows you" after every kill.

2. **Enhancement** — a new `ResetToSpawn(entity)` action lets behavior files teleport an NPC back
   to its scene-placed spawn position before making it visible again. Without this, the entity
   re-appears wherever it died (often right next to the player). The action is opt-in so
   designers can choose "respawn at origin" vs. "respawn at death location" per behavior file.

## Approach

### Bug fix — visibility guard in `npc_behavior_system`

Add `Option<&Visibility>` to the query. At the start of each entity's loop iteration:

```rust
if visibility.is_some_and(|v| *v == Visibility::Hidden) {
    velocity.linvel = Vec3::ZERO;
    continue;
}
```

Zeroing velocity on skip prevents residual drift when the entity re-appears.

### Enhancement — `Action::ResetToSpawn(String)`

`NpcAgent` already stores `origin: Vec3` (the world-space position the NPC was spawned at,
used by the Return state). The new action reads this and teleports the entity.

**Executor logic:** look up entity in `SpawnRegistry` → get `NpcAgent.origin` → set
`Transform.translation` → zero `Velocity.linvel`. Warns + no-ops for non-NPC entities.

**Designer usage in behavior files:**
```ron
entry_actions: [
    ResetToSpawn("{self}"),                              // teleport to spawn point first
    SetStat(key: "{self}.health", value: 80.0),
    SetEntityVisible(entity: "{self}", visible: true),
    SpawnEffect(key: "respawn_glow", entity: "{self}"),
],
```
Omitting `ResetToSpawn` leaves the entity at its death position — both are valid authored choices.

## Files changed

| File | Change |
|---|---|
| `capabilities/npc.rs` | add `Option<&Visibility>` to query; skip + zero velocity for hidden entities |
| `schema/actions.rs` | add `ResetToSpawn(String)` with doc comment |
| `runtime/scene_manager/mod.rs` | add `npc_agents`, `transforms`, `npc_velocities` queries to `SceneStateParams` |
| `runtime/scene_manager/action_executor.rs` | handle `Action::ResetToSpawn` |
| `runtime/scene_manager/message_interpreter.rs` | `{self}`/`{target}` substitution for `ResetToSpawn` |
| `behaviors/enemy_orc.behavior.ron` | add `ResetToSpawn("{self}")` to alive entry_actions |
| `behaviors/enemy_snake.behavior.ron` | same |
| `behaviors/enemy_spider.behavior.ron` | same |
| `docs/20_data_formats.md` | document new action |
| `crates/ironhold_core/src/CLAUDE.md` | add `{self}` support note |
