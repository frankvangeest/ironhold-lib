---
name: project_container_panel_paging_plan_review
description: Plan-review 2026-11-16 of container_panel_paging (pages + click-to-take): last-stack-first removal vs clicked slot, footer height jump, wheel vs camera zoom, silent full-bag click
metadata:
  type: project
---

Plan `planning/features/container_panel_paging.md` (planned at 7bddcbd). Verdict given: Needs more design work (small).

Open points raised (check at impl review):
- Built-in take must empty the CLICKED slot, not `remove_from_slots` last-stack-first (with pages the change lands on a page the player can't see).
- Plan never says whether page_count follows max_slots (stable) or filled stacks (compacting -> page vanishes under the player).
- Footer shown/hidden via Display changes panel height per container; ask to reserve the row.
- Wheel over panel: camera zoom may also fire (unverified); trackpad needs one-page-per-gesture threshold.
- A click with a full bag shows nothing (built-in floating texts exist only for Buy).
- take_item default-backed but take_all_from_container still needs a rule (asymmetry).
- A rule on take_item cannot read the clicked quantity; doc "hook inventory.added for a sound".
- Stale comments: prefabs.ron chest_swords, state_machine.ron line ~208; docs/30 ~1088 "only TakeAll fires looted".

Related: [[project_corpse_loot_v2_pattern]], [[project_item_sell_destroy_s4_plan_review]], [[project_em_dash_font_glyph_gap]].
