import sys
import json

from _hook_common import emit_context

try:
    data = json.load(sys.stdin)
    fp = data.get("tool_input", {}).get("file_path", "").replace("\\", "/")
    if "action_executor" in fp:
        emit_context(
            "REMINDER: action_executor.rs changed — check that action_name() in\n"
            "crates/ironhold_cli/src/commands/query.rs has an arm for every Action variant.\n"
            "A missing arm causes a compile error; an exhaustive match is enforced there."
        )
except Exception:
    pass
