---
name: keybinding-parse-key-vocabulary
description: InputMap::parse_key is the single shared RON key-name vocabulary for all keybindings (player inputs, flycam, scene_key_bindings, action-bar slots); the dual runtime-warn + CLI-validate-error pattern for validating RON keys
metadata:
  type: project
---

`InputMap::parse_key(&str) -> Option<KeyCode>` (`schema/player.rs`) is the **single source of truth** for every designer-authored key name in RON. All keybinding consumers route through it: player `inputs:`, flycam forward/back/etc, `scene_key_bindings`, and (as of the action-bar-custom-hotkeys feature) per-slot `ActionSlotDef.key`. This is the canonical way to make a new keybound feature data-driven — never introduce a private key lookup table (the feature that added this *deleted* a hardcoded `DIGIT_KEYS` table in `action_bar.rs` in favour of per-slot `parse_key` resolution).

**Vocabulary is a fixed enumerated match** (letters KeyA/A, digits Digit0/0, Numpad0-9, F1-F12, modifiers, Space/Escape/Enter/Tab/Backspace/Delete, arrows). This is NOT a hidden-hardcoding blocker — it's a comprehensive, documented enumeration shared everywhere, and the "not supported" set (mouse buttons, modifier chords like `"Shift+1"`) is documented. Adding a new bindable key = adding one match arm (rare, low-risk).

**Single-lowercase-letter case-insensitivity** (added in the action-bar feature): a 1-char lowercase ASCII letter is upcased before the match (`"q"` -> `"Q"`), so bare single letters are case-insensitive; multi-char names (`"Escape"`, `"KeyQ"`) stay case-sensitive. When reviewing changes touching this, check that `docs/20_data_formats.md`'s "Valid key name strings" table (~line 1628-1640) reflects it — that table's "Case is significant" line is the authoritative shared reference and is easy to leave stale.

**Dual runtime-warn + CLI-validate-error pattern** (good pattern to confirm, not flag): RON key mistakes are surfaced two ways —
- Runtime (`scene_loader.rs`): lenient `warn!` only; an unparseable/duplicate slot key degrades gracefully (`resolved_key: None` -> slot never fires), the game still runs.
- CLI (`ironhold_cli validate.rs`): strict — pushes a `CrossFileError` (`error_type: "invalid_key"`/`"duplicate_key"`), exit code 1, so a designer catches it before shipping rather than missing a browser-console warning.
This runtime-lenient / CLI-strict split is the correct shape for RON-authoring validation; mirror it for future RON-key or RON-reference checks. The CLI crate deliberately avoids a direct `bevy` dependency — it names `parse_key`'s `KeyCode` return via an inferred `HashMap<_, ...>` rather than importing the type.
