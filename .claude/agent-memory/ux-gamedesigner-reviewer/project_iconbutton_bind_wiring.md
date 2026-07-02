---
name: iconbutton-bind-wiring
description: IconButton UI node's bind field is designer-wired (not auto-populated); scene RON lacks a wiring comment
metadata:
  type: project
---

`UiNodeDef::IconButton` (scene_v2.rs) swaps `icon_on`/`icon_off` textures based on a `GameVariables` key given by `bind`, resolving `"true"` vs anything-else each frame. `icon_off` also shows when the key is MISSING.

**Why:** Unlike targeting's auto-written vars (target_display/target_name/target_id), the `bind` variable for IconButton must be kept in sync by designer-authored `SetVariable` rules in logic/state_machine.ron or rules.ron. This is NOT automatic. Canonical example: 3rd_person_game_demo hud_audio_toggle, wired via global_on rules on `audio.muted`/`audio.unmuted` events setting `audio_muted` to "true"/"false".

**How to apply:** When reviewing IconButton usage, confirm (a) the scene RON node has a comment noting the bind var is manually wired, and (b) docs/20_data_formats.md distinguishes bind-wired vars from the auto-written GameVariables table (~line 637). As of 2026-07-01 the docs entry (~line 592-622) is good and does state the wiring requirement at line 622, but the scene RON block lacks a wiring comment and the auto-written table doesn't cross-note the distinction. Relates to [[auto_written_gamevariables_undocumented]] and [[audio_no_gamevariable]].
