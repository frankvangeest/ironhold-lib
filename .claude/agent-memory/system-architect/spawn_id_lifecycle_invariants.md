---
name: spawn-id-lifecycle-invariants
description: Non-obvious invariants around SpawnRegistry id reuse, Action::Spawn transform capture timing, Despawn's warn-on-missing, and container.looted's empty-skip — found reviewing monster_corpse_loot
metadata:
  type: project
---

Four load-bearing facts about the spawn/despawn/container lifecycle that keep resurfacing in
design reviews (verified 2026-08-24, `f9849ca`-era — hash updated after the 2026-09-03 `pkg/`
history purge; the original citation, `452e2e2`, was a pkg-only rebuild commit fully pruned during
that purge, so this points to its parent instead, the actual code state at that same point in time):

1. **`Action::Spawn` captures its `Transform` at action-execution time**, into
   `QueuedSpawn.transform` (`runtime/scene_manager/mod.rs`), and `drain_spawn_queue_system`
   only reads it. So any entity-relative resolution (`at_entity`-style, mirroring
   `SpawnEffect.entity`) is safe against the source entity being despawned in the same
   executor run — the value is already snapshotted. `SPAWNS_PER_FRAME = 2` means the
   spawn itself can lag several frames behind, but its position won't drift.

2. **`SpawnRegistry.entities` is a `BTreeMap<String, Entity>` — reusing a spawn id
   silently orphans the previous entity.** It can then never be `Despawn`ed (the id now
   maps to the newer entity) and leaks until scene unload. Any design that authors a
   *derived* stable id (`"{self}_corpse"`) requires the source id itself to be unique per
   instance; "monster respawns with a fresh id" becomes a hard requirement, not a
   preference. **Escape hatch (`feature/monotonic-entity-id`, 2026-08-29): the `{new_id}`
   token in `Spawn.id`**, resolved in the executor arm off `SpawnRegistry.counter`. Its
   tradeoff: the resolved id is *unknowable to RON* — no spawn-completion event carries it,
   nothing writes it to a game var — so anything that later `Despawn`s / `TransferItem`s
   that entity must act from the entity's **own** behavior FSM via `{self}`, not from the
   spawner's. Verify it's still on `main` before recommending it.

3. **`Action::Despawn` on an unknown id `warn!`s** (`action_executor.rs`) — it is a
   functional no-op but not a silent one. Any "two timers race, loser no-ops" design
   therefore emits a guaranteed warning per instance. The clean fix is behavior-FSM state
   gating (put the two outcomes in different states so only the state-appropriate event has
   a transition) rather than relying on the no-op.

4. **`Action::TakeAllFromContainer` returns early on an empty container
   (`if items_to_transfer.is_empty() { continue; }`) so `container.looted:{id}` never
   fires for a zero-loot container.** Any RON logic keyed on `container.looted` silently
   never runs for empty containers. Same arm also removes from the container *before*
   `add_to_slots` respects the player's `max_slots` — items are destroyed when the player
   inventory is full (pre-existing; amplified by anything that multiplies container count).

**Why:** all four were only findable by reading the executor arm, not from schema docs, and
each one invalidates an otherwise-reasonable plan.

**How to apply:** cite these when reviewing any feature that spawns/despawns entities from
RON on a timer, derives one spawn id from another, or hangs logic off `container.looted`.
See [[deferred_despawn_double_queue]] for the adjacent double-despawn class.
