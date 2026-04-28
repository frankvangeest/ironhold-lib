#!/usr/bin/env python3
"""
GLB preview image generator.

Renders a 3/4-view preview PNG for each .glb file using Blender headless.
Output is placed next to the source GLB as {stem}-preview.png.
Add --avif to also produce a {stem}-preview.avif alongside the PNG.

Usage:
    # Single file
    python tools/glb_preview/preview.py assets/shared/models/props/anvil.glb

    # Whole folder (recursive)
    python tools/glb_preview/preview.py assets/shared/models/props/

    # Also produce AVIF (PNG + AVIF, both written)
    python tools/glb_preview/preview.py assets/shared/models/props/ --avif

    # AVIF only — PNG is used as intermediate then deleted
    python tools/glb_preview/preview.py assets/shared/models/props/ --avif-only

    # Force overwrite existing previews
    python tools/glb_preview/preview.py assets/shared/models/props/ --force

    # Custom output directory
    python tools/glb_preview/preview.py assets/shared/models/props/ --output-dir /tmp/previews

    # Override Blender path (or set BLENDER_EXE env var)
    python tools/glb_preview/preview.py model.glb --blender "C:/path/to/blender.exe"

Blender path resolution order:
    1. --blender CLI flag
    2. BLENDER_EXE environment variable
    3. blender_path.txt in this directory
"""

import argparse
import os
import subprocess
import sys
from pathlib import Path

SCRIPT_DIR = Path(__file__).parent
BLENDER_SCRIPT = SCRIPT_DIR / "_blender_script.py"
BLENDER_PATH_FILE = SCRIPT_DIR / "blender_path.txt"

DEFAULT_SIZE = 512
DEFAULT_AVIF_QUALITY = 80


def _find_blender(cli_override: str | None) -> str:
    if cli_override:
        return cli_override
    if "BLENDER_EXE" in os.environ:
        return os.environ["BLENDER_EXE"]
    if BLENDER_PATH_FILE.exists():
        return BLENDER_PATH_FILE.read_text().strip()
    return "blender"  # hope it's on PATH


def _collect_glbs(sources: list[str]) -> list[Path]:
    paths = []
    for src in sources:
        p = Path(src)
        if p.is_dir():
            paths.extend(sorted(p.rglob("*.glb")))
        elif p.suffix.lower() == ".glb" and p.is_file():
            paths.append(p)
        else:
            matched = sorted(Path(".").glob(src))
            matched = [m for m in matched if m.suffix.lower() == ".glb"]
            if not matched:
                print(f"Warning: no GLB files matched '{src}'", file=sys.stderr)
            paths.extend(matched)
    return paths


def _render_png(blender: str, glb: Path, dest: Path, size: int,
                light_strength: float = 1.0,
                camera_az: float = 45.0, camera_el: float = 30.0) -> bool:
    cmd = [
        blender,
        "--background",
        "--python", str(BLENDER_SCRIPT),
        "--",
        "--input", str(glb.resolve()),
        "--output", str(dest.resolve()),
        "--size", str(size),
        "--light-strength", str(light_strength),
        "--camera-az", str(camera_az),
        "--camera-el", str(camera_el),
    ]
    result = subprocess.run(cmd, capture_output=True, text=True)
    if result.returncode != 0:
        print(f"  ERROR rendering {glb.name}:")
        for line in result.stderr.splitlines():
            if "error" in line.lower() or "traceback" in line.lower():
                print(f"    {line}")
        return False
    return True


def _png_to_avif(png: Path, quality: int) -> bool:
    try:
        from PIL import Image
    except ImportError:
        print("  ERROR: Pillow is required for --avif. Install with: pip install Pillow>=11.0",
              file=sys.stderr)
        return False
    avif = png.with_suffix(".avif")
    img = Image.open(png)
    img.save(avif, format="AVIF", quality=quality)
    return True


def main():
    p = argparse.ArgumentParser(
        description="Render a preview PNG (and optionally AVIF) for each GLB using Blender headless.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__,
    )
    p.add_argument("sources", nargs="+", metavar="SOURCE",
                   help="GLB file(s), directory, or glob pattern")
    p.add_argument("--blender", metavar="EXE", default=None,
                   help="Path to blender executable (overrides BLENDER_EXE and blender_path.txt)")
    p.add_argument("--output-dir", metavar="DIR", default=None,
                   help="Write previews here instead of next to each GLB")
    p.add_argument("--size", type=int, default=DEFAULT_SIZE, metavar="PX",
                   help=f"Render resolution (default: {DEFAULT_SIZE})")
    p.add_argument("--avif", action="store_true",
                   help="Also convert each PNG to AVIF using Pillow (both files are kept)")
    p.add_argument("--avif-only", action="store_true",
                   help="Convert to AVIF and delete the intermediate PNG (AVIF only output)")
    p.add_argument("--avif-quality", type=int, default=DEFAULT_AVIF_QUALITY, metavar="Q",
                   help=f"AVIF encoding quality 0–100 (default: {DEFAULT_AVIF_QUALITY})")
    p.add_argument("--light-strength", type=float, default=0.3, metavar="F",
                   help="Light energy multiplier — use <1.0 to dim, >1.0 to brighten (default: 0.3)")
    p.add_argument("--camera-az", type=float, default=45.0, metavar="DEG",
                   help="Camera azimuth: horizontal rotation around the model in degrees (default: 45)")
    p.add_argument("--camera-el", type=float, default=30.0, metavar="DEG",
                   help="Camera elevation: angle above the horizon in degrees (default: 30)")
    p.add_argument("--force", action="store_true",
                   help="Overwrite existing preview files")
    args = p.parse_args()
    if args.avif_only:
        args.avif = True  # avif-only implies avif

    blender = _find_blender(args.blender)
    output_dir = Path(args.output_dir) if args.output_dir else None
    glbs = _collect_glbs(args.sources)

    if not glbs:
        print("No GLB files found.", file=sys.stderr)
        sys.exit(1)

    skipped = rendered = failed = 0

    for glb in glbs:
        dest_dir = output_dir if output_dir is not None else glb.parent
        png = dest_dir / f"{glb.stem}-preview.png"
        avif = dest_dir / f"{glb.stem}-preview.avif"

        # Skip if all requested outputs already exist
        want_avif = args.avif
        avif_only = args.avif_only
        png_exists = png.exists()
        avif_exists = avif.exists() if want_avif else True
        final_exists = (avif_exists if avif_only else png_exists) and avif_exists
        if final_exists and not args.force:
            rel = avif if avif_only else png
            rel = rel.relative_to(Path.cwd()) if rel.is_relative_to(Path.cwd()) else rel
            print(f"  skip  {rel}")
            skipped += 1
            continue

        dest_dir.mkdir(parents=True, exist_ok=True)
        print(f"  ...   {glb.name}", end="", flush=True)

        if not png_exists or args.force:
            ok = _render_png(blender, glb, png, args.size,
                             args.light_strength, args.camera_az, args.camera_el)
            if not ok:
                print()
                failed += 1
                continue

        if want_avif and (not avif_exists or args.force):
            if not _png_to_avif(png, args.avif_quality):
                print()
                failed += 1
                continue

        if avif_only and png.exists():
            png.unlink()

        out = avif if avif_only else png
        rel = out.relative_to(Path.cwd()) if out.is_relative_to(Path.cwd()) else out
        suffix = " + .avif" if (want_avif and not avif_only) else ""
        print(f"\r  ->    {rel}{suffix}")
        rendered += 1

    status = f"{rendered} rendered, {skipped} skipped"
    if failed:
        status += f", {failed} FAILED"
    print(f"\nDone: {status}.")
    if failed:
        sys.exit(1)


if __name__ == "__main__":
    main()
