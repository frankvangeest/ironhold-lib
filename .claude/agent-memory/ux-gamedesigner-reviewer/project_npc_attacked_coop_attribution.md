---
name: npc-attacked-coop-attribution
description: entity.attacked:<id> carries no attacker; in co-op the NPC investigates the player NEAREST the NPC (tie = lowest spawn id). Only documented in docs/30 line ~109; docs/20 NPC section never mentions entity.attacked
metadata:
  type: project
---

`entity.attacked:<id>` is a designer-authored `EmitEvent("entity.attacked:{target}")` in skill slot do_actions
(3rd_person_game_demo main.scene.ron, first_person_starter). Event has victim only. Since feature/npc_hit_nearest_player
(reviewed 2026-10-23): co-op picks nearest player to the NPC, tie -> lowest spawn id, missing NPC -> ignored.

Doc state at review: explained ONLY in docs/30 event list (~109). docs/20 "NPC behaviour" section (~3407) lists only 3
npc.* events (missing npc.investigating / npc.investigation_failed), never mentions entity.attacked, and
`investigate_timeout_secs` row says "attacker's last-known position" with no co-op caveat. `Alert` variant's hit
reaction undocumented. "Lowest spawn id" ordering (string vs numeric; player10 < player2) unverified.

**How to apply:** when NPC aggro/attribution changes again (e.g. an attacker field is added), update docs/30 ~109 AND
the docs/20 NPC section; push for an explicit "player 2 shoots, player 1 is closer -> NPC walks to player 1" sentence.
Related: [[local-coop-system]], [[player-count-change-assumptions]].
