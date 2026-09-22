import sys
import json

from _hook_common import emit_context

try:
    data = json.load(sys.stdin)
    fp = data.get("tool_input", {}).get("file_path", "").replace("\\", "/")
    if fp.endswith("assets.ron"):
        emit_context(
            "REMINDER: assets.ron changed — run: python tools/asset_checker/check.py\n"
            "This verifies all referenced asset paths resolve on disk."
        )
except Exception:
    pass
