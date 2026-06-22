---
name: engine-limits-dialogue-audio-itemgate
description: Three Ironhold engine features needed by world/quest design, status unconfirmed as of 2026-06-22 (item-gated interact, zone audio, conditional dialogue choices)
metadata:
  type: project
---

Three engine capabilities the `3rd_person_game_demo` world design depends on, whose
support is UNCONFIRMED as of 2026-06-22. All three are logged in
`planning/claude_suggestions.md` under a "World Design / Gameplay" section.

1. **Item-gated interactable** — can an `interactable` require the player to hold a
   specific inventory item before firing? Needed for the Seal Door requiring `old_key`.
   Workaround if unsupported: gate on a GameVariable set by the existing
   `buy_item:old_key` event (rewards buying, not possessing).
2. **Zone-based ambient audio swap** — trigger a music/SFX bed change on TriggerZone
   enter/exit. Needed for the village-safe vs. field-tense audio gradient. May be
   authorable via `entity.entered/exited` rules + PlayMusicLoop/StopMusic.
3. **Conditional dialogue choices** — show/hide a dialogue node's `choices` based on a
   GameVariable (e.g. Maren's reward branch only after `sorcerer_defeated`).

**Why:** these are the gaps between the world design's intent and what was verifiably
authorable from reading the schema/RON at the time.

**How to apply:** before designing around any of these as if they work, VERIFY against
the current engine (grep schema/actions.rs, dialogue schema, interactable def, or ask
Frank). They may have shipped since 2026-06-22. Related: [[project-greywatch-world]].
