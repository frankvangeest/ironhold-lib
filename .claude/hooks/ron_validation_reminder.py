import sys
import json

from _hook_common import emit_context

try:
    data = json.load(sys.stdin)
    fp = data.get("tool_input", {}).get("file_path", "").replace("\\", "/")
    if fp.endswith(".ron") and "assets/projects/" in fp:
        parts = fp.split("/")
        try:
            idx = parts.index("projects") + 1
            project_name = parts[idx]
            emit_context(
                f"REMINDER: RON file changed — validate the project:\n"
                f"  cargo run -p ironhold_cli -- validate assets/projects/{project_name}\n"
                f"Add --strict to also catch orphaned/unreferenced keys."
            )
        except (ValueError, IndexError):
            pass
except Exception:
    pass
