# Feature: CLI `validate` cross-checks UI trigger reachability

_Status: Done_
_Planned at: `74c63a5` (2026-07-02)_

## Plan re-verification (2026-09-04, `2fd73c0`)

Re-checked against current `main` before coding, per the mandatory step-1 plan-freshness check
(`git log 74c63a5..HEAD` touches `validate.rs`/`query.rs`/`scene_loader.rs`/`project.rs` extensively
— 6 CLI-validate features and several core features landed since this plan was written). Findings:

- **`_project_config: Option<ProjectConfig>` is no longer discarded** — `cli_validate_hardening.md`
  (`7acd354`) already threaded it through for the `initial_scene` check. The "stop discarding it"
  task below is done as a side effect; nothing left to do there.
- **`collect_handled_events` will NOT be extracted from `query.rs`'s private `collect_logic`/
  `collect_fsm`, contrary to the original Approach section.** `query.rs`'s `EventRecord` carries
  `action_kinds`/`is_transition` for `query events`' own display output, computed in the same walk
  that discovers each event string — there's no clean seam to split "which events are handled" from
  "what does each occurrence trigger" without either changing `query events`' output shape or a
  fragile index-zip between two independently-ordered walks. `query.rs` is left untouched; a new
  `collect_handled_events` is added instead. This is a deliberate, lower-risk deviation from "avoid
  a second copy of the FSM-walk" — the walked surface (4 fixed schema fields) is small and stable,
  and all 4 post-implementation reviewers independently confirmed it's the right call (see "Post-
  implementation review" below). **Where it lives changed from the plan's original design during
  review**: not `commands/utils.rs` taking `project_dir` and re-parsing from disk (`Vec<HandledEvent>`,
  source + event) — that shape caused a real bug (see below) — but `validate.rs`, next to
  `collect_actions`, taking the same already-parsed `Option<&LogicRulesAsset>`/
  `Option<&StateMachineAsset>`/`&[(String, StateMachineAsset)]` `do_validate` already builds, and
  returning `HashSet<String>` directly (no per-event source needed by the one caller).
- **New scope, not in the original plan: `GameSceneV2.scene_key_bindings`** (added by
  `local_coop_hot_join_leave.md`/`gamepad_hot_join.md`-era work, after this plan was written) is the
  keyboard-equivalent, per-scene-overriding sibling of `global_key_bindings` — same "value is a
  bare trigger string, fires `ui.button_pressed:{trigger}` with no `ui.` stripping" shape
  (`scene_v2.rs:80-85`), same bug class this feature exists to catch. `scene_unclaimed_gamepad_bindings`
  (the gamepad analog) already gets a validate check today, but only for *button-name recognition*,
  not trigger reachability — same gap as `global_key_bindings` had before this feature.
  **`scene_key_bindings` is now in scope alongside `global_key_bindings`**, checked per-scene (its
  derived event only needs to be reachable from that scene's own rules/FSM/behaviors — no
  cross-scene union needed, unlike `spawn_point`, since `scene_key_bindings` is authored directly on
  the scene it applies to).
- Confirmed still accurate: `ButtonDef`/`IconButtonDef.action: String` (`#[serde(default)]`, so
  omission is legal RON) and the `strip_prefix("ui.")` derivation in
  `scene_manager/scene_loader.rs:1739`/`1765` are unchanged from the plan's citations. No shipped
  project has an empty (`action: ""`, i.e. omitted) button `action` field — grepped every
  `Button((...))` block in every `assets/projects/*/scenes/*.scene.ron`, none lack an `action:`
  key. **Revised during review**: rather than folding this into the generic unmatched-trigger
  message (this doc's original plan), an empty trigger now gets its own distinct message ("has no
  action configured") — debug-detective's review found the generic message reads as a validator
  bug (`fires "ui.button_pressed:" on click`) rather than a clear diagnostic.

## Post-implementation review (2026-09-04)

4 parallel reviews (`alignment-reviewer`, `system-architect`, `debug-detective`,
`ux-gamedesigner-reviewer`) converged strongly. Two real, fixed-before-merge issues:

1. **The first implementation re-parsed `rules.ron`/`state_machine.ron`/behaviors from disk via a
   `commands/utils.rs` helper, independently of `do_validate`'s own already-parsed
   `rules`/`state_machine`/`behaviors` locals** (all 4 reviewers). A malformed logic file then
   degraded silently in the re-parse (`silent_parse`'s `.ok()`) rather than surfacing through the
   same `file_results` error path every other check already uses — and since a file that fails to
   parse contributes zero handled events either way, this meant every button/binding depending on
   that file got reported `unreachable_trigger` on top of the file's own already-reported parse
   error, a redundant pile-up with no way to tell "the button is genuinely miswired" from "the
   handler file the button depends on happens to be broken right now." Fixed by (a) moving
   `collect_handled_events` into `validate.rs` to take the already-parsed structs directly (no
   second disk read, no second parse-error-swallowing path) and (b) making
   `check_ui_trigger_reachability` skip entirely (return no errors) whenever any of
   `logic/rules.ron`/`logic/state_machine.ron`/a `behaviors/*.behavior.ron` file has a non-empty
   `FileResult.errors` entry — the parse error is still reported once, with nothing fabricated on
   top of it.
2. **The error message said "on click"/"the button will do nothing" even for
   `global_key_bindings`/`scene_key_bindings` mismatches** (`ux-gamedesigner-reviewer`, blocker) —
   factually wrong for a key binding (there is no button), actively misleading for debugging. Fixed
   by threading a `verb`/`consequence` phrase pair through the shared `check` closure, distinct for
   click-sources vs. key/gamepad-binding sources.

Also closed during review, all 3 non-UX reviewers converging independently: **gamepad-binding
coverage was missing** — `global_unclaimed_gamepad_bindings`/`scene_unclaimed_gamepad_bindings`
fire the identical `ui.button_pressed:{trigger}` event with the identical raw-value derivation as
the keyboard bindings (`runtime/input.rs`'s `unclaimed_gamepad_trigger_system`), and
`local_coop_demo/scenes/room8.scene.ron`'s real `scene_unclaimed_gamepad_bindings: {"South":
"join"}` was a live instance of exactly the bug class this feature exists to catch, on the
hardest-to-diagnose input path (needs a physical controller to even notice). Added both maps to
the check (re-scanned all shipped projects afterward — clean).

Also fixed: `debug-detective`'s mutation-testing found `valid_ui_trigger`'s fixture never actually
exercised `FsmTransition.on` (deleting that loop from `collect_fsm_events` left all
`validate_cross_file` tests green) — added a `start_button`/`transitions:` pair to close the gap.
Added a dedicated `bad_rules_parse_no_cascade` regression fixture for finding 1 above.

3 gaps were deliberately deferred to `planning/backlog.md` rather than expanding this branch's
scope (all logged 2026-09-04): `ironhold_cli validate` hardcoding `logic/rules.ron`/
`logic/state_machine.ron` convention paths instead of resolving `ProjectConfig.rules_path`/
`state_machine_path` (a `do_validate`-wide gap affecting every existing check, not specific to this
one — found by `debug-detective`, latent on every shipped project); the five engine-hardcoded panel
triggers (`close_inventory`/`close_shop`/`close_container`/`take_all_from_container`/
`buy_item:{item_key}`) remaining uncovered (`system-architect`); and the lack of a symmetric
"orphan rule" `--strict` check (`system-architect`/`alignment-reviewer` — `quick_scene`'s
`start_game`/`test_actions` rules are the concrete live example, deliberately left alone since
fixing them isn't this check's job).

`quick_scene`'s `pause_button` — the real pre-existing dead-RON bug this check found on its first
run against a shipped project — was originally "fixed" with a `Log`/`PlaySound`-only rule
(satisfies the validator, but a button literally labeled "Pause" that doesn't pause is still the
same "I clicked it and nothing happened" experience from the player's seat, just now silent instead
of loud). Both `ux-gamedesigner-reviewer` and `system-architect` flagged this. Resolved by renaming
the button (`pause_button` → `ping_button`, `text: "Pause"` → `"Ping"`, `action: "ui.pause"` →
`"ui.ping"`) to honestly describe what it does — the simplest possible button (Log + PlaySound,
no gameplay effect) — with a RON comment explaining it's deliberately the minimal
action-must-have-a-matching-rule example, rather than inventing pause/time-scale mechanics that
don't exist anywhere in the engine's `Action` schema.

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

Observed at `4f9ae82` (2026-06-28), logged in `planning/claude_suggestions.md`; re-verified
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
- [x] Add `collect_handled_events(rules, state_machine, behaviors) -> HashSet<String>` to `validate.rs`, next to `collect_actions` (moved here from `commands/utils.rs`'s original `project_dir`-taking design during review — see "Post-implementation review")
- [x] ~~Stop discarding `_project_config` in `do_validate`~~ — already done by `cli_validate_hardening.md`
- [x] Implement `check_ui_trigger_reachability` covering `Button`, `IconButton`, `global_key_bindings`, `scene_key_bindings`, and (added during review) `global_unclaimed_gamepad_bindings`/`scene_unclaimed_gamepad_bindings`
- [x] Wire into `do_validate` alongside `cross_file_checks`, folded into the existing `cross_errors` list — no new print bucket needed
- [x] `cargo test -p ironhold_cli` — `bad_ui_trigger_button`, `bad_ui_trigger_key_binding` (mistyped fixtures), `valid_ui_trigger` (every trigger source × every handler source, including `transitions[].on`), `bad_rules_parse_no_cascade` (parse-error regression, added during review)
- [x] Manually verified against every shipped project (`cargo run -p ironhold_cli -- validate` + `cargo test -p ironhold_cli --test validate_projects`) — zero false positives; found and fixed one real pre-existing bug (`quick_scene`'s `pause_button`/`ping_button`, see "Post-implementation review")
- [x] `docs/60_contributing.md`'s "Checks performed" list updated with the `unreachable_trigger` bullet
- [x] Cross out the `claude_suggestions.md` entry once shipped

## Open questions
- None blocking. Confirmed no false-positive source (dialogue choice buttons don't go through `scene.ui`); confirmed exact-string match is the correct comparison (no wildcard event matching exists anywhere in the interpreter).

## Acceptance criteria
- Given a scene button with `action: "toggle_mute"` and no rule/transition/binding on `"ui.button_pressed:toggle_mute"` anywhere in the project's logic files, `ironhold_cli validate` exits 1 and reports the mismatch by button id and derived event string.
- Given a `global_key_bindings`/`scene_key_bindings`/`global_unclaimed_gamepad_bindings`/`scene_unclaimed_gamepad_bindings` entry whose value has no matching handler, the same error class fires, with wording correctly distinguishing "clicked" (buttons) from "pressed" (key/gamepad bindings).
- Given `logic/rules.ron`, `logic/state_machine.ron`, or a `behaviors/*.behavior.ron` file fails to parse, the check is skipped entirely (its own parse error is reported once, no derived `unreachable_trigger` noise fabricated on top).
- Every shipped example project still validates clean — zero new false positives (verified against all 9 projects covered by `validate_projects.rs`, plus a manual full-project-directory scan of all `assets/projects/*`).
- `cargo test -p ironhold_cli` passes, including the new fixture-based regression tests.
