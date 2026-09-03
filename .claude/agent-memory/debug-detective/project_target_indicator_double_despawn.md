---
name: target-indicator-double-despawn
description: FIXED — target_indicator_system's double-despawn (two loops over one query snapshot, deferred despawn, no dedup) is now guarded by a HashSet; kept as a reference example of the failure class
metadata:
  type: project
---

**FIXED.** `target_indicator_system` (crates/ironhold_core/src/capabilities/target_indicator.rs)
now declares `let mut despawn_queued: HashSet<Entity> = HashSet::new();` (~line 113) and every
despawn call site guards on `despawn_queued.insert(indicator_entity)`, so a ring that both loops
would have queued for despawn in the same frame is only ever despawned once.

**Kept for reference — the general failure class is still worth recognizing elsewhere:**
iterating one query snapshot in two separate passes, each queuing a deferred `Commands::despawn`,
with no dedup, double-despawns any entity both passes touch in the same frame (the second despawn
logs "Entity NNNv0 is invalid; its index now has generation 1" at flush, since Commands are
deferred so the snapshot still contains the entity for the second pass). The `try_despawn()`
convention documented in `crates/ironhold_core/src/CLAUDE.md` ("Despawning: prefer `try_despawn()`
when an entity may already be gone") is the lower-ceremony default for new code hitting this shape;
this file's `HashSet` guard was kept here specifically for clarity at this call site. Related:
[[project_ui_pick_blocking]].
