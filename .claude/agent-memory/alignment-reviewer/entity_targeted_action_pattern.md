---
name: Entity-targeted action pattern (six-touchpoint checklist)
description: When adding an Action variant that targets a single entity by spawn ID, every reviewer must check these six locations or the {self} substitution silently breaks in behavior files
type: project
---

For any new `Action` variant that takes an `entity` / `target` / `event` string referring to a specific spawned entity (e.g. `ShowDamagePopup`, `SetEntityVisible`, `EmitEventAfterDelay`, `PlayAnimationOn`, `Despawn`, `SpawnEffect`), all six of these locations must be updated. Missing #4 in particular silently breaks designer-authored behavior files — the action parses, executes for non-`{self}` targets, but always misses for behavior-driven targets.

**Variants with optional entity reference** (e.g. `SpawnEffect { entity: Option<String> }`) need the same treatment — wrap the `replace` in `entity.map(|e| e.replace("{self}", spawn_id))`. The compiler will not flag the omission since `Option::None` is a valid value; only runtime tests catch it.

1. **`schema/actions.rs`** — add the variant with a doc comment explaining `{self}` semantics and a designer-facing RON example.

2. **`runtime/scene_manager/action_executor.rs`** — match arm that resolves `entity` via `spawn_params.registry.entities.get(&entity_id).copied()` and warns when missing.

3. **`runtime/scene_manager/message_interpreter.rs::rewrite_self`** — add a match arm that replaces `{self}` in the entity/target string before the action is pushed onto `ActionQueue`. ALL action variants that take entity references MUST appear in this function, or `{self}` will be passed through verbatim and the executor's registry lookup will fail at runtime.

4. **`crates/ironhold_core/src/CLAUDE.md`** — append a line to the "Supported `{self}` targets in actions" list. This is the designer-facing contract.

5. **`tests/ron_validation.rs`** — add a parse test (`from_str::<Action>("…")`) to prove the variant deserializes correctly.

6b. **`capabilities/action_bar.rs::action_needs_target`** — *the `{target}` twin of #3, and the one
   nobody remembers.* This is a hand-maintained allowlist `match` deciding whether a slot's action
   requires a live target (→ emit `action_bar.no_target` and refuse to fire). It has already drifted
   out of sync with `message_interpreter.rs::rewrite_target`: as of 2026-09-14 `rewrite_target`
   substitutes `{target}` in 16 variants but `action_needs_target` only recognizes 11, missing
   `SetVariable`, `ResetToSpawn`, `AddItem`/`RemoveItem`/`TransferItem`, `OpenShop`,
   `OpenContainer`, `EmitEventAfterDelay`, and `Spawn`'s `id`/`spawn_point` (only `at_entity` is
   checked). Consequence for a designer: an action-bar slot using `{target}` in any unlisted field
   fires with no target, and `action_bar.rs:228`'s `player_target.0.as_deref().unwrap_or("")`
   silently rewrites `{target}` → `""` — no event, no warning. Any new `{target}`-capable field must
   be added here too, or the right fix is to derive this from `rewrite_target` (return a `changed`
   bool) instead of duplicating the arm list a third time.

6. **The matching domain test file** (`tests/{domain}_tests.rs` — e.g. `action_tests.rs`, `spawn_tests.rs`; see `tests/CLAUDE.md`'s file layout table) — add a behavior test pushing the action onto `ActionQueue`, calling `app.update()`, and asserting the side effect (component changed, event emitted, etc.). `integration_tests.rs` was split into domain files 2026-07-02 — do not recreate it.

## Patterns that signal a problem when reviewing such an action

- The variant appears in `actions.rs` and `action_executor.rs` but NOT in `rewrite_self` — designer can write it in `state_machine.ron` only, not in a `.behavior.ron` file with `{self}`.
- The match arm in the executor uses a raw `Entity` ID — should be `&str` that hits the registry.
- The action mutates state directly (e.g. `Commands::despawn`) instead of going through `commands.entity(e).insert(...)` — both are fine, but inserting components keeps the ECS reactive (e.g. `Visibility::Hidden` cascades to label observers through `world_label_screen_pos_system`).
- The action pushes side-channel resource state (like `DelayedEventQueue`) — that resource MUST be cleared in `Action::LoadScene` alongside `preloaded`, `preloaded_glbs`, and `pending_spawns`, or stale entries fire after scene transitions.

## Designer-reachability test

A designer must be able to write a `behaviors/{name}.behavior.ron` file that does:
```ron
states: [(
  name: "dead",
  entry_actions: [ NewAction(entity: "{self}", …) ],
  …
)]
```
…and have `{self}` correctly substituted with the spawn ID at runtime. If they have to use literal `entity: "dummy_01"` (no `{self}` magic), the action is not fully designer-reachable for reusable prefabs.
