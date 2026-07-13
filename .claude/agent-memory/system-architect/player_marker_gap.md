---
name: player-marker-gap
description: RESOLVED — a dedicated Player marker + PlayerOwnership + PlayerIndex now exist in capabilities/player.rs; historical note on the CharacterController-as-player-signal era
metadata:
  type: project
---

**RESOLVED (verified 2026-07-13).** The gap this memory originally documented is closed. `capabilities/player.rs` now defines:
- `Player` (marker, ~line 23) — inserted unconditionally at every player spawn site (GLB `spawn_player_entity`, primitive inline in `scene_loader.rs`, dynamic character-select). Distinct from `CharacterController` on purpose: "a future networked remote player may carry `Player` without local input handling."
- `PlayerOwnership { Local, Remote }` (~line 29) — always `Local` today; forward-compat hook for Beta 0.6 LAN co-op so nameplate/UI/camera can tell "me" from "other players" without a schema pass.
- `PlayerIndex(u32)` (~line 41) — forwarded from `PrefabDef.player_index`; first real consumer is the split-screen HUD corner label + `PLAYER_LABEL_COLORS`.

Nameplate gating already uses `Option<&Player>` to pick `player_enabled` vs `enabled` (see CLAUDE.md "Dynamic spawning" / [[nameplate-gating]]).

**Historical context (pre-2026-07-13):** `CharacterController` used to be the de-facto "is the local player" signal, queried `With<CharacterController>` across ~6 capabilities (npc, camera, targeting, interactable, collectible, action_bar). Some of those queries may still use `With<CharacterController>` — that's now correct where the intent is specifically "locally-controlled player" (tab-cycle input, movement), and should be `With<Player>` only where the intent is "any player entity incl. future remote." Don't assume they were all migrated.

**How to apply:** When advising on features that must distinguish players, `Player`/`PlayerOwnership`/`PlayerIndex` already exist — recommend querying them rather than proposing to add them. Use `CharacterController` for "locally-input-controlled" and `Player` for "is a player entity (any ownership)." See [[player-spawn-paths]].
