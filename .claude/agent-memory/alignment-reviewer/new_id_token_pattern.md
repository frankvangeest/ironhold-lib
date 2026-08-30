---
name: new-id-token-pattern
description: The {new_id} RON substitution token on Action::Spawn.id — executor-side resolution, the four doc sites, and the "unaddressable id" constraint designers hit next
metadata:
  type: project
---

`{new_id}` (feature/monotonic-entity-id, 2026-08-29) is the third RON substitution token after
`{self}`/`{target}`, and the only one resolved **executor-side** rather than interpreter-side.

## Shape (reuse this for any future counter/generator token)

- Scoped to exactly **one field**: `Action::Spawn.id: Option<String>` — verified to be the only
  `Option<String>` id-style field on any `Action` variant. No schema change, no new resource, no
  new variant.
- Resolved in `action_executor.rs`'s `Action::Spawn` arm, in the same `match` that already
  computes the `id: None` fallback, sharing `SpawnRegistry.counter`.
- Counter reset (`scene_loader.rs`, `spawn_registry.counter = 0`) sits inside the **non-overlay**
  branch, so `LoadSceneOverlay` does not reset it. Uniqueness only needs to hold within one loaded
  scene — correct, since `LevelEntity` teardown removes every prior-scene entity.

**Why executor-side is the right call, and its bonus:** `rewrite_self`/`rewrite_target`
(`message_interpreter.rs`) and `dialogue.rs::substitute_self_in_action` are pure value transforms
with no mutable counter access. Resolving late means `{new_id}` works from **all** action sources
including dialogue `do_actions`, where `{self}` silently does *not* (dialogue's substitution fn
still has no `Action::Spawn` arm — it falls through `other => other`). Interpreter-side tokens only
replace their own literal, so `{new_id}` passes through both passes untouched.

## The constraint designers hit next (document it before they do)

An id containing `{new_id}` is **unaddressable by literal from any other RON file**. It can only be
reached by:
- `{self}` inside the spawned entity's own `.behavior.ron` (this is why the fully-`{self}`-relative
  `lootable_corpse.behavior.ron` works), or
- `{target}` if the prefab is targetable.

A `Despawn("thing_{new_id}")` or `EmitEventAfterDelay(event: "corpse.decay:thing_{new_id}")` in
`rules.ron`/`state_machine.ron` gets a **silent literal** — `{new_id}` is not substituted anywhere
outside `Spawn.id`, and nothing warns. `ironhold_cli`'s `validate.rs` matches
`Action::Spawn { prefab, .. }` and never inspects `id`, so there is no CLI guard either. A
"`{new_id}` used outside `Spawn.id`" validate error is the obvious cheap fix.

## Doc sites (four, not three)

Updated in the shipping branch: `schema/actions.rs` Spawn doc comment,
`crates/ironhold_core/src/CLAUDE.md` (own `**{new_id}` substitution**` section after the `{self}`
targets list), `docs/20_data_formats.md` Spawn row.
**Missed:** `docs/30_runtime_events_and_logic.md` — *two* sites, its `Spawn { ... }` action bullet
(~line 294) and its `### {self} substitution rules` list (~line 429). That file is the designer-
facing mirror of the CLAUDE.md list; check both files whenever a substitution token changes.

## Test-coverage note

`spawn_tests.rs`'s two `{new_id}` tests build `Action::Spawn` Rust literals with pre-substituted
prefixes (`"orc_corpse_{new_id}"`), so they never exercise the `{self}` + `{new_id}` interaction
that the feature plan listed as an acceptance criterion. The ordering invariant worth pinning is
that interpreter-side `{self}` substitution runs first and leaves `{new_id}` intact.
