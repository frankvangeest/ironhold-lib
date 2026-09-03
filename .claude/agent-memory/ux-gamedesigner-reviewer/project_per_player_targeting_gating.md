---
name: per-player-targeting-gating
description: The party-mode target_hud dead-end is now documented in docs/20 (~591-593); remaining live footgun is target_next defaulting to "Tab", which browsers intercept in WASM builds
metadata:
  type: project
---

**CLOSED:** the docs used to describe the ring-tint/target-var-blanking behavior as happening "once
split-screen is active" without mentioning it also fires in party-mode 2-player scenes (where
`target_hud:` produces no readout at all — a dead end). `docs/20_data_formats.md` (~591-593) now
states this explicitly: the legacy `target_display`/`target_name`/`target_id` vars go blank
whenever 2+ players are present "including party mode", and separately warns that `target_hud:`
only works for split-screen scenes — "a party-mode 2-player scene has no `SplitViewportSlot` camera
for `target_hud:` to attach to, so it gets no readout at all today (blank legacy vars, no
replacement) — a known Phase 1 gap, not yet built." Do not re-flag this conflation as undocumented.

**Still open — recurring trap:** `target_next` default is `"Tab"`, which browsers intercept for
focus nav in WASM builds (documented at the InputMap table). Any playtest-aid player prefab using
`target_next: "Tab"` will appear to have broken targeting in the web build — prefer `"KeyT"` etc.

**How to apply:** when reviewing a new project/scene that authors `target_next`, check it isn't left
at the `"Tab"` default before a web playtest.
