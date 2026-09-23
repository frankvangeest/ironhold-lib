import re
import subprocess
import sys
import json

from _hook_common import block


def command_stages_pkg(cmd: str) -> bool:
    """True if `cmd` runs `git add` with a pkg/ path argument. Tokenizes each `git
    add` invocation's own arguments rather than scanning the whole command string,
    so a commit message that merely mentions "pkg/" can't trigger a false block."""
    for m in re.finditer(r"git\s+add\b([^\n;&|]*)", cmd):
        for token in m.group(1).split():
            token = token.strip("'\"")
            if token == "pkg" or token.startswith("pkg/") or "/pkg/" in token:
                return True
    return False


def commit_has_staged_pkg() -> bool:
    """True if any currently-staged file is under pkg/ -- covers `git commit` running
    after an earlier, separate `git add pkg/...` command."""
    try:
        result = subprocess.run(
            ["git", "diff", "--cached", "--name-only"],
            capture_output=True, text=True,
        )
        return any(f.startswith("pkg/") for f in result.stdout.splitlines())
    except Exception:
        return False


try:
    data = json.load(sys.stdin)
    cmd = data.get("tool_input", {}).get("command", "")
    if command_stages_pkg(cmd) or ("git commit" in cmd and commit_has_staged_pkg()):
        block(
            "BLOCKED: Staging pkg/ is not allowed unless this is a verified RELEASE build.\n"
            "Dev builds bloat the repo and may exceed the GitHub Pages 100 MB limit.\n"
            "Verify the build is release: ls -lh pkg/ironhold_web_bg.wasm\n"
            "A release build requires: cargo clean && wasm-pack build crates/ironhold_web --target web --out-dir ../../pkg\n"
            "If this IS a release build, commit pkg/ manually from a terminal to bypass this check."
        )
except Exception:
    pass
