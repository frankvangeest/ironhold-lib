---
name: action-bar-cost-spent-on-executor-failure
description: Action bar gates run before flush; cost is a separate queued ModifyStat and cooldown commits at flush, so an action that fails in the executor (UseItem consume>held, no use:) still burns both
metadata:
  type: project
---

`action_bar_input_system` gates (cooldown, S5 no-item `held == 0`, cost, no-target) run before the interpreters. A slot that passes gets
`do_actions + [ModifyStat(-cost)]` in `pending`; `flush_pending_intent_system` pushes them and commits the cooldown unconditionally. The
cost is its own action, not part of a transaction with the slot's action, so when the slot's action fails at execute time (e.g.
`UseItem` with `consume: 2` and 1 held, or an item with no `use:`), the mana is still deducted and the cooldown still runs, and only
`item.use_failed` fires.

**Why:** found reviewing S5 item slots (2026-11-15, `feature/action_bar_item_slots`): the S5 gate is `held == 0`, not `held < consume`.
**How to apply:** for any new bar gate that mirrors an executor-side check, verify the gate's threshold equals the executor's exactly
(`item_actions.rs::use_item` takes `use_def.consume`, `consume: 0` means "must hold 1"); otherwise the refusal path is the costly executor one.
Related: [[executor-followup-push-runs-after-queued]].
