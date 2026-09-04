---
name: schema-tightening-blast-radius
description: deny_unknown_fields (and any stricter-parse change) converts a silent field drop into a whole-FILE parse failure; the five Action-bearing loader paths handle that failure very unevenly, and three of them fail silently forever
metadata:
  type: project
---

Adding `#[serde(deny_unknown_fields)]` (or any parse-tightening) to a schema type does not just
"produce a clear error" — it **changes the granularity of the failure from one field to the whole
file**. The review question is therefore never only "does existing RON still parse?" (the usual
compatibility sweep) but "**what does the loader for each file that can contain this type do when
that file fails to parse?**"

**Why:** established while reviewing `feature/action-deny-unknown-fields` (2026-09-04, commit
`4b8c865`, which added the attribute to `schema/actions.rs`'s `Action`). The change itself is
correct and all shipped RON stayed green, but `Action` is embedded in five different file kinds and
their loaders' failure handling ranges from decent to nonexistent:

| Container / file | Loader | Behavior on parse failure |
|---|---|---|
| `LogicRule` → `logic/rules.ron` | `runtime/scene_manager/project_loader.rs:166` | `warn!("rules failed to load — proceeding without it")` — pathless, discards the serde error `e` via `Failed(_)` |
| `FsmState`/`FsmEventBinding` → `logic/state_machine.ron` | `project_loader.rs:175` | same pathless `warn!` |
| `StateMachineAsset` → `behaviors/*.behavior.ron` | `entity_spawner.rs:559` (`resolve_pending_behaviors_system`) | **no `Failed` arm at all** — entity keeps `PendingBehavior` forever, zero log at any level |
| `ActionSlotDef.do_actions` → `scenes/*.scene.ron` | `scene_loader.rs` `spawn_scene_v2` | **no `Failed` arm** — `ready_to_spawn` never becomes true, app sits in `AppState::LoadingScene` indefinitely |
| `DialogueChoiceDef.do_actions` → `dialogues/*.dialogue.ron` | `capabilities/dialogue.rs:137` | **no `Failed` arm** — `dialogue_assets.get()` returns `None`, silent `return` every frame; `ActiveDialogue` stays `is_active()`, and auto-wire is gated on `!is_active()`, so that NPC's conversation is dead until the next scene load. Does **not** touch `panels_open`, so it is not a global input lock |

Note the in-file inconsistency worth citing: `project_loader.rs`'s catalog arms two blocks below
(lines 187/199/211/223) already do it right — `error!` + resolved path + the error `e`. The
model_fixes/rules/state_machine arms (157/166/175) are the odd ones out.

**Related structural gap found at the same time:** `Action`'s own leaf field types are all
primitives plus the unit enum `QualityLevel`, so `Action` needs no recursive follow-up — but its
*immediate parents in the very same files* still lack the attribute and still silently swallow
typos: `StateMachineAsset` (project.rs:67), `FsmState` (:127), `FsmTransition` (:141),
`FsmEventBinding` (:152), `LogicRule` (:310), `DialogueDef` (dialogue.rs:9), `DialogueCondition`
(dialogue.rs:60). A typo'd `whn:` on a `LogicRule` is a *worse* silent bug than the Action-field
typo this feature closed (the state guard silently becomes `None`, so the rule fires in every
state).

**How to apply:** when reviewing any schema-tightening change, (1) enumerate every file kind that
can contain the type, (2) check each loader for a `LoadState::Failed` / `Assets::get() == None` arm,
and (3) check the type's immediate *parent* structs for the same attribute — tightening a leaf while
its parent stays permissive moves the silent-typo surface up one level rather than eliminating it.
See also [[cli-validate-coverage-model]] for the design-time half of the same question.
