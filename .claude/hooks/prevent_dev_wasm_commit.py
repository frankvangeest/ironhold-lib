import sys
import json

try:
    data = json.load(sys.stdin)
    cmd = data.get("tool_input", {}).get("command", "")
    if ("git add" in cmd or "git commit" in cmd) and "pkg/" in cmd:
        print(
            "WARNING: You are staging pkg/ — confirm this is a RELEASE build (not --dev).\n"
            "Dev builds bloat the repo and may exceed the GitHub Pages 100 MB limit.\n"
            "Verify: ls -lh pkg/ironhold_web_bg.wasm\n"
            "A release build must be built with: cargo clean && wasm-pack build crates/ironhold_web --target web --out-dir ../../pkg"
        )
except Exception:
    pass
