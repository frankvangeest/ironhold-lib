---
name: interactable-multifire-and-floatingtext-despawn
description: interactable_system fires for EVERY in-range Interactable (never nearest-only), so overlapping interact radii double-fire; and ShowFloatingText + Despawn in one do_actions list makes the text permanently invisible
metadata:
  type: project
---

Two non-obvious runtime interactions that bite any "press F on a prop" feature.

**1. `interactable_system` has no nearest-only selection.** Its inner loop writes one
`GameEvent::Trigger` per in-range `Interactable`, per pressing player — `hit_any` is only used
to suppress `player.attack_missed`. So two entities whose interact radii overlap BOTH fire on a
single F press, and the two rules both run in the same `action_executor_system` drain
(`while let Some(action) = action_queue.pop()`, one full drain per run).

Concrete shipped case found 2026-09-14 in `3rd_person_game_demo/scenes/main.scene.ron`:
`merchant_01` (-8,0,10) has `interactable.radius: 3.0`, a newly-added `seal_door` (-11,0,7) has
`radius: 2.0`; centre distance is 4.243 < 5.0, so there is a ~2.6 m × 0.76 m lens. Worse, the
lens sits on the merchant side, so the *first* point at which the door becomes interactable
(t = 2.0 m) is still inside merchant range (2.24 m < 3.0) — a player walking merchant → door and
pressing F at the first opportunity always triggers both. Rule of thumb: for two interactables
A and B, require `dist(A,B) > radius_A + radius_B`, and check it against the natural approach
path, not just the centres.

**Why:** Found while adversarially reviewing `feature/item_gated_interactable`; both the
overlap and the fires-for-all semantics were invisible from the diff alone.

**How to apply:** Whenever a new `interactable:` prefab is placed, sum its radius with every
nearby interactable's radius (and remember `merchant`/`chest` prefabs carry radius 2.0-3.0 plus
a slightly larger `trigger_zone`) before accepting the placement.

**2. `ShowFloatingText(entity: X)` followed by `Despawn("X")` in the same `do_actions` list
renders nothing at all.** `Action::ShowFloatingText` spawns two *sibling* `Text2d` entities
carrying `WorldLabel { tracked_entity: Some(e) }` — not children of X. Both the popup spawn and
the `try_despawn()` of X are deferred commands that flush together, so from the popup's very
first frame `world_label_screen_pos_system` (`lib.rs`, the `tracked_q.get(tracked)` arm) fails
the lookup and sets `Visibility::Hidden`, permanently. The popup then just ticks out its
`DamagePopup` duration invisibly — no warning, no log. Same trap for `SetEntityVisible(false)`
on the tracked entity (there is an explicit `tracked_vis == Hidden -> Hidden` branch too).
To show a "you destroyed it" message, target a surviving entity (the player) or delay the
despawn with `SetDespawnTimer`/`EmitEventAfterDelay`.

See [[project_panels_open_refcount_leak]] for the other interactable-adjacent shipped-demo trap.
