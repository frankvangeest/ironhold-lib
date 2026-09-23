import sys
import json

from _hook_common import emit_context

try:
    data = json.load(sys.stdin)
    fp = data.get("tool_input", {}).get("file_path", "").replace("\\", "/")
    if "ironhold_core/src/schema/" in fp:
        emit_context(
            "REMINDER: schema file changed — run: cargo check -p ironhold_cli\n"
            "Also verify `query actions` / `query events` output if Action or event types changed."
        )
except Exception:
    pass
