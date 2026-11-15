---
name: project_item_use_s3_plan_review
description: Item system S3 (UseItem, use: block, default-backed buy_item/use_item clicks) plan review 2026-11-14 - do_actions naming, unreachable_trigger must drop buy_item, transitions must not suppress default, no-use: click footgun
metadata:
  type: project
---

Plan-reviewed `planning/features/item_use.md` at 60e28b0 (2026-11-14). Facts worth checking at impl review:

- Every action-list field in the schema is `do_actions` / `entry_actions` / `exit_actions`; there is NO bare
  `actions:` field anywhere. Plan proposed `use: (actions: [...])` - recommended `do_actions`.
- `unreachable_trigger` (docs/15 ~92, docs/20 ShopPanel note ~1418) currently REQUIRES a rule per `buy_item:<key>`;
  once Buy is default-backed that check tells designers to write rules S3 made optional. Plan task list omitted it.
- Plan: "MatchedUiEvents records bindings OR transitions". Transitions carry no actions, so a tutorial transition on
  `ui.button_pressed:use_item:x` would silently cancel the drink. The intent.slot precedent (docs/20 ~1176) counts
  bindings only.
- Default-backed = a state-scoped buy rule no longer gates buying in other states; blocking needs an explicit
  rule with empty do_actions (verify empty list counts as a match + passes ron_lint).
- docs/20 ~5085 + docs/30 BuyItem entry promise "a later release lets a rule replace the built-in buy messages";
  S3 does not deliver that (BuyItem itself still prints them) - sentence must be reworded.
- Clicking a slot holding an item without `use:` (sword, key) -> use_failed, so a generic "No potions left" rule
  fires on a sword click. Recommended: no default UseItem for items without `use:`.
- Demo: no drink sound (only "click"), `respawn_glow` effect reusable; player_mana max 60 with regen 5/s makes a
  small mana potion near-worthless; demo has no on-screen hint that slots are clickable.
- Right-click on a slot in 3rd_person (orbit_button "Right") / left-click (target clear) click-through to the world
  is undocumented anywhere - needs a test.

**How to apply:** at S3 impl review and S4/S5 plan reviews, check these closed; see [[project_item_txn_s2_plan_review]],
[[project_panel_world_blocking]], [[project_ui_trigger_wiring]], [[project_fsm_designer_traps]].
