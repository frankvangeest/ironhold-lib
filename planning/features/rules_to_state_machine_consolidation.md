# Feature: Consolidate rules.ron onto state_machine.ron

_Status: Ready (revised after plan-review: system-architect + ux-gamedesigner-reviewer, 2026-09-23)_
_Planned at: `59f10e9` (2026-09-23)_
_Background: `planning/investigations/rules_vs_state_machine_architecture.md` (the pros/cons, the
code-level equivalence proof, and the content survey). This plan builds on that investigation and
does not repeat it. Frank's decision (2026-09-23): one deliberate breaking change, with no
deprecation bridge and no load-time auto-translation. Pre-1.0. The only external consumer,
`Ironhold-fps-demo`, already uses `state_machine_path` only, and Frank is the only content author._

Single-phase feature, so there is no Phases table. See "Why one phase" under Approach.

## What

The `rules_path` / inline `rules:` authoring path goes away completely. After this change every
project's logic lives in `state_machine.ron` (`StateMachineAsset`). That is the same schema the
per-entity `.behavior.ron` files already use, so ironhold ends up with one logic dialect instead
of two.

For a designer, the simplest logic file stays just as short as today's. The FSM schema is loosened
so that a flat file needs only `schema_version` + `global_on`:

```ron
(
    schema_version: 1,
    global_on: [
        ( event: "scene.ready:main", do_actions: [ Log("Scene ready") ] ),
    ],
)
```

Designers learn one progression: "start with `global_on`, add `states` + `transitions` when you
need modes". There is no longer a point where they must switch file formats. `Action::EnterState`
is removed as well (see Approach §5). A `.project.ron` that still says `rules_path:` or `rules:`
becomes a hard parse error that names the field (see §4 for exactly how that surfaces, including
the runtime loading-screen hang).

**User-facing terminology:** one `( event: …, do_actions: … )` entry is still called a **rule** in
every designer-facing doc. "Binding" (the Rust type is `FsmEventBinding`) is reserved for internal
code and test naming only.

## Why

- **Two dialects mean the same thing.** Rules are a strict subset of the FSM. Every live rules
  project uses only the part that equals `global_on`. The investigation lists five confusion bugs
  caused by the two formats living side by side (§2 Cons).
- **Running both at once is a hazard class nobody detects:** both interpreters fire on the same
  event, they disagree about the current state within one frame, and `EnterState` skips the FSM's
  entry/exit hooks. Deleting the second interpreter removes the whole class instead of guarding
  against it.
- **Every future logic feature would otherwise be built twice.** Examples: intent handling, the
  magic-string event validator, conflict checks, and debug overlays.
- **Now is the cheapest time it will ever be:** pre-1.0, one author, zero external rules.ron
  users, and a content set small enough to migrate mechanically and prove equivalent.

## Approach

### Why one phase

Frank rejected a bridge. The investigation's staged plan relied on a load-time rules-to-FSM
conversion (its step 3) so the steps could ship separately. Without that bridge, the pieces cannot
ship usefully on their own:
- The schema loosening does nothing until content uses it.
- Migrated content with the old interpreter still registered is a half-state that is only worth
  keeping for one commit, as a proof point.
- The dead-file cleanup would have its doc fixes rewritten again by the main docs pass.

So this is **one feature branch, one merge into `integration`, one breaking change**. Inside the
branch the work is split into **five ordered commits**. The commit boundaries are proof and review
checkpoints, not release points (see Tasks). The branch is sized for roughly **2.5–3.5 days** of
work (revised up from 1.5–2.5 after plan review): the one-binary-at-a-time core test loop, ~43 CLI
test references, 7 core test files, the new `validate()` wiring and its fallout triage, the
migration script's abort paths, two baseline captures with binary rebuilds, the docs pass across
~12 files, and the WASM dev build + playtest. Most of it is mechanical, not design.

### 1. Cleanup: the two dead files and the stale claims

**Delete `assets/projects/3rd_person_game_demo/logic/rules.ron` and
`assets/projects/terrain_demo/logic/rules.ron`.** Both projects set only `state_machine_path`, and
`project_loader.rs` has no convention-path fallback, so neither file has ever loaded. Deleting
them cannot change runtime behavior.
- `terrain_demo`'s `dance` rule was checked, as the investigation cautioned. It listens for
  `ui.button_pressed:dance`, and no `terrain_demo` scene declares a `dance` button or key binding.
  `grep -rn dance assets/projects/terrain_demo` only hits the rule itself and
  `prefabs/animation/player_policy.ron`'s clip map. So the event cannot fire even if the file were
  loaded. Its `quit` rule is already in the FSM's `global_on`.
- The 23 rules in `3rd_person_game_demo` (menu, options, pause, all using `when:`/`EnterState`) are
  covered by that project's `state_machine.ron`. This is the old logic that the FSM replaced.
- **Non-goal, noted for docs:** that dead file also holds the only `target.changed` →
  `ShowFloatingText(entity: "{target}", …)` pair in the repo, which `docs/30_…:121` cites as the
  canonical `{target}` example. Grep confirms the pattern exists nowhere else in shipped content.
  This branch only rewrites the doc reference (§6) so it no longer cites a dead file. Porting the
  two bindings into `3rd_person_game_demo`'s live `state_machine.ron` would be a real behavior
  change (they have never run), so it is an optional follow-up, not part of this migration.

The stale "replaces rules.ron" wording (`docs/00_overview.md:121`, `docs/30_…:45`, the
`StateMachineAsset` doc comment, the `ProjectConfig` field comment) is **not** fixed separately.
§6's docs pass rewrites or deletes all four, because after this change "replaces" is simply true.

### 2. Schema: make the minimal FSM as short as rules.ron, and make `validate()` actually run

In `schema/project.rs`, `StateMachineAsset` changes as follows:

| Field | Today | After |
|---|---|---|
| `schema_version` | required, must be `1` | unchanged. **No version bump, because the change only loosens the schema** |
| `initial_state: String` | required, `validate()` rejects `""` | `#[serde(default)]` (→ `""`). `validate()` rejects `""` **only when `states` is non-empty**: "if you declare states, name the starting one" |
| `states: Vec<FsmState>` | required (an empty list is allowed) | `#[serde(default)]` |
| `transitions: Vec<FsmTransition>` | required | `#[serde(default)]` |
| `global_on` | already `#[serde(default)]` | unchanged |

- **This is additive at the parse level.** Every file that is valid today stays valid. Only files
  that were rejected before (missing fields) are now accepted.
- **Behavior is preserved exactly.** Today a rules-only project runs with `LogicState` = `""`,
  because no FSM is loaded and `LogicState` is `Default`. A migrated flat FSM defaults
  `initial_state` to `""`, and `project_loader.rs` inserts `LogicState("")`, which is the same
  value. That matters because `LogicState` is visible outside the engine: `update_debug_state` →
  the `#debug-state` DOM JSON → `test_web.py`.
- `validate()` already skips the check that `initial_state` is in `states` when `states` is empty,
  and transitions to undeclared states already fail. So a flat file cannot quietly half-declare an
  FSM — **provided `validate()` actually runs**, which today it mostly does not (next bullet).
- The `StateMachineAsset` doc comment is rewritten to "the project's logic file, and the schema of
  every behavior file". The word "replaces" goes away.

**Blocking design fix (plan review): wire `StateMachineAsset::validate()` into every load path.**
Today `validate()` has exactly one caller, `project_loader.rs:270` (the top-level project FSM).
It is never called for any of the 21 `.behavior.ron` files (`entity_spawner.rs`'s
`resolve_pending_behaviors_system` never calls it), and `ironhold_cli validate` never calls it at
all. Right now the *parse* is the only loud gate: a behavior file that forgets `initial_state:` is
a hard serde error. Loosening `initial_state` to `#[serde(default)]` removes that gate — without
this fix, such a file would silently run in state `""` with zero entry actions and zero state
bindings, and nothing would report it. So Commit 2 must also:
- call `validate()` in `ironhold_cli validate` for the project FSM **and** every behavior file the
  CLI already discovers, reporting each failure as a hard error (exit 1) naming the file;
- call `validate()` in `resolve_pending_behaviors_system` and `error!` on failure (naming the
  behavior path and the entity's spawn id). Match `project_loader.rs:270`'s existing precedent —
  log and continue attaching — rather than inventing a new "reject the behavior" policy here; the
  point is loudness, and a behavior-attachment policy change is out of scope;
- **triage the fallout before moving on.** The 21 behavior files and every CLI fixture's FSM have
  never been validated before, so this may surface real, pre-existing content errors. Fix genuine
  content bugs in this commit; if a CLI fixture is deliberately invalid, update its expected
  output. This is also why the §3 baseline is captured *after* Commit 2 (see Tasks).

Per-entity behaviors share this type, so they get the same loosening. That is harmless once
`validate()` runs: a behavior with only `global_on` is a legitimate "always react" entity, and one
that declares `states` without `initial_state` is now rejected loudly.

### 3. Migration mechanism: a text transform plus a parsed-equivalence proof

**Scale** (measured at `59f10e9`):
- 10 live `rules.ron` files. `integration_tests/logic/rules.ron` is shared by 3 `.project.ron`
  files, which is how the investigation counted 11 "projects". Together they hold about 139 rules.
- 38 rules files in the CLI fixtures (`crates/ironhold_cli/tests/fixtures/*/logic/rules.ron` + one
  `my_custom_rules.ron`), about 91 rules.
- In total about 48 files and about 230 rules, with about 100 comment lines in the live files
  alone. Most are designer notes in `particles_demo`, `local_coop_demo`,
  `dynamic_animation_control`, and `effect_mayhem_demo`.
- **Zero `when:` and zero `EnterState` in any of them**, once the two dead files are gone.

**Doing this by hand is not realistic.** 48 files is enough for a slip to go unnoticed, and a
hand-typed `on:`→`event:` rename is exactly where one would happen.

**A permanent `ironhold migrate-rules` CLI subcommand is also the wrong tool.** It is the
compatibility scaffolding Frank rejected. It would outlive the only moment it is useful. And a
parse-then-reserialize tool would **destroy every comment**: `Action` is `Deserialize`-only, and
RON reserialization does not keep comments anyway.

**Recommended: a one-off, comment-preserving text transform, proven by a parsed-equivalence test.**

1. **The script** (Python, run from the repo root). It is committed in the migration commit and
   deleted in the removal commit, so it stays in history for review but never ships. For each
   `logic/*rules*.ron` in `assets/projects/` and `crates/ironhold_cli/tests/fixtures/`:
   - **Abort** on any `when:` or `EnterState`. None are expected, so a hit means a person needs
     to look at that file.
   - **Abort if the target `logic/state_machine.ron` already exists.** Never overwrite. At
     `59f10e9` this is true for `valid_ui_trigger`, `unset_rules_path_with_convention_file`, and
     `state_machine_only_ignores_dead_rules_ron` — all three are hand-handled (below).
   - **Abort on any `.project.ron` `rules_path:` line it cannot cleanly pattern-match** as
     exactly `rules_path: "logic/rules.ron"` (or `Some("logic/rules.ron")`). Never skip
     silently. Known hits: `rules_path_case_mismatch` (`"Logic/Rules.ron"`) and
     `rules_path_custom_filename_is_discovered` (`my_custom_rules.ron`) — both hand-handled.
   - The script carries an explicit `HAND_HANDLED` set of fixture directories (the list under
     "Files that need hand work" below). It prints and skips exactly those; every other anomaly
     aborts the whole run. This makes "what did the script not touch" an explicit, reviewable
     list instead of an absence.
   - Rewrite `schema_version: 2` (or `1`) to `schema_version: 1`. **This is not a downgrade:**
     `state_machine.ron` has always been version 1; the `2` was `LogicRulesAsset`'s own counter.
   - Rewrite the top-level `rules:` key to `global_on:`.
   - Rewrite each rule tuple's `on:` to `event:`. **Only at rule-tuple depth**: track
     paren/bracket depth, not a blind regex, **and skip string literals (including escaped
     quotes) while tracking depth.** Today a paren inside a string is balanced by luck
     (`camera_modes/logic/rules.ron:9` has one inside a `Log(...)` string), but the tracker must be
     correct, not lucky. No `Action` variant has an `on` field today, but the depth check means the
     script does not depend on that.
   - Write the result as `logic/state_machine.ron`, keeping comments, blank lines, and CRLF/LF
     line endings as they were.
   - Flip the matching `.project.ron` line from `rules_path: "logic/rules.ron"` to
     `state_machine_path: "logic/state_machine.ron"`. This includes `bad_action_item_key`, which
     also sets `rules_path` and gets the same treatment as every other fixture.
   - **Leave `rules.ron` on disk** for step 2.
   - Fix any comment text that says "rules.ron" by hand, found with grep afterward.
2. **Proof: a temporary test** (`crates/ironhold_core/tests/rules_migration_equivalence.rs`). It
   also lives for exactly one commit. It walks every directory that has both a `rules.ron` and the
   generated `state_machine.ron`, parses both, and asserts:
   - every parsed rule's `when` is `None` — the one field the transform cannot carry over, so the
     test must prove there was nothing to lose (belt-and-braces with the script's own abort);
   - `rules.iter().map(|r| (&r.on, &r.do_actions))` == `global_on.iter().map(|b| (&b.event, &b.do_actions))`.
     `Action: PartialEq` already exists, so this compares the parsed actions, not text;
   - `states.is_empty() && transitions.is_empty() && initial_state.is_empty()`;
   - `StateMachineAsset::validate()` is `Ok`.

   **Explicit skip list:** `bad_rules_parse_no_cascade/logic/rules.ron` is deliberately
   unparseable. The test must name it in an allowlist and skip it (asserting that it indeed fails
   to parse, so the allowlist can't silently mask a real regression), or Commit 3 goes red.

   This check is at the parsed level, so it catches mistakes in the script, not only syntax errors.
3. **Behavioral cross-check.** Capture `tools/bin/ironhold --json query actions <p>` and
   `--json validate --strict <p>` for every shipped project, then diff after the removal commit.
   - **Capture the "before" baseline after Commit 2, not before Commit 1.** A pre-Commit-1
     baseline for `3rd_person_game_demo`/`terrain_demo` includes the two dead files' actions and an
     `unset_logic_path_with_convention_file` `--strict` warning, both of which Commit 1 legitimately
     removes; and Commit 2's new `validate()` wiring may legitimately add diagnostics. Capturing
     immediately before the migration isolates exactly what Commit 3+4 change.
   - **Rebuild `tools/bin/ironhold` from the branch's current source immediately before each
     capture** (both before and after). A stale cached binary would silently compare against old
     code. Use the two cache-build lines in the root `CLAUDE.md`.
   - Expected: action lists identical; warning/error sets identical except for file-path strings
     (`rules.ron` → `state_machine.ron`). `stats --json` is not diffed, since its keys change by
     design (§4).

**Why a flat `global_on` list is behavior-identical to the rules interpreter** (from the code at
`59f10e9`, `runtime/scene_manager/message_interpreter.rs`):
- Both walk this frame's `UiEvent`s, then `GameEvent`s, then `SceneEvent`s, in reader order.
- Both format event names the same way (`ui.button_pressed:`, `scene_path_stem`).
- For each event, both push every matching binding's actions in file order through the same
  `rewrite_target`.
- Both insert `intent.slot.*` keys into `HandledIntentSlots` when a binding matches. Rules did
  this only for `GameEvent`s, but intents only ever arrive as `GameEvent`s.
- The only difference in matching is the same-frame state timing, and it only affects `when:`
  rules. There are none.
- **One real, minor, unobservable difference, documented rather than left silently true:**
  `fsm_interpreter_system` returns early when `LoadedStateMachine` is `None`, *before* draining its
  `MessageReader`s, whereas `message_interpreter_system` always drained them. The only frame where
  that matters is the project-load frame, before `LoadedStateMachine` is inserted: its
  `scene.requested:<initial>` event is never seen by the FSM interpreter. No live content binds
  `scene.requested:` for the initial scene, so nothing observable changes. Note it in the
  `fsm_interpreter_system` doc comment during Commit 4.

**Files that need hand work, not the script** (this is also the script's `HAND_HANDLED` set):
- **`valid_ui_trigger`** sets both paths today. Merge its rules into the existing
  `state_machine.ron` `global_on`, **placed first**. Rules actions were queued before FSM actions,
  so this keeps the order.
- **`unset_rules_path_with_convention_file`**: keep only its state_machine half (Commit 4 port).
- **`state_machine_only_ignores_dead_rules_ron`**: deleted in Commit 4 (premise gone).
- **`rules_path_case_mismatch`**, **`rules_path_custom_filename_is_discovered`**: ported to
  `state_machine_path_*` equivalents in Commit 4 (see Tasks).
- **`bad_rules_parse_no_cascade`**: deliberately unparseable; ported in Commit 4.
- **`inline_rules_are_discovered`**: inline `rules:` in a `.project.ron`. Replaced in Commit 4.
- **`blank_project`**, the `/new-project` template. The script handles it, then hand-edit the
  result into the minimal form shown under What **plus a 2–3 line explanatory comment** (it is the
  file every new designer copies first), e.g. "Rules in `global_on` fire on every event, whatever
  the state. Add `states:` + `transitions:` + `initial_state:` when you need modes like menu /
  playing / paused." `.claude/commands/new-project.md` copies the directory and does not name
  `rules.ron`, so it needs no change. `assets/projects/CLAUDE.md:321`'s "new project" steps do.

### 4. Removal

**Deleted from `ironhold_core`:**
- `schema/project.rs`:
  - `LogicRulesAsset`, together with its `validate()`
  - `LogicRule`
  - `ProjectConfig.rules` and `ProjectConfig.rules_path`
  - the V1/V2 field comments, which are rewritten
- `runtime/scene_manager/message_interpreter.rs`:
  - `message_interpreter_system` and `match_rules`
  - the "runs alongside `message_interpreter_system`" paragraph on `fsm_interpreter_system`
    (replaced by the early-return note from §3)
  - `intent_slot_key`/`scene_path_stem`/`rewrite_target` **stay**, because the two FSM
    interpreters use them
  - optional: add the `debug!("No binding matched …")` from `match_rules` to
    `fsm_interpreter_system`, so the "why didn't my event fire" trace isn't lost
- `runtime/scene_manager/mod.rs`:
  - `LoadedRules`
  - `PendingProjectLoads.rules`
  - the `LogicState` doc comment (:49), which mentions `EnterState`
  - **`SceneStateParams.logic_state` (:635)** — its only user is the `EnterState` executor arm
- `runtime/scene_manager/project_loader.rs`:
  - `PendingCatalogAssets.rules`
  - the `rules_path` load
  - the "both set" `warn!` (just corrected in `5b9209a`; it goes away completely)
  - the rules `LoadState::Failed` arm (:182)
  - both `insert_resource(LoadedRules(…))` sites (inline path :121, pending path :259-264)
  - **reword the state-machine load-failure `error!` (:194).** Today: "every state transition in
    this file is now inactive", which reads as irrelevant to someone with a flat zero-transition
    file. Replace with wording that covers the whole file generically, e.g. "proceeding without it
    — no project logic (global_on rules, states, transitions) will run".
- `lib.rs`:
  - `.init_resource::<LoadedRules>()` (:141)
  - `ImplicitRonPlugin::<LogicRulesAsset>` (:197)
  - `message_interpreter_system` from the interpreter `.chain()` (:259)
  - **fix the `DebugState.logic_state` doc comment (:61)**, which says "set by
    `Action::EnterState`" — rewrite to "the FSM's current state; empty for a flat (`global_on`
    only) logic file"
- **`Action::EnterState`** (see §5): `schema/actions.rs:138`, the executor arm
  (`action_executor.rs:532`), and `query.rs:596`'s label.

**Schedule re-anchoring. This is the easiest step to get wrong.** `message_interpreter_system` is
not only a chain member. It is the **ordering anchor** for 9 other registrations:
- `lib.rs`: `spawn_scene_v2` :227, `global_input_system` :238,
  `unclaimed_gamepad_trigger_system` :242, the stat pipeline chain :249, `audio_state_system`
  :252, `interactable_system` :327, `dialogue_tick_system` :333,
  `tick_delayed_events_system` :336
- `capabilities/action_bar.rs:97`: the action-bar chain, which the TargetingPlugin race fix also
  hangs off transitively

Every `.before(message_interpreter_system)` becomes `.before(fsm_interpreter_system)`:
- The chain was `message → fsm → entity_fsm → …`, so `before(message)` already implied
  `before(fsm)`. No edge to any remaining system is lost.
- **The compiler will not catch a missed site in a comment or doc**, but it will catch every code
  site, because the symbol no longer exists.
- **Do not** introduce a `SystemSet` here. The repo's convention is bare `.before(fn)` (see
  `crates/ironhold_core/src/CLAUDE.md`). Changing that convention is a separate decision.
- WASM is single-threaded, so ordering is observable there. That is one more reason the
  re-anchoring must be exact, not approximate.

**Deleted or changed in `ironhold_cli`:**
- `commands/validate.rs`:
  - the `rules` half of `resolve_logic_files`: the `rules_path` branch, the inline-`rules`
    fallback, and the no-`.project.ron` `"logic/rules.ron"` convention fallback. **The
    `state_machine.ron` convention fallback stays** for fixtures that have no `.project.ron`.
  - the `rules` field of the logic-files struct (:746) and of the context (:84)
  - the rules loops in `collect_all_actions` (:835), `collect_handled_events` (:2428), and
    `check_orphan_ui_rules` (:2923)
  - the rules half of the "convention file exists but field unset" check (:3055-3067)
  - every doc comment that names `rules_path`/`LogicRulesAsset`
  - (the `StateMachineAsset::validate()` wiring was already added in Commit 2, §2)
- `commands/query.rs`:
  - the `LogicRulesAsset` branch of `query rules` (:458-525) and of `query actions` (:666)
  - the `EnterState` label
  - **Fold in the open `claude_suggestions.md` item (lines 477/484):** `query`/`stats` still
    hardcode `"logic/state_machine.ron"`. Make them resolve `state_machine_path` through the same
    `resolve_logic_files` that `validate` uses. These are the same lines being rewritten, and
    without the fix the only logic file left would still be resolved two different ways. The
    command stays named `query rules`, with its help text updated to describe what it lists now.
- `commands/stats.rs`: `rule_count` (:119) becomes FSM counts (global rules, states,
  transitions). The `--json` key changes from `"rules"` to those counts. That is acceptable
  because nothing outside the repo consumes it.
- **`commands/utils.rs::resolve_catalog_paths`: warn on an unparseable `.project.ron`.** It calls
  `silent_parse` on the project file and, on failure, silently falls back to convention paths.
  After this change, a stale `rules_path:` makes the `.project.ron` fail to parse, so `query` and
  `stats` would quietly report convention-path results with zero diagnostic (validate and the
  runtime both already fail loudly). Add an `eprintln!` warning (stderr, so `--json` stdout stays
  clean) naming the file and the parse error whenever a `.project.ron` exists but fails to parse.

**No `schema_version` bump for `ProjectConfig`.** Decision and reasoning:
- `ProjectConfig` has `#[serde(deny_unknown_fields)]`. Once the fields are deleted, any
  `.project.ron` that still says `rules_path:` or `rules:` **fails to parse**, with a RON error
  that names the field (`unknown field 'rules_path', expected one of …`). How that surfaces,
  stated honestly:
  - **at runtime: a permanent loading-screen hang.** The browser console shows a `bevy_asset` load
    error naming the field, but `check_project_loaded` has no `LoadState::Failed` arm for the
    `ProjectConfig` handle itself, so it spins forever. This is pre-existing behavior for *any*
    `.project.ron` parse error (a sibling of the backlog's "No retry or timeout for a failed
    ordinary scene asset load" item), not something this plan introduces — but this plan makes it
    newly reachable by old content, so it must be documented in the "Removed" callout and logged
    as its own backlog bug (see §6 Planning). Fixing it is out of scope here.
  - in `ironhold validate`: exit 1 with a parse error naming the field
  - in `query`/`stats`: the new stderr warning above

  That is the hard, loud error a version bump would exist to produce, and it comes for free.
- `schema_version` exists to tell apart formats that cannot be told apart by their structure. Here
  the old format *can* be told apart: the removed field is present.
- A bump would also mean editing every `.project.ron` (16 shipped, plus the CLI fixtures) and
  every inline `ProjectConfig` RON string in `ron_validation.rs`, only to change a number.
- `ProjectConfig::validate()` keeps accepting 1–3. V1 still means something, because inline
  `model_fixes` is V1.
- `StateMachineAsset` stays at `1`, per §2.

### 5. `Action::EnterState`: removed (decided — Frank, 2026-09-23)

`EnterState` exists as rules.ron's way to change state.
- Once this change lands, the only places it could still be written are FSM bindings and
  `.behavior.ron` files. In an FSM binding it would change `LogicState` **without** running exit
  or entry hooks, which is exactly the out-of-band desync the investigation flagged. In a behavior
  file it would change the **project's** logic state, not the entity's.
- `docs/30_…:397` already tells authors "you do not write `EnterState` in FSM data".
- After §1 there are **zero** content uses. What remains is 2 tests in `fsm_tests.rs`, one
  `query.rs` label, and docs.
- **Verified against the schema before deciding, not assumed**: `FsmTransition.from: Option<String>`
  is documented as "omit (or `None`) to match any current state" — so the common `EnterState`
  pattern ("go to state X from wherever the game currently is") is exactly **one** transition with
  `from` omitted, not one transition per possible source state. There is no capability regression:
  every use case `EnterState` covered maps onto `global_on`/per-state `on:`/`transitions` with no
  loss, and the one thing that changes (a real transition always runs the target state's
  `entry_actions` and the source state's `exit_actions`) is a correctness improvement over
  `EnterState`'s silent hook-skipping, not a tradeoff.

**The replacement recipe is `EmitEvent("name")` + a transition on that event.** Full worked example
(a dialogue choice that starts a quest, replacing the old `EnterState("on_quest")`):

```ron
// dialogues/npc_quest_giver.dialogue.ron — a choice's do_actions
do_actions: [
    SetVariable("quest_started", "true"),
    EmitEvent("quest.started"),
],
```

```ron
// logic/state_machine.ron
(
    schema_version: 1,
    initial_state: "exploring",
    states: [
        (
            name: "exploring",
            on: [ ( event: "quest.started", do_actions: [ Log("Quest offered") ] ) ],
        ),
        (
            name: "on_quest",
            entry_actions: [
                Log("Quest started"),
                SetVariable("quest_label", "Quest: find the crystal"),
                PlayMusicLoop(key: "quest_theme"),
            ],
            exit_actions: [ StopMusic ],
        ),
    ],
    transitions: [
        ( from: "exploring", on: "quest.started", to: "on_quest" ),
    ],
)
```

(`entry_actions`/`exit_actions` at the project level have no `{self}` to resolve — that substitution only applies
inside a per-entity `.behavior.ron` file — so this example deliberately uses `Log`/`SetVariable`/
`PlayMusicLoop` rather than an entity-targeted action like `ShowFloatingText`, to stay correct as
a project-level FSM example rather than accidentally implying `{self}` works here.)

The dialogue choice never names a state directly — it only emits an event. The transition is what
actually changes `LogicState`, and only the transition's firing runs `on_quest`'s `entry_actions`
(the music swap, the `quest_label` UI variable a `Label` widget can bind to) and `exploring`'s
`exit_actions`. An `EnterState("on_quest")` in the old system would have changed the state with
**neither** of those side effects running — exactly the silent desync bug class this removal
closes. For a gameplay-driven (rather than dialogue-driven) condition, the same pattern applies:
emit a `GameEvent` (`stat_threshold_system` already does this for stat-threshold conditions) and
add a transition on it, instead of reaching for `EnterState` from a capability system.

This worked example is not just internal reasoning — §6 requires it to actually land in
designer-visible docs (not only `crates/ironhold_core/src/CLAUDE.md`, a dev-only file): the
"Removed: rules.ron" callout in `docs/20_data_formats.md`, that same file's Action table (the new
`EmitEvent` row), and `docs/30_runtime_events_and_logic.md`'s new "Project logic" section all get
this exact example (or an equivalent one using existing shipped-project vocabulary). Rewrite the
`crates/ironhold_core/src/CLAUDE.md` "Conditions on rules" section to match.

This is an `Action` enum removal. Existing RON that uses it would fail to parse. There is none.

### 6. Docs, including the absorbed migration-guide item

**Coverage rule:** the line numbers below are a starting point from `59f10e9`, not the complete
list. The authoritative completeness check is the second grep in Acceptance criteria (over
`assets/`, `docs/`, `README.md`, `index.html`). Run it at the end of Commit 5 and fix every hit
outside the "Removed" callout. This is deliberately a grep rather than an exhaustive line list in
this plan: line numbers drift with every intervening merge, and the grep catches the ~40 shipped
RON comments (`local_coop_demo`, `particles_demo`, `dynamic_animation_control`,
`entity_logic_demo`'s scene/prefab files) that a hand-maintained list would miss.

- **`README.md`** (designer-facing, was missing from the original list): it presents "Rules
  workflow (`logic/rules.ron`) — simpler projects" as a real option (~:58-60, ~:166). Replace with
  one "Game logic" section: a flat `global_on` example first, then "add states/transitions when
  you need modes".
- **`docs/20_data_formats.md`:**
  - Delete the `rules_path` row (:107) and the `rules` row (:119). Rewrite the
    `state_machine_path` row (:108) and the project example (:133).
  - Delete the `## logic/rules.ron — LogicRulesAsset` section (:3770+).
  - Rewrite the ":4061 despite the v2 vs v3 workflow framing" paragraph.
  - Update the file tree (:62) and the asset-type table (:83).
  - Rewrite about **15 example snippets** that use `( on: …, do_actions: … )` rule syntax into
    `global_on` `( event: …, do_actions: … )` form (the `// logic/rules.ron` headers at :1709,
    :2525, :3091, :3135, :4009, :4182, and others — the grep finds the rest).
  - Delete the `EnterState` row (:3852). **Add an `EmitEvent("name")` row** to the Action table
    (today only `EmitEventAfterDelay` is listed), cross-referencing the state-change recipe.
  - Update the deny_unknown_fields error example (:3813-3825), which quotes a `LogicRulesAsset`
    loader error.
  - **Add a "Removed: `rules.ron`" callout.** This is the whole absorbed "migration guide"
    deliverable (see Open questions). Contents:
    - the mapping table: rule without `when:` → `global_on`; `when: S` → a rule inside state
      `S`'s `on:` list; `EnterState("S")` → `EmitEvent("go_s")` + a transition `on: "go_s"`, with
      the dialogue-choice worked example from §5;
    - **the `on:` overload callout.** In the new format `on` means three different things: a
      state's `on:` is a *list of rules*; a transition's `on:` is an *event-name string*; a rule
      itself uses `event:`, **not** `on:`. Old rules.ron muscle memory (`( on: "x", do_actions: … )`)
      produces a plausible-looking file that fails to parse — show the wrong and right forms side
      by side;
    - the verbatim error text, so it is greppable: `unknown field 'rules_path', expected one of …`
      (and the `rules` variant), plus the note that at runtime this shows as a loading screen that
      never finishes, with the error in the browser console;
    - one line that `schema_version: 1` in a migrated file is not a downgrade (§3).
- **`docs/30_runtime_events_and_logic.md`:**
  - Rewrite `## Logic rules: mapping Events → Actions` and "When to use `logic/rules.ron`"
    (:210-245) into "Project logic: `state_machine.ron`". Teach it flat-first (`global_on`), then
    add states and transitions, using today's pause example rewritten as transitions. Include:
    - **per-event evaluation order**: `global_on` → the current state's `on:` → the first matching
      transition;
    - **the `initial_state` entry-actions gotcha**: `initial_state`'s `entry_actions` do **not**
      run at boot — entry actions only run when a transition enters a state. So when a designer
      follows "start flat, add states later", boot-time work stays in a
      `global_on` `scene.ready:<scene>` rule (or moves to a transition out of a boot state);
      otherwise the "add states later" step silently drops it;
    - the `EmitEvent` + transition recipe from §5, with the worked example;
    - the same `on:` overload callout as docs/20 (or a link to it).
  - **:121**: rewrite the `{target}` example so it no longer cites the deleted
    `3rd_person_game_demo/logic/rules.ron` (§1 non-goal). Inline the `target.changed` →
    `ShowFloatingText(entity: "{target}", …)` pair as a standalone snippet instead of citing a file.
  - Fix :43, :45, :59, :181, :270, :317, and :342, plus any further grep hits.
  - About 7 snippets need rewriting.
- **`docs/00_overview.md`:** rewrite the quick-start logic section (:116, :121, :145-165, 3
  snippets) to `state_machine_path` + `global_on`. Fix :63.
- **`docs/STATUS.md`:**
  - Delete the "rules.ron workflow (schema v2)" section (:132-136).
  - Fix the 0.3 row (:18), the state-gated-rules row (:33), the `EnterState` ABI line (:100),
    and the debug-state JSON example that shows `EnterState` (:123).
- **`docs/10_architecture.md`:** :29 (`logic_state` description) and :113 (the project-level
  rules bullet).
- **`docs/60_contributing.md`:** the validate checks list (:243-285, rules mentions — and add the
  new FSM/behavior `validate()` check) and the `query rules`/`query actions` descriptions
  (:334-336). Also note that `query`/`stats` now honor `state_machine_path` and warn on an
  unparseable `.project.ron`.
- **`index.html:849`**: "rules catalogs" wording → "logic files" (or equivalent).
- **`crates/ironhold_core/src/CLAUDE.md`:**
  - the pipeline step 2 (:13) and the interpreter chain list (:116-120, now 3 members)
  - "Conditions on rules" (:71-76, per §5)
  - the deny_unknown_fields container list (:49-50)
  - the schedule-anchor passages (:298-302, :470, :686, :1600) → `fsm_interpreter_system`
  - the `{target}` passages that say "global rules.ron" (:161, :184, :194, :198, :231)
- **Root `CLAUDE.md`:**
  - the data-driven loop step 2 ("`LogicRules` (from `logic/rules.ron`)")
  - the asset layout tree
  - the "projects may have `rules.ron`, `state_machine.ron`, or both" note
  - the `query rules` one-liner
- **`assets/projects/CLAUDE.md`:** the file table (:17), the `EnterState` example (:52), 5
  snippets, and the new-project steps (:321).
- **`crates/ironhold_core/tests/CLAUDE.md`:** check for rules fixtures guidance.
- **Agent and command definitions:**
  - `.claude/agents/ron-gameplay-scripter.md`. It is the logic-authoring agent. Lines 34/65/86
    present rules.ron as an option. Line 65 also lists `on_enter`/`on_exit`/`when`, which are not
    real field names (the real ones are `entry_actions`/`exit_actions`). Fix it while there.
  - `.claude/agents/{alignment-reviewer,debug-detective,system-architect}.md`
  - `.claude/commands/query.md:16`
  - Check `.opencode/prompts/` too. None were found at `59f10e9`.
- **Agent memory cleanup** (`.claude/agent-memory/**`, ~60 files mention rules) — **do this only
  after Commits 1-4 have actually landed**, i.e. as part of this same Commit 5, not speculatively
  before. Frank's explicit concern (2026-09-23): a stale memory file describing
  `message_interpreter_system`/`LogicRulesAsset`/`rules_path` as live, current behavior is exactly
  the kind of thing that causes a *future* review agent to hallucinate — confidently citing code
  that no longer exists, or recommending a fix framed against a system that's gone. This is not
  optional documentation polish; treat it with the same rigor as the code-level acceptance
  criteria.
  - Update or retire every file whose claims become false. At minimum:
    - `alignment-reviewer/rules_vs_state_machine_coexistence.md`
    - `debug-detective/project_logic_file_on_disk_is_not_loaded.md`
    - `system-architect/capability_patterns.md`, `system-architect/cli_validate_coverage_model.md`,
      `system-architect/rules_vs_fsm_consolidation.md` (mark shipped)
  - **Grep all of `.claude/agent-memory/` for `rules\.ron|rules_path|EnterState|LogicRulesAsset|LoadedRules|message_interpreter_system`
    and read every hit, not just the ones above** — the named files are a floor, not the whole
    scope. For each hit: if the memory describes now-removed code as current, correct or delete it;
    if it's a historical note that's still accurate as history (e.g. "this bug existed in the old
    rules interpreter, fixed by X"), leave it, since deleting true history isn't the goal — only
    false-as-of-now claims are.
  - Per the root `CLAUDE.md` step 10, these edits land in the primary checkout (not the feature
    worktree) and are committed separately on `integration`, alongside the feature merge.
- **Planning:**
  - Close the backlog's "Schema version v2→v3 migration guide" (Designer Experience) as
    superseded when this ships. Relocate it into Done next to this item with a
    "superseded by …" note.
  - Link this plan from the backlog's "Consolidate rules.ron…" item.
  - Strike claude_suggestions 477/484 when §4's query/stats fold-in lands.
  - **Log as separate backlog follow-ups (not part of this branch):**
    - Bug: a `.project.ron` parse error (now reachable by a stale `rules_path:`) hangs the loading
      screen forever — no `LoadState::Failed` arm for the `ProjectConfig` handle in
      `check_project_loaded`. Sibling of the existing scene-asset hang item.
    - Feature: a **permanent** `ironhold validate` diagnostic for a leftover `logic/rules.ron`
      sitting unreferenced on disk ("rules.ron is no longer supported — move its rules into
      state_machine.ron's `global_on`"). This plan's tripwire test only protects this repo's CI,
      not `Ironhold-fps-demo` or future projects that may still carry a stray file.
    - Optional: port `3rd_person_game_demo`'s dead `target.changed` `{target}` pair into its live
      `state_machine.ron` if Frank wants that behavior (§1 non-goal).

## Tasks

Everything is on one branch, `feature/rules_to_state_machine_consolidation`. The five commits run
in order. Before starting: `git worktree list`. **Confirm no other in-flight feature branch edits
any `logic/rules.ron`, `validate.rs`, or `message_interpreter.rs`.** Such a branch would conflict
on merge or bring back a dead file. §4's tripwire test catches the second case after the fact,
but it is better to avoid it.

**Commit 1: cleanup**
- [ ] Delete `3rd_person_game_demo/logic/rules.ron` and `terrain_demo/logic/rules.ron`. Record the
  `dance` finding and the `{target}`-example non-goal from §1 in the commit message.

**Commit 2: schema loosening + `validate()` wiring**
- [ ] `StateMachineAsset`: `#[serde(default)]` on `initial_state`, `states`, `transitions`.
  `validate()` rejects an empty `initial_state` only when `states` is non-empty. Rewrite the doc
  comment.
- [ ] Wire `StateMachineAsset::validate()` into `ironhold_cli validate` (project FSM + every
  behavior file) as a hard error, and into `resolve_pending_behaviors_system` as an `error!`
  (log-and-continue, matching `project_loader.rs:270`).
- [ ] Triage any new diagnostics on shipped content and CLI fixtures: fix real content bugs;
  update expected output for deliberately-invalid fixtures.
- [ ] `ron_validation.rs`: add tests.
  - A minimal `schema_version` + `global_on` file parses and validates.
  - `states` non-empty with `initial_state` missing fails `validate()`.
  - A file with every field present still parses (regression).
- [ ] CLI test (`validate_cross_file.rs`, new fixture): a **behavior file** with non-empty
  `states` and no `initial_state` → exit 1, error names the file. Plus the same for the project
  FSM.

**Baseline capture (after Commit 2, before Commit 3; not committed)**
- [ ] Rebuild `tools/bin/ironhold` from current source. Capture `--json query actions` and
  `--json validate --strict` for every shipped project into the scratchpad (§3 step 3).

**Commit 3: content migration (old interpreter still present; this is the proof point)**
- [ ] Write the script (§3), including: abort-on-existing-target, abort-on-unmatched-`rules_path`,
  the explicit `HAND_HANDLED` set, and string-literal-aware depth tracking. Run it over
  `assets/projects/` + `crates/ironhold_cli/tests/fixtures/`.
- [ ] Hand-merge `valid_ui_trigger` (rules first in `global_on`). Hand-edit `blank_project` into
  the minimal form plus its explanatory comment.
- [ ] Add `tests/rules_migration_equivalence.rs` (with the `when == None` assertion and the
  `bad_rules_parse_no_cascade` allowlist). It must pass.
- [ ] `cargo test -p ironhold_core --test ron_lint --test ron_validation` on the generated files.

**Commit 4: removal**
- [ ] Core deletions from §4, including `Action::EnterState`, `SceneStateParams.logic_state`, the
  `DebugState.logic_state` doc comment fix, and the reworded state-machine load-failure `error!`.
- [ ] Add the `fsm_interpreter_system` early-return note (§3) to its doc comment.
- [ ] Re-anchor the 9 schedule sites to `fsm_interpreter_system`.
- [ ] CLI deletions from §4. Make `query`/`stats` resolve `state_machine_path`. Add the
  `resolve_catalog_paths` stderr warning for an unparseable `.project.ron`.
- [ ] Delete every `rules.ron`/`my_custom_rules.ron`, the script, and
  `rules_migration_equivalence.rs`.
- [ ] Core tests:
  - `fsm_tests.rs`: **delete** `test_enter_state_action_updates_logic_state` and
    `test_state_gated_rule_only_fires_in_matching_state`, which test removed features. **Port to
    `global_on`** `test_rules_no_match_does_not_queue_action` and the four
    `test_rules_scene_event_{ready,loaded,requested,unloading}_triggers_action`. They are the only
    coverage of the `requested`/`unloading` event-name formatting.
  - `entity_logic_tests.rs` (5 tests, incl. the documented
    `test_rule_overridden_intent_still_resolves_target_against_primary_player_only` scope
    boundary) and `ui_tests.rs` (2 tests): these used `LoadedRules` only to inject a global
    binding. Rewrite them to `LoadedStateMachine(Some(…))`. Add a
    `support::global_bindings(&[(&str, Vec<Action>)]) -> LoadedStateMachine` helper so the rewrite
    stays one line per test. Internal test names/assert messages may say "binding".
  - `ron_validation.rs`:
    - delete the `LogicRulesAsset` parse tests (:854-905)
    - port the deny_unknown_fields typo test (:2326) to `FsmEventBinding`
    - port the SpawnEffect round-trip (:3899) into a `global_on`
    - drop `rules: []`/`rules_path` from the inline `ProjectConfig` strings (:23, :41, :56, :97,
      :110) and the `rules_path.is_none()` assert (:87)
    - **add** `project_config_with_rules_path_is_rejected` and
      `project_config_with_inline_rules_is_rejected` (parse error). These lock in the hard-error
      behavior.
  - `scene_lifecycle_tests.rs:194`: remove the two fields from the struct literal. The compiler
    will force this.
  - `support/mod.rs:47`: remove `init_asset::<LogicRulesAsset>()`.
  - `assets_schema_version_regression.rs`: drop the `LogicRulesAsset` branch. **Add a tripwire:**
    no file named `rules.ron` exists under `assets/projects/` or `crates/ironhold_cli/tests/fixtures/`.
    After this change any such file is dead by definition. This guards against a stale parallel
    branch bringing one back.
  - `local_coop_tests.rs:518/535` and `corpse_loot_interact_tests.rs:115`: comment wording only
    (stale `message_interpreter` references).
- [ ] CLI tests (`validate_cross_file.rs`, about 43 references):
  - The ~32 fixtures with no `.project.ron` now use the `logic/state_machine.ron` convention
    fallback. Update expected-output strings that quote `logic/rules.ron`.
  - **Port the rules_path-semantics fixtures instead of deleting them.** They are the only coverage
    of `resolve_logic_files`' configured-path handling, and `state_machine_path` has almost no
    equivalents:
    - `rules_path_case_mismatch` → `state_machine_path_case_mismatch`
    - `rules_path_custom_filename_is_discovered` → `state_machine_path_custom_filename_is_discovered`
    - `rules_path_pointed_at_wrong_file_type` → `state_machine_path_pointed_at_wrong_file_type`
    - `bad_rules_parse_no_cascade` → `bad_state_machine_parse_no_cascade`
    - `bad_rules_path_and_state_machine_path` → `bad_state_machine_path` (missing file)
    - `unset_rules_path_with_convention_file`: keep only its state_machine half
    - `orphan_ui_rule`: convert its content
    - `bad_action_item_key`: already migrated by the script; verify its expected output
  - **Delete** `state_machine_only_ignores_dead_rules_ron` (its premise no longer exists).
  - **Replace** `inline_rules_are_discovered` with `project_with_rules_path_fails_to_parse`
    (exit 1, the error names `rules_path`).
  - `query_stats_configured_paths.rs`: add a relocated-`state_machine_path` case for `query rules`
    / `query actions` / `stats`, and an unparseable-`.project.ron` case asserting the stderr
    warning (and that stdout `--json` stays valid).
- [ ] Full suites (one binary at a time, per root `CLAUDE.md`) + `cargo check -p ironhold_cli` +
  `cargo test -p ironhold_cli`.
- [ ] Rebuild `tools/bin/ironhold`, then run the §3 behavioral cross-check against the
  post-Commit-2 baselines.

**Commit 5: docs + planning**
- [ ] Everything in §6, including `README.md`, the docs/20 `EmitEvent` row and "Removed" callout
  (mapping table, `on:` overload, verbatim error text, loading-hang note), docs/30's evaluation
  order + `initial_state` gotcha + recipe, the docs/30:121 `{target}` rewrite, and `index.html:849`.
- [ ] Run the **second acceptance grep** (over `assets/`, `docs/`, `README.md`, `index.html`) and
  fix every hit outside the "Removed" callout — this is what catches the ~40 shipped RON comments.
- [ ] Log the three backlog follow-ups from §6 Planning.
- [ ] Agent-memory updates (§6) — committed separately on `integration` per step 10.

**Then the normal workflow:**
- [ ] Review trio + `ux-gamedesigner-reviewer` (schema/docs/assets changed) + `wasm-perf-reviewer`
  (the interpreter chain is a per-frame system, but the change only removes work, so a quick pass
  is expected; behavior `validate()` runs once per behavior resolution, not per frame).
- [ ] Step 6 CLI spot-check (`cargo run -p ironhold_cli -- query actions …` on a migrated project
  and on `3rd_person_game_demo`), then rebuild `tools/bin/ironhold`.
- [ ] WASM dev build + playtest (checklist below).

**Playtest checklist.** Each item is a migrated project whose rule-driven behavior is visible.
Every one should behave exactly as on `main`.
- `quick_scene`, `blank_project`: boots, scene-ready log.
- `particles_demo`, `effect_mayhem_demo`: every effect button fires.
- `camera_modes`: every mode-switch button/key.
- `dynamic_animation_control`: every animation/control button.
- `local_coop_demo`: join/leave, portal `LoadScene`.
- `entity_logic_demo`: rule-driven spawns/interactions.
- `foliage_demo`: boots.
- `integration_tests` (via `test_web.py`).
- Plus the FSM regression projects `3rd_person_game_demo` (pause/menu nav, behavior-driven
  monsters/corpses — exercises the new behavior `validate()` path) and `terrain_demo`.
- Browser console: no asset-load errors, no new `Invalid StateMachineAsset` errors.

## Open questions

None remaining that block starting — the one live question (`Action::EnterState`) is resolved
below. Everything else was already decided in the plan or by Frank during plan-review.

- **`Action::EnterState`: decided — remove it (Frank, 2026-09-23).** Confirmed via the schema
  (`FsmTransition.from: Option<String>`, "omit to match any current state") that this is not a
  capability regression — see §5's full reasoning and worked `EmitEvent` + transition example.
- **Backlog "Schema version v2→v3 migration guide": decided, not open. It is superseded and
  closed when this ships.** Its premise is already stale: every shipped project is already at
  `schema_version: 3`, and "rename `rules_path` → `state_machine_path`" is exactly what this
  feature does for all content. Its only lasting value, a record of the mechanical mapping, is
  §6's "Removed: rules.ron" callout. Frank can overrule this if he wants a longer standalone
  guide, but with no external authors there is nobody for it to serve.
- **Decided within the plan (flagged for review, not for Frank):**
  - no `ProjectConfig` version bump (§4)
  - `initial_state` defaults to `""`, to keep `LogicState` identical (§2)
  - behavior `validate()` failures log-and-continue at runtime, matching the project-FSM precedent
    (§2); hard error in the CLI
  - the `query`/`stats` state_machine-path fix and the unparseable-`.project.ron` warning are
    folded in (§4)
  - the `ProjectConfig` loading-hang is documented and logged, not fixed here (§4)
  - `query rules` keeps its name
  - no `SystemSet` introduced for the re-anchoring
  - docs completeness is enforced by grep, not an exhaustive line list (§6)

## Acceptance criteria

- Given the repo after this change, when grepping `crates/` for `rules_path|LogicRule|LoadedRules|LogicRulesAsset|message_interpreter_system|EnterState`,
  then there are zero code hits.
- Given the repo after this change, when grepping `assets/`, `docs/`, `README.md`, and
  `index.html` for `rules\.ron|rules_path|EnterState`, then the only hits are inside docs/20's
  "Removed: rules.ron" callout (and any docs/30 link to it).
- Given `assets/projects/` and `crates/ironhold_cli/tests/fixtures/`, when the schema-regression
  test runs, then it finds no `rules.ron` file anywhere.
- Given a `.project.ron` containing `rules_path:` or `rules:`, when run through `ironhold validate`,
  then it fails with a parse error naming the field (exit code 1); when loaded at runtime, the
  console shows a load error naming the field (the loading-screen hang is the documented,
  pre-existing behavior, logged separately); when run through `query`/`stats`, a stderr warning
  names the parse failure. None of the three silently ignores the field.
- Given a `state_machine.ron` with only `schema_version` + `global_on`, when loaded, then it parses,
  validates, and `LogicState` is `""`.
- Given a project `state_machine.ron` **or a `.behavior.ron`** with non-empty `states` and no
  `initial_state`, when run through `ironhold validate`, then it exits 1 naming the file; when the
  behavior is resolved at runtime, an `error!` is logged naming the file.
- Given each migrated project, when comparing `query actions --json` and `validate --strict --json`
  from the post-Commit-2 baseline against post-Commit-4 (freshly rebuilt binary both times), then
  the action lists are identical and the diagnostics are identical except for file paths.
- Given a relocated `state_machine_path`, when running `query rules`/`query actions`/`stats`, then
  they read the configured file, not the convention path.
- Given the full `ironhold_core` + `ironhold_cli` suites and `python test_web.py` (existing
  baselines, no `--update-baselines`), when run on `integration`, then all pass.
- Given the playtest checklist, when Frank runs each project in the WASM dev build, then every
  rule-driven interaction behaves as it does on `main`.
