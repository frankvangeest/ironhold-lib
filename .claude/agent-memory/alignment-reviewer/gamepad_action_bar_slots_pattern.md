---
name: gamepad-action-bar-slots-pattern
description: ActionSlotDef.gamepad_key 5-touchpoint additive-binding pattern; the "South collides with gamepad_jump default" footgun; gamepad_key-without-gamepad_index silent no-op validation gap
metadata:
  type: project
---

`planning/features/gamepad_action_bar_slots.md` (reviewed 2026-07-31, ALIGNED). Adds
`ActionSlotDef.gamepad_key: Option<String>` so an owner_player-scoped bar fires from that player's
own pad. Builds on [[per_player_action_bar_pattern]], [[keybinding_parse_key_vocabulary]],
[[local_coop_pattern]].

**Five touchpoints (complete here — reuse for any new per-slot binding):** schema
`ActionSlotDef` (`#[serde(default)]` on a `deny_unknown_fields` struct) → `ActionSlotUi`
runtime field → one `scene_loader.rs` resolve site (ActionBar has exactly ONE spawn arm, so no
3-spawn-path footgun, unlike `PrefabDef` markers) → `action_bar_input_system` read →
runtime-`warn!` + CLI-`validate` collision pair. **No new Action variant, no new event** — a
gamepad press emits the identical `intent.slot.{key}:{entity}` / `action_bar.*` contract, so
existing `rules.ron` hooks work unchanged and `query.rs`'s exhaustive match was untouched.

**Two collision checks have deliberately DIFFERENT scopes — don't "unify" them.** Keyboard
(`warn_cross_bar_duplicate_keys`) is scene-wide because `CooldownMap`/`PendingIntentActions`/
`HandledIntentSlots` are keyed by `slot_key` alone. Gamepad
(`warn_same_player_gamepad_duplicate_slots`) is keyed `(owner_player.unwrap_or(0), GamepadButton)`
because `gamepad_key` is never in the pipeline key space — its only failure mode is same-player
double-fire. Two *different* players sharing `"South"` must NOT be flagged (different pads); there
is a CLI fixture asserting the non-flag: `gamepad_action_bar_different_players_share_button`.

**FOOTGUN — `gamepad_key: "South"` collides with `InputMap.gamepad_jump`'s default `"South"`.**
Same for `"East"`=run, `"West"`=interact, `"North"`=target_next. Nothing checks slot `gamepad_key`
against the owning player's own `gamepad_*` InputMap fields, so the recommended docs/demo value
double-binds jump+ability on one press. Contrast the keyboard discipline in
`local_coop_demo/room3.scene.ron` (G/L chosen explicitly disjoint from movement/jump/run/
target_next). Prefer `"DPadUp"`/`"RightTrigger"` in any new example. Re-flag on any touch.

**Validation gap — CLOSED as of this update.** A slot declaring `gamepad_key` whose owning player
prefab has no `inputs.gamepad_index` is still a silent no-op at the mechanism level (the current
`BoundGamepad`/`gamepad_bind_system` model — see `gamepad_binding_pattern.md` — never falls back
to "any connected pad"; `resolve_gamepad`, the function this note originally cited, was deleted
during that hardening pass). But the diagnostic gap is closed: `scene_loader.rs`'s
`warn_gamepad_key_without_gamepad_index` (runtime `warn!`, called from the same site as the other
scene-load checks) and `ironhold_cli validate`'s matching `gamepad_key_without_gamepad_index`
error now both do exactly the owner_player→prefab cross-check this note was asking for — the same
`unwrap_or(0)` "None/Some(0) both mean the primary player" normalization
`warn_missing_player_stat_templates` uses. No longer a silent, undiagnosed no-op; re-flag only if
a future refactor removes either check.

**Accepted, documented limitations (do not re-flag as blockers):** `key` stays required, so every
gamepad-routed slot also has a live keyboard binding any keyboard can fire (`owner_player` has
always routed target/cost only, never gated the physical device — pre-existing, resolved in plan
review); `key_hint` has no gamepad-glyph auto-derivation. NOTE the docs suggest `key_hint: "Ⓐ"`
(U+24B6) — Bevy's default embedded font almost certainly lacks that codepoint, so an ASCII hint is
safer advice.

**Fixed here:** the long-standing wrong comment in `local_coop_demo/prefabs/prefabs.ron` claiming
keyboard bindings are "ignored while gamepad_index is set" (see [[gamepad_hot_join_pattern]]) now
correctly says input is additive, at all three `// gamepad_index` seams.
