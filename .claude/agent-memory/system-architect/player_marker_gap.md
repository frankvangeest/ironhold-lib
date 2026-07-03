---
name: player-marker-gap
description: No dedicated Player marker exists; CharacterController is the de-facto "is local player" signal across 6 capabilities; NpcAgent is "is NPC"
metadata:
  type: project
---

There is NO dedicated `Player` marker component in ironhold_core. `CharacterController` (capabilities/player.rs ~17) does double duty as the "this entity is the local player" signal and is queried `With<CharacterController>` in 6 capabilities: npc, camera, targeting, interactable, collectible, action_bar.

`NpcAgent` is the "is an NPC" signal. There is no LocalPlayer/RemotePlayer/Owner distinction anywhere — every player is assumed the sole local player.

Consequence surfaced during nameplate review (2026-07-03): `NameplateFactionFilter::FriendlyOnly` in `nameplate_visibility_system` (capabilities/nameplate.rs ~184) implements "friendly" as `!NpcAgent`, which lumps the player in with friendly NPCs and decorative characters — there is no true "is the player" test.

**Why:** This gap is load-bearing for Beta 0.6 multiplayer (networking_multiplayer.md line 97 plans a player-identity concept) and for any "my nameplate vs. other players' nameplates" UX.

**How to apply:** When advising on any feature that needs to distinguish the player from other entities, recommend adding a dedicated `Player` marker inserted via `tag_spawned_entity` (the CLAUDE.md-designated single source of truth for spawn metadata), ideally with a `PlayerOwnership { Local, Remote }` field defaulting to Local as a cheap forward-compat hook. Routing the 6 `With<CharacterController>` "is player" queries onto a real marker is a standalone refactor worth doing regardless of the triggering feature.
