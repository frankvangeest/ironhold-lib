---
name: action-bar-click-activation
description: Action bar slots become left-click/tap activatable (feature/action_bar_mouse_click, 2026-09-30); "Activating slots" note in docs/20; key-only wording hotspots to grep when slot input changes
metadata:
  type: project
---

Every ActionBar slot is clickable (no opt-out field); click acts for the bar's `owner_player` regardless of the viewport under the cursor (mouse = shared hardware like keyboard), whereas world click-select routes by viewport. Canonical note: docs/20 "Activating slots" (placed right after the pipeline events table).

**Why:** when a new input source for slots lands, the keyboard-only wording survives in spots the plan's task list doesn't name.

**How to apply:** on any slot-input change grep these hotspots: docs/20 "Gamepad-routed action bar slots" opener ("either device"), docs/20 "Intent event layer" ("slot key is pressed"), docs/30 `intent.slot.*` bullet ("key n is pressed"), ActionBar RON comments in primitive_world/stats_demo ("press keys 1-3"). Touch support was still unconfirmed at review time -- watch for docs that both claim and disclaim tap. Related: [[action-bar-single-player-assumptions]].
