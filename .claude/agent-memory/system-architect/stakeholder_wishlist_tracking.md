---
name: stakeholder-wishlist-tracking
description: My top-5 architect items in planning/stakeholder_priority_list.md — db1ede0 (2026-09-03) list, its 2026-10-01 status, and the refreshed 2026-10-01 top-5
metadata:
  type: project
---

`planning/stakeholder_priority_list.md` holds a 5-stakeholder snapshot (commit `db1ede0`, 2026-09-03). My section is "System-Architect — stability, maintainability, future-proofing".

**Old list (db1ede0) → status at 2026-10-01 refresh (HEAD ce77bdd):**
1. Rapier float divergence — CHANGED: cause was wrong (enhanced-determinism always on); v1 fixed-timestep shipped `6f720de`; v2 cross-platform harness still Queued (Beta 0.5).
2. `Action` deny_unknown_fields — SHIPPED (merge `33842ff`).
3. Camera/input config on PrefabDef — STILL OPEN (Icebox); scene-level `camera_modes` map exists but per-player `camera_mode`/`split`/`party` still on PrefabDef.
4. Test-suite trust — CHANGED: flake fixed/closed (`80f5ab1`), but warning-assert infra, ambiguity detection, schedule-graph assertions all still Queued; zero `SystemSet`s in src.
5. `spawn_scene_v2` 16-param ceiling — STILL OPEN, still exactly 16 (moved to `runtime/scene_manager/scene_loader.rs:41`, file now ~3570 lines).

**Refreshed top-5 (2026-10-01):** (1) determinism harness before the ocean/buoyancy/boat physics batch (requested 2026-09-30) lands; (2) schedule-ordering contract (first SystemSet + ambiguity detection + graph assertion); (3) input-ownership arbitration — `UiPointerCaptured` primitive (backlog Bugs, left-mouse-on-UI); (4) loader failure visibility — `.project.ron` parse failure hangs forever, no scene-fetch retry; (5) scene_manager monolith / 16-param ceiling.

Also observed 2026-10-01: backlog `## Active` still holds the rules.ron consolidation bullet although its Done entry exists — the exact drift CLAUDE.md step 10 warns about.

**Why:** Frank periodically asks each persona to grade progress against its own wishlist; the snapshot is a recurring reference point.

**How to apply:** Re-verify against `git log <snapshot>..HEAD` and current backlog before quoting. See [[determinism-networking]], [[schedule-ordering-mechanism]], [[panel-input-blocking]], [[schema-tightening-blast-radius]], [[fragile-modules]].
