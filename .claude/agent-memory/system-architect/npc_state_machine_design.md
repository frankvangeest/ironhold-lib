---
name: npc-state-machine-design
description: NpcState design rationale — why states are runtime-only (not schema), the Investigating state split from Chase, and the dist_opt.or hack to remove
metadata:
  type: project
---

`NpcState` (capabilities/npc.rs) is a **runtime enum, never serialized** — adding a variant is NOT a schema change and never breaks RON backward-compat. NPC *tuning* values go into `NpcDef` schema with `default_npc_*` named-default fns (catalog.rs ~1065-1086 pattern); NPC *states* stay in Rust. Keep this split.

**Investigating-state design (proposed 2026-06-19, Frank's team):** add `Investigating` between Chase and Return for hit-from-range kiting. The architecturally important decision: it lets Chase revert to **visibility-only** `in_chase` and **delete the `dist_opt.or(target_dist_opt)` hack** (npc.rs ~227) that was stretching Chase to cover beyond-visual-range pursuit. Chase = live visible entity; Investigating = walk toward a stale last-known `Vec3`. Different movement inputs → different states; do not fold into Return (which means "go home to origin").

**Why:** overloading Return/Chase with mode flags conflates runtime states and raises cognitive load; the existing duplicated `was_hit` check (npc.rs ~237 and ~292) is already a smell. Centralize hit-handling into one pre-match block instead of per-state arms.

**How to apply:** if reviewing NPC AI changes — (1) new states are fine, not schema breaks; (2) push for centralized hit/aggro handling not per-arm duplication; (3) snapshot attacker pos in `npc_hit_relay_system` (Update, closest to the hit event) not in `npc_behavior_system` (FixedUpdate, 1+ tick later — see [[schedule-update-vs-fixedupdate]]); (4) Rapier ray casts already make this non-deterministic — don't claim determinism (see [[determinism-networking]]). `NpcHitQueue` HashMap<String,Vec3> swap and Option<Vec3> on NpcAgent are perf-trivial at 2-8 NPCs/scene.
