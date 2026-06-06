import sys
import json

try:
    data = json.load(sys.stdin)
    fp = data.get("tool_input", {}).get("file_path", "").replace("\\", "/")
    if "capabilities/mod.rs" in fp or (
        "ironhold_core/src/lib.rs" in fp
    ):
        print(
            "REMINDER: Capability wiring file changed — verify any new capability is registered in ALL of:\n"
            "  1. capabilities/mod.rs       — pub mod + pub use\n"
            "  2. ironhold_core/src/lib.rs  — .add_plugins(MyCapabilityPlugin) and system scheduling\n"
            "  3. schema/                   — RON-serializable type in scene/prefab struct (if designer-configurable)\n"
            "  4. schema/actions.rs         — new Action variants (if capability dispatches actions)\n"
            "  5. docs/ + CLAUDE.md         — crates/ironhold_core/src/CLAUDE.md capability notes\n"
            "For composite prefabs: wire capabilities in BOTH branches of scene_loader.rs (single-mesh AND composite)."
        )
except Exception:
    pass
