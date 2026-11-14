---
name: executor-followup-push-runs-after-queued
description: An action the executor pushes onto ActionQueue (e.g. BuyItem's OpenShop refresh) runs AFTER everything already queued that frame, so a same-queue CloseShop/LoadScene is undone
metadata:
  type: project
---

`action_executor_system` drains `ActionQueue` with `while let Some(a) = pop()` (FIFO), so an arm that pushes a follow-up
action (S2a's `buy_item` pushes `OpenShop(merchant_id)` to refresh rows) runs it at the BACK, after every action that was
already queued in the same do-list or frame. A do-list `[BuyItem(x), CloseShop]` or `[BuyItem(x), LoadScene(..)]` then
re-opens the shop: Shop goes back into `open_panels` (modal -> `world_input_blocked`), `ui.panel_opened` fires again, and
after LoadScene the old merchant (still in the registry, teardown is deferred) is re-targeted in the new scene.

**Why:** found reviewing feature/item_transaction_primitive (2026-11-14); the refresh-by-requeue looks harmless in
tests because tests push one action per update.

**How to apply:** when an arm pushes a follow-up, ask "what if a panel-closing or scene-changing action is already
queued behind me?" Prefer doing the follow-up inline (a helper), or guard it (only if the panel is still open for the
same merchant). Related: [[loadscene-teardown-atomicity]], [[panels-open-refcount-leak]].
