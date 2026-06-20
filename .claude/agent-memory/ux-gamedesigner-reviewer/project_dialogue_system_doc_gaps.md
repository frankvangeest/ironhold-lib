---
name: dialogue-system-doc-gaps
description: Dialogue system (DialogueDef) recurring doc gaps — hint_text undocumented, condition absent-key semantics, end-of-nodes behaviour
metadata:
  type: project
---

The NPC dialogue system (`dialogues/*.dialogue.ron`, schema v1) is documented in `docs/20_data_formats.md` (DialogueDef section ~line 1943, DialoguePanel ~line 685, actions appendix ~1937) and `docs/30_runtime_events_and_logic.md` (events ~122). Canonical example: `3rd_person_game_demo` — prefab `friendly_npc_male`, scene entity id `npc_01`, dialogue file `dialogues/npc_intro.dialogue.ron`, panel id `npc_dialogue_panel`.

Recurring gaps observed at review (2026-06):
- **`hint_text` on `InteractableDef` is undocumented.** Ships in `friendly_npc_male` prefab (`interactable: (radius, hint_text: "Talk")`) but the PrefabDef `interactable` row only documents `radius: f32`. Check this whenever interactable/dialogue prefabs change.
- **`dialogue.started:{npc_id}` payload is the scene-placed spawn id** (e.g. `npc_01`), NOT the prefab key. Docs example historically used a non-existent `npc_guard_01`.
- **`dialogue.ended:{dialogue_path}` is keyed by file path, not npc id** — multi-NPC scenes sharing one `.dialogue.ron` cannot distinguish which NPC closed it via the ended event.
- **`DialogueCondition` absent-key semantics unexplained** — whether an unset GameVariable compares equal to `""` (HasVariable) or `0` (VariableGte) is not stated, which breaks the shipped `HasVariable(value:"")` example on a fresh run.
- **Shipped `npc_intro.dialogue.ron` has no `do_actions`/`condition`** — only the docs snippet shows them, so there is no runnable example of the quest-variable round-trip.

**Why:** these are the designer-facing artifacts; a non-programmer copies examples verbatim and has no source/error feedback when an id or field is wrong.
**How to apply:** when reviewing dialogue changes, cross-check hint_text, the spawn-id-vs-prefab distinction, and condition absent-key behaviour against the shipped example, not just the prose.
