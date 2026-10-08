"""Shared helpers for the OpenCode tooling (`opencode_sync_check.py`, `opencode_probe.py`).

Finding the `opencode` binary on this Windows machine is the awkward part: OpenCode is installed under an nvs node
(24.21) and is only on PATH after `nvs use 24.21` in the *same* terminal. A Python script cannot run `nvs use` for you
(it would only change a child process's PATH), so `find_opencode()` looks, in order, at the `OPENCODE_BIN` environment
variable, then PATH, and `not_found_message()` says exactly what to run (and where it found an nvs install, if one is
present, so the fix is one `set OPENCODE_BIN=...` away).
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
