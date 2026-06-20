---
name: dialogue-system
description: Architecture of the dialogue capability — catalog-vs-structural-file distinction, auto-wire seam, dual condition system, known invariants
metadata:
  type: project
---

The dialogue system (`capabilities/dialogue.rs` + `schema/dialogue.rs`) is the engine's conversation feature. Key architectural facts worth remembering across reviews:

**Catalog vs structural-file distinction (important general principle).** `dialogue_path`, `LoadScene(String)`, and prefab `model` paths are *structural project files* referenced by project-relative path and resolved via `resolve_project_path` — they are NOT catalog-keyed and do NOT violate the no-hardcoded-assets rule. The AssetCatalog only holds *content assets*: `models`/`textures`/`audio`/`materials`/`decals`. So when reviewing a "raw path" reference, first ask: is this a structural file (scene/prefab/dialogue) or a content asset (texture/audio/model/decal)? Only the latter must go through `LoadedAssetCatalog`. The dialogue `portrait` field IS a texture catalog key and therefore MUST resolve through `LoadedAssetCatalog.textures`.

**Auto-wire seam.** `dialogue_tick_system` (Update, `.after(button_system).after(interactable_system).before(message_interpreter_system)`) detects `entity.interacted:{id}` for entities carrying `DialoguePath` and pushes `Action::StartDialogue` to ActionQueue — it does NOT mutate ActiveDialogue directly. The executor is the only writer of dialogue *lifecycle* state. This keeps the seam designer-interceptable in state_machine.ron. `ActiveDialogue` is cleared in the LoadScene executor arm.

**Dual condition system (deliberate deviation).** `DialogueCondition` (HasVariable/VariableGte/StatAtLeast) is a second condition vocabulary separate from the FSM `when:`/LogicState guards. Accepted because per-choice visibility filtering cannot be expressed by the global FSM state. Do NOT try to unify them; do NOT add a third dialect — extend DialogueCondition if more conditions are needed.

**Known invariant — auto-advance only when no choices.** A node with non-empty `choices` must NEVER set `auto_advance_timer`, or a same-frame auto-advance can desync a late choice click (click routes into the new node's choices with the old index). If you see `auto_advance_timer = node.advance_delay_secs` set unconditionally, that is the bug.

See [[schedule-update-vs-fixedupdate]] for why the Update ordering is correct, and [[capability-patterns]] for the action-touchpoint checklist.
