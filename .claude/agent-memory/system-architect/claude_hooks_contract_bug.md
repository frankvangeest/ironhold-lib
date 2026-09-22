---
name: claude-hooks-contract-bug
description: All 8 .claude/hooks/*.py scripts are no-ops in Claude Code — "BLOCKED" hooks exit 1 (non-blocking) and reminders print plain stdout (debug-log only); real pkg/ guard is .githooks/pre-commit
metadata:
  type: project
---

Found 2026-09-22 (at `20287fc`) during the OpenCode compatibility plan. Claude Code contract (code.claude.com/docs/en/hooks): only **exit 2** blocks PreToolUse (stderr is the reason fed to Claude); exit 1 is non-blocking; PostToolUse plain stdout on exit 0 goes to the debug log only — to reach Claude it needs JSON `hookSpecificOutput.additionalContext` or exit 2 + stderr.

- `prevent_dev_wasm_commit.py`, `check_glb_previews.py`: print "BLOCKED" to stdout + `sys.exit(1)` → never block.
- The 6 reminder scripts (schema/action_executor/assets_ron/ron_validation/action_docs/capability_registration): plain stdout, exit 0 → Claude never sees them.
- Planned as a backlog bug + prerequisite of opencode_compatibility v2 (a TS plugin that runs the same scripts via `.claude/settings.json`'s hooks block).

**Why:** anyone assuming "the hook will catch it" (pkg/ staging, schema→CLI check reminder) is relying on a guard that does nothing; `.githooks/pre-commit` is the only real pkg/ guard.
**How to apply:** don't cite these hooks as a mitigation in reviews until the fix lands; check the exit-code/output contract on any new hook script. Related: [[opencode-toolchain-facts]].
