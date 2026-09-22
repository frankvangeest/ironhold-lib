# Ironhold Library — Agent Onboarding

`CLAUDE.md` files (starting with the one at the repo root, plus one per crate/tools/docs folder)
are the source of truth for this project's architecture, workflow, and hard-learned lessons — read
them, not a duplicate summary here. This file used to carry its own copy of several of those rules;
that copy has been removed because it drifted out of date (it claimed camera-following logic runs
in `FixedUpdate` — it actually runs in `Update`; see `crates/ironhold_core/src/CLAUDE.md`'s
"Physics & movement must use `FixedUpdate`" section for the real, current rule) and because
maintaining the same rules in two places is exactly the kind of drift this project's own planning
conventions (`planning/CLAUDE.md`) exist to prevent elsewhere.

If you are OpenCode: this file is discovered first (`AGENTS.md` wins over `CLAUDE.md` in
per-directory discovery), and the project config (`.opencode/opencode.json`) additionally loads
the root `CLAUDE.md` via its `instructions` field specifically so you see the real rules too — read
both. Nested `CLAUDE.md` files (per-crate, per-tool, per-docs-folder) load automatically the first
time you touch a file in that folder, the same way they do for Claude Code.

## Running outside Claude Code

Some of `CLAUDE.md`'s conventions reference Claude Code-specific mechanics. If you are not Claude
Code, translate them:

| Claude Code mechanic | Under OpenCode |
|---|---|
| `Agent` tool / "launch agents in parallel" | OpenCode's `task` tool (same parallel/background semantics, but every invocation starts cold — no context-inheriting fork) |
| A slash command (`/code-review`, `/ship`, etc.) | The identically-named OpenCode command |
| `Monitor`, `ToolSearch`, `EnterWorktree`/`isolation: "worktree"`, `Cron*`/`ScheduleWakeup`, Claude's `Skill` tool | No equivalent — skip and proceed with the rest of the instructions |

**Merge gate (applies to every AI tool other than Claude Code, including OpenCode):** any review
you produce is **advisory only**. The mandatory review that gates a merge into `integration` per
this repo's branching model is Claude Code's own `/code-review` — nothing else substitutes for it,
regardless of which model or tool produced the review.

**Agent memory:** `.claude/agent-memory/<agent-name>/` is a durable knowledge base the Claude-side
review agents read and write. If you are not Claude Code, you may **read** it but must **never
write to it directly** — write anything worth remembering to
`.opencode/memory-inbox/<agent-name>.md` instead, tagged with your own model id, for later triage.
This exists so a less-verified or weaker model can never silently corrupt the shared knowledge
base.

See `.opencode/README.md` for the full OpenCode setup (providers, model routing tiers, the
paid-escalation opt-in convention) and `planning/features/opencode_compatibility.md` for the design
this was built from.
