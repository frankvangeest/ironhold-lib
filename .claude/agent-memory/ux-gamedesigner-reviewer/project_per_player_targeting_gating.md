---
name: per-player-targeting-gating
description: Per-player targeting's ring-tint + target-var-blanking trigger on CharacterController count>=2, NOT split-screen; party mode is a doc dead-end
metadata:
  type: project
---

Per-player split-screen targeting (Phase 1, `per_player_split_screen_targeting.md`) has THREE
behaviors with TWO different triggers — the docs conflate them:

- **Ring per-player tinting** and **`target_display`/`target_name`/`target_id` blanking**: gated on
  `player_count >= 2` where count = number of `CharacterController` entities
  (`targeting.rs` `is_multiplayer = ... .count() >= 2`). This fires in **party-mode** 2-player
  scenes too, not just split-screen. Confirmed in code 2026-07-13.
- **Per-viewport `target_hud:` readout**: only spawns per `SplitViewportSlot` camera — so party
  mode and single-player get NOTHING.

**Why this matters:** docs (`20_data_formats.md` "Per-player split-screen targeting") describe the
blanking/tinting as happening "once split-screen is active", and cross-reference tells a designer
whose `target_display` Label went blank to "use `target_hud:` instead". In a **party-mode** 2-player
scene the Label blanks but `target_hud` produces no readout — a dead-end the docs don't warn about.

**How to apply:** when reviewing changes to this area, check the docs distinguish "2+ players
present" (count-based: tint + blank) from "real split viewport present" (target_hud). Flag any
copy that ties the count-based behaviors to "split-screen" specifically.

Related recurring trap: `target_next` default is `"Tab"`, which browsers intercept for focus nav in
WASM builds (documented at the InputMap table, line ~1683). Any playtest-aid player prefab using
`target_next: "Tab"` will appear to have broken targeting in the web build — prefer `"KeyT"` etc.
