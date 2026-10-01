---
name: bevy-interaction-press-edge-semantics
description: Bevy 0.18 ui_focus_system facts that make `Ref<Interaction>::is_changed() && == Pressed` a sound per-press edge detector (action bar clicks), plus its real gaps
metadata:
  type: project
---

Verified from bevy_ui-0.18.0/src/focus.rs (2026-09-30, action_bar_mouse_click review):
- `Pressed` is written only when `mouse_clicked` (Left just_pressed OR any touch just_pressed) AND guarded by `if *interaction != Pressed` — so no Pressed->Pressed rewrite, and a node spawned under an already-held button becomes Hovered, never Pressed.
- Press+release in the same frame: set Pressed, pushed to `entities_to_reset`, forced to None next frame — edge still observable.
- Hidden (`!inherited_visibility`) nodes are `set_if_neq(None)` — never clickable.
- A missed `just_released` (focus lost mid-press) leaves the node stuck Pressed; the next click on it produces NO change tick (guard), so one click is swallowed, and `click_select_system`'s "any Interaction Pressed" guard blocks world targeting until a release arrives.
- `run_if` skipping a system does not advance its last_run, so is_changed catches changes made while skipped (no lost edges).

**Why:** future reviews of any Interaction-edge code can reuse these instead of re-reading focus.rs.
**How to apply:** tests that drive `Interaction` by hand must write Pressed once after a warm-up update; assertions that only check *absence* (suppressed/not-fired) pass vacuously if the click never registered — require a positive signal too. See [[project_ui_pick_blocking]], [[project_test_harness_message_buffers_never_rotate]].
