# Feature: OpenCode compatibility probe, verified-facts table and CLAUDE.md maintenance guide

_Status: Ready (revised after plan-review, 2026-10-08)_
_Planned at: `37de6f4` (2026-10-08); revised at `edc93f1` after plan-review (system-architect, ux-gamedesigner-reviewer)_

The GLM 5.3 flash driver pilot that was v2 of the first draft is now its own plan,
`planning/features/opencode_glm_driver_pilot.md` (Draft, blocked on two decisions from Frank and on this feature's
probe).

## What
Developer tooling only; nothing changes for people authoring games. Three things for the engine developer who keeps
Claude Code and OpenCode working side by side:

1. **`tools/opencode_probe.py`** — a repeatable check of what OpenCode loads for a file: which instruction files it
   attaches (the same mechanism a session uses), without calling a model by default.
2. **A "Verified compatibility facts" table in `.opencode/README.md`** — each OpenCode behaviour we depend on, with a
   row id, status, a copy-pasteable re-verify command, what breaks if it changes, the OpenCode version and the date.
3. **`docs/dev/claude_md_maintenance.md`** — a checklist for editing any `CLAUDE.md`, `.claude/rules` stub or
   `AGENTS.md`, so both tools keep loading the right rules.

Terms used below: the **default driver** is the top-level `model` in `.opencode/opencode.json` (verified with
`opencode debug config`: `openrouter/poolside/laguna-s-2.1:free`; `build` has no model of its own). The **tiers**
(E paid, R free reasoning, C free code, F free light, ...) are defined in `.opencode/README.md` ("Model tiers").
**Findings F1-F10** are in `planning/features/opencode_compatibility.md`. **Safety=Y, `governs`, `b:N` anchors** are
`tools/claude_md_audit.py` terms (see its header comment).

## Why
- The core CLAUDE.md split (`planning/features/done/core_claude_md_split.md`) started from a wrong assumption: that an
  `AGENTS.md` containing `@CLAUDE.md` forwards to the real file. It does not: OpenCode 1.18.33 attaches the stub's
  literal text and the stub shadows the `CLAUDE.md` beside it, so the folder's real rules are silently lost. Only a
  live run found it, after 17 stubs had been added.
- What we know about OpenCode is spread over `opencode_compatibility.md`, `.opencode/README.md`,
  `.opencode/opencode_free_models.md` and `tools/opencode_sync_check.py`, and none of it says how to re-verify a claim.
  `opencode_sync_check.py` checks configuration drift (agents, commands, model ids); nothing checks load behaviour.

## Approach

### The probe, model-free by default (`tools/opencode_probe.py`, run from the repo root or with `--dir`)
Plan-review found that OpenCode can run its read tool without a model: `opencode debug agent build --tool read
--params "{filePath:'<repo-relative path>',limit:2}"` prints JSON where `result.metadata.loaded` lists the attached
instruction files, nearest first, and `result.output` holds the `Instructions from: <abs path>` text. No auth, no
tokens, no model quality involved. (Verified 2026-10-08 on 1.18.33.) Details that matter:
- **Quoting:** the params are a JavaScript-style literal with *single* quotes and repo-relative forward-slash paths; JSON
  double quotes are mangled by the `.cmd`/`.ps1` shims and backslashes are eaten. Call the real binary with a list of
  arguments (no shell).
- **Finding OpenCode (shared helper, `tools/_opencode_common.py`):** `OPENCODE_BIN`, then `shutil.which("opencode")`,
  then a hint. Not found: exit 2 with exactly: `opencode_probe: 'opencode' not found on PATH.` / `On this Windows machine
  OpenCode is installed under nvs node 24.21. In the same terminal run:  nvs use 24.21  then  python
  tools/opencode_probe.py` / `Only want the checks that need no model? Run: python tools/opencode_probe.py --static`.
  `opencode_sync_check.py` uses the same helper, which also fixes its live-model check silently skipping from Bash.
- **No session history:** every debug tool run creates a session ("Debug tool run (build)", not retitlable). The probe
  sets `XDG_DATA_HOME` and `XDG_STATE_HOME` to a temp dir for its child processes, which keeps the real session list
  unchanged (verified). Acceptance checks this.
- **Fragility:** the `debug` interface is not a documented contract and may change between versions. The probe warns
  when `opencode --version` differs from the last verified version in the facts table, and `--live` (below) is the
  fallback.

**Probe files and the expected attach set.** One positive file per folder that owns a `CLAUDE.md` (for example
`crates/ironhold_core/src/lib.rs`, `.../capabilities/collectible.rs`, `.../runtime/input.rs`,
`.../runtime/scene_manager/entity_spawner.rs`, `.../schema/actions.rs`, `.../tests/fsm_tests.rs`, an
`assets/**/*.wgsl`, an `assets/projects/**` file, one `tools/*/` script) **plus negative files whose expected set is
empty** (`Cargo.toml`, a `docs/dev/*.md`, `crates/ironhold_web/src/lib.rs`, a `.claude/rules/*.md`). The rule, derived
rather than hardcoded: for each folder from the file's own folder up to, **not including**, the worktree root, take the
first of `AGENTS.md` > `CLAUDE.md` > `CONTEXT.md`. The root is excluded because its files already arrive at session
start (`AGENTS.md` by discovery, `CLAUDE.md` through `instructions` in `opencode.json`). So `lib.rs` expects exactly
`crates/ironhold_core/src/CLAUDE.md`. Compare with `os.path.normcase(Path(p).resolve())` (output is absolute with
backslashes). Enumerate probe candidates with `git ls-files` (no symlinks are tracked); the static scan walks the
filesystem, because OpenCode does, skipping `.git`, `target`, `pkg`, `node_modules` and `.claude/worktrees`.
`--dir <worktree>` points the probe at a feature worktree; one extra case reads a file of a sibling worktree from the
primary checkout (the OpenCode version of the Claude Code worktree finding, R18), reported as *unverified* until run.

**Static checks (no OpenCode needed) live in `tools/opencode_sync_check.py`**, the "run before leaning on OpenCode"
tool that already exists: no `AGENTS.md`/`CONTEXT.md` below the root (the root is exempt), and no `instructions` entry
that matches `.claude/rules` (OpenCode never reads those, so they cannot be depended on). `opencode_probe.py --static`
simply calls these.

**`--live` (the only mode that calls a model):** runs `opencode run --format json -m <model> "<read three lines of FILE and
reply DONE>"` for the *token baseline* (`step_finish.part.tokens.input`; machine-specific, see V8) and the skill count
(`opencode debug skill`). Defaults to free models only; a paid fallback needs an explicit `--allow-paid`. A run without
a `read` tool event is an **error to retry**, never "attached nothing" (free endpoints sometimes return an empty
response). Each run uses `--title "opencode-probe <utc>"` and deletes its own sessions afterwards
(`session list` + `session delete`). It enforces an optional `--max-cost` by summing `step_finish.part.cost`.

**Output.** Header: OpenCode version, mode, date (and the version warning). One line per probe file:
`PASS crates/ironhold_core/src/lib.rs  attached: crates/ironhold_core/src/CLAUDE.md  (2.1 KB)`; on mismatch
`FAIL ...  missing: ...  unexpected: ...`. Then a summary (`N checks failed`, and the baseline in `--live`). `--json` for
machine output (like `ironhold --json` and `opencode_sync_check.py`), `--only <path>` to check one file after an edit,
`--help` documents the exit codes (0 all pass, 1 a check failed, 2 tool error, as `ironhold validate`). `--live` prints an
estimate first ("8 probe files, about 8 model calls, about 2 min"). Not at the repo root and no `--dir`: exit 2 with a
clear message. HTML anchors (`<!-- b:N -->`) are reported as informational bytes only.

**Self-test:** `--selftest` with an embedded fixture (recorded debug-run JSON and a throwaway tree), matching
`claude_md_audit.py`; `tools/` has no pytest setup.

### Verified-facts table (`.opencode/README.md`)
New section placed directly after "The rule that matters most" (not near "Known gaps"), and "Checking for config drift"
gets a sentence saying `opencode_sync_check.py` checks configuration and the probe checks load behaviour. Columns: **ID**,
fact, **status** (verified / unverified / contradicted), **re-verify** (a copy-pasteable command), **what breaks if it
changes**, OpenCode version, date. The existing "Tested OpenCode version: 1.18.31" line is folded into it. Seed rows
(2026-10-08, 1.18.33):

| ID | Fact | Status |
|---|---|---|
| V1 | Reading a file attaches the nearest `AGENTS.md`>`CLAUDE.md`>`CONTEXT.md` of every folder from the file's folder up to, excluding, the root | verified |
| V2 | A subfolder `AGENTS.md` shadows the `CLAUDE.md` beside it and `@CLAUDE.md` inside it is **not** expanded (the model gets the literal text) | verified |
| V3 | Root files are not attached; they arrive at session start (`AGENTS.md` by discovery, `CLAUDE.md` through `instructions`), and nothing loads twice | verified |
| V4 | HTML comments (`<!-- b:N -->`) are **not** stripped under OpenCode (51 anchors reach the model via `entity_spawner.rs`, about 15 bytes each) | verified |
| V5 | `.claude/rules/*.md` are never attached (the read tool only looks for AGENTS/CLAUDE/CONTEXT), so the pointer lines in the folder files are the only route to `docs/dev` topic pages | verified |
| V6 | `opencode debug agent build --tool read --params "{filePath:'...',limit:2}"` runs the read tool with no model and exposes `metadata.loaded` | verified |
| V7 | The default driver is the top-level `model` (free Laguna); `build` has no model of its own | verified |
| V8 | The session token baseline (31.8k input tokens in a trivial GLM run) is **machine-specific**: 27 skills from `~/.agents/skills` and the synced `~/.claude/skills`, plus `~/.config/opencode/AGENTS.md`, are in it | verified |
| V9 | OpenCode here lives under nvs node 24.21 (`nvs use 24.21` in the same terminal first); not on the Bash PATH | verified |
| V10 | Free models can return 429 or an empty response | verified |
| U1 | A nested `.claude/worktrees/agent-*` attaches that worktree's root `CLAUDE.md` as an ancestor | unverified |
| U2 | Reading a sibling worktree's file from the primary checkout attaches that worktree's folder files | unverified |

### Maintenance guide (`docs/dev/claude_md_maintenance.md`)
Section order, so "what must I check?" is answerable in under a minute:
1. **Before you edit: checklist** (at most 10 items, each ending with how it is verified): rule is in the folder whose
   subtree contains every file it governs (files directly in `src/`, such as `lib.rs`, see only the parent), *probe*;
   no `AGENTS.md` below the root, *`opencode_sync_check.py`*; `.claude/rules` stub is pointer-only with the exact `paths:`
   key and no `@import`, *human review*; Safety=Y rules load for the governed files, *human review (the audit cannot
   judge `governs` globs)*; headings cited elsewhere unchanged word for word, *`claude_md_audit.py full`*; size budget
   respected, *same*; loaded set verified in both tools, */context* and the probe; review agents get the worktree path
   and are told to read the folder files, *human step*.
2. **Where does my rule go?** a three-column table: rule applies to / put it in / loads under Claude Code and OpenCode.
3. **Never do:** `AGENTS.md` stubs, `@CLAUDE.md` forwarding, adding to the root file (14.8k tokens on every session).
4. **How to verify:** the exact commands.
5. **Why (incidents):** the 17-stub story, cited to `planning/features/done/core_claude_md_split.md`.
6. **Glossary:** `b:N` anchors, Safety=Y, `governs`, `load-bearing:` comment, `/context`.

Links: **one bullet** in the existing "Updating documentation" list of the root `CLAUDE.md` (it is 41k chars and loads
every session; the "Never add a subfolder AGENTS.md" warning stays in the guide, and root `AGENTS.md` links to the
guide), one line in `docs/dev/60_contributing.md`, a **dev row only** in the `docs/README.md` table (Audience
"Developer (AI tooling)"; leave the designer opener alone), and `.opencode/README.md`. Not `planning/CLAUDE.md`, and no
row in `docs/15_authoring_tools.md` (it documents the `ironhold` CLI; the probe is not a `validate` check).

### Hand-off outside the repo
One line for the `juva-nvs` skill in the `ai-workspace` repo ("opencode is installed under nvs node 24.x; run `nvs use
24.21` first": use the same version string everywhere), drafted here and landed by Frank; the global `~/.claude/` files
are managed by that repo and are not edited from here.

## Tasks
- [ ] `tools/_opencode_common.py` (`find_opencode()`: `OPENCODE_BIN`, `shutil.which`, nvs hint) and use it in both tools
- [ ] `tools/opencode_probe.py`: model-free probe over the debug read path, tree-derived expectations, negative files,
  temp `XDG_*` isolation, version warning, `--static`, `--only`, `--dir`, `--json`, `--selftest`
- [ ] `--live` (baseline tokens, skill count, retries on empty responses, free-only unless `--allow-paid`, session cleanup, `--max-cost`)
- [ ] `tools/opencode_sync_check.py`: subfolder `AGENTS.md`/`CONTEXT.md` check and "`instructions` must not match `.claude/rules`"
- [ ] One-off checks while implementing: attach set identical with and without plugins (`--pure`); settle U1 and U2
- [ ] Facts table in `.opencode/README.md` (seed rows above), fold in the old tested-version line, and the drift-section sentence
- [ ] `docs/dev/claude_md_maintenance.md` with the section order above, and the four links
- [ ] Draft the `juva-nvs` line and hand it to Frank
- [ ] Log a `planning/claude_suggestions.md` entry: the baseline is inflated by 27 machine-local skills and a global `AGENTS.md`
- [ ] Tests: `--selftest` fixtures for expectation computation and debug-JSON parsing
- [ ] Docs: `.opencode/README.md`, a cross-reference in `planning/features/opencode_compatibility.md`

## Plan-review (2026-10-08) and what changed
**system-architect, needs more design work:** (1) the model-free debug read path replaces the model-driven probe (no empty
response ambiguity, no 429, no cost, no session pollution); (2) the fallback chain could spend money, so free-only plus
`--allow-paid`; (3) the v2 default-driver change breaks the free-by-default guarantee and moves to its own plan; (4) the
expected-attach rule made precise, negative files and path normalisation added; (5) HTML comments, `.claude/rules`
and double loading were already settled by live runs and seeded as facts instead of tasks; (6) the token baseline is
machine-specific; (7) v2's design could not yet produce a trustworthy result (moved); (8) static checks belong in
`opencode_sync_check.py`; plus `--selftest`, version-drift warning, root-budget (one bullet), `--pure` check, and cutting
`--follow`, the fallback chain and the default-mode baseline from v1. **ux-gamedesigner-reviewer, needs more design
work:** designer safety confirmed with a placement rule (dev row only in `docs/README.md`); guide restructured
checklist-first; facts table gains status, "what breaks" and row ids and moves up; exact not-found wording with
`nvs use 24.21` and exit 2; output format, `--only`, `--json`, cost estimate, cleanup; jargon glossed. Findings that
needed Frank are in the pilot plan.

## Open questions
- Does the attach set differ with `--pure` (plugins off)? Settle during implementation (task above).
- U1 and U2 (nested agent worktrees, sibling worktree from the primary checkout): settle with the probe, record in the table.
- Should `opencode_probe.py` run as part of `/release`? Recommendation: no; run it manually after OpenCode upgrades or
  `CLAUDE.md` edits, and let the static half run inside `opencode_sync_check.py`.

## Acceptance criteria
- Given OpenCode on `PATH`, when `python tools/opencode_probe.py` runs, then for every probe file it prints the attached
  instruction files, equals the tree-derived expectation (including the empty set for the negative files), makes no
  model call, leaves `opencode session list` unchanged, and exits 0.
- Given a subfolder `AGENTS.md` is added, when `python tools/opencode_probe.py --static` or `opencode_sync_check.py`
  runs (no OpenCode needed), then it fails and names the file.
- Given OpenCode is not on `PATH`, when the probe runs, then it exits 2 with the message above (not a traceback).
- Given `opencode --version` differs from the facts table's last verified version, when the probe runs, then it warns.
- Given `--live` and a rate-limited free model, when the call fails or returns no `read` event, then it retries and never
  reports an empty attach set as a pass; and it deletes the sessions it created.
- Given the facts table, when OpenCode is upgraded, then each row's re-verify cell is a command that can be pasted.
- Given someone edits a `CLAUDE.md`, when they open `docs/dev/claude_md_maintenance.md`, then the checklist is at the
  top and every item names its check (`claude_md_audit.py`, `/context`, the probe, `opencode_sync_check.py`) or says it
  is a human review step.
- `python tools/opencode_probe.py --selftest` passes.
