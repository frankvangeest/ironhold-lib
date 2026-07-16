---
name: action_bar_per_player_keying
description: Per-player action bars key CooldownMap/PendingIntentActions/HandledIntentSlots by slot_key alone; safety rests on a disjoint-keys invariant enforced by a scene-load warn + CLI error
metadata:
  type: project
---

Per-player action bars (`per_player_split_screen_targeting.md` Phase 2) add
`ActionBarDef.owner_player: Option<u32>` and rewrite `action_bar_input_system`
(`capabilities/action_bar.rs`) from `find`+`return` to a `filter` loop over every pressed slot,
resolving the acting player per-slot via `owns_slot(owner_player, PlayerIndex)` and reading that
player's own `PlayerTarget` (not global `CurrentTarget`) for `{target}` rewrite / no-target gate /
intent-event player id.

**Load-bearing invariant:** `CooldownMap`/`PendingIntentActions`/`HandledIntentSlots` are STILL
keyed by the literal `slot_key` string alone, scene-wide (composite-keying deliberately deferred).
This is only safe because slot keys are disjoint across bars. If two bars share a key: same-frame
`pending.insert` drops one press, cooldown from one blocks the other, and (worst) a rules.ron rule
handling one bar's intent suppresses the other's pending slot via `HandledIntentSlots` +
`intent_slot_key()` in `message_interpreter.rs`.

**Why:** Mitigation is a scene-load `warn!` (`scene_loader.rs::warn_cross_bar_duplicate_keys`,
compares by resolved KeyCode) + a hard CLI error (`cross_bar_duplicate_key` in
`ironhold_cli/validate.rs`, exit 1). Both distinguish bars by `bar.id`, so duplicate `ActionBar` ids
defeat the runtime cross-bar warn (CLI still errors but mislabels as `duplicate_key`). There is no
uniqueness validation on `ActionBar.id`.

**How to apply:** When reviewing any future action-bar change, check the disjoint-keys invariant
still holds and that any new bar-identity logic doesn't rely on `bar.id` uniqueness. If a real
project needs same-key bars, that's the trigger to finally composite-key the three resources by
(player, slot_key). See also [[event_pipeline_intent_layer]] and [[targeting_currenttarget_mirror]].
