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
Fix: spread the 4 C-tier consumers across **3 different upstream providers**, so at most 2 share
any single provider's bottleneck at once:

| Consumer | Model | Provider |
|---|---|---|
| Default `model` (`build`/`general`) | `openrouter/cohere/north-mini-code:free` | Cohere |
| `integration-test-author` | `openrouter/nex-agi/nex-n2.5-pro:free` | Nex AGI |
| `ron-gameplay-scripter` | `openrouter/poolside/laguna-s-2.1:free` | Poolside (kept — still the highest-rated pick when it's actually available) |
| `data-format-doc-writer` | `openrouter/nex-agi/nex-n2.5-mini:free` | Nex AGI (lighter sibling of the test-author's model — doc writing needs less than active Rust authoring) |

If one of these starts rate-limiting too, the fix is the same: edit that agent's `model` in
`opencode.json` to a different provider's free model — there's no config-level fallback list to
maintain instead. `.opencode/opencode_free_models.md` has the fuller candidate list.

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

## Checking for config drift

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
