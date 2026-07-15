---
name: actionslotdef-label-undocumented
description: ActionSlotDef has a `label` field used in every shipped action bar but it is missing from the docs field table; collides in naming with proposed key_label
metadata:
  type: project
---

`ActionSlotDef` already has a `label: String` field. It is used in every shipped action bar slot:
`3rd_person_game_demo/scenes/main.scene.ron` ("Attack"/"Heavy Strike"/"Poke"/"Mana Blast"/"Heal"/"Inventory"),
`primitive_world/scenes/main.scene.ron` ("Heal"/"Speed Boost"/"Fire Burst"),
`stats_demo/scenes/main.scene.ron` ("Heal"/"Speed+"/"Mana+"). The canonical docs example (`docs/20_data_formats.md` ~L955) also uses `label: "Heal"`.

**BUT** `label` is NOT listed in the `ActionSlotDef` field table in `docs/20_data_formats.md` (~L876-884, which lists only key/icon/icon_index/icon_color/do_actions/cooldown_secs/cost). Pre-existing doc gap.

The action bar renders TWO distinct texts per slot per the `action_bar_custom_hotkeys.md` plan's own analysis: a corner key-hint (`Text::new(key.clone())`) and, separately, `label`. Any change to slot text/hint must account for both.

**Why it matters:** the `action_bar_custom_hotkeys.md` plan proposes adding `key_label: Option<String>` to override the corner key-hint, without ever mentioning the existing `label`. Two near-identical field names (`label` vs `key_label`) with a subtle distinction is a designer confusion trap. Consider `key_hint` instead, and document `label` at the same time.

**How to apply:** when reviewing any action-bar RON/doc change, verify the docs field table lists BOTH `label` and any new hint field, and that at least one example shows both in one slot so the distinction is unambiguous.
