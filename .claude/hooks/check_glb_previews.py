import json
import struct
import sys
from pathlib import Path
import subprocess


def _glb_has_meshes(path: str) -> bool:
    """Return False for animation-only GLBs (no mesh objects). Reads only the JSON chunk."""
    try:
        with open(path, "rb") as f:
            if f.read(4) != b"glTF":
                return True
            f.read(8)  # version + total length
            chunk_len = struct.unpack("<I", f.read(4))[0]
            if f.read(4) != b"JSON":
                return True
            data = json.loads(f.read(chunk_len))
            return len(data.get("meshes") or []) > 0
    except Exception:
        return True  # safe default: assume renderable


try:
    data = json.load(sys.stdin)
    cmd = data.get("tool_input", {}).get("command", "")

    if "git commit" not in cmd:
        sys.exit(0)

    result = subprocess.run(
        ["git", "diff", "--cached", "--name-only"],
        capture_output=True, text=True
    )
    staged = result.stdout.strip().splitlines()
    # Only check GLBs that actually contain mesh geometry — animation-only GLBs
    # have no renderable content so no AVIF preview is required for them.
    glbs = [f for f in staged if f.endswith(".glb") and _glb_has_meshes(f)]

    if not glbs:
        sys.exit(0)

    missing = []
    for glb in glbs:
        avif = Path(glb).parent / f"{Path(glb).stem}-preview.avif"
        if not avif.exists():
            missing.append(glb)

    if missing:
        paths = "\n".join(f"  {g}" for g in missing)
        glb_args = " ".join(missing)
        print(
            f"BLOCKED: {len(missing)} staged GLB(s) missing an AVIF preview:\n{paths}\n\n"
            f"Generate previews, then verify none are blank:\n"
            f"  python tools/glb_preview/preview.py {glb_args} --avif-only\n"
            f"  python tools/glb_preview/preview.py assets/shared/models/ --check\n\n"
            f"Stage the new .avif files and re-commit."
        )
        sys.exit(1)

except Exception:
    pass
