---
name: claude-hooks-contract-bug
description: .claude/hooks/*.py used to be no-ops in Claude Code (exit 1 / plain stdout); FIXED 2026-09-22 via shared _hook_common.py (block() = exit 2 + stderr, emit_context() = additionalContext JSON) — check new hooks use it
metadata:
  type: project
---

Found 2026-09-22 (at `20287fc`) during the OpenCode compatibility plan; fixed the same day by the coordinator (working tree, to be committed as opencode_compatibility "v0").

Claude Code contract (code.claude.com/docs/en/hooks): only **exit 2** blocks PreToolUse (stderr is the reason fed to Claude); exit 1 is non-blocking; PostToolUse plain stdout on exit 0 goes to the debug log only — to reach Claude it needs JSON `hookSpecificOutput.additionalContext`. The original 8 scripts printed "BLOCKED" + exit 1, or plain reminders on exit 0 → silently did nothing for the whole life of the hooks.

Fix: `.claude/hooks/_hook_common.py` with `block(text)` (stderr + exit 2) and `emit_context(text)` (PostToolUse additionalContext JSON); all 8 scripts use it. Scripts import it via their own dir (must be run by path, as settings.json does).

**Why:** a hook that "looks like" a guard but violates the contract is worse than none — reviewers cite it as a mitigation.
**How to apply:** any new hook script must go through `_hook_common`; flag raw `print` + `sys.exit(1)` patterns. The OpenCode v2 bridge plugin maps exactly these two output shapes. Related: [[opencode-toolchain-facts]].
