---
name: project-opencode-agents-md-shadows-claude-md
description: OpenCode picks AGENTS.md over CLAUDE.md per folder, so a one-line `@CLAUDE.md` AGENTS.md stub can REPLACE a folder's rules for OpenCode unless OpenCode expands `@` imports (unverified)
metadata:
  type: project
---

`planning/features/opencode_compatibility.md` F2 (read from OpenCode's `session/instruction.ts`): OpenCode tries
`AGENTS.md`, then `CLAUDE.md`, then `CONTEXT.md` and attaches the first match in each folder. Nested `CLAUDE.md` files
already loaded on first touch with no `AGENTS.md` present. The core CLAUDE.md split (feature/core_claude_md_split,
2026-10-08) added 17 `AGENTS.md` files that contain only `@CLAUDE.md`. That contradicts the split plan's own Decision 1 /
R8 / acceptance line ("no `AGENTS.md` in a subfolder"). OpenCode's public docs say it does not parse file references in
`AGENTS.md` automatically. If that is true, every folder's rules reach OpenCode as the literal 11-byte string.

**Why:** a stub that is harmless to Claude Code (Claude Code ignores `AGENTS.md`) can silently strip OpenCode's rules.
Neither test suite nor any audit script exercises OpenCode loading.

**How to apply:** when a diff adds or edits a folder `AGENTS.md`, ask for a live OpenCode check: touch a file in that
folder and see what got attached. Never accept "forwarding stub" as safe on the Claude-side evidence alone.
Related: [[project-test-web-no-scene-exclusion]] (a tool's discovery rule decides what actually runs).
