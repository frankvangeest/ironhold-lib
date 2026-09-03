---
name: npc-locomotion-bridge
description: npc_behavior_system writes LocomotionState in FixedUpdate; animation_resolver reads it in Update; cross-schedule bridge for GLB NPC animation
metadata:
  type: project
---

`npc_behavior_system` (capabilities/npc.rs, runs in FixedUpdate, chained after trigger_zone_system) drives all NPC AI/movement and, since the GLB-enemy work, also writes `LocomotionState` for NPC entities that have an AnimationPolicyComponent (loco_opt is `Option<&mut LocomotionState>` in the query — `None` for primitive NPCs).

Cost profile on web:
- Per-tick work scales with NPC count. find_nearest_visible_player snapshots player positions into a `Vec` once per tick (small, players count) — acceptable.
- LocomotionState write is change-detection-guarded on all three fields: `if loco.moving != moving`, `if loco.running != running`, and (fixed since original review) `if !loco.is_grounded { loco.is_grounded = true; }` (npc.rs ~L503). No field marks the component Changed unless its value actually flips.

**Why is_grounded mattered:** `animation_resolver_system` (animation_resolver.rs, Update) queries `&LocomotionState` (read-only, no `Changed<>` filter) and iterates all policy entities every frame regardless, so an over-firing change flag on `is_grounded` was never a live cost — but it was still a latent footgun if anyone later added `Changed<LocomotionState>` filtering there. That gap is closed now that all three fields are guarded; no action needed. Verified current (2026-09-03) at `capabilities/npc.rs` ~L503.

**How to apply:** When reviewing LocomotionState writers, enforce the change-detection guard on ALL fields written, not just some. Cross-schedule (FixedUpdate writer / Update reader) means FixedUpdate may run 0 or N times per render frame — never gate animation on write frequency.

Related: [[dynamic-labels-system]] for the change-detection-discipline pattern this project enforces.
