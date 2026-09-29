---
name: capability-patterns
description: Standard patterns for adding new capabilities, actions, and events; consult before advising on any new gameplay system
metadata:
  type: project
---

## Adding a new capability

1. Create `capabilities/{name}.rs` with a Bevy Plugin struct.
2. Register in `capabilities/mod.rs` and `lib.rs` (GamePlugin).
3. Emit typed messages (`GameEvent::Trigger`, `UiEvent::ButtonPressed`, `SceneEvent::*`) — never push to ActionQueue.
4. If the capability has configurable parameters, add a schema type in `schema/` with `#[derive(Deserialize)]` and `#[serde(default)]` on all optional fields.
5. Expose the schema type from a scene RON or prefab field so a designer can activate it without code.

## Adding a new Action

Four required touchpoints:
1. `schema/actions.rs` — add the variant with a doc comment
2. `action_executor.rs` — add a `match` arm
3. Ensure `#[derive(Deserialize)]` covers inner types
4. Document in `docs/20_data_formats.md` (actions table), `docs/30_runtime_events_and_logic.md` (appendix), and `docs/STATUS.md` (Engine ABI)

Entity-targeted actions (those that reference a spawn ID) need two additional touchpoints:
5. `rewrite_self()` AND `rewrite_target()` in `crates/ironhold_core/src/runtime/scene_manager/action_substitution.rs` — must handle `{self}`/`{target}` substitution in any field that holds a spawn ID
6. `crates/ironhold_core/src/CLAUDE.md` — add to the `{self}` targets list

**Recurring anti-pattern — substitution-enumeration trap:** `rewrite_self()` and `rewrite_target()` in `action_substitution.rs` (`runtime/scene_manager/action_substitution.rs`) are explicit `match` over Action variants ending in `other => other`. Any new entity-targeted action that is NOT added to both match arms silently passes through with literal `"{self}"`/`"{target}"` strings — so it works from a global `state_machine.ron` `global_on:` handler but is unreachable from behavior files and dialogue choices, with no compile error and no warning. Previously observed concretely in the inventory system (AddItem/RemoveItem/TransferItem/OpenShop were all omitted) — this has since been fixed; all four are now handled in both match arms (confirmed in `crates/ironhold_core/src/CLAUDE.md`'s `{self}` targets list). ALWAYS check both functions when reviewing a new entity-targeted action — the trap itself (silent pass-through, no warning) is still real even though the specific inventory-action instance of it was fixed.

**Live third instance of the same trap (found 2026-09-16 triage):** there is a *third* hand-maintained enumeration that must stay in sync with `rewrite_target()` — `action_needs_target()` in `capabilities/action_bar.rs` (the gate that decides whether a slot press emits `action_bar.no_target:{key}` instead of firing). It covers strictly fewer variants than `rewrite_target` does. Variants `rewrite_target` substitutes but `action_needs_target` does **not** gate: `SetVariable`, `EmitEventAfterDelay`, `ResetToSpawn`, `AddItem`, `RemoveItem`, `TransferItem`, `OpenShop`, `OpenContainer`, and `Spawn.id`/`Spawn.spawn_point` (only `Spawn.at_entity` is gated). Consequence: a slot whose only `{target}` use is one of those fires with an **empty** spawn id and the designer gets no `no_target` event — a silent no-op instead of a diagnosable one. Not biting any shipped project (`local_coop_demo`, the only project using `owner_player` bars, only uses `ModifyStat`/`ShowDamagePopup`, both gated). When reviewing a new entity-targeted action, check **three** enumerations, not two.

## state_machine.ron is the only logic format

`rules.ron` (a simpler event→action mapping with no state tracking) was removed outright in
`rules_to_state_machine_consolidation` (shipped 2026-09-28) — see [[rules_vs_fsm_consolidation]]
for the removal rationale and traps. Every project now authors logic exclusively in
`state_machine.ron`: `global_on` for state-independent event→action bindings, per-state `on:`
handlers for state-dependent behavior, and `transitions` for state changes. Interpreted by
`fsm_interpreter_system` (scene-level) and `entity_fsm_interpreter_system` (per-entity/NPC).
`ProjectConfig.rules`/`rules_path` and `Action::EnterState` no longer exist — a state change is a
`transitions` entry driven by an emitted event, not a direct action.

## Feature spec splitting

When drafting a feature plan, **actively look for scope seams** and split into separate feature files when you find them. A seam exists when:
- Part of the work is a small doc/config/default fix that can land independently (e.g. a "washed out icons" doc fix vs. a full shader feature).
- Part requires schema/runtime changes and part is tools-only — they carry different risk profiles and review requirements.
- Part is a hard dependency and part is a polish follow-up (e.g. "draggable windows" and "cursor grab icon").
- The combined scope would be too large to review, play-test, and commit in one pass.

When you split, say so explicitly in your response: name each part, explain the seam, and write a separate `.md` file per part. Do not silently combine them into one file.

## Schema stability rules

- **Additive change** (new optional field with `#[serde(default)]`): backward-compatible. Existing RON files still parse.
- **Rename/removal**: breaking change. All existing RON files referencing the old name will fail to parse. Requires a migration plan.
- **Type change** (e.g., `f32` → `Vec2`): breaking change even if field name stays the same.
- Rule of thumb: always add, never rename unless you're auditing and updating all usages.

## Physics and movement

All player movement, physics processing, and camera-follow logic must run in `FixedUpdate`. Systems in `Update` that read physics state cause visible stuttering. Rapier3D is the physics backend.

## Inspector feature gate

`bevy_egui` inspector code must be gated behind `#[cfg_attr(feature = "inspector", ...)]`. Never mix inspector UI with game UI cameras. The inspector uses its own camera/layer.
