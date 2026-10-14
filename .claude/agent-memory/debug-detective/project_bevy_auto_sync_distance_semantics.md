---
name: bevy-auto-sync-distance-semantics
description: Bevy 0.18 auto ApplyDeferred nodes are shared per "distance" (hidden cross-chain ordering), ignore_deferred postpones rather than drops a sync, and a new deferred-respecting .chain() between two Commands systems always adds a sync
metadata:
  type: project
---

Verified in `bevy_ecs-0.18.0/src/schedule/auto_insert_apply_deferred.rs` during the D3 (gameplay_pipeline_system_sets) plan review, 2026-10-14:

- One `ApplyDeferred` node per **distance** (number of syncs from the graph start), shared by the whole schedule (`get_sync_point(distance)`). Any system feeding sync_k is therefore ordered before every system fed by sync_k, even across unrelated chains. Adding a sync anywhere upstream shifts distances and reshuffles these hidden orderings for unrelated systems; `conflicting_systems()` is computed after this pass, so an incidental shared sync can mask (really order) a pair that has no declared edge.
- `*_ignore_deferred` edges do not drop the flush: the target is marked "pending" and the sync lands on its next normal outgoing edge.
- A new normal `.chain()` between two systems that both take `Commands` adds a sync even inside a set whose other edges are `ignore_deferred` (D3's `EmitSet::Scene` check_project_loaded->spawn_scene_v2 and `DirectActions` resolve_pending_behaviors->despawn_timer chains).
- Conflicts from exclusive systems / `AccessConflicts::All` come back with an EMPTY component list, so a conflict filter keyed on resource ComponentIds silently drops them.

**How to apply:** when a plan counts "new sync points", trace distances along every new normal edge whose source has deferred params; prefer `chain_ignore_deferred()` where same-frame command visibility is not needed. See [[project_runif_skip_stale_change_baseline]].
