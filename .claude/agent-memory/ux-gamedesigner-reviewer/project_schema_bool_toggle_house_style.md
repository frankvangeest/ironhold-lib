---
name: schema-bool-toggle-house-style
description: This schema overwhelmingly uses bool for binary opt-in toggles, not two-variant enums; recommend bool for new on/off fields
metadata:
  type: project
---

For a new binary on/off designer field, this codebase's house style is a `bool` (default
`false`), NOT a two-variant enum where one variant = "today's behavior".

**Why:** a schema-wide survey (2026-07-19) found the opt-in toggle field is almost always a bool:
`allow_manual_zoom`, `merged_allow_manual_zoom` (both siblings inside the same `player.rs` as
`SplitScreenDef`), `double_sided`, `unlit`, `cast_shadows`, `additive`, `physics`, `sensor`,
`double_jump`, `stackable`, `show_max`, `show_nameplates`, `show_player_nameplate`, `mute_on_start`,
`requires_los`, `click_selectable`, `targetable`, `absolute`. Enums are reserved for genuinely
multi-state axes (`SplitOrientation` = Vertical/Horizontal/Grid, `TargetHudDisplay`,
`faction_filter`), not for "default vs one alternative".

**How to apply:** when a plan proposes a two-variant enum `Default`/`Alternative`, push back toward
`something_only: bool` / `allow_something: bool` unless a third state is concretely planned — adding
a third mode later is a schema change either way, so "future-proofing" is a weak justification.
A bool named after the non-default state (e.g. `own_viewport_only: true`) reads naturally and
matches `allow_manual_zoom`. This is a Friction/consistency point, not a blocker.

Related: `[[local-coop-system]]` (SplitScreenDef is authored on the first player's `camera.split`
block in prefabs.ron; first-entity-wins).
