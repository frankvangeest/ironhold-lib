# Feature: Flycam model-never-renders warning

_Status: Done_
_Planned at: `1124de0` (2026-08-19)_
_Revised after plan review (system-architect + ux-gamedesigner-reviewer), 2026-08-19 — see "Plan review changes" below._
_Shipped 2026-08-19 — playtest confirmed by Frank, no console errors. See `planning/backlog.md`'s matching entry for the post-implementation-review summary._
_Branched from `integration` (`1124de0`), not `main`, because this builds directly on `flycam_scene_conflicts.md`'s spectator-mode code, which had not yet been promoted to `main`._

## What
A `tags: ["flycam"]` prefab's `model:` (and `children:`) fields are documented as ignored (by
design — a flycam is a camera-only entity, see `docs/20_data_formats.md`'s "Special tag: `flycam`"
section) but nothing tells a designer they've authored one that will never appear.
`scene_loader.rs`'s `is_flycam` branch records the spawn transform/mode and `continue`s before ever
consulting `prefab.model`/`prefab.children` — either, if non-empty, is unconditionally, silently
discarded. Confirmed empirically: a `kind: Prop, model: "anvil"` flycam-tagged entity placed in a
live scene never renders, no console output at all.

This adds a scene-load `warn!` and a matching `ironhold_cli validate` **hard error** for this case
— no runtime behavior change, purely a diagnostic so the mistake fails loud instead of silent. See
`planning/backlog.md`'s "A flycam-tagged prefab's own `model:` never renders, with no warning".

## Why
Found by debug-detective during the investigation that also produced the now-shipped
`planning/features/done/flycam_scene_conflicts.md` — that fix covered the two *entity-dropping*
bugs from the same investigation; this is the third, narrower finding, deliberately scoped out of
that change to keep it bounded. Every shipped flycam prefab in this repo already avoids the trap by
convention (`model: ""`, no `children:`), so this is purely a foot-gun for future authoring, not a
fix to any existing broken scene.

## Plan review changes
Reviewed pre-implementation by `system-architect` and `ux-gamedesigner-reviewer`. Both returned
"refine". Changes folded into the Approach below:

- **Severity confirmed: hard validate error, not `--strict`.** Precedent isn't
  `duplicate_flycam_entity` (an entity-dropping bug) — it's `invalid_binding` (an unrecognized
  gamepad button name, `validate.rs:341-347`), whose consequence is "will have no effect," the same
  class as this case. `strict_checks` is currently pure orphan detection (unused catalog keys) —
  this isn't an orphan, it's a referenced-but-ignored field, so it doesn't belong there. `--strict`
  also isn't in the default `validate` workflow, so a strict-only warning would rarely surface in
  practice. Zero false-positive risk: `flycam` is a prefab-level tag, so the model/children are dead
  in *every* scene that prefab is placed in, unconditionally — no "legitimate hidden marker mesh"
  escape hatch exists (the flycam spawn never even calls `tag_spawned_entity`, so the entity has no
  `SpawnId`/render output regardless).
- **Cover `children:`, not just `model:`.** Both reviewers independently flagged this: a designer
  reaching for `model:` to give a flycam a visible body is equally likely to reach for `children:`
  (the composite-primitive path) — shipping a model-only diagnostic would wrongly imply "use
  children instead" by omission. Add `PrefabDef::flycam_ignored_fields(&self) -> Vec<&'static str>`
  (`schema/catalog.rs`) returning the non-default ignored field names present (`"model"`,
  `"children"`); both `scene_loader.rs` and `validate.rs` call it instead of re-deriving the check.
  Also add `PrefabDef::is_flycam(&self)`/`is_player(&self)` to the same `impl` block, replacing the
  ad-hoc `prefab.components.tags.contains(&TAG_FLYCAM.to_string())` checks duplicated across
  `scene_loader.rs` and hardcoded string literals in `validate.rs` — `schema/` is the correct home
  since the CLI only ever reaches into `ironhold_core::schema::*`, never `runtime::`.
- **CLI check scoped to the prefab catalog, not per-scene-entity.** The condition is entirely
  prefab-local (nothing about the scene participates), unlike `duplicate_flycam_entity`'s
  genuinely scene-dependent combination. Scene-scoping it would emit one error per instantiation
  across every scene that uses the prefab, all pointing at the wrong file. Emit one `CrossFileError`
  per offending prefab with `source_file: "prefabs/prefabs.ron"` instead. (The `gamepad_index`
  duplicate check's per-scene scoping doesn't apply here — that one exists because
  `local_coop_demo` legitimately reuses the same value across catalog variants that are never
  co-instantiated; no equivalent legitimate case exists for a flycam prefab's ignored fields.)
- **Dual `tags: ["player", "flycam"]` gets its own distinct message, not the same one.** This is the
  one case where a non-empty `model:` is deliberate (a real player character) — `is_flycam` is
  checked and `continue`s before `is_player` is ever consulted, so a dual-tagged prefab silently
  never spawns as a player at all, not just "model ignored." Detect `is_player && is_flycam`
  separately and name the actual problem: the flycam tag is suppressing the player entirely: point
  at `camera_mode: Flycam(...)` on a `tags: ["player"]`-only prefab as the supported way to get a
  flying player character (already documented, `docs/20_data_formats.md`'s two-authoring-paths
  note). The ignored-fields warning stays scoped to flycam-only prefabs.
- **Warning/error message must state a fix, not just "this is by design."** Mirror
  `duplicate_flycam_entity`'s name-offenders → consequence → remedy shape. Exact wording (used
  verbatim in both the `warn!` and the CLI message):
  > `Flycam prefab '{key}' sets {fields} — a flycam is camera-only and never renders a body, so
  > {this/these field(s)} will never appear. Set {field}: "" (or remove children) to silence this.
  > To give a flying camera a visible body, use camera_mode: Flycam(...) on a "player" prefab
  > instead, or spawn the body as a separate non-flycam entity at the same position.`
  For the dual-tag case:
  > `Prefab '{key}' has both "player" and "flycam" tags — the flycam tag makes it spawn as a
  > camera-only entity and its player components never spawn at all. Use camera_mode: Flycam(...)
  > on a "player"-only prefab instead if you want a flying player character.`
- **Tests restated as achievable.** Bevy's `warn!` isn't capturable in these integration tests
  (same reason the duplicate-flycam-tags core test asserts *behavior*, not log text). Core test:
  a flycam prefab with `model: "..."` spawns exactly one `FlycamCameraMode` camera and zero
  `Mesh3d`/`SpawnRegistry` entries for that entity id. CLI tests: new fixtures + `validate_cross_file.rs`
  cases asserting exit 1 and that stdout names the prefab key and field list, for both the
  ignored-fields case and the dual-tag case. `cargo test -p ironhold_cli --test validate_projects`
  must stay green (proves the new hard error doesn't retroactively break any of the 13 shipped
  projects — none use a non-empty `model`/`children` on a flycam prefab today).
- **Playtest aid can't be a shipped project scene.** The check is a hard error, so adding an
  offending prefab to `camera_modes` (the sibling feature's approach) would break that project's own
  `validate` and the `validate_projects` smoke test. Instead: temporarily set
  `model: "anvil"` on `camera_modes`' `flycam_demo` prefab locally, confirm the console `warn!`
  fires with the expected wording and the scene still behaves identically, then revert before
  committing. Re-run `cargo test -p ironhold_core --test ron_lint --test ron_validation` after
  reverting per the step-10 caveat (a RON file was touched, even if reverted, since the test run
  needs to reflect the final committed state).
- **Docs: three edits, not one, plus an inline RON example since no shipped project can demonstrate
  the mistake (it would fail `validate`).**
  1. `docs/20_data_formats.md:1882` — "The `model` field is ignored (and warns — see below — if you
     set one)."
  2. New note after the existing "Duplicate flycam entities" note (~line 1954), matching its
     bold-lead format, with the ❌/✅ RON pair:
     ```ron
     // ❌ never renders — a flycam has no body
     "flycam_with_body": ( kind: Prop, model: "anvil", components: ( tags: ["flycam"] ) ),
     // ✅ camera-only, as intended
     "flycam":           ( kind: Prop, model: "",      components: ( tags: ["flycam"] ) ),
     ```
     Note this fires at scene load (**visible in the browser console** — the only diagnostic channel
     available to a WASM-only designer with no `ironhold_cli` access) and fails `ironhold_cli
     validate`.
  3. Spectator-mode "what doesn't work" bullet (~line 2001-2002, "The flycam entity's own `model:`
     field still never renders") — append "— and now warns at scene load / fails `ironhold_cli
     validate`."
  Also refresh `assets/projects/terrain_demo/prefabs/prefabs.ron`'s flycam comment ("The model field
  is required by the schema but is ignored for flycam entities") to mention the new diagnostic,
  since it's the comment most likely actually read at authoring time.

## Approach
1. **`schema/catalog.rs`** — add to `impl PrefabDef`: `is_flycam()`, `is_player()` (tag checks,
   replacing ad-hoc `.contains()` calls), and `flycam_ignored_fields() -> Vec<&'static str>`
   (checks `model`/`children` non-default, only when `is_flycam()`).
2. **`scene_loader.rs`** — inside the `is_flycam` branch, before the `continue`: if `is_player()`
   too, warn with the dual-tag message. Else if `flycam_ignored_fields()` is non-empty, warn with
   the ignored-fields message naming entity id, prefab key, and the field list.
3. **`ironhold_cli/validate.rs`** — new checks beside `duplicate_flycam_entity`: iterate the prefab
   catalog once (not per-scene), emit one `CrossFileError` (`source_file: "prefabs/prefabs.ron"`)
   per prefab matching `is_player() && is_flycam()` (`error_type: "flycam_player_tag_conflict"`) and
   one per prefab with non-empty `flycam_ignored_fields()` (`error_type:
   "flycam_model_never_renders"`).
4. **Docs + tests** per "Plan review changes" above.

## Tasks
- [x] `schema/catalog.rs`: `PrefabDef::is_flycam()`, `is_player()`, `flycam_ignored_fields()`,
      `flycam_ignored_fields_remedy()`. Post-implementation review (all 4 agents) extended coverage
      to `shape`/`primitive`, not just `model`/`children` — the dominant `kind: Primitive`
      single-shape authoring idiom used everywhere else in this repo was a total blind spot in the
      first pass.
- [x] `scene_loader.rs`: use the new helpers; add dual-tag warn and ignored-fields warn before the
      existing `continue`. Wording revised post-review to use a field-specific remedy instead of a
      hardcoded "set model: \"\"" (wrong advice for a `children`-only offender), and the dual-tag
      message now also offers "remove the player tag" as an alternative fix.
- [x] `ironhold_cli/validate.rs`: two new prefab-catalog-scoped `CrossFileError` checks
      (`flycam_player_tag_conflict`, `flycam_model_never_renders`), sorted by prefab key for
      deterministic output; `duplicate_flycam_entity` also converted to use the new `is_flycam()`
      helper.
- [x] Tests: core (`camera_modes_tests.rs`) — separate regression tests for `model:`, `children:`,
      and the dual-tag runtime case (zero `CharacterController`); CLI (`validate_cross_file.rs`) —
      fixtures for all three error shapes (model, children, shape/primitive) plus the dual-tag
      conflict, with assertions tightened post-review to actually distinguish the two error types
      (the original dual-tag assertion could not tell them apart) and confirm no cross-firing;
      `validate_projects` confirmed green.
- [x] Docs: the `docs/20_data_formats.md` edits (intro note, new diagnostic note with ❌/✅ RON pair,
      dual-tag note, spectator-mode bullet refresh) + prefab comment refreshes in `terrain_demo`,
      `custom_materials`, and `foliage_demo` (the latter also relabeled a flycam-only prefab that
      was miscommented as "Player").
- [x] Manual playtest: temporarily set `model: "chest_01"` on `camera_modes`' `flycam_demo` prefab
      (and pointed `initial_scene` at `flycam_test.scene.ron` to reach it without manual
      navigation) — confirmed both `ironhold_cli validate` (exit 1, exact designed message) and the
      native runtime console `warn!` (verbatim designed wording) fired correctly, then reverted both
      edits; re-ran `ron_lint`/`ron_validation` (197 + 1 passed). Separately, Frank confirmed the
      WASM dev-build playtest of the unmodified `camera_modes` project (Flycam + Spectator rooms) —
      no behavior change, no unexpected console warnings.
- [x] `planning/backlog.md`: marked this item Done.

## Open questions
- None remaining — plan review resolved severity, scope, the dual-tag edge case, message wording,
  and the playtest-aid approach. Post-implementation review (alignment-reviewer, system-architect,
  debug-detective, ux-gamedesigner-reviewer, all 2026-08-19) found and fixed: the `shape`/`primitive`
  coverage gap (real, high-value — see Tasks above), a CLI test that couldn't actually distinguish
  the two error types, hardcoded non-field-specific remedy wording, and a core test built on the
  wrong `PrefabDef.kind` (risked a vacuous assertion). Four narrower, lower-value findings were
  deferred to `planning/claude_suggestions.md` rather than fixed here: `Action::Spawn`/
  `Action::JoinPlayer` don't check the flycam tag (sibling gap to `flycam_scene_conflicts.md`'s
  already-logged ones); partial adoption of the new `is_player()`/`is_flycam()` helpers (~9 sites
  still hand-roll the tag check); an adjacent pre-existing panic (`scene_loader.rs:513`,
  `prefab.shape.as_ref().unwrap()` on a shapeless top-level `Primitive`); and nondeterministic
  ordering in one pre-existing sibling validate loop.

## Acceptance criteria
- Given a `tags: ["flycam"]`-only prefab with non-empty `model:` and/or `children:`, scene load logs
  a `warn!` naming the entity id, prefab key, and the ignored field(s), stating they'll never
  render, and prescribing both the silencing fix (`model: ""`/remove children) and the supported
  alternative (`camera_mode: Flycam` on a player prefab, or a separate entity) — and
  `ironhold_cli validate` reports one hard error per offending *prefab* (not per scene entity).
- Given a prefab with both `"player"` and `"flycam"` tags, scene load logs a distinct warning naming
  the actual failure (player never spawns) and the fix, and `ironhold_cli validate` reports it as
  its own error type.
- Given a flycam-only prefab with `model: ""` and no `children:` (every shipped prefab today),
  behavior is byte-identical to today (regression test) — no warning, no validate error, and
  `cargo test -p ironhold_cli --test validate_projects` stays green.
- No change to what actually renders — a flycam remains camera-only either way.
