# OpenCode setup for ironhold-lib

Design doc: `planning/features/opencode_compatibility.md` (read that first for the full reasoning
— this file is the quick-reference for actually using the setup).

**OpenCode version:** load behaviour was last verified on `1.18.33` (2026-10-08, see the table below; the probe warns when
`opencode --version` differs). The configuration itself (`instructions`, `{file:}`, delegation, the `-deep` permission
deny) was live-tested on `1.18.31` (2026-09-22; `planning/features/opencode_compatibility.md`'s verification checklist
says what was and was not covered). The design was researched against `sst/opencode@dev`, so if a release changes
behaviour, `opencode debug config` is the first thing to check.

## The rule that matters most

**Any review OpenCode produces is advisory only.** The mandatory review before merging a feature
into `integration` is Claude Code's own `/code-review` — nothing here replaces it. See
`AGENTS.md`'s "Merge gate" note.

## Providers and models

Three model sources, none of them Anthropic (`.opencode/` is only ever read by OpenCode — your
Claude Code sessions keep using `opus`/`sonnet` from `.claude/agents/` regardless of anything
here):

| Provider | What it's used for |
|---|---|
| OpenRouter (`openrouter/...`) | Free reasoning-tier models, plus the one paid model (DeepSeek v4.1 flash) |
| OpenCode Zen (`opencode/...`) | Free models for the two always-escalatable review agents and for run-and-report commands |
| Google Gemini (`google/...`) | Free tier, low-volume creative/UX consultation only |

Check all three are configured: `opencode auth list`. Your Gemini key should already be there.

**Before relying on Gemini being free:** check in [AI Studio](https://aistudio.google.com) that
the project your key belongs to has **no billing account linked**. If billing is on, every call to
`google/gemini-3.8-flash` is billed (~$0.75/$3.75 per million tokens as of 2026-09-22) — see
`planning/features/opencode_compatibility.md` finding F10. If billing turns out to be on, switch
`ux-gamedesigner-reviewer`/`game-world-designer` in `opencode.json` to
`openrouter/thinkingmachines/inkling:free` instead.

## Verified compatibility facts

<!-- opencode-verified-version: 1.18.33 -->
What we depend on in OpenCode, how to re-verify it, and what breaks if it changes. **Re-verify after every OpenCode upgrade and
after editing any `CLAUDE.md` or `.opencode/opencode.json`** with `python tools/opencode_probe.py` (model-free, about 40 s; add
`--live` for the token baseline). Editing instruction files: `docs/dev/claude_md_maintenance.md`. All rows were verified on
OpenCode `1.18.33` on 2026-10-08 unless the status says otherwise.

**Running OpenCode on this Windows machine:** it is installed under nvs node 24.21 and is on `PATH` only after `nvs use 24.21`
**in the same terminal** (it is not on the Bash `PATH`). Or set `OPENCODE_BIN` to
`C:\ProgramData\nvs\node\24.21.0\x64\node_modules\opencode-ai\bin\opencode.exe`.

| ID | Fact | Status | Verified on | Re-verify | What breaks if it changes |
|---|---|---|---|---|---|
| V1 | Reading a file attaches, for each folder from the file's own folder up to (not including) the root, the first of `AGENTS.md` > `CLAUDE.md` > `CONTEXT.md` | verified | 1.18.33, 2026-10-08 | `python tools/opencode_probe.py` | folder rules stop reaching OpenCode and the lazy per-folder loading is lost |
| V2 | A subfolder `AGENTS.md` shadows the `CLAUDE.md` beside it, and `@CLAUDE.md` inside it is **not** expanded: the model gets the literal text and the real rules are lost | verified | 1.18.33, 2026-10-08 | `python tools/opencode_probe.py --static` checks the repo has no stub; to re-test OpenCode itself create a temp folder `x/` with `x/AGENTS.md` containing `@CLAUDE.md`, `x/CLAUDE.md` and `x/f.rs`, run `python tools/opencode_probe.py --only x/f.rs` (expected attach: `x/AGENTS.md`, not `x/CLAUDE.md`), then delete `x/` | if `@file` starts being expanded, stubs become possible and this row, the sync check and the guide must change |
| V3 | Root files are not attached: `AGENTS.md` arrives by discovery and `CLAUDE.md` through `instructions` in `opencode.json`; nothing loads twice | verified | 1.18.33, 2026-10-08 | `python tools/opencode_probe.py --only Cargo.toml` (attaches nothing); `opencode debug config` | root rules missing or doubled (about 14.8k tokens) |
| V4 | HTML comments are **not** stripped: `<!-- b:N -->` anchors reach the model (2.7 KB across the 21 probe files) | verified | 1.18.33, 2026-10-08 | `python tools/opencode_probe.py --only crates/ironhold_core/src/capabilities/action_bar.rs` prints the line `HTML comments inside the attached files ... N bytes` (the full run prints the 2769-byte total) | wasted context; if it grows, move the anchors to a sidecar |
| V5 | `.claude/rules/*.md` are never attached (the read tool only looks for AGENTS/CLAUDE/CONTEXT), so the pointer lines in the folder files are the only route to `docs/dev` topic pages | verified | 1.18.33, 2026-10-08 | `python tools/opencode_probe.py --only .claude/rules/lootable-corpse.md` (attaches nothing) | topic references become unreachable under OpenCode if the pointers go |
| V6 | `opencode debug agent build --tool read --params "{filePath:'<repo path>',limit:2}"` runs the read tool with no model and lists the attached files in `result.metadata.loaded` (single quotes, repo-relative forward slashes) | verified | 1.18.33, 2026-10-08 | `python tools/opencode_probe.py` | the model-free probe stops working if the debug interface changes; `--live` is the fallback |
| V7 | The default driver is the top-level `model` in `.opencode/opencode.json`, currently free Laguna; `build` has no model of its own | verified | 1.18.33, 2026-10-08 | `opencode debug config` | changing it changes which agents are paid (`planning/features/opencode_glm_driver_pilot.md`) |
| V8 | The fixed token baseline of a trivial session is **machine- and model-specific**: 25.5k input tokens with `opencode/nemotron-3.5-lightning-free`, 31.8k with GLM 5.3 flash; 27-28 skills from `~/.agents/skills` and the synced `~/.claude/skills`, plus `~/.config/opencode/AGENTS.md`, are in it | verified | 1.18.33, 2026-10-08 | `python tools/opencode_probe.py --only crates/ironhold_core/src/capabilities/action_bar.rs --live` | session cost; compare only on the same machine and model |
| V9 | OpenCode lives under nvs node 24.21 (see above) | verified | 1.18.33, 2026-10-08 | `nvs use 24.21` then `opencode --version` | the tools exit 2 with "not found" |
| V10 | Free models can return 429 (shared upstream pool, whatever your own quota) or an empty response | verified | 1.18.33, 2026-10-08 | `python tools/opencode_probe.py --live` (retries; an empty response is never counted as a pass) | flaky free-tier runs; use a twin agent |
| V11 | The `m365` provider in the global config (`~/.config/opencode/opencode.jsonc`, machine-local) points at a local proxy (`C:\ProgramData\m365-copilot-proxy`, `http://localhost:4141/v1`, started with `pnpm run proxy 4141`); model `m365/gpt-5.5-think-deeper`; works only while the proxy runs | verified from the config and the proxy README, not run | 1.18.33, 2026-10-08 (config and README read; proxy not run) | `python tools/opencode_probe.py --live --model m365/gpt-5.5-think-deeper` (preflights the proxy) | m365 agents fail like a 429 |
| V12 | Plugins do not change the attach set (`--pure` gives the same files) | verified | 1.18.33, 2026-10-08 | `python tools/opencode_probe.py --pure --only crates/ironhold_core/src/runtime/scene_manager/action_executor.rs` | plugin-injected instructions would make runs diverge |
| V13 | A nested `.claude/worktrees/agent-*` folder attaches its own root instruction file by the same `AGENTS.md` > `CLAUDE.md` > `CONTEXT.md` rule; every real agent worktree has a root `AGENTS.md` (the onboarding pointer), so that file loads, not its `CLAUDE.md` | verified | 1.18.33, 2026-10-08 | create `.claude/worktrees/agent-x/AGENTS.md`, `.claude/worktrees/agent-x/CLAUDE.md` and `.claude/worktrees/agent-x/f.rs`, run `python tools/opencode_probe.py --only .claude/worktrees/agent-x/f.rs` (expected attach: `agent-x/AGENTS.md` only), then delete `.claude/worktrees/` (the static scan skips that folder on purpose) | what an agent worktree inside the repo loads |
| V14 | Reading a sibling worktree's file from the primary checkout attaches that worktree's folder files **and its root `AGENTS.md`** (Claude Code loads no folder file for it). This holds because OpenCode tests "inside the project" by plain string prefix and feature worktrees are named `ironhold-lib-<slug>`; the reverse (worktree reading a primary-checkout file) and unrelated folders attach nothing | verified | 1.18.33, 2026-10-08 | from the **primary checkout** (PowerShell after `nvs use 24.21`; Python is easier than shell quoting): `python -c "import json,subprocess;print(subprocess.run(['opencode','debug','agent','build','--tool','read','--params',json.dumps({'filePath':'C:/path/to/ironhold-lib-SLUG/crates/ironhold_core/src/lib.rs','limit':2})],capture_output=True,text=True).stdout)"` and read `result.metadata.loaded`. `--dir` is NOT this test: it sets the working directory to the worktree | the "give review agents the worktree path and tell them to read the folder files" rule is Claude Code only; renaming worktree folders so they no longer share the primary checkout's name prefix changes it |
| V15 | `XDG_DATA_HOME`/`XDG_STATE_HOME` set to a temp dir keep debug runs out of the real session list | verified | 1.18.33, 2026-10-08 | compare `opencode session list` before and after a probe run (the probe warns if it changed) | session-history pollution |
| V16 | The read tool refuses most binary files (`Cannot read binary file` for `.glb` and `.avif`; PNG, JPEG and WebP are readable), so a folder whose only files are binaries cannot be probed (today `assets/shared/models`) and the probe warns instead of skipping it silently | verified | 1.18.33, 2026-10-08 | `python tools/opencode_probe.py` prints `WARNING: not probed ... assets/shared/models` | a CLAUDE.md in such a folder is not covered by the probe |

## Model tiers

| Tier | Model | Used by |
|---|---|---|
| E (paid) | `openrouter/deepseek/deepseek-v4.1-flash` | Only `system-architect-deep` / `debug-detective-deep` |
| R (free reasoning) | `opencode/nemotron-3-ultra-free` or `openrouter/nvidia/nemotron-3-ultra-550b-a55b:free` | `system-architect`, `debug-detective`, `alignment-reviewer`, `/code-review`/`/plan-review`/`/ship-feature`/`/release` orchestration, the built-in `plan` agent |
| R-lite | `openrouter/nvidia/nemotron-3-super-120b-a12b:free` | `wasm-perf-reviewer` |
| C (free code) | Split across 3 providers (2026-09-23), see below | `integration-test-author`, `ron-gameplay-scripter`, `data-format-doc-writer`, the default `build`/`general` agents |
| G (free, low-volume) | `google/gemini-3.8-flash` | `ux-gamedesigner-reviewer`, `game-world-designer` |
| F (free, light) | `opencode/nemotron-3.5-lightning-free` | `/query`, `/validate`, `/wasm-dev`, `/rust-docs` |
| T (titles) | `opencode/nemotron-3.5-lightning-free` | `small_model` (background title generation) only |
| X (explore) | `openrouter/thinkingmachines/inkling-small:free` | The built-in `explore` agent |

## C tier is split across providers, on purpose

OpenCode has **no fallback/retry-with-a-different-model mechanism** — `model` is a single string
per agent, full stop (confirmed against the docs, not assumed). So when a free model's *upstream*
provider rate-limits its own `:free` tier (a capacity limit shared across every OpenRouter user
hitting that model, not something your OpenRouter credit balance fixes — that only raises
OpenRouter's own daily-request cap, a separate thing), every agent pointed at that one model stalls
at once with no automatic recovery.

This happened for real (2026-09-23): `openrouter/poolside/laguna-s-2.1:free` — the original,
single C-tier model backing the default `build`/`general` agent plus all three authoring
subagents — started returning `"[Poolside] ... is temporarily rate-limited upstream"` mid-session.
First fix: spread the 4 C-tier consumers across 3 different upstream providers, moving the default
`build`/`general` model to `openrouter/cohere/north-mini-code:free` (Cohere) so at most 2 agents
shared any single provider's bottleneck.

**Reverted the default back to Laguna the same day** — North Mini Code turned out to have a worse
problem than the rate limit it was meant to work around. In live use it stopped making real edits
partway through a multi-step implementation and started re-describing the plan instead of acting
on it — consistent with two things Cohere's own model card documents: (1) it explicitly requires
preserving the model's reasoning content between tool calls, warning that dropping it "force[s]
[the model] to reconstruct its plan on the next turn" (local serving needs Cohere's own "Melody"
parser for this; whether a generic harness talking to it via OpenRouter preserves that state
correctly is unverified and plausibly the actual cause), and (2) independent review found it can
"multiply fabricated completions under the pressure of a loop" — i.e. get *worse*, not better, over
repeated agentic turns. Both match the observed symptom closely enough that this isn't treated as
a fluke. Laguna's rate limit is an intermittent, recoverable annoyance; this looked like a standing
reliability problem for exactly the sustained multi-step editing a `build` agent does. North Mini
Code is not used anywhere in this repo's routing anymore.

| Consumer | Model | Provider |
|---|---|---|
| Default `model` (`build`/`general`) | `openrouter/poolside/laguna-s-2.1:free` | Poolside (reverted; if it rate-limits again, `nex-agi/nex-n2.5-pro:free` is the next thing to try, not `cohere/north-mini-code:free`) |
| `integration-test-author` | `openrouter/nex-agi/nex-n2.5-pro:free` | Nex AGI |
| `ron-gameplay-scripter` | `openrouter/poolside/laguna-s-2.1:free` | Poolside |
| `data-format-doc-writer` | `openrouter/nex-agi/nex-n2.5-mini:free` | Nex AGI (lighter sibling of the test-author's model — doc writing needs less than active Rust authoring) |

If one of these starts rate-limiting (or, per the above, misbehaving) again, the fix is the same:
edit that agent's `model` in `opencode.json` to a different provider's free model — there's no
config-level fallback list to maintain instead, and don't assume a model swap is safe just because
it's free and rated well for coding; check for known agentic-loop/tool-call reliability issues
first, the way this one was found. `.opencode/opencode_free_models.md` has the fuller candidate
list.

## The paid escalation opt-in

Every agent's **plain name** (`system-architect`, `debug-detective`) is the **free** tier — this is
what every shared command template and any automatic delegation invokes. Nothing can accidentally
spend money.

To get the paid DeepSeek-backed review, you have to ask for it by name, on purpose:
```
@system-architect-deep is this design actually sound?
```
or
```
opencode run --agent system-architect-deep "..."
```
The plain and `-deep` names share the exact same underlying prompt (`.claude/agents/<name>.md`) —
only the model differs. `-deep` agents cannot be auto-delegated to (`permission.task` denies
`*-deep` globally), so no run — including the orchestrator inside `/code-review` — can pick the
paid tier for you. Estimated cost: about $0.06 per `-deep` run.

## Memory inbox

OpenCode agents can **read** `.claude/agent-memory/<name>/` (the same knowledge base Claude's
review agents use) but cannot write to it. Anything an OpenCode agent wants to remember lands in
`.opencode/memory-inbox/<name>.md` instead, dated and tagged with the model that wrote it. Check
this folder periodically (or during a Claude Code review cycle) and promote anything worth keeping
into the real `.claude/agent-memory/<name>/` files by hand — discard the rest.

## Known limitations research (2026-09-23)

The North Mini Code incident (above) showed benchmark ratings alone don't predict whether a model
is safe for a given unsupervised agentic role. Every model currently routed in `opencode.json` was
then researched the same way — reasoning-state/tool-call caveats, fabrication-under-loop reports,
vendor-only vs. independently-verified benchmarks, real-world usable context, free-tier
reliability. Full findings with sources: `.opencode/opencode_free_models.md`'s "Known Limitations
by Model" section. None came back an outright AVOID, but three findings are worth acting on:

- **`poolside/laguna-s-2.1:free`'s headline benchmark (70.2% Terminal-Bench 2.1) requires "max
  thinking" enabled** — without it, Poolside's own numbers drop to 60.4%. `opencode.json` doesn't
  set a thinking-effort parameter for this model, and it's unconfirmed whether OpenRouter's free
  endpoint defaults to max thinking. If Laguna's real-world output quality looks weaker than the
  benchmark suggests, this is the first thing to check.
- **`nvidia/nemotron-3-ultra-550b-a55b:free`/`opencode/nemotron-3-ultra-free`** (backing
  `system-architect`, `debug-detective`, `plan`, and all three review-orchestration commands) has a
  documented reasoning-state-drop on every new turn, plus a measured ~16% failure/empty-response
  rate on OpenRouter's free endpoint. A review that comes back suspiciously thin or shallow may be
  this, not an actual clean bill of health.
- **`nex-agi/nex-n2.5-pro:free`** (`integration-test-author`) has zero independent agentic-coding
  track record — every benchmark claim is Nex-AGI's own. Generated tests from it are worth a closer
  read than usual before trusting them, since a wrong-but-plausible assertion is easy to miss.

## Machine-local note

`~/.config/opencode/AGENTS.md` (outside this repo) was created separately from this plan, at
Frank's request, to stop OpenCode falling back to the global Juva `CLAUDE.md` layer in *other*
projects that have no instructions file of their own. It has no effect on this repo (which has its
own `AGENTS.md`) and isn't part of this repo's config.

## Checking for config drift

Two tools, two jobs: `tools/opencode_sync_check.py` checks the **configuration** (agents, commands, prompts, model ids, plus two
static checks: no subfolder `AGENTS.md`, no `instructions` entry matching `.claude/rules`), and `tools/opencode_probe.py` checks
what OpenCode actually **loads** (the facts table above).

`.opencode/opencode.json` pulls every prompt from `.claude/agents/*.md`/`.claude/commands/*.md`
rather than duplicating them, so there's exactly one copy of each — but nothing stops the two
sides from drifting apart on their own (a new Claude agent added with no OpenCode entry, a renamed
`.claude` file breaking a `{file:}` reference, a `-deep` twin's prompt diverging from its
plain-named counterpart, or a configured model quietly disappearing from a provider's live list —
this last one is exactly what broke `deepseek-v4-flash-free` a couple of hours after it was first
verified present, during v1's own testing). Run this periodically, e.g. before a session that
leans on OpenCode:

```bash
python tools/opencode_sync_check.py
# Skip the live `opencode models` check (e.g. opencode isn't installed here):
python tools/opencode_sync_check.py --skip-models
```

It only checks "did something that used to be true stop being true" — it deliberately does not try
to recommend a better free model for a tier when one exists; that's a judgment call for a periodic
manual review of `.opencode/opencode_free_models.md`, not a mechanical check.

## Known gaps

See `planning/features/opencode_compatibility.md` §6 for the full list (no context-inheriting
forks, no per-subagent worktree isolation, no `Monitor`/`Cron`/`ScheduleWakeup`, free-model
reliability, etc.). The reminder hooks (`cargo check` after a schema change, and similar) now fire
under OpenCode too (phase v2, `.opencode/plugins/claude_hooks_bridge.ts`) — live-tested for the
reminder half; the blocking half (`git add pkg/`, etc.) hasn't been independently fired outside a
TUI session.

**Resolved, 2026-09-23:** a new `feature/{slug}` worktree used to have zero `.opencode/` config at
all (cut from `main`, which lags `integration` — see root `CLAUDE.md`'s Branching Model section).
`.githooks/post-checkout` now auto-syncs it on a worktree's first checkout, so this is no longer a
manual step to remember. It's still a one-off workaround, not a fix to the underlying lag — the
permanent fix is `main` catching up to `integration` at the next release promotion, after which the
hook is simply a no-op.
