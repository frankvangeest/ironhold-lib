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
| `FsmState`/`FsmEventBinding` → `logic/state_machine.ron` | `project_loader.rs:175` (line numbers as of the original 2026-09-04 review) | pathless `warn!` |
| `StateMachineAsset` → `behaviors/*.behavior.ron` | `entity_spawner.rs:559` (`resolve_pending_behaviors_system`) | **no `Failed` arm at all** — entity keeps `PendingBehavior` forever, zero log at any level |
| `ActionSlotDef.do_actions` → `scenes/*.scene.ron` | `scene_loader.rs` `spawn_scene_v2` | **no `Failed` arm** — `ready_to_spawn` never becomes true, app sits in `AppState::LoadingScene` indefinitely |
| `DialogueChoiceDef.do_actions` → `dialogues/*.dialogue.ron` | `capabilities/dialogue.rs:137` | **no `Failed` arm** — `dialogue_assets.get()` returns `None`, silent `return` every frame; `ActiveDialogue` stays `is_active()`, and auto-wire is gated on `!is_active()`, so that NPC's conversation is dead until the next scene load. Does **not** touch `panels_open`, so it is not a global input lock |

(Historical: a fifth row, `LogicRule` → `logic/rules.ron` via `project_loader.rs:166`'s matching
pathless `warn!`, existed at the time of this review. `LogicRule`/`rules.ron` were removed outright
in `rules_to_state_machine_consolidation`, 2026-09-28 — there are only four Action-bearing loader
paths now.)

Note the in-file inconsistency worth citing: `project_loader.rs`'s catalog arms two blocks below
(lines 187/199/211/223 as of the original review) already do it right — `error!` + resolved path +
the error `e`. The model_fixes/state_machine arms are the odd ones out.

**Related structural gap found at the same time:** `Action`'s own leaf field types are all
primitives plus the unit enum `QualityLevel`, so `Action` needs no recursive follow-up — but its
*immediate parents in the very same files* still lack the attribute and still silently swallow
typos: `StateMachineAsset` (project.rs:67), `FsmState` (:127), `FsmTransition` (:141),
`FsmEventBinding` (:152), `DialogueDef` (dialogue.rs:9), `DialogueCondition`
(dialogue.rs:60). (`LogicRule` was a sixth instance of this same gap at the time — moot now that
the type itself no longer exists, removed alongside `rules.ron` in
`rules_to_state_machine_consolidation`, 2026-09-28. All of `StateMachineAsset`/`FsmState`/
`FsmTransition`/`FsmEventBinding` now carry `#[serde(deny_unknown_fields)]` themselves — verify
before citing this paragraph as still describing a live gap for those four; it may only still
apply to `DialogueDef`/`DialogueCondition`.) A typo'd `event:` on an `FsmEventBinding`
(`schema/project.rs`:137-140) is a *worse* silent bug than the Action-field typo this feature
closed — the binding simply never matches anything, with no parse error and no runtime
diagnostic (state scoping is now structural — which list a binding sits in, `global_on` vs a
state's own `on:` — not a separate condition field to typo the way `LogicRule.when` once was).

**How to apply:** when reviewing any schema-tightening change, (1) enumerate every file kind that
can contain the type, (2) check each loader for a `LoadState::Failed` / `Assets::get() == None` arm,
and (3) check the type's immediate *parent* structs for the same attribute — tightening a leaf while
its parent stays permissive moves the silent-typo surface up one level rather than eliminating it.
See also [[cli-validate-coverage-model]] for the design-time half of the same question.
