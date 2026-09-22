#!/usr/bin/env python3
"""
OpenCode config drift checker.

.opencode/opencode.json pulls every agent/command prompt from .claude/agents/*.md and
.claude/commands/*.md via {file:...} substitution rather than duplicating them (see
planning/features/opencode_compatibility.md Section 2) -- deliberately, so there is exactly one
copy of each prompt. That design only holds if the two directories and the config stay in sync;
nothing enforces that automatically. This script checks for the three ways they can drift apart:

  1. A .claude/agents/ or .claude/commands/ file with no matching entry in opencode.json (a new
     Claude Code agent/command added without a matching OpenCode entry).
  2. An opencode.json agent/command entry whose {file:...} reference doesn't resolve to a real
     file (a renamed/deleted .claude file, or a typo).
  3. A "-deep" agent whose prompt doesn't exactly match its plain-named twin's -- the whole design
     for the free/paid split (see the plan's "Naming and defaults" section) depends on both using
     the identical underlying prompt, differing only in description/model.

It also checks a fourth, unrelated failure mode found the hard way during v1's live testing
(2026-09-22: `opencode/deepseek-v4-flash-free` had already disappeared from the live Zen model
list by the time it was tested, only a couple of hours after being verified present): every model
ID referenced anywhere in opencode.json is checked against the live `opencode models` list, if the
`opencode` CLI is available. This only checks "did a configured model disappear" -- deciding
whether a *new* free model would be a better fit for a given tier is a judgment call, not a
mechanical check, and is deliberately not attempted here (see the plan's v3 task note).

Usage:
    # Run all checks (uses the opencode CLI if it's on PATH)
    python tools/opencode_sync_check.py

    # Skip the live model-availability check (e.g. opencode isn't installed here)
    python tools/opencode_sync_check.py --skip-models

Exit code: 0 if no drift found, 1 if any check reports a problem, 2 on a tool-level error
(missing/malformed opencode.json).
"""

import argparse
import json
import re
import shutil
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
OPENCODE_DIR = REPO_ROOT / ".opencode"
CONFIG_PATH = OPENCODE_DIR / "opencode.json"
CLAUDE_AGENTS_DIR = REPO_ROOT / ".claude" / "agents"
CLAUDE_COMMANDS_DIR = REPO_ROOT / ".claude" / "commands"

FILE_REF_RE = re.compile(r"\{file:([^}]+)\}")


def load_config() -> dict:
    if not CONFIG_PATH.is_file():
        print(f"ERROR: {CONFIG_PATH} does not exist.")
        sys.exit(2)
    try:
        return json.loads(CONFIG_PATH.read_text(encoding="utf-8"))
    except json.JSONDecodeError as e:
        print(f"ERROR: {CONFIG_PATH} is not valid JSON: {e}")
        sys.exit(2)


def file_refs(text: str) -> list[str]:
    """Every {file:...} token in `text`, as paths relative to .opencode/."""
    return FILE_REF_RE.findall(text)


def check_missing_file_refs(config: dict) -> list[str]:
    """opencode.json entries whose {file:...} target doesn't resolve to a real file."""
    problems = []
    for section in ("agent", "command"):
        for name, entry in config.get(section, {}).items():
            text = entry.get("prompt") or entry.get("template") or ""
            for ref in file_refs(text):
                resolved = (OPENCODE_DIR / ref).resolve()
                if not resolved.is_file():
                    problems.append(
                        f"{section} '{name}': {{file:{ref}}} does not resolve to a real file "
                        f"(expected {resolved})"
                    )
    return problems


def check_unreferenced_claude_files(config: dict) -> list[str]:
    """.claude/agents|commands/*.md files that no opencode.json entry's {file:} points at."""
    problems = []

    referenced_agent_files: set[Path] = set()
    for entry in config.get("agent", {}).values():
        for ref in file_refs(entry.get("prompt", "")):
            referenced_agent_files.add((OPENCODE_DIR / ref).resolve())

    for md in sorted(CLAUDE_AGENTS_DIR.glob("*.md")):
        if md.resolve() not in referenced_agent_files:
            problems.append(
                f".claude/agents/{md.name} has no matching entry in opencode.json's 'agent' block"
            )

    referenced_command_files: set[Path] = set()
    for entry in config.get("command", {}).values():
        for ref in file_refs(entry.get("template", "")):
            referenced_command_files.add((OPENCODE_DIR / ref).resolve())

    for md in sorted(CLAUDE_COMMANDS_DIR.glob("*.md")):
        if md.resolve() not in referenced_command_files:
            problems.append(
                f".claude/commands/{md.name} has no matching entry in opencode.json's 'command' block"
            )

    return problems


def check_deep_twins_match(config: dict) -> list[str]:
    """A '-deep' agent's prompt must exactly match its plain-named twin's."""
    problems = []
    agents = config.get("agent", {})
    for name, entry in agents.items():
        if not name.endswith("-deep"):
            continue
        base_name = name[: -len("-deep")]
        base_entry = agents.get(base_name)
        if base_entry is None:
            problems.append(
                f"agent '{name}' has no plain-named twin '{base_name}' to compare its prompt against"
            )
            continue
        if entry.get("prompt") != base_entry.get("prompt"):
            problems.append(
                f"agent '{name}' and its plain twin '{base_name}' have different prompts -- "
                f"per the design (opencode_compatibility.md, 'Naming and defaults'), a '-deep' "
                f"agent should differ only in description/model, never in prompt content"
            )
    return problems


def collect_model_ids(config: dict) -> set[str]:
    ids = set()
    for key in ("model", "small_model"):
        if config.get(key):
            ids.add(config[key])
    for section in ("agent", "command"):
        for entry in config.get(section, {}).values():
            if entry.get("model"):
                ids.add(entry["model"])
    return ids


def check_models_still_exist(config: dict) -> tuple[list[str], bool]:
    """Cross-check every referenced model ID against the live `opencode models` list.

    Returns (problems, ran) -- `ran` is False if the opencode CLI wasn't available, so the caller
    can distinguish "checked, found nothing wrong" from "couldn't check at all".
    """
    opencode_bin = shutil.which("opencode")
    if opencode_bin is None:
        return [], False

    try:
        result = subprocess.run(
            [opencode_bin, "models"], capture_output=True, text=True, timeout=30
        )
    except Exception as e:
        print(f"WARNING: could not run `opencode models` ({e}); skipping model-liveness check.")
        return [], False

    if result.returncode != 0:
        print(
            f"WARNING: `opencode models` exited {result.returncode}; skipping model-liveness check."
        )
        return [], False

    live_models = {line.strip() for line in result.stdout.splitlines() if line.strip()}
    if not live_models:
        print("WARNING: `opencode models` returned no models; skipping model-liveness check.")
        return [], False

    problems = []
    for model_id in sorted(collect_model_ids(config)):
        if model_id not in live_models:
            problems.append(
                f"model '{model_id}' is referenced in opencode.json but not in the live "
                f"`opencode models` list -- it may have been renamed, removed, or expired"
            )
    return problems, True


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument(
        "--skip-models",
        action="store_true",
        help="Skip the live `opencode models` availability check (e.g. opencode isn't installed here).",
    )
    args = parser.parse_args()

    config = load_config()

    checks = [
        ("Missing {file:} targets", check_missing_file_refs(config)),
        ("Unreferenced .claude/ files", check_unreferenced_claude_files(config)),
        ("-deep prompt drift", check_deep_twins_match(config)),
    ]

    models_ran = False
    if not args.skip_models:
        model_problems, models_ran = check_models_still_exist(config)
        checks.append(("Disappeared models", model_problems))

    any_problems = False
    for title, problems in checks:
        if problems:
            any_problems = True
            print(f"\n{title}: {len(problems)} problem(s)")
            for p in problems:
                print(f"  - {p}")
        else:
            print(f"{title}: OK")

    if not args.skip_models and not models_ran:
        print(
            "\nNote: the live model-availability check did not run (opencode CLI not found, or "
            "the call failed) -- results above don't cover model disappearance this run."
        )

    if any_problems:
        print("\nDrift found -- see above.")
        return 1

    print("\nNo drift found.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
