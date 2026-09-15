---
name: floating-text-tracks-entity
description: ShowFloatingText/ShowDamagePopup track the target entity live — pairing one with Despawn of that same entity in one do_actions list makes the text permanently invisible
metadata:
  type: project
---

`Action::ShowFloatingText` (and `ShowDamagePopup`) spawn a `WorldLabel { tracked_entity: Some(e) }`
that re-reads the target entity's transform **every frame**. The world-label update system hides
the label (`Visibility::Hidden`) and skips it the moment `tracked_entity` no longer resolves.

**Consequence — the authoring trap:** a `do_actions` list that does
`ShowFloatingText(entity: "X", ...)` followed by `Despawn("X")` produces **zero visible frames**
of text. Both commands flush at the same sync point; the very next label update finds the tracked
entity gone. The popup silently ticks out its duration while hidden. There is no warning — the
`ShowFloatingText` handler's own registry lookup succeeds (it runs before the `Despawn`), so even
the `entity not found in spawn registry` warn does not fire.

Two correct patterns, both with shipped precedent in `3rd_person_game_demo`:
- Retarget the text to a surviving entity — `ShowFloatingText(entity: "player_01", ...)` is the
  project's dominant convention (all `action_bar.activated:*` rules, all monster-kill messages).
- Keep the text on the dying entity but delay its removal:
  `SetDespawnTimer(entity: "X", delay_secs: 1.5)` instead of `Despawn("X")` — precedent in
  `behaviors/lootable_corpse.behavior.ron` and the three `enemy_*.behavior.ron` death branches.

**Why:** found live in the `item_gated_interactable` shipped example, where the unlock feedback
("The seal breaks...") was invisible because the door despawned in the same action list. A demo
that teaches this pattern propagates it into every designer's project.
**How to apply:** whenever reviewing a `do_actions` list, check whether any `ShowFloatingText` /
`ShowDamagePopup` targets an entity that a later action in the *same* list despawns.
