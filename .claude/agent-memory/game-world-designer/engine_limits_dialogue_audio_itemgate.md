---
name: engine-limits-dialogue-audio-itemgate
description: Dialogue conditions shipped; item-gated interact and sound zones still missing, tracked in planning/backlog.md L106/L108
metadata:
  type: project
---

Three engine capabilities the `3rd_person_game_demo` world design depends on. Originally logged
2026-06-22 as unconfirmed; re-verified 2026-09-03.

1. **Conditional dialogue choices — SHIPPED, confirmed 2026-09-03.**
   `DialogueChoiceDef.condition: Option<DialogueCondition>`
   (`crates/ironhold_core/src/schema/dialogue.rs`) supports `HasVariable { key, value }`
   (`GameVariables[key] == value`), `VariableGte { key, min }` (`GameVariables[key] >= min`), and
   `StatAtLeast { stat_key, min }`. A choice with a `condition` is hidden when it evaluates false.
   Promoted to backlog `planning/claude_suggestions.md` line ~364 (2026-06-23). Safe to design
   around directly — e.g. Maren's reward branch only after `sorcerer_defeated` is now authorable.
   (Note: `planning/backlog.md`'s own entry for this — line 107 — is stale, still unchecked with an
   outdated field-name spec; that's a backlog-hygiene issue, not a reason to doubt the schema.)

2. **Item-gated interactable — still missing, confirmed 2026-09-03.**
   `InteractableDef` (`crates/ironhold_core/src/schema/catalog.rs` ~line 976) only has `radius` and
   `hint_text` — no inventory-requirement field. Spec already written and queued:
   `planning/backlog.md` line 106 — `requires_item: "key_id"` field on `PrefabDef.interactable`;
   fires `entity.interact_blocked:{id}` when the player lacks the item, `entity.interacted:{id}`
   when they have it. Needed for the Seal Door requiring `old_key`. Until this ships, workaround is
   gating on a GameVariable set by the existing `buy_item:old_key` event (rewards buying, not
   possessing). Check the backlog entry directly for current status rather than re-grepping the
   schema — it's the source of truth now.

3. **Sound zones / zone-based ambient audio — still missing, confirmed 2026-09-03.**
   `TriggerZoneDef` only has `radius` — no audio-swap variant. Spec already written and queued:
   `planning/backlog.md` line 108 — new `kind: SoundZone` trigger zone variant with `audio_key`,
   `volume`, `fade_distance`; entering fades audio in, leaving fades it out; authorable entirely in
   scene RON via the existing trigger zone + `PlayMusicLoop`/`StopMusic` actions, no new systems
   needed. Needed for the village-safe vs. field-tense audio gradient. Check the backlog entry
   directly for current status rather than re-grepping the schema — it's the source of truth now.

**How to apply:** item 1 can be designed around as shipped. For items 2 and 3, check
`planning/backlog.md` L106/L108 for current status before designing around them as if they exist —
they may have shipped since 2026-09-03. Related: [[project-greywatch-world]].
