# Feature: OpenCode compatibility probe, verified-facts table and CLAUDE.md maintenance guide

_Status: Draft_
_Planned at: `37de6f4` (2026-10-08)_

## Phases

| Phase | Backlog item | Status | Completed |
|---|---|---|---|
| v1 | Probe script, verified-facts table, CLAUDE.md maintenance guide | Queued | — |
| v2 | GLM 5.3 flash pilot and the default-driver decision | Queued (needs v1's probe and a fixed task set) | — |

## What
Developer tooling only; nothing changes for people authoring games. Three things for the engine developer who
uses Claude Code and OpenCode side by side:

1. **`tools/opencode_probe.py`** — a repeatable check of what OpenCode actually loads for a file (which instruction
   files it attaches, how big they are, what it ignores), runnable by anyone after an OpenCode upgrade or a
   `CLAUDE.md` change.
2. **A "verified facts" table in `.opencode/README.md`** — each OpenCode behaviour we depend on, with the OpenCode
   version, the date and how it was verified.
3. **`docs/dev/claude_md_maintenance.md`** — what to account for when editing any `CLAUDE.md`, `.claude/rules` stub
   or `AGENTS.md`, so both tools keep loading the right rules.

v2 then uses the probe to pilot **GLM 5.3 flash** (`openrouter/z-ai/glm-5.3-flash`) as the default OpenCode driver.

## Why
- The core CLAUDE.md split (`planning/features/done/core_claude_md_split.md`) started from a wrong assumption: that an
  `AGENTS.md` containing `@CLAUDE.md` forwards to the real file. It does not (OpenCode 1.18.33 attaches the literal
  text and the stub shadows the `CLAUDE.md`). Only a live run found it, after 17 stubs had been added.
- What we know about OpenCode is spread over `planning/features/opencode_compatibility.md` (findings F1-F10),
  `.opencode/README.md`, `.opencode/opencode_free_models.md` and `tools/opencode_sync_check.py`, and none of it says
  how to re-verify a claim. `opencode_sync_check.py` checks config drift (agents, commands, model ids), not load
  behaviour.
- The free models are unreliable (a `:free` model returned 429 during the split work), and the free/`-deep` split in
  `opencode.json` has no cheap-paid default driver between "free" and "paid DeepSeek, only on request". A cheap
  model such as GLM 5.3 flash may fill that gap, but that must be measured, not assumed.

## Approach

### v1 — probe, facts, guide

**Probe (`tools/opencode_probe.py`, run from the repo root).**
- Finds `opencode` on `PATH`; if absent it prints the recipe for this machine (`nvs use 24.21`, then
  `opencode`). OpenCode ships under node 24.21 through nvs here (see `juva-nvs` skill hand-off below).
- For each *probe file* it runs `opencode run --format json -m <model> "<prompt>"` where the prompt only asks the
  model to read the first 3 lines of the file with the read tool and reply `DONE`, so a model's quality does not
  matter. It parses the JSON lines: `tool_use` events for the `read` tool carry the output with the
  `Instructions from: <path>` lines (and `metadata.loaded`), and `step_finish` carries `tokens` and `cost`.
- Probe files are chosen from the tree, one per folder that owns a `CLAUDE.md` (for example
  `crates/ironhold_core/src/lib.rs`, `.../capabilities/collectible.rs`, `.../runtime/input.rs`,
  `.../runtime/scene_manager/entity_spawner.rs`, `.../schema/actions.rs`, `.../tests/fsm_tests.rs`,
  `assets/shared/shaders/*.wgsl`, one `tools/*/` script). The expected attach set is **computed from the tree**: every
  ancestor folder below the root that has a `CLAUDE.md`. (The root file is loaded through `instructions` in
  `.opencode/opencode.json`, not through attachment.) Folder files are expected to attach only for files inside that
  folder's subtree, so `lib.rs` must see the parent only.
- Checks: (a) attached set equals expected; (b) **static, no model:** no `AGENTS.md` in a subfolder (it shadows the
  `CLAUDE.md` and OpenCode does not expand `@CLAUDE.md`); (c) bytes attached per probe and `tokens.input` of the
  trivial session, reported as OpenCode's equivalent of `/context` (the first GLM run showed a 31.8k-token fixed
  baseline); (d) whether HTML comments (`<!-- b:N -->` audit anchors) reach the model, reported as wasted bytes;
  (e) that `.claude/rules/*.md` are *not* attached, so the pointer lines in the folder files are the only route to
  `docs/dev` topic pages under OpenCode.
- Resilience: on a 429/`isRetryable` error it retries once and then falls to the next model in `--model` fallbacks
  (default list: the configured F-tier free model, then `openrouter/z-ai/glm-5.3-flash`). `--static` skips every model
  call. Exit 0 = all checks pass, 1 = a check failed, 2 = tool error. Sessions are tagged so they are easy to delete.
- Optional `--follow`: asks the model, after reading a capabilities file, which `docs/dev` page its folder file points
  to for a named topic; this is a quality test and is reported separately, never gating.

**Verified-facts table (`.opencode/README.md`, new section "Verified compatibility facts").** Columns: fact, how to
re-verify, OpenCode version, date. Seeded from 2026-10-08 (OpenCode 1.18.33):
- nested `CLAUDE.md` attaches on first read of a file in its subtree, ancestors included, root excluded;
- `AGENTS.md` shadows `CLAUDE.md` in the same folder and `@CLAUDE.md` inside it is not expanded;
- result lines read `Instructions from: <abs path>`;
- the probe's trivial-session baseline (input tokens);
- model availability through the configured providers (OpenRouter, Zen, Google) and that `:free` models can 429;
- how to run OpenCode on this machine (nvs).
Open items start as "unverified" until the probe settles them (comments, `.claude/rules`, double loading of the root
`AGENTS.md` plus `instructions: ["CLAUDE.md"]`).

**Maintenance guide (`docs/dev/claude_md_maintenance.md`).** Written for whoever edits instruction files; the root
`CLAUDE.md` gets a 3-line pointer only (it loads on every session, 14.8k tokens today). Content: folder scope (a
folder file loads only for its subtree, so rules for files directly in `src/` such as `lib.rs` belong in the parent or in
a `// load-bearing:` comment); never add `AGENTS.md` stubs; `.claude/rules` stubs are Claude-only, pointer-only, need
the exact `paths:` key and no `@import`; HTML anchors are stripped by Claude Code (OpenCode: per the probe); size
budgets and `python tools/claude_md_audit.py full`; Safety=Y rules need a home that loads for the governed files and
the audit cannot judge `governs` globs; keep cited headings verbatim; give review agents the worktree path and tell
them to read directory files; verify with `/context` (Claude Code) and the probe (OpenCode).

**Hand-off outside the repo.** One line for the `juva-nvs` skill in the `ai-workspace` repo ("opencode is installed
under nvs node 24.x; run `nvs use 24` first"), drafted here and landed by Frank; the global `~/.claude/` files are
managed by that repo and are not edited from here.

### v2 — GLM 5.3 flash pilot
- Fixed task set (4-5 tasks, same prompt for every model): follow a folder rule (a change that must respect a "never"),
  edit a RON file then run `validate`, a read-only question that needs a `docs/dev` pointer, a two-file refactor, and
  one review task. Each task has a mechanical pass criterion (a `ron_lint`/`validate`/`cargo check` result or a rule
  not violated).
- Models: `openrouter/z-ai/glm-5.3-flash` (pinned), the current default driver, and the E-tier DeepSeek as the
  reference. Record success, wall time, `tokens.input`/`output` and `cost` per task. Pilot budget cap: about $1.
- Decision rules: switch the default `build`/`general` driver only if GLM passes at least as many tasks as the current
  default and its cost per task is acceptable; **pin** the model id (the `~z-ai/glm-flash-latest` alias can change
  behaviour silently, and `opencode_sync_check.py` only catches disappearance), keeping the alias as an opt-in.
- If adopted: update `.opencode/opencode.json` and the tier table in `.opencode/README.md`, note it in
  `.opencode/opencode_free_models.md` (it is a paid model, so the "nothing can accidentally spend money" rule for plain
  agent names needs restating: a paid default driver changes that guarantee and must be a deliberate choice), and run
  `python tools/opencode_sync_check.py`.

## Tasks
- [ ] v1: `tools/opencode_probe.py` (probe table from the tree, JSON parsing, static checks, retries and model fallbacks)
- [ ] v1: tests for the probe's pure parts (expected-attach computation, JSON-line parsing) with recorded fixtures
- [ ] v1: "Verified compatibility facts" section in `.opencode/README.md`, seeded and with re-verify commands
- [ ] v1: `docs/dev/claude_md_maintenance.md` and the 3-line pointer in root `CLAUDE.md`; mention it in `docs/README.md`
- [ ] v1: settle the open facts with the probe (HTML comments, `.claude/rules`, double loading) and record the results
- [ ] v1: draft the `juva-nvs` line for the `ai-workspace` repo and hand it to Frank
- [ ] v2: fixed task set and scoring script; run the pilot; write the results into the plan
- [ ] v2: decide pinned-vs-alias and the default driver; update `opencode.json`, README, free-models note
- [ ] Docs: `.opencode/README.md`, `planning/features/opencode_compatibility.md` cross-reference, `planning/CLAUDE.md` if the folder list changes

## Open questions
- Is there an `opencode debug`-style subcommand that lists the loaded instructions without a model call? (It would
  remove the model dependency from the probe; check `opencode debug --help` first thing.)
- Which setting is "the main driver": the top-level `model`, the `build` agent, or both? (`opencode.json` has a
  top-level `model` and per-agent models; the first read suggests Gemini 3.8 flash is the default.)
- Does the root `AGENTS.md` plus `instructions: ["CLAUDE.md"]` load anything twice, and how much of the 31.8k-token
  baseline is instructions versus OpenCode's own prompt and tool schemas?
- Should the probe live in CI-like routine (before a release batch) or stay a manual step after OpenCode upgrades?
- Windows only? The probe should work wherever `opencode` is on `PATH`; the nvs recipe is only the Windows hint.

## Acceptance criteria
- Given a repo checkout and OpenCode on `PATH`, when `python tools/opencode_probe.py` runs, then it reports for each
  probe file the attached instruction files, whether they equal the tree-derived expectation, and the baseline
  token count, and exits 0 when everything matches.
- Given a subfolder `AGENTS.md` is added, when `python tools/opencode_probe.py --static` runs (no model), then it
  fails and names the file.
- Given a rate-limited model, when the probe runs, then it falls back to the next configured model instead of failing.
- Given the facts table, when OpenCode is upgraded, then each row says exactly how to re-verify it.
- Given someone edits a `CLAUDE.md`, when they open `docs/dev/claude_md_maintenance.md`, then every item in the
  checklist maps to a check (`claude_md_audit.py`, `/context`, or the probe) or to a human review step.
- v2: the pilot results (success, time, tokens, cost per task per model) are recorded and the default-driver decision
  follows the stated rule, with the model pinned.
