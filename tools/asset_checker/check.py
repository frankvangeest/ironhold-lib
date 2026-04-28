#!/usr/bin/env python3
"""
Asset catalog integrity checker.

Reads every assets.ron in assets/projects/ and verifies that every file path
it references actually exists on disk (relative to the assets/ root).

Optionally reports unreferenced (orphaned) files inside assets/shared/.
The terminal shows a grouped summary; the full file list is written to
logs/asset-checker/orphans.log.

Usage:
    # Check all projects for missing file references
    python tools/asset_checker/check.py

    # Also report orphaned files in assets/shared/
    python tools/asset_checker/check.py --orphans

    # Check a single project only
    python tools/asset_checker/check.py --project custom_materials

    # Show every checked path, not just problems
    python tools/asset_checker/check.py --verbose
"""

import argparse
import re
import sys
from collections import defaultdict
from pathlib import Path

# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------

ASSET_ROOT = Path("assets")
PROJECTS_DIR = ASSET_ROOT / "projects"
LOG_DIR = Path("logs/asset-checker")
ORPHAN_LOG = LOG_DIR / "orphans.log"

ASSET_EXTS = frozenset(
    ["glb", "gltf", "png", "jpg", "jpeg", "webp", "hdr", "wav", "ogg", "mp3", "wgsl"]
)

ORPHAN_IGNORE_SUFFIXES = frozenset([".json", ".avif", ".md"])
ORPHAN_IGNORE_STEMS_SUFFIX = "-preview"

# ---------------------------------------------------------------------------
# Path extraction
# ---------------------------------------------------------------------------

_EXT_PATTERN = "|".join(ASSET_EXTS)
_PATH_RE = re.compile(
    r'"([^"]*\.(?:' + _EXT_PATTERN + r'))(?:#[^"]*)?\"',
    re.IGNORECASE,
)


def extract_refs(ron_path: Path) -> list[tuple[int, str]]:
    refs = []
    for lineno, line in enumerate(ron_path.read_text(encoding="utf-8").splitlines(), start=1):
        if line.lstrip().startswith("//"):
            continue
        for m in _PATH_RE.finditer(line):
            refs.append((lineno, m.group(1)))
    return refs


# ---------------------------------------------------------------------------
# Checking
# ---------------------------------------------------------------------------

def check_project(assets_ron: Path, assets_root: Path) -> tuple[list, list, set]:
    missing, ok, referenced = [], [], set()
    for lineno, ref in extract_refs(assets_ron):
        resolved = assets_root / ref
        if resolved.exists():
            ok.append((assets_ron, lineno, ref))
            referenced.add(resolved.resolve())
        else:
            missing.append((assets_ron, lineno, ref))
    return missing, ok, referenced


# ---------------------------------------------------------------------------
# Orphan detection
# ---------------------------------------------------------------------------

def collect_shared_files(shared_dir: Path) -> list[Path]:
    results = []
    for p in sorted(shared_dir.rglob("*")):
        if not p.is_file():
            continue
        if p.suffix.lower() in ORPHAN_IGNORE_SUFFIXES:
            continue
        if p.stem.endswith(ORPHAN_IGNORE_STEMS_SUFFIX):
            continue
        if p.suffix.lstrip(".").lower() not in ASSET_EXTS:
            continue
        results.append(p)
    return results


def group_by_subdir(files: list[Path], relative_to: Path) -> dict[str, list[Path]]:
    """Group files by their immediate parent directory relative to `relative_to`."""
    groups: dict[str, list[Path]] = defaultdict(list)
    for f in files:
        try:
            rel = f.relative_to(relative_to)
        except ValueError:
            rel = f
        key = str(rel.parent)
        groups[key].append(f)
    return dict(sorted(groups.items()))


def write_orphan_log(orphans: list[Path], shared_dir: Path) -> None:
    LOG_DIR.mkdir(parents=True, exist_ok=True)
    lines = [f"Asset orphan report: {len(orphans)} unreferenced files in {shared_dir}\n"]
    groups = group_by_subdir(orphans, shared_dir)
    for subdir, files in groups.items():
        lines.append(f"\n  {subdir}/  ({len(files)} files)")
        for f in files:
            lines.append(f"    {f.name}")
    ORPHAN_LOG.write_text("\n".join(lines) + "\n", encoding="utf-8")


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------

def main() -> None:
    p = argparse.ArgumentParser(
        description="Check that every asset path in assets.ron files exists on disk.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__,
    )
    p.add_argument("--project", metavar="NAME", default=None,
                   help="Check only this project (folder name under assets/projects/)")
    p.add_argument("--orphans", action="store_true",
                   help="Report files in assets/shared/ not referenced by any catalog")
    p.add_argument("--verbose", action="store_true",
                   help="Print every checked path, not only problems")
    args = p.parse_args()

    if not ASSET_ROOT.is_dir():
        print(f"Error: assets root '{ASSET_ROOT}' not found. Run from the repo root.", file=sys.stderr)
        sys.exit(1)

    if args.project:
        candidates = [PROJECTS_DIR / args.project / "assets.ron"]
        if not candidates[0].exists():
            print(f"Error: {candidates[0]} not found.", file=sys.stderr)
            sys.exit(1)
    else:
        candidates = sorted(PROJECTS_DIR.rglob("assets.ron"))

    if not candidates:
        print("No assets.ron files found.")
        sys.exit(0)

    all_missing: list = []
    all_ok: list = []
    all_referenced: set = set()

    for catalog in candidates:
        missing, ok, referenced = check_project(catalog, ASSET_ROOT)
        all_missing.extend(missing)
        all_ok.extend(ok)
        all_referenced.update(referenced)

    # ---------------------------------------------------------------------------
    # Report: missing references
    # ---------------------------------------------------------------------------
    if args.verbose:
        for (catalog, lineno, ref) in all_ok:
            print(f"  ok    {ref}  ({catalog}:{lineno})")

    if all_missing:
        print(f"\nMISSING ({len(all_missing)} references to non-existent files):")
        for (catalog, lineno, ref) in all_missing:
            print(f"  MISS  {ref}")
            print(f"        {catalog}:{lineno}")
    else:
        print(f"References: {len(all_ok)} checked, 0 missing. All good.")

    # ---------------------------------------------------------------------------
    # Report: orphaned files
    # ---------------------------------------------------------------------------
    if args.orphans:
        shared_dir = ASSET_ROOT / "shared"
        if not shared_dir.is_dir():
            print("\nNo assets/shared/ directory found — skipping orphan check.")
        else:
            shared_files = collect_shared_files(shared_dir)
            orphans = [f for f in shared_files if f.resolve() not in all_referenced]

            if orphans:
                groups = group_by_subdir(orphans, shared_dir)
                total = sum(len(v) for v in groups.values())
                print(f"\nORPHANS: {total} unreferenced files in assets/shared/ (by subdirectory):")
                for subdir, files in groups.items():
                    print(f"  {subdir}/  ({len(files)} file{'s' if len(files) != 1 else ''})")
                write_orphan_log(orphans, shared_dir)
                print(f"\n  Full list written to: {ORPHAN_LOG}")
            else:
                print(f"\nOrphans: 0 — every file in assets/shared/ is referenced.")

    if all_missing:
        sys.exit(1)


if __name__ == "__main__":
    main()
