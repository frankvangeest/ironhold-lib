import sys
import json

try:
    data = json.load(sys.stdin)
    fp = data.get("tool_input", {}).get("file_path", "").replace("\\", "/")
    if "ironhold_core/src/schema/actions" in fp and fp.endswith(".rs"):
        print(
            "REMINDER: schema/actions.rs changed — new Action variants need entries in ALL 3 doc surfaces:\n"
            "  1. docs/20_data_formats.md          — 'Available actions' table (~line 1143)\n"
            "  2. docs/30_runtime_events_and_logic.md — Actions appendix + Action model section (~line 258)\n"
            "  3. docs/STATUS.md                   — Engine ABI list (~line 85)\n"
            "Also: if the new variant targets entities, add it to the {self} targets list in crates/ironhold_core/src/CLAUDE.md."
        )
except Exception:
    pass
