# Investigation: rules.ron vs state_machine.ron — keep both, or consolidate?

_Investigated at `f64a970` (2026-09-23) on `integration`; the interpreter code was read in
`feature/fix_stale_logic_path_warning` at `5b9209a`. Only the `project_loader.rs` warn text
differs between the two._

**Status:** open architectural question. **Frank decides.** Nothing here has been implemented.

**Triggered by:** `feature/fix_stale_logic_path_warning`. That fix corrected a `warn!` that
wrongly said rules.ron is disabled when `state_machine_path` is set. During the playtest Frank
asked "why warn at all when both are active?" That question leads to a bigger one: should the
engine have two project-level logic systems at all?

---

## 1. Facts (from the code, not the docs)

### Runtime

| | `rules_path` / inline `rules:` | `state_machine_path` |
|---|---|---|
| Schema | `LogicRulesAsset { schema_version, rules: Vec<LogicRule> }`, `LogicRule { on, when: Option<String>, do_actions }` (`schema/project.rs`) | `StateMachineAsset { schema_version, initial_state, states, transitions, global_on }` (`schema/project.rs`) |
| Interpreter | `message_interpreter_system` → `match_rules` (`runtime/scene_manager/message_interpreter.rs`) | `fsm_interpreter_system` (same file) |
| State model | Reads `LogicState`. Only changes it through `Action::EnterState`, which the executor applies later in the frame and which runs no entry/exit hooks | Owns `LogicState`. Transitions change it **straight away**, so later events in the same frame see the new state, and they run exit then entry actions |
| Gating | Single string: `when: "<state>"` or nothing | `global_on`, per-state `on:`, and transitions (first match wins) |
| Also used for | nothing else | **Per-entity behaviors.** Every `behaviors/*.behavior.ron` is a `StateMachineAsset` run by `entity_fsm_interpreter_system` |
| Third form | inline V1 `ProjectConfig.rules`, used when `rules_path` is unset | — |

Both interpreters are always registered and nothing gates one against the other. Chain order is
`message_interpreter_system`, then `fsm_interpreter_system`, then `entity_fsm_interpreter_system`,
then `action_executor_system`.

**rules.ron can express nothing that state_machine.ron cannot.** The mapping is exact:
- a rule with no `when:` = a `global_on` binding
- a rule with `when: "S"` = an `on:` binding inside state `S`
- `EnterState` from rules = a transition, minus the entry/exit hooks

The only real difference is timing inside one frame. Rules check *every* event of the frame
against the state as it was at the **start** of the frame. The FSM moves forward one event at a
time. That only matters for rules that use `when:`.

### Shipped content

- 11 projects are rules-only: `blank_project`, `camera_modes`, `dynamic_animation_control`,
  `effect_mayhem_demo`, `entity_logic_demo`, `foliage_demo`, `integration_tests` x3,
  `local_coop_demo`, `particles_demo`, `quick_scene`.
- 5 projects are FSM-only: `3rd_person_game_demo`, `custom_materials`, `primitive_world`,
  `stats_demo`, `terrain_demo`.
- **No shipped project sets both.** The only config that does is the CLI fixture
  `crates/ironhold_cli/tests/fixtures/valid_ui_trigger/`.
- **None of the live rules projects uses `when:` or `EnterState`.** The only file that uses
  `when:` (18 of its 23 rules) is `3rd_person_game_demo/logic/rules.ron`, and that file is dead:
  the project never sets `rules_path`.
- **Every live rules.ron is therefore just a flat `global_on` list.** In practice the state
  features of rules.ron are unused.
- There are two dead rules.ron files. Neither is loaded, and each is covered by the project's
  state_machine.ron:
  - `3rd_person_game_demo/logic/rules.ron`: 23 rules of old menu/pause logic.
  - `terrain_demo/logic/rules.ron`: 2 rules. `quit` is already in the FSM's `global_on`, and
    `dance` looks like a leftover.
- No shipped project uses inline V1 `rules:`. Only `tests/ron_validation.rs` fixtures do.
- `blank_project` is rules-only, and it is the template `/new-project` copies. So every new
  project starts on rules.ron today.

### Tooling and docs

- `ironhold_cli validate` has a rules branch next to every FSM branch:
  - `resolve_logic_files` (rules_path, inline fallback, convention fallback)
  - `collect_all_actions`
  - `collect_handled_events`
  - `check_orphan_ui_rules`
- `query rules`/`actions`/`events` and `stats` hardcode `logic/rules.ron`. As a result they
  report the two dead files as if they were live. This is already tracked as a known gap.
- Nothing checks whether a `when:` value is a real FSM state name, or whether both interpreters
  handle the same event.
- The docs already lean toward the FSM, but inconsistently:
  - `docs/00_overview.md:121`: "use `state_machine_path` instead of `rules_path`"
  - `docs/30_runtime_events_and_logic.md:~45`: "Replaces `rules.ron` for FSM projects"
  - `StateMachineAsset`'s doc comment: "Replaces `logic/rules.ron`"
  - the `ProjectConfig` field comment: "replaces rules_path"
  - `planning/backlog.md`'s "Schema version v2→v3 migration guide" item already plans to
    "rename `rules_path` → `state_machine_path` … convert `rules.ron` to the FSM format"

  So an unfinished intent to consolidate already exists. It was never made official.

---

## 2. Option A — keep both

Two sub-variants: (A1) two *alternative* schemas, one per project; (A2) both allowed together in
one project, e.g. rules for UI and the FSM for gameplay.

### Pros
- **No migration cost.** Nothing to rewrite, no test churn, no risk to the demo baselines.
- **Easy to start with.** A two-line rules.ron (`blank_project`) is the smallest possible "hello
  logic". The FSM needs `initial_state`, `states`, and `transitions` even when all three are
  empty. `transitions` has no `serde(default)` today.
- **Clear learning path.** "Start with rules, move to the FSM when you need states" is a common
  pattern that designers already understand.
- **The code cost is modest.** `match_rules` is about 30 lines, and the CLI rules branches are
  small loops. The *code* is not the expensive part (see the cons).
- **A2 has a real purpose on paper:** a flat, always-on UI layer kept apart from gameplay states.

### Cons
- **Two dialects mean the same thing.** Rules are a strict subset of the FSM, and the live rules
  projects use only the part that equals `global_on`. There are two ways to write the same
  mapping, with different field names (`on:` vs `event:`) and different files.
- **The per-entity behaviors already use the FSM schema.** A designer who writes any
  `.behavior.ron` must learn the FSM anyway. rules.ron is the *only* logic file that is not an
  FSM. Dialogue choices are a separate, narrower surface. Keeping rules.ron keeps a third form.
- **Coexistence has caused a string of confusion bugs, not a one-off:**
  - the false "NOT loaded" warn, which shipped and has only now been fixed
  - four doc and comment places that say "replaces", which is still wrong
  - two dead rules.ron files that look live, including one with 23 real-looking rules
  - `query`/`stats` count the dead files as live
  - Frank himself believed `3rd_person_game_demo` runs both

  In ironhold, the cost of two systems shows up as confusion, not as lines of code.
- **Running both in one project (A2) is unsafe, and nothing detects it:**
  1. **Double firing.** Both interpreters match the same event in the same frame and both queue
     actions: rules actions first, then FSM actions. Nothing warns about it.
  2. **Split view of state.** Rules check all of a frame's events against the state at the start
     of the frame. The FSM changes state mid-frame. For "event 1 causes a transition, event 2 in
     the same frame is gated", the two interpreters disagree about the current state.
  3. **`EnterState` from rules.ron bypasses the FSM.** It changes `LogicState` without running the
     exit/entry actions of the state being left or entered. Those actions do things like
     spawning, showing UI, or pausing, so the result is a desync that is hard to debug.
  4. **`when:` names are not checked** against the FSM's `states`. A typo means the rule silently
     never fires.
- **Every new logic feature has to be built twice or picked for one.** Examples: intent handling
  (`HandledIntentSlots`, already duplicated), `{target}` substitution, any future conflict
  checker, and the planned event validator (backlog "Magic-string event/action validator"). This
  is the same reasoning `crates/ironhold_core/src/CLAUDE.md` uses against adding a general
  condition system: don't add authoring machinery unless the existing pattern is genuinely
  insufficient. The existing pattern is the FSM, and it is sufficient.
- **The docs need two sections and a "which should I use / what if I use both" explanation**, and
  that explanation is the part that keeps going stale.

---

## 3. Option B — consolidate on state_machine.ron

### Pros
- **One logic schema for everything.** The project level and the entity level use the same
  `StateMachineAsset` and the same mental model. That is a real simplification of the designer
  API, which is what the schema is for.
- **The whole A2 hazard class goes away.** One interpreter means one evaluation order, one owner
  of `LogicState`, and no double firing between interpreters. The same-frame timing split is
  gone too. If `EnterState` is also retired in favor of transitions, the out-of-band state change
  is gone as well.
- **The migration is mechanical and keeps behavior the same for all 11 live projects.** None uses
  `when:`/`EnterState`, so every rule becomes `global_on: [(event: <on>, do_actions: <same>)]` in
  the same order. The timing difference cannot affect them. About 160 rules in total. It could be
  a script, or a small `ironhold migrate-rules` subcommand.
- **Smaller maintenance surface:**
  - removed: `message_interpreter_system`, `LogicRulesAsset`/`LogicRule`, inline V1 `rules:`,
    the `rules_path` load and failure path in `project_loader.rs`
  - removed: the CLI's rules branch in `resolve_logic_files` and in three collectors/checks, plus
    the hardcoded paths in `query`/`stats`
  - removed: one docs section, and the coexistence warn this bug fixed
  - one place to add future logic features (conflict checks, event validation, debug overlays)
- **Finishes an intent the project already has** (the backlog migration-guide item, the docs
  steer, the "replaces" wording) instead of leaving it half done.

### Cons
- **Churn:**
  - 11 project files and 11 rules.ron rewrites
  - `blank_project` and `/new-project` change
  - tests that build rules directly (`entity_logic_tests`, `ui_tests`, `fsm_tests`,
    `ron_validation`, the `valid_ui_trigger` CLI fixture) need rewriting, roughly 40–50 references
  - docs in `00`, `20`, `30`, `60`, and the CLAUDE.md files need updating
- **Breaking schema change.** Removing `rules_path`/`rules:` breaks any outside RON that uses
  them. Nothing outside the repo is known, but that is the reason the schema has
  `schema_version`. It needs a version bump and a clear load error that points to the migration
  tool.
- **The FSM is more verbose for trivial cases today.** It needs `initial_state`, `states: []`,
  and `transitions: []`. A 1-rule project gets about 4 lines of boilerplate unless the schema is
  loosened first (see below).
- **Loses the simple starting point** unless the minimal FSM is made just as short.
- **`LogicState` default.** `""` is the default today, and `StateMachineAsset::validate` rejects
  an empty `initial_state`. The schema has to choose a default, for example `initial_state`
  defaults to `"default"`, so a flat file needs no state declaration.
- **Doing all of it at once is too much for one feature branch.** It needs to be staged.

---

## 4. The key observation

The argument for keeping rules.ron comes down to "it is shorter for trivial projects". That is a
*syntax* problem, and it can be fixed inside the FSM schema by adding `serde(default)` (an
additive, non-breaking change):
- `transitions` defaults to `[]`
- `states` already allows an empty list
- `initial_state` gets a default

After that, a minimal state_machine.ron is just:

```ron
(
    schema_version: 1,
    global_on: [
        ( event: "scene.ready:main", do_actions: [ Log("Scene ready") ] ),
    ],
)
```

That is exactly as long as today's `blank_project` rules.ron. The easy start is kept, and the
second dialect is not needed for it.

---

## 5. Recommendation (for Frank's decision — not to be acted on unilaterally)

**Consolidate on state_machine.ron, in stages. Do not keep coexistence (A2) as a supported
pattern.**

1. **Now, independent of the rest, low risk:**
   - Delete the two dead files, `3rd_person_game_demo/logic/rules.ron` and
     `terrain_demo/logic/rules.ron`. First confirm each rule is covered by the project's FSM or
     can be dropped. `terrain_demo`'s `dance` is the one to check.
   - Fix the four "replaces" doc and comment places so they state the current behavior.
   - In `docs/`, make "use state_machine.ron for new projects" the official guidance.
2. **Additive schema change (non-breaking):** make `StateMachineAsset.transitions`,
   `initial_state`, and `states` defaultable, so a flat FSM file is as short as rules.ron. Switch
   `blank_project` (the `/new-project` template) to a state_machine.ron.
3. **Runtime consolidation, keeping the RON format working:** at load time in `project_loader.rs`,
   convert `LoadedRules` into the FSM.
   - A rule with no `when:` becomes a `global_on` binding, placed *before* the file's own
     `global_on`. This keeps today's order, where rules actions are queued before FSM actions.
   - A rule with `when: S` becomes an `on:` binding in state `S`, creating a hook-less state if
     needed.
   - Then delete `message_interpreter_system`. Every existing rules.ron keeps working, and the
     second interpreter, the double firing, and the same-frame state split are gone right away.
   - Log a one-time deprecation `info!`/`warn!` when `rules_path`/inline `rules:` is used.
   - Known behavior change: a `when:` rule now sees mid-frame transitions. No live project uses
     `when:`.
   - This is the step that gets most of the architectural benefit for the least churn.
4. **Migrate content:** convert the 11 rules projects and the test fixtures, using a small
   `ironhold` CLI subcommand or a one-off script. Rewrite `tests/*` to use FSM fixtures.
5. **Remove, at a `ProjectConfig` schema_version bump (v4):** drop `rules_path`, inline `rules:`,
   `LogicRulesAsset`, and `LogicRule`, with a load error that names the migration command.
   - At the same point, decide whether `Action::EnterState` survives. Recommendation: keep it only
     if a concrete use case appears. Otherwise remove it, because it is the one way to change state
     without running the hooks.

Steps 1–2 are cheap and valuable whatever is decided later. Step 3 is the real decision point.
Steps 4–5 can wait indefinitely once step 3 lands, because by then rules.ron is only a
deprecated input format, not a second runtime system.

**If Frank prefers to keep rules.ron as a first-class alternative (A1)**, the minimum to make it
safe:
- make "both set" a CLI validate error, or at least a `--strict` warning, not only a runtime
  `warn!`
- add a validate check that each `when:` value is a real FSM state name
- fix `query`/`stats` to resolve `rules_path` as `validate` does
- document A2 as unsupported

This is less work up front, but the dual-maintenance cost stays forever.

## 6. Open questions for Frank

- Is anyone known to author projects outside this repo? That decides how gentle step 5 must be.
- Does "start with rules, move to the FSM later" matter as a *teaching* story? If yes, step 2's
  minimal FSM keeps that story within one format ("start with `global_on`, add states later").
- Should this be a backlog item, possibly replacing or merging with the existing "Schema version
  v2→v3 migration guide" item, which already assumes this direction?
