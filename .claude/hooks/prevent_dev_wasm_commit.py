import sys
import json

try:
    data = json.load(sys.stdin)
    cmd = data.get("tool_input", {}).get("command", "")
    if ("git add" in cmd or "git commit" in cmd) and "pkg/" in cmd:
        print(
            "BLOCKED: Staging pkg/ is not allowed unless this is a verified RELEASE build.\n"
            "Dev builds bloat the repo and may exceed the GitHub Pages 100 MB limit.\n"
            "Verify the build is release: ls -lh pkg/ironhold_web_bg.wasm\n"
            "A release build requires: cargo clean && wasm-pack build crates/ironhold_web --target web --out-dir ../../pkg\n"
            "If this IS a release build, commit pkg/ manually from a terminal to bypass this check."
        )
        sys.exit(1)
except Exception:
    pass
