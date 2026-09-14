---
name: stakeholder-wishlist-tracking
description: My top-5 architect items in planning/stakeholder_priority_list.md (db1ede0, 2026-09-03) and their status as of the 2026-09-14 check-in
metadata:
  type: project
---

`planning/stakeholder_priority_list.md` holds a 5-stakeholder snapshot (commit `db1ede0`, 2026-09-03). My section is "System-Architect — stability, maintainability, future-proofing": (1) Rapier cross-platform float divergence gating Beta 0.5-0.9, (2) `Action` `deny_unknown_fields`, (3) scene-singleton config misplaced on `PrefabDef`, (4) test-suite trust (flakiness + no "warning was logged" infra), (5) `spawn_scene_v2` at the 16-param ceiling.

Status at the 2026-09-14 check-in: only #2 shipped (`4b8c865`/`b203f1f`/`3677859`). Everything after it in `db1ede0..HEAD` was CLI-validate hardening plus two small fixes. #1 has no investigation file and Beta 0.5 is untouched; #3 sits in Icebox; #4's both halves are open (targeting race root-caused `c936bdc`, 3rd recurrence 2026-09-14, unfixed); #5 unchanged at exactly 16 params.

**Why:** Frank periodically asks each agent persona to grade progress against its own wishlist, so the snapshot is a recurring reference point, not a one-off doc.

**How to apply:** Re-verify against `git log db1ede0..HEAD` and current `planning/backlog.md` before quoting any of these statuses — this note is a point-in-time reading. My standing recommendation coming out of 2026-09-14 is to pull #4's targeting-race fix forward next (cheap, unblocks trust in the merge gate), and to open a `planning/investigations/rapier_determinism.md` spike before more physics-adjacent features accrete. See [[determinism-networking]] and [[fragile-modules]].
