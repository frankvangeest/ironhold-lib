---
name: npc-locomotion-bridge
description: npc_behavior_system writes LocomotionState in FixedUpdate; animation_resolver reads it in Update; cross-schedule bridge for GLB NPC animation
metadata:
  type: project
---

`npc_behavior_system` (capabilities/npc.rs, runs in FixedUpdate, chained after trigger_zone_system) drives all NPC AI/movement and, since the GLB-enemy work, also writes `LocomotionState` for NPC entities that have an AnimationPolicyComponent (loco_opt is `Option<&mut LocomotionState>` in the query — `None` for primitive NPCs).

Cost profile on web:
- Per-tick work scales with NPC count. find_nearest_visible_player snapshots player positions into a `Vec` once per tick (small, players count) — acceptable.
- LocomotionState write is change-detection-guarded: `if loco.moving != moving`, `if loco.running != running`. Good. BUT `loco.is_grounded = true` is written UNCONDITIONALLY every tick — marks the component Changed every FixedUpdate tick even when nothing moved.

**Why is_grounded matters:** `animation_resolver_system` (animation_resolver.rs, Update) queries `&LocomotionState` (read-only, no `Changed<>` filter), so an over-firing change flag does NOT currently re-run extra work there — animation_resolver iterates all policy entities every frame regardless. So the unconditional write is cheap *today* but is a latent footgun if anyone adds `Changed<LocomotionState>` filtering. Fix is trivial: `if loco.is_grounded != true { loco.is_grounded = true; }`.

**How to apply:** When reviewing LocomotionState writers, enforce the change-detection guard on ALL three fields, not just two. Cross-schedule (FixedUpdate writer / Update reader) means FixedUpdate may run 0 or N times per render frame — never gate animation on write frequency.

Related: [[dynamic-labels-system]] for the change-detection-discipline pattern this project enforces.
