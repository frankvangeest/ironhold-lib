import sys
import json
import subprocess
from pathlib import Path

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
    glbs = [f for f in staged if f.endswith(".glb")]

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
