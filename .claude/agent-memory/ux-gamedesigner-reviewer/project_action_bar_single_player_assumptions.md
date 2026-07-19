---
name: action-bar-single-player-assumptions
description: The ActionBar has three baked-in single-player assumptions (keyboard-only, global cost pool, scene-wide slot_key) that become designer traps in split-screen co-op
metadata:
  type: project
---

The `ActionBar` UI widget carries three single-player assumptions that are durable engine facts (verify against current code before citing, but true as of 2026-07-15). They all become designer-visible traps the moment a scene has 2+ players, and they are the crux of `per_player_split_screen_targeting.md` Phase 2.

- **Keyboard-only.** Slots are activated via `parse_key`-resolved keyboard keys (`DIGIT_KEYS` today, any `parse_key` name after `action_bar_custom_hotkeys.md`). There is NO gamepad/`InputMap` path. In the canonical co-op config (1 keyboard + 1 gamepad player, which Phase 1 targeting explicitly supports), the gamepad player's action bar renders fully but can never fire. This looks exactly like a bug.
- **Cost pool is global.** `SlotCost` checks/deducts against the single global `LoadedStats` resource, NOT per-entity `StatMap` (the `"{self}.stat"` addressing NPCs use). In a two-bar split-screen scene, spending a cost stat dims BOTH bars — a false per-player signal, not just a "shared economy" limitation.
- **`slot_key` is scene-wide identity.** `CooldownMap`/`PendingIntentActions`/`HandledIntentSlots` are keyed by the bare `slot_key` string across the whole scene, not per bar/player. Two bars reusing the same key (the natural copy-paste path) collide: fire-first + shared cooldown bleed.

**Also:** intent interception (`intent.slot.{key}` rules in `rules.ron`) is a first-class, doc-encouraged pattern (`docs/20_data_formats.md` ~L912-929, incl. a `{target}`-using "redirect" example), but `{target}` inside those rules resolves via the interpreter against the PRIMARY player only (`CurrentTarget`) — so the encouraged pattern is the one that silently breaks in co-op. The `intent.slot.{key}:{player_id}` event suffix lets a rule match WHICH player pressed, but not target that player. See [[per-player-targeting-gating]].

**`ActionSlotDef.key` is REQUIRED (`String`, not `Option`) — there is no unbound or gamepad-only slot.** Every slot always carries a live binding on the single global shared keyboard, regardless of the bar's `owner_player`. Consequence for the mixed keyboard+gamepad co-op config: a gamepad player's slot still needs a `key`, and any keyboard press of that key fires it (acting on the gamepad player's own target). `gamepad_action_bar_slots.md` (planned 2026-07-19) adds an optional sibling `gamepad_key: Option<String>` but does NOT make `key` optional — so a "pure gamepad" slot remains unauthorable and the phantom keyboard binding persists. Flag this whenever a plan claims to deliver gamepad action bars.

**How to apply:** when reviewing any co-op/split-screen action-bar change, check the demo isn't cost-gated (would show the dimming artifact — NOTE: cost pool is now per-entity as of `per_player_stat_pools.md`, room3 uses per-player mana, so this specific point may be stale — verify), isn't reusing a gamepad player for the ability demo (dead bar, until `gamepad_action_bar_slots.md` ships), and uses disjoint slot keys (cross-bar duplicate keyboard detection now exists via `warn_cross_bar_duplicate_keys` — but `docs/20_data_formats.md` ~L897 still staley claims cross-bar keys are "not currently cross-checked"). Push for load-time `warn!`s (not just opt-in CLI `validate()`) since designers live in the running build.
