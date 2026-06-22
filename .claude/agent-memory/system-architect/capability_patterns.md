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
5. `rewrite_self()` AND `rewrite_target()` in `message_interpreter.rs` — must handle `{self}`/`{target}` substitution in any field that holds a spawn ID
6. `crates/ironhold_core/src/CLAUDE.md` — add to the `{self}` targets list

**Recurring anti-pattern — substitution-enumeration trap:** `rewrite_self()` and `rewrite_target()` in `message_interpreter.rs` are explicit `match` over Action variants ending in `other => other`. Any new entity-targeted action that is NOT added to both match arms silently passes through with literal `"{self}"`/`"{target}"` strings — so it works from global rules.ron but is unreachable from behavior files and dialogue choices, with no compile error and no warning. Observed concretely in the inventory system (AddItem/RemoveItem/TransferItem/OpenShop all omitted). ALWAYS check both functions when reviewing a new entity-targeted action.

## Rules.ron vs state_machine.ron

- `rules.ron` — simple event→action mapping; no state tracking. Use for projects where all events trigger the same response regardless of game state.
- `state_machine.ron` — FSM with named states, entry/exit actions, and `when:` condition guards. Use when behavior depends on the current game state (e.g., playing vs paused, hp_low vs hp_ok).
- Both can coexist: rules.ron fires unconditionally; state_machine.ron fires in context. The interpreter chain runs both: `message_interpreter_system` then `fsm_interpreter_system`.

## Schema stability rules

- **Additive change** (new optional field with `#[serde(default)]`): backward-compatible. Existing RON files still parse.
- **Rename/removal**: breaking change. All existing RON files referencing the old name will fail to parse. Requires a migration plan.
- **Type change** (e.g., `f32` → `Vec2`): breaking change even if field name stays the same.
- Rule of thumb: always add, never rename unless you're auditing and updating all usages.

## Physics and movement

All player movement, physics processing, and camera-follow logic must run in `FixedUpdate`. Systems in `Update` that read physics state cause visible stuttering. Rapier3D is the physics backend.

## Inspector feature gate

`bevy_egui` inspector code must be gated behind `#[cfg_attr(feature = "inspector", ...)]`. Never mix inspector UI with game UI cameras. The inspector uses its own camera/layer.
