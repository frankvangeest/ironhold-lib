---
name: parse_key-case-sensitivity-trap
description: InputMap::parse_key is case-sensitive uppercase-only for letters; DIGIT_KEYS used lowercase "i" — a migration trap for action_bar_custom_hotkeys / Phase 2
metadata:
  type: project
---

`InputMap::parse_key` (`schema/player.rs`) is **case-sensitive** and has **no lowercase
single-letter arms** and no `.to_uppercase()` normalization: letters match only `"KeyA"`/`"A"`
(uppercase), digits match `"Digit1"`/`"1"`. So `parse_key("i")` returns `None`, not
`KeyCode::KeyI`.

**Why this matters:** the legacy action-bar `DIGIT_KEYS` table (`capabilities/action_bar.rs`)
hardcoded a lowercase `(KeyCode::KeyI, "i")` entry, and `3rd_person_game_demo`'s inventory slot
ships `key: "i"` (`scenes/main.scene.ron`). The `action_bar_custom_hotkeys.md` plan's Migration
section wrongly claimed `parse_key("i")` resolves and "no migration is needed" — following it
verbatim ships a silently-dead inventory slot (the exact failure the feature exists to remove).
`"i"` is the ONLY letter slot key across all projects; every other action-bar slot is a digit
(`"1"`..`"9"`), which parses fine.

**How to apply:** any migration off `DIGIT_KEYS` to `parse_key` (this feature, and its follow-on
`per_player_split_screen_targeting.md` Phase 2, which rebuilds the same `action_bar_input_system`)
must first resolve the lowercase-`"i"` gap: either add a lowercase-letter arm to `parse_key`
(non-breaking, keeps the `"i"` slot identity/hint/events intact) OR migrate the RON to `"KeyI"`
(changes slot_key identity → breaks `action_bar.activated:i` event wiring and flips the on-screen
hint from "i" to "I"). Prefer extending `parse_key`. Related: [[event_pipeline_intent_layer]].
