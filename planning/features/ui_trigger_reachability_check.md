# Feature: CLI `validate` cross-checks UI trigger reachability

_Status: Ready_
_Planned at: `3570198` (2026-07-02)_

## What

`ironhold_cli validate` gains a new cross-file check: for every scene `Button`/`IconButton`
and every `global_key_bindings` entry, derive the `ui.button_pressed:{trigger}` event it fires
at runtime and confirm at least one rule/transition/binding in `rules.ron`, `state_machine.ron`,
or a `.behavior.ron` file actually matches it. A mismatch is reported as a normal (non-`--strict`)
cross-file error, in the same list as today's "prefab key not found" / "audio key not found"
errors.

## Why

`scene_loader.rs` silently derives each button's trigger via
`btn.action.strip_prefix("ui.").unwrap_or(&btn.action)` (same pattern for `IconButton`, and the
raw value for `global_key_bindings`). If a designer mistypes the `action` field — e.g. writes
`action: "toggle_mute"` instead of `"ui.toggle_mute"`, or authors a rule against
`"ui.button_pressed:toggle_mut"` (typo) — the button still renders, is still clickable, and the
click still fires a `UiEvent`. Nothing errors at any point; the event is simply produced,
matched against zero rules, and dropped. The only observable symptom is "I clicked the button
and nothing happened," which gives no lead on where to look. `validate` already catches the
equivalent class of mistake for prefab/effect/audio/decal/modifier keys — this closes the same
gap for the UI→logic wiring path, which today has zero cross-file coverage (confirmed:
`validate.rs` has no reference to `UiNodeDef`, `Button`, or `IconButton` anywhere).

Observed at `517afe7` (2026-06-28), logged in `planning/claude_suggestions.md`; re-verified
still-unaddressed on 2026-07-02 (`strip_prefix("ui.")` call sites and the absence of any
button-aware check in `validate.rs` both confirmed against current `main`).

## Approach

**Trigger derivation (mirrors runtime exactly):**
- `UiNodeDef::Button(btn)` / `UiNodeDef::IconButton(btn)` → `trigger = btn.action.strip_prefix("ui.").unwrap_or(&btn.action)` → derived event `format!("ui.button_pressed:{trigger}")`. Matches `scene_loader.rs` lines ~1391/1413.
- `ProjectConfig.global_key_bindings: HashMap<String, String>` → each **value** is used directly as the trigger (no `ui.` stripping — see doc comment at `project.rs:195`) → derived event `format!("ui.button_pressed:{value}")`.
- Dialogue choice buttons (`dialogue_choice:{n}`) are **out of scope** — they are spawned dynamically by `dialogue.rs` from `DialogueChoiceDef`, never appear as a `UiNodeDef::Button` in scene RON, and are matched directly by `dialogue_tick_system`, not through `rules.ron`/`state_machine.ron`. No false-positive risk since the check only walks `scene.ui`.

**Handled-event collection:** the exact-match semantics already used at runtime
(`rule.on == event_name`, `binding.event == *event_name`, `t.on == *event_name` in
`message_interpreter.rs`) mean a plain string-set membership check is correct — no glob/prefix
matching needed. `query.rs::collect_logic`/`collect_fsm` already walk `rules.ron` +
`state_machine.ron` + `behaviors/*.behavior.ron` and collect every `rule.on` / `state.on[].event`
/ `global_on[].event` / `transitions[].on` string, but that logic is private to `query.rs`.
Extract the **event-collecting half** (not the action-collecting half, which `validate.rs`
already has its own version of via `collect_actions`) into a small shared helper in
`commands/utils.rs`, e.g. `collect_handled_events(project_dir) -> Vec<EventOccurrence>`, and have
both `query.rs` and the new `validate.rs` check call it. This avoids a second copy of the FSM-walk
that could silently drift from `query events` output, the same class of duplication risk already
flagged elsewhere in this project (`attach_prefab_features` vs `spawn_prefab_instance`).

**New check** (`crates/ironhold_cli/src/commands/validate.rs`):
- Currently `_project_config: Option<ProjectConfig>` is parsed and discarded. Stop discarding it — the new check needs `global_key_bindings`.
- Add `check_ui_trigger_reachability(project_config, scenes, handled_events) -> Vec<CrossFileError>`, called from `do_validate` alongside the existing `cross_file_checks`.
- For each derived event not present in `handled_events`, push a `CrossFileError` with `error_type: "unreachable_trigger"`, message naming the button `id`/text (or key binding key) and the derived event string, so the designer can grep straight to the mismatch.
- This is a plain cross-file error (always runs), not gated behind `--strict` — `--strict` is reserved for "defined but never used" orphans; this is "referenced but never resolves," matching the existing missing-key checks' severity class.

## Tasks
- [ ] Extract `collect_handled_events` (or equivalent) into `commands/utils.rs`; refactor `query.rs::collect_logic` to use it for its `events` half
- [ ] Stop discarding `_project_config` in `do_validate`; thread it into the new check
- [ ] Implement `check_ui_trigger_reachability` covering `Button`, `IconButton`, and `global_key_bindings`
- [ ] Wire into `do_validate` / `cross_file_checks` call site; extend `print_human`/`print_json` if a new bucket is needed (or fold into existing `cross_errors` list — no new bucket required)
- [ ] `cargo test -p ironhold_cli` — extend `validate_cross_file` test with a fixture project containing one correctly-wired button and one mistyped one
- [ ] Manually verify against a real project: `cargo run -p ironhold_cli -- validate assets/projects/3rd_person_game_demo` (and others) stay clean (no false positives)
- [ ] Update root `CLAUDE.md` build-commands section if `validate --strict` behavior notes need a mention (likely not — this isn't a strict-only check)
- [ ] Cross out the `claude_suggestions.md` entry once shipped

## Open questions
- None blocking. Confirmed no false-positive source (dialogue choice buttons don't go through `scene.ui`); confirmed exact-string match is the correct comparison (no wildcard event matching exists anywhere in the interpreter).

## Acceptance criteria
- Given a scene button with `action: "toggle_mute"` and no rule/transition/binding on `"ui.button_pressed:toggle_mute"` anywhere in the project's logic files, `ironhold_cli validate` exits 1 and reports the mismatch by button id and derived event string.
- Given a `global_key_bindings` entry whose value has no matching handler, the same error class fires.
- Every shipped example project (`quick_scene`, `3rd_person_game_demo`, `terrain_demo`, `custom_materials`, `primitive_world`, `entity_logic_demo`, `particles_demo`) still validates clean — zero new false positives.
- `cargo test -p ironhold_cli` passes, including the new fixture-based regression test.
