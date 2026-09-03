---
name: parse_key-case-sensitivity-trap
description: InputMap::parse_key was uppercase-only for letters; feature/action-bar-custom-hotkeys added a single-lowercase-letter normalization. Fixed on that branch, verify on main before relying.
metadata:
  type: project
---

`InputMap::parse_key` (`schema/player.rs`) was historically **case-sensitive** with
**uppercase-only letter arms** and no normalization: `parse_key("i")` returned `None`, not
`KeyCode::KeyI`. This was a live trap: the legacy action-bar `DIGIT_KEYS` table hardcoded a
lowercase `(KeyCode::KeyI, "i")` entry and `3rd_person_game_demo`'s inventory slot ships
`key: "i"`, so migrating off `DIGIT_KEYS` to `parse_key` would silently kill that slot.

**Status:** FIXED and CONFIRMED on `main`/current tree — `parse_key` in `schema/player.rs` carries
the normalization pass described below (verified directly in source). The fix adds a normalization pass at the top of `parse_key`:
a **single** lowercase ASCII letter (`s.len() == 1 && is_ascii_lowercase`) is upper-cased before
the match; multi-character names (`"escape"`, `"keyq"`, `"f2"`) stay case-sensitive and still
return `None` if not authored in canonical form. So `parse_key("i")`/`("q")` now resolve;
`parse_key("keyq")`/`("f2")` still don't.

**Blast radius to remember:** `parse_key` is a shared helper with 8+ callers beyond the action
bar (`runtime/input.rs` global/scene bindings, `project_loader.rs` + `scene_loader.rs` validation
`.is_none()` checks, `scene_loader.rs` flycam `.unwrap_or(default)`, `targeting.rs` target_next).
The normalization is strictly **widening** — it only converts former-`None` single lowercase
letters to `Some`, never changes an existing `Some`. So it can only revive previously-dead
bindings across all those callers, never break a working one. Low risk, but it's a shared-helper
change tested only via the action bar. Related: [[event_pipeline_intent_layer]],
[[parse_key-action-bar-resolution]].
