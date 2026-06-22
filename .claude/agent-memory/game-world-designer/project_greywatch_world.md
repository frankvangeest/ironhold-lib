---
name: project-greywatch-world
description: Core world-design decisions for the 3rd_person_game_demo project (village "Greywatch", zone layout, Old Key meaning)
metadata:
  type: project
---

The `3rd_person_game_demo` project has a v1 world design at
`assets/projects/3rd_person_game_demo/design/world_design.md` (authored 2026-06-22).

Key canon decided (logged in that doc's Decision Log):
- Village = **Greywatch** (proposed, pending Frank). Theme: struggling frontier
  hamlet at dusk; "holds the light, you carry it outward." Small-town survival RPG tone.
- **Single gameplay scene** for v1 (`main.scene.ron`), expanded — NOT split per zone.
  Spatial continuity carries the core "leaving the light" emotion. Future scene-cut
  seam = the Seal Door → ruins interior.
- **Zone layout** (matches scene coords: +Z south=spawn, −Z north=danger):
  Snake Marsh SW (Tier 1), Spider Woods NW (Tier 2), Graveyard NE (Tier 3),
  Ruins of the Seal far-N across the stone bridge (Tier 4). Difficulty maps to
  existing prefab stats: snake 50HP < spider 75HP < zombie 120HP < sorcerer (boss).
- **The Old Key** (`old_key` item) = Founder's Key to the ruins' Seal Door. Merchant
  Edrin sells it unknowingly. Explains why the player can re-seal what the sorcerer can't.
- **Named NPCs v1**: Maren (Elder, female, quest+reward), Halvard (gate guard — REUSE
  existing `npc_intro.dialogue.ron` verbatim), Brann (blacksmith), Edrin (merchant,
  existing). Tilly (child) = stretch goal. Pending Frank confirmation.
- **Only new combat prefab = Sorcerer mini-boss** using `wizard.glb` (already on disk).
  All snake/spider/zombie prefabs reused as-is.

**Why:** Frank asked for a proper village layout + monster zones + progression for a
small-team demo. Design maximizes world depth per unit of art effort by reusing
existing prefabs/shared models.

**How to apply:** When extending this project, keep the warmth-temperature gradient
(warm amber village → cold/sickly ruins), the compass-spread zones, and the
containment/"re-seal" framing rather than generic "kill the evil." Three engine
features the design needs are tracked — see [[engine-limits-dialogue-audio-itemgate]].
