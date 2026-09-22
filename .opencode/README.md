# OpenCode setup for ironhold-lib

Design doc: `planning/features/opencode_compatibility.md` (read that first for the full reasoning
— this file is the quick-reference for actually using the setup).

**Tested OpenCode version:** `1.18.31` (confirmed 2026-09-22 — config loads clean, `instructions`/
`{file:}`/delegation/the `-deep` permission deny were all live-tested against this exact version;
see `planning/features/opencode_compatibility.md`'s verification checklist for what was and wasn't
covered). The design itself was researched against the `sst/opencode@dev` source tree, so if a
future OpenCode release changes behavior, `opencode debug config` is the first thing to check.

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

## Model tiers

| Tier | Model | Used by |
|---|---|---|
| E (paid) | `openrouter/deepseek/deepseek-v4.1-flash` | Only `system-architect-deep` / `debug-detective-deep` |
| R (free reasoning) | `opencode/nemotron-3-ultra-free` or `openrouter/nvidia/nemotron-3-ultra-550b-a55b:free` | `system-architect`, `debug-detective`, `alignment-reviewer`, `/code-review`/`/plan-review`/`/ship` orchestration, the built-in `plan` agent |
| R-lite | `openrouter/nvidia/nemotron-3-super-120b-a12b:free` | `wasm-perf-reviewer` |
| C (free code) | `openrouter/poolside/laguna-s-2.1:free` | `integration-test-author`, `ron-gameplay-scripter`, `data-format-doc-writer`, the default `build`/`general` agents |
| G (free, low-volume) | `google/gemini-3.8-flash` | `ux-gamedesigner-reviewer`, `game-world-designer` |
| F (free, light) | `opencode/deepseek-v4-flash-free` | `/query`, `/validate`, `/wasm-dev`, `/rust-docs` |
| T (titles) | `opencode/nemotron-3.5-lightning-free` | `small_model` (background title generation) only |
| X (explore) | `openrouter/thinkingmachines/inkling-small:free` | The built-in `explore` agent |

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

## Machine-local note

`~/.config/opencode/AGENTS.md` (outside this repo) was created separately from this plan, at
Frank's request, to stop OpenCode falling back to the global Juva `CLAUDE.md` layer in *other*
projects that have no instructions file of their own. It has no effect on this repo (which has its
own `AGENTS.md`) and isn't part of this repo's config.

## Known gaps

See `planning/features/opencode_compatibility.md` §6 for the full list (no context-inheriting
forks, no per-subagent worktree isolation, no `Monitor`/`Cron`/`ScheduleWakeup`, free-model
reliability, etc.). The reminder hooks (`cargo check` after a schema change, and similar) don't
fire under OpenCode yet — that's phase v2, not yet built.
