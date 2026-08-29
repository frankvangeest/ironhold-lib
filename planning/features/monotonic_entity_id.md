# Feature: Monotonic per-entity id generation for RON action substitution

_Status: Ready_
_Planned at: `2c3e035` (2026-08-29)_

## What
Adds a `{new_id}` substitution token, usable inside `Action::Spawn`'s `id: Option<String>` field
alongside the existing `{self}`/`{target}` tokens. `{new_id}` resolves to a small, monotonically
increasing integer, unique for the lifetime of the current scene, letting a designer compose a
guaranteed-unique spawn id (e.g. `id: "{self}_corpse_{new_id}"`) instead of relying on a fixed,
reused literal.

## Why
Confirmed via `monster_corpse_loot.md` v2 playtest (2026-08-26): a respawned monster always
reuses its original spawn id (e.g. `"zombie_02"`), so a corpse id derived purely from `{self}`
(`"{self}_corpse"`) is also always the same literal string across every generation of that slot.
The shipped workaround is a `Despawn("{self}_corpse")` guard immediately before every corpse
`Spawn`, which sacrifices an still-mid-decay older corpse early if the same slot is killed again
before that corpse decays/is looted — an accepted, documented tradeoff (see
`crates/ironhold_core/src/CLAUDE.md`'s "Corpse id collisions..." section), not a fix. `{new_id}`
lets a future revision of that behavior (or any other feature needing per-spawn uniqueness) compose
an id that can never collide, without needing its own bespoke counter mechanism.

Backlog entry: `planning/backlog.md` ▸ Bugs ▸ "Monotonic per-entity id generation for RON action
substitution".

## Approach
`Action::Spawn.id: Option<String>` is the **only** field of that type on any `Action` variant
(verified against `schema/actions.rs`) — so this is scoped to exactly one field, not a general
substitution-engine change.

**Reuse the existing `SpawnRegistry.counter: u64`** (`runtime/scene_manager/mod.rs`) rather than
introduce a new resource. It already backs the auto-generated-id fallback
(`action_executor.rs`'s `Action::Spawn` handler: `id.unwrap_or_else(|| { registry.counter += 1;
format!("{}_{}", prefab, registry.counter) })`) and is reset to `0` on every `LoadScene` — which is
safe here for the same reason it's already safe for the fallback: every entity from the prior scene
(corpses included) is despawned on `LoadScene` via the `LevelEntity` teardown, so id uniqueness only
ever needs to hold *within* one loaded scene's lifetime, not across a scene transition.

**Resolve `{new_id}` at the single point `Action::Spawn.id` is consumed** — inside
`action_executor.rs`'s `Action::Spawn` arm, right where `spawn_id` is currently computed — not in
`rewrite_self`/`rewrite_target` (`message_interpreter.rs`) or dialogue's `substitute_self_in_action`.
Reasons:
- Those functions are pure value transforms with no access to a mutable counter resource; threading
  `ResMut<SpawnRegistry>` through all four call sites (`message_interpreter_system`,
  `fsm_interpreter_system`, `entity_fsm_interpreter_system`, `dialogue_tick_system`) for a token used
  by exactly one field would be a much bigger change than the field warrants.
- `{self}`/`{target}` substitution already runs before the action reaches the executor, so an
  authored `id: "{self}_corpse_{new_id}"` arrives at the executor as `"zombie_02_corpse_{new_id}"`
  — `{new_id}` passes through both existing substitution passes untouched (they only ever look for
  their own literal token) and is resolved exactly once, at the last possible moment, by the code
  that already resolves the fallback-generated case.

```rust
// action_executor.rs, replacing the current `id.unwrap_or_else(...)`:
let spawn_id = match id {
    Some(raw) if raw.contains("{new_id}") => {
        spawn_params.registry.counter += 1;
        raw.replace("{new_id}", &spawn_params.registry.counter.to_string())
    }
    Some(raw) => raw,
    None => {
        spawn_params.registry.counter += 1;
        format!("{}_{}", prefab, spawn_params.registry.counter)
    }
};
```

No schema change (no new field — `{new_id}` is a token inside an existing `String`, exactly like
`{self}`/`{target}`), no new `Action` variant, no new resource.

## Tasks
- [ ] Implement `{new_id}` resolution in `action_executor.rs`'s `Action::Spawn` arm (above).
- [ ] Doc comment update on `Action::Spawn` in `schema/actions.rs` (mention `{new_id}` alongside
      the existing `{self}`/`{target}` support note for `at_entity`).
- [ ] `crates/ironhold_core/src/CLAUDE.md` — add `{new_id}` to the "Supported `{self}` targets in
      actions" list (it's technically independent of `{self}`, but designers will look for it in
      the same place), with a short note on why it's scene-scoped, not session-scoped.
- [ ] `docs/20_data_formats.md` — document `{new_id}` in the `Action::Spawn` section.
- [ ] Tests (`crates/ironhold_core/tests/entity_logic_tests.rs` or `spawn_tests.rs`, whichever
      already covers `Action::Spawn`'s id-generation fallback): two back-to-back
      `Spawn(id: "corpse_{new_id}")` actions in the same test produce two distinct, non-colliding
      spawn ids; combined with `{self}` in the same string resolves both correctly
      (`"{self}_corpse_{new_id}"` → e.g. `"zombie_02_corpse_1"`).
- [ ] `cargo check -p ironhold_cli` spot-check — no schema shape changed, but confirm `query
      actions`/`validate` don't choke on the new doc comment or literal token (they don't inspect
      `id` string contents today, so this should be a no-op check).

## Open questions
- None — approach is fully determined by the existing `SpawnRegistry.counter` precedent.

## Acceptance criteria
- Given a rule/behavior with `Spawn(prefab: "x", id: "{self}_corpse_{new_id}")` fired twice for the
  same `{self}` value in one scene, when both spawns execute, then the two resulting spawn ids are
  different and both resolve correctly in `SpawnRegistry`.
- Given `Spawn(id: "loot_{new_id}")` with no `{self}`/`{target}` involved, the id still resolves to
  a unique literal each time.
- Given the pre-existing `id: None` fallback path, its behavior and output format are completely
  unchanged (same shared counter, same `"{prefab}_{n}"` format when omitted entirely).
- A scene reload resets the counter (matches existing, documented `SpawnRegistry` behavior) —
  this is fine since no entity carrying a `{new_id}`-derived id can survive a `LoadScene` anyway.
