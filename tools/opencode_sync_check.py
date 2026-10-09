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

Two further static checks need no `opencode` binary at all (they guard what OpenCode loads; see the verified-facts table in
.opencode/README.md, rows V1, V2 and V5, and tools/opencode_probe.py for the live counterpart):

  (a) No `AGENTS.md` or `CONTEXT.md` in a subfolder. OpenCode takes the first of AGENTS.md > CLAUDE.md > CONTEXT.md per
     folder and does NOT expand `@CLAUDE.md` inside an AGENTS.md, so such a file shadows the folder's real CLAUDE.md and
     the model sees only its literal text (found 2026-10-08 with OpenCode 1.18.33). The repo root is exempt.
  (b) No `instructions` entry in opencode.json that matches `.claude/rules/` (OpenCode never reads those files on its own,
     so loading them through `instructions` would also load every stub at session start).

A paid model id may only sit on the top-level `model`, the `build` agent and `*-deep` agents; every other agent, every
command and `small_model` must use a free model (classifier in tools/_opencode_common.py), because the driver is paid
and everything it delegates to is meant to be free (planning/features/opencode_glm_driver_pilot.md).

Every agent (except `build`), the built-in `general` subagent and every command must name its own `model`, or it inherits the
paid driver; and `permission.external_directory` must deny the M365 proxy folder (its credential files), with no agent or
command overriding that.

Each role in ALT_ROLES (tools/_opencode_common.py) needs a `<role>-alt` twin with the same prompt, a description starting
'Fallback for', and a model on a different upstream lab; the driver prompt (.opencode/prompts/driver.md) must name every such role.

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
import os
import re
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from _opencode_common import (  # noqa: E402
    ALT_ROLES,
    PAID_ALLOWED_AGENTS,
    find_opencode,
    model_lab,
    not_found_message,
    paid_model_problems,
)

REPO_ROOT = Path(__file__).resolve().parent.parent
OPENCODE_DIR = REPO_ROOT / ".opencode"
CONFIG_PATH = OPENCODE_DIR / "opencode.json"
CLAUDE_AGENTS_DIR = REPO_ROOT / ".claude" / "agents"
CLAUDE_COMMANDS_DIR = REPO_ROOT / ".claude" / "commands"

FILE_REF_RE = re.compile(r"\{file:([^}]+)\}")

# Folder names the static scan never enters anywhere in the tree, plus build output that is only skipped at the repo top level
# (an `AGENTS.md` under, say, `assets/projects/x/target/` is still a real problem).
SKIP_DIRS = {".git", "node_modules", "__pycache__"}
TOP_LEVEL_SKIP = {"target", "pkg"}
SHADOWING_NAMES = ("AGENTS.md", "CONTEXT.md")


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


PROXY_FOLDER_PATTERN = "C:/ProgramData/m365-copilot-proxy/**"


def check_explicit_models(config: dict) -> list[str]:
    """Every agent except `build`, the built-in `general` subagent and every command names its own model.

    The driver (the top-level `model`) may become a paid model; anything without an explicit `model` inherits it and would
    silently become paid too (verified: `general` and the model-less commands inherit the default, planning/features/
    opencode_glm_driver_pilot.md, 'Facts established').
    """
    problems = []
    agents = config.get("agent", {})
    for name, entry in agents.items():
        if name not in PAID_ALLOWED_AGENTS and name != "general" and not entry.get("model"):
            problems.append(f"agent '{name}' has no explicit `model` and would inherit the (possibly paid) driver model")
    if not agents.get("general", {}).get("model"):
        problems.append("the built-in `general` subagent has no explicit `model` (add `agent.general.model` with a free model)")
    for name, entry in config.get("command", {}).items():
        if not entry.get("model"):
            problems.append(f"command '{name}' has no explicit `model` and would inherit the (possibly paid) driver model")
    return problems


def check_proxy_folder_denied(config: dict) -> list[str]:
    """`external_directory` must deny the M365 proxy folder (its credential and TOTP files live there), and nothing may override it.

    A `read` deny with the same absolute pattern does NOT work (the read tool matches a project-relative path); the
    `external_directory` deny does (fact V19 in .opencode/README.md). The rule is harmless on a machine without the proxy.
    """
    problems = []
    perm = config.get("permission", {}).get("external_directory")
    if not isinstance(perm, dict) or perm.get(PROXY_FOLDER_PATTERN) != "deny":
        problems.append(f"top-level permission.external_directory must be an object that denies '{PROXY_FOLDER_PATTERN}' (fact V19)")
    for section in ("agent", "command"):
        for name, entry in config.get(section, {}).items():
            override = entry.get("permission", {}).get("external_directory") if isinstance(entry.get("permission"), dict) else None
            if override is not None and (not isinstance(override, dict) or override.get(PROXY_FOLDER_PATTERN) != "deny"):
                problems.append(f"{section} '{name}' overrides external_directory without denying '{PROXY_FOLDER_PATTERN}'")
    return problems


def check_alt_twins(config: dict) -> list[str]:
    """Every role in ALT_ROLES has a `<role>-alt` twin: same prompt, on a different upstream lab, described as a fallback.

    A twin only helps when it fails differently from its primary: Nemotron via `opencode/` and via `openrouter/` is one upstream
    pool (planning/features/opencode_glm_driver_pilot.md), so the check compares labs, not gateways. The driver prompt must name
    every role so it can tell the driver which agents have a fallback.
    """
    problems = []
    agents = config.get("agent", {})
    for role in ALT_ROLES:
        twin_name = role + "-alt"
        twin = agents.get(twin_name)
        if twin is None:
            problems.append(f"role '{role}' has no '{twin_name}' fallback agent")
            continue
        base = agents.get(role, {})
        if base.get("prompt") is not None and twin.get("prompt") != base.get("prompt"):
            problems.append(f"agent '{twin_name}' must use the same prompt as '{role}' (only description/model may differ)")
        if twin.get("mode") != "subagent":
            problems.append(f"agent '{twin_name}' must have mode 'subagent'")
        if not str(twin.get("description", "")).startswith("Fallback for"):
            problems.append(f"agent '{twin_name}' description must start with 'Fallback for' (the driver picks agents by description)")
        twin_model, base_model = twin.get("model", ""), base.get("model", "")
        twin_lab, base_lab = model_lab(twin_model), model_lab(base_model)
        if twin_lab is None or base_lab is None:
            problems.append(f"cannot tell the upstream lab of '{twin_model}' or '{base_model}' (extend ZEN_LABS in tools/_opencode_common.py)")
        elif twin_lab == base_lab:
            problems.append(f"agent '{twin_name}' ({twin_model}) is on the same upstream lab ('{twin_lab}') as '{role}'; a fallback must fail differently")
    for name in agents:
        if name.endswith("-alt") and name[: -len("-alt")] not in ALT_ROLES:
            problems.append(f"agent '{name}' is not for a role in ALT_ROLES (tools/_opencode_common.py)")
    driver = OPENCODE_DIR / "prompts" / "driver.md"
    if not driver.is_file():
        problems.append(".opencode/prompts/driver.md is missing")
    else:
        text = driver.read_text(encoding="utf-8")
        for role in ALT_ROLES:
            if f"`{role}`" not in text:
                problems.append(f".opencode/prompts/driver.md does not name the role `{role}`")
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


def check_no_subfolder_instruction_files(root: Path = REPO_ROOT) -> list[str]:
    """AGENTS.md / CONTEXT.md below the repo root shadow the CLAUDE.md beside them in OpenCode and are not @-expanded."""
    problems = []
    for dirpath, dirnames, filenames in os.walk(root):
        rel_dir = Path(dirpath).relative_to(root)
        at_top = rel_dir == Path(".")
        # `.claude/worktrees` holds ephemeral agent worktrees (git-excluded); never descend into them.
        dirnames[:] = [d for d in dirnames
                       if d not in SKIP_DIRS and not (at_top and d in TOP_LEVEL_SKIP) and not (rel_dir == Path(".claude") and d == "worktrees")]
        if at_top:
            continue
        # File names are matched case-insensitively: OpenCode on Windows attaches `agents.md` as `AGENTS.md` too.
        upper = {n.upper(): n for n in filenames}
        for name in SHADOWING_NAMES:
            if name.upper() in upper:
                actual = upper[name.upper()]
                beside = " (it shadows the CLAUDE.md beside it)" if "CLAUDE.MD" in upper else ""
                problems.append(
                    f"{(rel_dir / actual).as_posix()}: a subfolder {name} is attached as literal text, `@CLAUDE.md` is not "
                    f"expanded{beside}; delete it (docs/dev/claude_md_maintenance.md)"
                )
    return sorted(problems)


def expand_braces(pattern: str) -> list[str]:
    """`{a,b}` alternatives expanded (one level at a time, recursively); a pattern without braces is returned as is."""
    m = re.search(r"\{([^{}]*)\}", pattern)
    if not m:
        return [pattern]
    out: list[str] = []
    for alt in m.group(1).split(","):
        out += expand_braces(pattern[: m.start()] + alt + pattern[m.end():])
    return out


def glob_matches(pattern: str, path: str) -> bool:
    """Glob semantics close to what OpenCode's `instructions` use: `*` and `?` stay inside one path segment, `**` crosses them."""
    for pat in expand_braces(pattern):
        regex, i = "", 0
        while i < len(pat):
            if pat.startswith("**/", i):
                regex += "(?:.*/)?"
                i += 3
            elif pat.startswith("**", i):
                regex += ".*"
                i += 2
            elif pat[i] == "*":
                regex += "[^/]*"
                i += 1
            elif pat[i] == "?":
                regex += "[^/]"
                i += 1
            else:
                regex += re.escape(pat[i])
                i += 1
        if re.fullmatch(regex, path):
            return True
    return False


def check_instructions_not_matching_rules(config: dict, root: Path = REPO_ROOT) -> list[str]:
    """`instructions` globs must not match `.claude/rules/*.md` (OpenCode never reads them; loading would be a mistake)."""
    problems = []
    rules_dir = root / ".claude" / "rules"
    rules = sorted(p.relative_to(root).as_posix() for p in rules_dir.rglob("*.md")) if rules_dir.is_dir() else []
    for entry in config.get("instructions", []) or []:
        norm = str(entry).replace("\\", "/")
        while norm.startswith("./"):
            norm = norm[2:]
        root_prefix = root.as_posix().rstrip("/") + "/"
        if norm.lower().startswith(root_prefix.lower()):  # an absolute path inside this checkout
            norm = norm[len(root_prefix):]
        if norm.startswith(".claude/rules") or any(glob_matches(norm, r) for r in rules):
            problems.append(f"instructions entry '{entry}' matches .claude/rules (path-scoped Claude Code stubs; OpenCode must not load them)")
    return problems


def check_models_still_exist(config: dict) -> tuple[list[str], bool]:
    """Cross-check every referenced model ID against the live `opencode models` list.

    Returns (problems, ran) -- `ran` is False if the opencode CLI wasn't available, so the caller
    can distinguish "checked, found nothing wrong" from "couldn't check at all".
    """
    opencode_bin = find_opencode()
    if opencode_bin is None:
        print(not_found_message("opencode_sync_check", static_hint=False))
        print("(Skipping the live model-availability check; everything else still runs. `--skip-models` silences this.)")
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
        ("Paid models outside the allowed keys", paid_model_problems(config)),
        ("Agents/commands without an explicit model", check_explicit_models(config)),
        ("M365 proxy folder denied", check_proxy_folder_denied(config)),
        ("-alt fallback twins", check_alt_twins(config)),
        ("Subfolder AGENTS.md / CONTEXT.md", check_no_subfolder_instruction_files()),
        ("`instructions` matching .claude/rules", check_instructions_not_matching_rules(config)),
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
