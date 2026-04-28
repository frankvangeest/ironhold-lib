#!/usr/bin/env python3
"""
AVIF to PNG batch converter.

Usage:
    # Convert a single file (output next to source)
    python tools/avif2png/convert.py path/to/image.avif

    # Convert all AVIFs in a directory
    python tools/avif2png/convert.py assets/shared/models/avif/

    # Convert with explicit output directory
    python tools/avif2png/convert.py assets/shared/models/avif/ --output-dir assets/shared/models/

    # Force overwrite of existing PNGs
    python tools/avif2png/convert.py assets/shared/models/avif/ --force

    # Resize output (longest side, preserving aspect ratio)
    python tools/avif2png/convert.py assets/shared/models/avif/ --max-size 256
"""

import argparse
import sys
from pathlib import Path

from PIL import Image


def _collect_inputs(sources: list[str]) -> list[Path]:
    paths = []
    for src in sources:
        p = Path(src)
        if p.is_dir():
            paths.extend(sorted(p.glob("*.avif")))
        elif p.suffix.lower() == ".avif" and p.is_file():
            paths.append(p)
        else:
            # Treat as a glob pattern
            matched = sorted(Path(".").glob(src))
            matched = [m for m in matched if m.suffix.lower() == ".avif"]
            if not matched:
                print(f"Warning: no AVIF files matched '{src}'", file=sys.stderr)
            paths.extend(matched)
    return paths


def _convert(src: Path, dest: Path, max_size: int | None) -> None:
    img = Image.open(src)
    if img.mode not in ("RGB", "RGBA"):
        img = img.convert("RGBA" if img.mode in ("LA", "PA") else "RGB")
    if max_size is not None:
        img.thumbnail((max_size, max_size), Image.LANCZOS)
    dest.parent.mkdir(parents=True, exist_ok=True)
    img.save(dest, format="PNG")


def main() -> None:
    p = argparse.ArgumentParser(
        description="Convert AVIF files to PNG.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__,
    )
    p.add_argument("sources", nargs="+", metavar="SOURCE",
                   help="AVIF file(s), directory, or glob pattern")
    p.add_argument("--output-dir", metavar="DIR", default=None,
                   help="Write PNGs here instead of next to each source file")
    p.add_argument("--max-size", type=int, default=None, metavar="PX",
                   help="Resize so the longest side is at most PX (aspect ratio preserved)")
    p.add_argument("--force", action="store_true",
                   help="Overwrite existing PNG files (default: skip)")
    args = p.parse_args()

    inputs = _collect_inputs(args.sources)
    if not inputs:
        print("No AVIF files found.", file=sys.stderr)
        sys.exit(1)

    output_dir = Path(args.output_dir) if args.output_dir else None
    skipped = converted = 0

    for src in inputs:
        dest_dir = output_dir if output_dir is not None else src.parent
        dest = dest_dir / (src.stem + ".png")
        if dest.exists() and not args.force:
            print(f"  skip  {dest}  (exists; use --force to overwrite)")
            skipped += 1
            continue
        _convert(src, dest, args.max_size)
        print(f"  ->    {dest}")
        converted += 1

    print(f"\nDone: {converted} converted, {skipped} skipped.")


if __name__ == "__main__":
    main()
