"""Shared helpers for Claude Code hook scripts.

See code.claude.com/docs/en/hooks for the exit-code/stdout contract these wrap:
plain stdout on exit 0 or 1 never reaches the model (debug log only, or a bare
non-blocking error notice) — a reminder needs hookSpecificOutput.additionalContext,
and a hard block needs exit 2 with the message on stderr.
"""
import json
import sys


def emit_context(text: str) -> None:
    """PostToolUse: surface `text` to the model via hookSpecificOutput.additionalContext."""
    print(json.dumps({
        "hookSpecificOutput": {
            "hookEventName": "PostToolUse",
            "additionalContext": text,
        }
    }))


def block(text: str) -> None:
    """PreToolUse: hard-block the tool call. Only exit 2 + stderr reliably blocks."""
    print(text, file=sys.stderr)
    sys.exit(2)
