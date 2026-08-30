# Feature: Monotonic per-entity id generation for RON action substitution

_Status: Done_
_Planned at: `2c3e035` (2026-08-29)_
_Reviewed by system-architect, alignment-reviewer, debug-detective (2026-08-29) — findings folded
in below; see `planning/claude_suggestions.md` for the deferred/follow-up items._
_Playtest confirmed by Frank (2026-08-30), `entity_logic_demo`'s "respawning gem" demo — see
Tasks below for the 3 real issues the playtest itself caught (residue spawn silently rejected,
residues all stacking at one position, UI label overlap) and how each was fixed._

## What
Adds a `{new_id}` substitution token, usable inside `Action::Spawn`'s `id: Option<String>` field
alongside the existing `{self}`/`{target}` tokens. `{new_id}` resolves to a small, monotonically
increasing integer, unique among ids produced by `{new_id}`/the auto-generated fallback within the
current scene, letting a designer compose an id (e.g. `id: "{self}_corpse_{new_id}"`) that won't
reuse the same literal as an earlier spawn from the same source. It is **not** an absolute
uniqueness guarantee against a hand-authored literal id of the same shape — see Approach.

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
`Action::Spawn.id: Option<String>` is the only `Option<String>` field where minting a *new*
identity is meaningful — the other three `Option<String>` fields on `Action` variants
(`Spawn.spawn_point`, `SpawnEffect.entity`, `ProjectDecal.entity`) all *reference* an existing
name, so `{new_id}` would be meaningless there (correction from the first draft, which
mis-stated this as "the only `Option<String>` field on any `Action` variant" — system-architect
review). This is scoped to exactly one field, not a general substitution-engine change.

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

**Review findings folded in:**
- **Not an absolute uniqueness guarantee (debug-detective, blocking).** `SpawnRegistry.entities`
  is one flat namespace shared by scene-placed and dynamically-spawned entities; the counter
  resets to 0 per scene, so a designer-chosen short prefix (`id: "crate_{new_id}"`) can still
  collide with a literal scene-authored id (`"crate_1"`). Docs now state this precisely instead of
  claiming absolute uniqueness. `action_executor.rs` now `warn!`s if the resolved id is already
  registered (this also catches the pre-existing plain-literal-collision case, not just `{new_id}`
  misuse) — see `test_spawn_id_collision_orphans_old_entity` for the pre-existing silent-orphan
  behavior this diagnostic now surfaces.
- **Unresolved-token diagnostic (debug-detective).** A typo'd `{new_id}` (e.g. `{newid}`), or
  `{self}`/`{target}` authored somewhere they don't resolve (dialogue `do_actions`; see below),
  used to bake literal braces into a live spawn id with zero diagnostic. `action_executor.rs` now
  `warn!`s whenever the resolved `spawn_id` still contains `{`.
- **Unaddressability (system-architect).** An id built with `{new_id}` can't be referenced by
  literal string from another RON file — only the entity's own behavior (`{self}`) or `{target}`
  can address it afterward — documented in all four doc surfaces (`schema/actions.rs`,
  `crates/ironhold_core/src/CLAUDE.md`, `docs/20_data_formats.md`,
  `docs/30_runtime_events_and_logic.md`).
- **Dialogue-path gap (all three reviewers).** `capabilities/dialogue.rs`'s
  `substitute_self_in_action` has no `Action::Spawn` arm, so `{self}` doesn't resolve inside a
  dialogue choice's `Spawn.id` — pre-existing, unrelated to this feature, but the new doc comment
  originally implied unconditional `{self}` support. Now qualified in all four doc surfaces;
  fixing the dialogue gap itself is out of scope here (logged to `claude_suggestions.md`).
- **`docs/30_runtime_events_and_logic.md` was missed in the first pass** (alignment-reviewer) —
  now updated (the `Spawn` action bullet, plus a new `### {new_id} substitution` section; the
  `{self}` substitution example list's `Spawn` line now uses `{new_id}` instead of the exact
  colliding pattern this feature exists to replace).
- **Determinism note (system-architect).** A `{new_id}`/auto-generated spawn id is unique per
  scene but not reproducible across runs/framerates (its value depends on `ActionQueue` execution
  order) — added to `docs/40_determinism_and_networking.md` so it's never mistaken for a valid
  save/network-sync key.
- **`ironhold_cli validate` guard (alignment-reviewer + debug-detective).** `{new_id}` used
  anywhere other than `Spawn.id` used to pass validation silently and fail only as a confusing
  runtime no-op. Added a `misplaced_new_id_token` cross-file check (`validate.rs`) plus fixture
  `bad_new_id_placement` and test `new_id_token_outside_spawn_id_exits_1`.
- **End-to-end interpreter test (system-architect + debug-detective, the one gap both singled out
  as worth holding the feature for).** The original two tests pushed `Action::Spawn` directly onto
  `ActionQueue`, never exercising the real ordering invariant this design rests on (`{self}`
  resolved by the interpreter, `{new_id}` resolved later by the executor, passing through intact
  in between). Added `test_entity_fsm_new_id_composes_with_self_substitution`
  (`entity_logic_tests.rs`), driving a real behavior FSM through two kills of the same monster
  slot and asserting the two corpse ids differ.

## Tasks
- [x] Implement `{new_id}` resolution in `action_executor.rs`'s `Action::Spawn` arm.
- [x] Implement the collision and unresolved-token `warn!` diagnostics (added post-review).
- [x] Doc comment update on `Action::Spawn` in `schema/actions.rs`.
- [x] `crates/ironhold_core/src/CLAUDE.md` — `{new_id}` substitution section (uniqueness scope,
      unaddressability, dialogue-path gap all documented per review).
- [x] `docs/20_data_formats.md` — documented in the `Action::Spawn` table row.
- [x] `docs/30_runtime_events_and_logic.md` — Spawn bullet + new `{new_id}` subsection (missed in
      the first pass, added after alignment-reviewer flagged it).
- [x] `docs/40_determinism_and_networking.md` — non-reproducibility note (added post-review).
- [x] Tests: `spawn_tests.rs`'s two direct-`ActionQueue` tests, plus
      `entity_logic_tests.rs`'s real-interpreter end-to-end test (added post-review).
- [x] `ironhold_cli validate` guard for `{new_id}` outside `Spawn.id` (added post-review), plus its
      fixture/test.
- [x] `cargo check -p ironhold_cli` — clean.
- [x] Playtest aid — `assets/projects/entity_logic_demo`'s "respawning gem" (new prefab +
      `behaviors/respawning_gem.behavior.ron` + scene entity/labels): repeatedly collecting the
      same gem (identical `{self}` every time) spawns a residue with a fresh `{new_id}`-derived id
      each time, mirroring the corpse-id-reuse motivation directly. `ironhold_cli validate`,
      `ron_lint`, `ron_validation`, and `asset_checker.py` all pass against it.
- [x] WASM dev build — clean.
- [x] User playtest — confirmed by Frank (2026-08-30). Three real issues found and fixed during
      the playtest round itself, all in the demo aid, none in the `{new_id}` mechanism:
      1. **Residue never actually spawned** — `gem_residue` was first authored as `kind: Primitive`
         (`model: ""`), but `Action::Spawn`'s executor requires a resolvable asset-catalog model
         key before it will queue *any* dynamic spawn, regardless of prefab kind — a real,
         pre-existing engine limitation this demo surfaced (a primitive-shaped prop can be
         scene-placed but not dynamically `Action::Spawn`-ed). Fixed by giving `gem_residue` a real
         GLB (`kind: Prop`, `shared/models/rocks/rock_01.glb#Scene0`).
      2. **All residues stacked at one hardcoded position** — the behavior file cycles through 10
         fixed positions (`idle_0/collected_0` .. `idle_9/collected_9`) on a 2.5 m circle around the
         gem, spreading collections apart visually. Pure round-robin, not randomization (RON has
         no randomness) — an 11th+ collection reuses slot 0's spot.
      3. **UI label overlap** — the "camera hint" label's text was too long for its box width and
         wrapped onto the next label's row; shortened it and added a dedicated "gem hint" row.
      A separate, pre-existing WASM audio-autoplay gap (`AudioContext` not unlocking reliably) was
      also surfaced by the demo's pickup sound — logged as its own `planning/backlog.md` Bugs entry
      rather than fixed here (unrelated to `{new_id}`, needs its own `bevy_audio` investigation).

## Open questions
- None — approach is fully determined by the existing `SpawnRegistry.counter` precedent.

## Acceptance criteria
- Given a rule/behavior with `Spawn(prefab: "x", id: "{self}_corpse_{new_id}")` fired twice for the
  same `{self}` value in one scene, when both spawns execute, then the two resulting spawn ids are
  different and both resolve correctly in `SpawnRegistry`. **Covered end-to-end** by
  `test_entity_fsm_new_id_composes_with_self_substitution` (driven through the real entity FSM
  interpreter, not a direct `ActionQueue` push).
- Given `Spawn(id: "loot_{new_id}")` with no `{self}`/`{target}` involved, the id still resolves to
  a value distinct from any other `{new_id}`/auto-generated id this scene — but is not guaranteed
  distinct from a hand-authored literal id of the same shape (see Approach).
- Given the pre-existing `id: None` fallback path, its behavior and output format are completely
  unchanged (same shared counter, same `"{prefab}_{n}"` format when omitted entirely).
- A scene reload resets the counter (matches existing, documented `SpawnRegistry` behavior) —
  this is fine since no entity carrying a `{new_id}`-derived id can survive a `LoadScene` anyway.
- Given `{new_id}` authored outside `Spawn.id`, `ironhold_cli validate` reports
  `misplaced_new_id_token` rather than silently passing.
- Given a resolved spawn id that collides with an already-registered one (via `{new_id}` or a
  plain literal), a `warn!` is logged rather than silently orphaning the earlier entity.

## Follow-ups (logged to `planning/claude_suggestions.md`, out of scope here)
- Retrofitting `3rd_person_game_demo`'s corpse spawn (`integration`-only, not present on the
  `main` base this branch was cut from) to use `{new_id}` and retire its
  `Despawn("{self}_corpse")` guard is a separate follow-up — note it also requires moving corpse
  decay into the corpse's own behavior file, since a `{new_id}`-derived corpse id is no longer
  addressable by the monster that spawned it.
- Adding an `Action::Spawn` arm to `capabilities/dialogue.rs`'s `substitute_self_in_action` so
  `{self}` resolves in dialogue-authored `Spawn.id` too (pre-existing gap, not introduced here).
- The narrow `PendingEntitySpawns`/`SpawnRegistry.counter` reset-ordering window after a `LoadScene`
  followed immediately by 3+ queued spawns in the same action list (pre-existing, unrelated to
  `{new_id}` specifically, but this feature's uniqueness claims make it worth tightening).
