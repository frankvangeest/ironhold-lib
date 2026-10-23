---
name: npc-id-vs-spawn-id
description: NpcAgent.npc_id equals SpawnId for scene-placed and Action::Spawn NPCs, but composite-child NPCs get the nested PREFAB KEY as npc_id and no SpawnId at all
metadata:
  type: project
---

`NpcAgent.npc_id` is set from the same string as `SpawnId` on three paths: scene GLB (`spawn_prefab_instance` name = `entity_def.id`),
scene Primitive / composite-parent NPC (`entity_def.id`), and `Action::Spawn` (`queued.spawn_id`). So `entity.attacked:{target}`
(`{target}` = `PlayerTarget` spawn id) resolves to the right NPC in shipped content.

Exception: a composite's nested children are spawned by `spawn_prefab_instance(..., nested_key, ...)` and never go through
`tag_spawned_entity`, so an NPC child gets `npc_id == <prefab key>` (shared by every instance of that composite) and no `SpawnId`.
Two instances therefore collide on npc_id; anything keyed by npc_id (NpcHitQueue/NpcDeadQueue/NpcReviveQueue) hits all of them,
and a `find` by npc_id picks one in query order.

**Why:** found during the 2026-10-23 review of `feature/npc_hit_nearest_player` (relay now looks up the hit NPC by npc_id).
**How to apply:** when a change keys NPC behaviour by `npc_id` or assumes it equals `SpawnId`, check the composite-child path; see
[[spawn-ids-one-flat-namespace]] for the general collision story. Players also carry `SpawnId` (via `tag_spawned_entity`).
