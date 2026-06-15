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

    # Check for blank/failed previews (no Blender needed)
    python tools/glb_preview/preview.py assets/shared/models/props/ --check
    python tools/glb_preview/preview.py assets/shared/models/ --check

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


def _collect_previews(sources: list[str]) -> list[Path]:
    """Find all *-preview.avif (falling back to *-preview.png) files in the given sources."""
    paths = []
    for src in sources:
        p = Path(src)
        if p.is_dir():
            avifs = sorted(p.rglob("*-preview.avif"))
            pngs  = sorted(p.rglob("*-preview.png"))
            # Include PNGs that have no matching AVIF (so we catch both formats)
            avif_stems = {a.stem for a in avifs}
            extra_pngs = [p2 for p2 in pngs if p2.stem not in avif_stems]
            paths.extend(avifs + extra_pngs)
        elif p.is_file() and p.suffix.lower() in (".avif", ".png"):
            paths.append(p)
        elif p.suffix.lower() == ".glb" and p.is_file():
            # Accept a GLB path — check its preview siblings
            for ext in (".avif", ".png"):
                candidate = p.parent / f"{p.stem}-preview{ext}"
                if candidate.exists():
                    paths.append(candidate)
                    break
    return paths


# Files under this size are definitely empty AVIF/PNG headers with no content.
_EMPTY_BYTES = 500
# Renders with fewer visible (non-transparent) pixels than this are blank.
# Thin/narrow models on a transparent background can compress to <3 KB and still be valid.
_MIN_VISIBLE_PIXELS = 150


def _check_preview(path: Path) -> tuple[bool, str]:
    """Return (is_blank, reason). Empty string reason means image is OK."""
    if path.stat().st_size < _EMPTY_BYTES:
        return True, f"empty file ({path.stat().st_size} B)"
    try:
        from PIL import Image
        img = Image.open(path).convert("RGBA")
        data = img.getdata()
        visible = sum(1 for _r, _g, _b, a in data if a > 10)
        if visible < _MIN_VISIBLE_PIXELS:
            return True, f"only {visible} visible pixels"
    except Exception as e:
        return True, f"unreadable ({e})"
    return False, ""


def _run_check(sources: list[str]) -> int:
    """Check all preview images for blank/failed renders. Returns exit code."""
    previews = _collect_previews(sources)
    if not previews:
        print("No preview files found.", file=sys.stderr)
        return 1

    blank: list[Path] = []
    ok = 0
    for path in previews:
        is_blank, reason = _check_preview(path)
        rel = path.relative_to(Path.cwd()) if path.is_relative_to(Path.cwd()) else path
        if is_blank:
            print(f"  BLANK  {rel}  ({reason})")
            blank.append(path)
        else:
            ok += 1

    print(f"\nChecked {len(previews)}: {ok} ok, {len(blank)} blank.")

    if blank:
        # Derive the matching GLB for each blank preview so we can print the fix command.
        glbs = []
        for p in blank:
            # stem is e.g. "anvil-preview" → glb stem is "anvil"
            glb_stem = p.stem.removesuffix("-preview")
            glb = p.parent / f"{glb_stem}.glb"
            if glb.exists():
                rel = glb.relative_to(Path.cwd()) if glb.is_relative_to(Path.cwd()) else glb
                glbs.append(str(rel))
            else:
                print(f"  Warning: no GLB found for {p.name}", file=sys.stderr)

        if glbs:
            # Use forward slashes so the command is paste-safe in both Bash and PowerShell.
            glbs_fwd = [g.replace("\\", "/") for g in glbs]
            args_str = " ".join(f'"{g}"' if " " in g else g for g in glbs_fwd)
            print(f"\nTo regenerate:\n  python tools/glb_preview/preview.py {args_str} --avif-only --force")
        return 1

    return 0


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
    p.add_argument("--check", action="store_true",
                   help="Scan for blank/failed preview images and print a fix command (no Blender needed)")
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

    if args.check:
        sys.exit(_run_check(args.sources))
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
