---
name: opencode-toolchain-facts
description: Verified OpenCode behaviors that shape how this repo supports a second agent CLI — AGENTS.md shadows root CLAUDE.md, {file:} single-sourcing, "permissions" key hard-fails, fresh-context subagents, OpenRouter free-tier caps
metadata:
  type: project
---

Verified 2026-09-22 against sst/opencode@dev source + docs while writing `planning/features/opencode_compatibility.md` (plan at `20287fc`, Draft).

- **Root `AGENTS.md` shadows root `CLAUDE.md` in OpenCode** — instruction discovery stops at the first filename matched (AGENTS.md > CLAUDE.md > CONTEXT.md). Fix = `"instructions": ["CLAUDE.md"]` in opencode.json. Nested CLAUDE.md files still attach dynamically on first file read in that dir (src/CLAUDE.md ≈154KB ≈38K tokens lands on first core-file read).
- `OPENCODE_DISABLE_CLAUDE_CODE_PROMPT` also removes project CLAUDE.md from discovery (not just the global one) — never recommend it here.
- **A top-level `"permissions"` key makes OpenCode throw InvalidError** (v2-compat.ts) — not just ignored. Other unknown keys are silently dropped (`onExcessProperty: "ignore"`), though the published schema has additionalProperties:false.
- **Single-source mechanism**: `{file:path}` substitution in opencode.json (relative to the config file, multiple tokens per string, JSON-escaped) lets `agent.X.prompt` / `command.X.template` include `.claude/agents/*.md` / `.claude/commands/*.md` directly. Symlinking/copying into `.opencode/agents/` fails — Claude frontmatter (`tools:` string, `model: opus`) breaks OpenCode's agent schema.
- OpenCode reads `.claude/skills/` natively but NOT `.claude/agents|commands`.
- Subagents (task tool) always start with fresh context (task_id resume only); parallel + `background: true` exist. No fork, no worktree isolation.
- OpenCode default permission is allow-all incl. bash — omitting a permission block is less safe than the Claude setup. Last-match-wins pattern rules; webfetch takes no domain patterns.
- OpenRouter free tier: 20 RPM, 50 req/day under $10 purchased credit, 1000/day at ≥$10 — per account, so spreading across free models doesn't help.

**Why:** Frank runs OpenCode alongside Claude Code on free models; see [[claude-hooks-contract-bug]] for the related hook finding.
**How to apply:** when reviewing any `.opencode/` change, check it against these facts; flag any hand-copied agent prompt (drift) or any OpenCode agent granted write access to `.claude/agent-memory/` (plan decision: read-only + `.opencode/memory-inbox/`).
