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

- **`permission.task` deny removes a subagent from the task tool entirely, but users can still `@mention` it** — the mechanism behind the plan's free-by-default naming: plain agent names (what shared `.claude/commands` templates invoke) are free-tier; paid DeepSeek only as `<name>-deep`, task-denied via `"*-deep": "deny"`. Shared templates can't carry per-tool tier choices, so tier must be encoded in the agent-name → model mapping, never in the template.

**Verified live 2026-10-08, OpenCode 1.18.33 (opencode_compat_probe plan review):**
- **Model-free attach probe exists:** `opencode debug agent build --tool read --params "{filePath:'<repo-rel/forward-slash>',limit:2}"` runs the real read tool with no model; JSON `result.metadata.loaded` = attached instruction files (nearest first), `result.output` carries the full text. Use JS-literal single quotes (cmd/.ps1 shims mangle JSON double quotes; backslashes in params get eaten).
- Each debug tool run creates a session titled "Debug tool run (build)" (not taggable); `XDG_DATA_HOME=<tmp>` (+`XDG_STATE_HOME`) isolates the session DB — no history pollution, and the model-free path needs no auth.
- Attach results: src file -> folder file(s) up to but excluding the root; `lib.rs` -> `src/CLAUDE.md` only; files outside any CLAUDE.md folder (Cargo.toml, docs/dev, ironhold_web, .claude/rules) -> nothing. `<!-- b:N -->` anchors are NOT stripped (51 reach the model via entity_spawner.rs) — ~15 bytes each, negligible.
- `opencode` is not on Bash PATH (only after `nvs use 24.21` in PowerShell) — `opencode_sync_check.py`'s live model check silently skips from Bash.
- Default driver = top-level `model` (laguna :free); `build` agent has no model of its own. `opencode debug config` shows it model-free.
- 27 machine-local skills (Juva `~/.agents/skills` + claude.ai-synced `~/.claude/skills/synced`) appear in every repo session (`opencode debug skill`) — the token baseline is machine-specific.
- `opencode run` has `--title`, `--dir`, `--variant` (reasoning effort), `--pure`; `opencode session list|delete`, `opencode export`, `opencode stats` exist.
- **More model-free introspection (verified 2026-10-08 code review):** `opencode debug agent <name>` (no `--tool`) prints the agent's resolved JSON (model, permission list) — the hook for per-agent free/paid and read-permission assertions; `opencode debug paths` prints data/state dirs and is the positive way to prove XDG isolation (better than diffing `session list`, which silently no-ops if its table format changes). `session list` = `ses_<id>  <title>  <updated>` table. `--params` paths with backslashes fail loud ("cratesironhold_coresrclib.rs" not found, exit 1) — normalize `--only` input.
- **Shipped probe (`tools/opencode_probe.py`) silent-degradation spots:** `metadata.get("loaded", [])` defaults silently (a renamed key → negatives PASS, positives FAIL as exit 1 not exit 2); the V15 session-diff check; V2 (@-expansion) is only re-verifiable with a throwaway stub, never by the repo-state `--static` check. `claude_md_audit.py` does NOT check headings cited by code/tests.
- A paid top-level `model` would also make the task-delegatable `general` subagent and model-less commands (new-project, rust-idioms) paid — it breaks the free-by-default guarantee, not just restates it.

Frank's decisions (2026-09-22): OpenCode reviews are advisory-only (Claude `/code-review` is the merge gate); 3 providers (OpenRouter w/ $10 credit, Zen, Gemini free tier via `google/`); paid `deepseek/deepseek-v4.1-flash` is the sole paid tier — Zen `deepseek-v4-flash-free` is NOT a substitute (lighter flash-class; used for run-and-report tier); `~/.config/opencode/AGENTS.md` is machine-local, suppresses the global Juva layer. Gemini free tier only applies to non-billing GCP projects.

**Why:** Frank runs OpenCode alongside Claude Code on free models; see [[claude-hooks-contract-bug]] for the related hook finding.
**How to apply:** when reviewing any `.opencode/` change, check it against these facts; flag any hand-copied agent prompt (drift) or any OpenCode agent granted write access to `.claude/agent-memory/` (plan decision: read-only + `.opencode/memory-inbox/`).
