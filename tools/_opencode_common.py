"""Shared helpers for the OpenCode tooling (`opencode_sync_check.py`, `opencode_probe.py`).

Finding the `opencode` binary on this Windows machine is the awkward part: OpenCode is installed under an nvs node
(24.21) and is only on PATH after `nvs use 24.21` in the *same* terminal. A Python script cannot run `nvs use` for you
(it would only change a child process's PATH), so `find_opencode()` looks, in order, at the `OPENCODE_BIN` environment
variable, then PATH, and `not_found_message()` says exactly what to run (and where it found an nvs install, if one is
present, so the fix is one `set OPENCODE_BIN=...` away).

It also holds the one free/paid model classifier (`model_class`) and the rule for where a paid model may appear
(`paid_model_problems`), shared by `opencode_sync_check.py` (static repo config) and `opencode_probe.py` (live run
filter), so the two cannot disagree.
"""

import glob
import os
import shutil
import subprocess
from pathlib import Path

NVS_VERSION = "24.21"


def _real_binary_next_to(shim: str) -> str:
    """Prefer the real `opencode.exe` over the npm `.cmd`/`.ps1` shim next to it.

    The shims re-parse their arguments (JSON double quotes get mangled, backslashes are eaten), the real binary does not.
    """
    base = Path(shim).resolve().parent
    candidate = base / "node_modules" / "opencode-ai" / "bin" / "opencode.exe"
    return str(candidate) if candidate.is_file() else shim


def find_opencode() -> str | None:
    """Path to a runnable `opencode`: OPENCODE_BIN, then PATH. None if neither has it."""
    env_bin = os.environ.get("OPENCODE_BIN")
    if env_bin:
        return env_bin if Path(env_bin).is_file() else None
    found = shutil.which("opencode")
    return _real_binary_next_to(found) if found else None


def nvs_installs() -> list[str]:
    """`opencode.exe` files found under the machine-wide nvs node installs (Windows), newest first."""
    pattern = os.path.join(
        os.environ.get("ProgramData", r"C:\ProgramData"), "nvs", "node", "*", "x64", "node_modules", "opencode-ai", "bin", "opencode.exe"
    )
    return sorted(glob.glob(pattern), reverse=True)


def not_found_message(tool: str, static_hint: bool = True) -> str:
    env_bin = os.environ.get("OPENCODE_BIN")
    if env_bin and not Path(env_bin).is_file():
        lines = [f"{tool}: OPENCODE_BIN={env_bin} is not a file."]
    else:
        lines = [f"{tool}: 'opencode' not found on PATH."]
    lines.append(f"On this Windows machine OpenCode is installed under nvs node {NVS_VERSION}. In the same terminal run:")
    lines.append(f"    nvs use {NVS_VERSION}")
    lines.append(f"    python tools/{tool}.py")
    lines.append("Global npm tools exist per Node version: if the active version lacks it, `nvs use` one that has it, or run "
                 "`npm install -g opencode-ai` on the active (even-numbered LTS) version.")
    installs = nvs_installs()
    if installs:
        lines.append(f"(An nvs install exists at {installs[0]}; setting OPENCODE_BIN to it also works.)")
    if static_hint:
        lines.append(f"Only want the checks that need no model? Run: python tools/{tool}.py --static")
    return "\n".join(lines)


def opencode_version(binary: str) -> str | None:
    try:
        out = subprocess.run([binary, "--version"], capture_output=True, text=True, timeout=30)
    except (OSError, subprocess.SubprocessError):
        return None
    text = (out.stdout or "").strip().splitlines()
    return text[-1].strip() if out.returncode == 0 and text else None


# ------------------------------------------------------------------------------------------------ free / paid classifier
# Three classes. Anything not recognised is "paid": a new model fails closed instead of being spent on by accident.
#   free          costs nothing per token for this repo. Either an id carrying the OpenRouter `:free` / OpenCode Zen `-free`
#                 convention, or an id listed in FREE_MODELS below.
#   machine-local costs nothing extra but only exists on a machine that runs the local M365 Copilot proxy (provider `m365`,
#                 127.0.0.1:4141, part of the company licence). Exactly the ids in MACHINE_LOCAL_MODELS; never `m365/m365-copilot`
#                 (the auto tone, reported to confabulate).
#   paid          everything else.
FREE_MODELS = {
    # Free only while no billing account is linked to the Google AI key (Frank, 2026-10-08: none is). Remove this entry if one is.
    "google/gemini-3.8-flash",
}
MACHINE_LOCAL_MODELS = {
    "m365/gpt-5.5-think-deeper",
    "m365/gpt-5.6-think-deeper",        # pilot candidates (2026-10-09): free of cost, not preferred (tool calling failed the pilot rule)
    "m365/claude-sonnet-think-deeper",
}
# Where a paid model may appear in .opencode/opencode.json: the top-level `model` (the driver), the `build` agent (inherits it)
# and any agent whose name ends in `-deep` (explicit, asks first, never in an automatic chain).
PAID_ALLOWED_TOP_KEYS = {"model"}
PAID_ALLOWED_AGENTS = {"build"}
PAID_ALLOWED_SUFFIX = "-deep"


def model_class(model_id: str) -> str:
    """'free', 'machine-local' or 'paid' (the default for anything unknown)."""
    if model_id in MACHINE_LOCAL_MODELS:
        return "machine-local"
    # `:free` is OpenRouter's convention and `-free` is OpenCode Zen's; each only counts on its own gateway.
    if model_id in FREE_MODELS or (model_id.startswith("openrouter/") and model_id.endswith(":free")) \
            or (model_id.startswith("opencode/") and model_id.endswith("-free")):
        return "free"
    return "paid"


def is_free_model(model_id: str) -> bool:
    """True when using the model costs nothing extra (free or machine-local)."""
    return model_class(model_id) != "paid"


def paid_model_problems(config: dict) -> list[str]:
    """Paid model ids outside the allowed places (see PAID_ALLOWED_*): small_model, other agents, commands."""
    problems = []

    def check(where: str, model_id, allowed: bool) -> None:
        if model_id and model_class(model_id) == "paid" and not allowed:
            problems.append(f"{where} uses '{model_id}', which is not on the free list (paid models may only be the top-level "
                            f"`model`, `build` and `*{PAID_ALLOWED_SUFFIX}` agents; add the id to FREE_MODELS in "
                            f"tools/_opencode_common.py only if it really costs nothing)")

    check("small_model", config.get("small_model"), False)
    check("model", config.get("model"), True)
    for name, entry in config.get("agent", {}).items():
        check(f"agent '{name}'", entry.get("model"), name in PAID_ALLOWED_AGENTS or name.endswith(PAID_ALLOWED_SUFFIX))
    for name, entry in config.get("command", {}).items():
        check(f"command '{name}'", entry.get("model"), False)
    return problems


# ---------------------------------------------------------------------------------------------------- roster / fallback twins
# Roles that have a free `<role>-alt` twin on a DIFFERENT upstream lab (decided 2026-10-08: the roles that run on free models; the
# Gemini roles have none and simply stop and report). The roster table in .opencode/README.md is kept by hand;
# opencode_sync_check.py checks the twins in opencode.json and that .opencode/prompts/driver.md names every role.
ALT_ROLES = (
    "system-architect",
    "debug-detective",
    "alignment-reviewer",
    "wasm-perf-reviewer",
    "integration-test-author",
    "ron-gameplay-scripter",
    "data-format-doc-writer",
    "explore",
)
# OpenCode Zen ids carry no vendor segment, so map their name prefix to the upstream lab (extend when a new Zen model is routed).
ZEN_LABS = {"nemotron": "nvidia", "ling": "inclusionai", "deepseek": "deepseek", "mimo": "xiaomi", "muse": "meta"}


def model_lab(model_id: str):
    """Upstream model vendor ('nvidia', 'poolside', 'thinkingmachines', 'nex-agi', 'google', ...), or None when unknown.

    The gateway prefix (`opencode/` vs `openrouter/`) is NOT the lab: Nemotron through either is one upstream capacity pool, so a
    rate limit hits both. A fallback twin only helps if it is on another lab.
    """
    provider, _, rest = model_id.partition("/")
    if provider == "openrouter":
        return rest.split("/")[0] or None
    if provider == "opencode":
        for prefix, lab in ZEN_LABS.items():
            if rest.startswith(prefix):
                return lab
        return None
    return provider or None


# `opencode run` / `opencode debug` take their working directory from PWD or --dir, not from the process cwd (fact V21), so every call
# that targets a checkout other than the caller's must set it.
def opencode_env(root, base=None) -> dict:
    env = dict(os.environ if base is None else base)
    env["PWD"] = str(root)
    return env


# Unattended `opencode run` auto-approves `ask` (fact V20): a script that does not want a paid `*-deep` agent must deny it explicitly.
UNATTENDED_TASK_DENY = '{"permission":{"task":{"*":"allow","*-deep":"deny"}}}'
