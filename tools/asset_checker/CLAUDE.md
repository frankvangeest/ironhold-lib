# Asset Catalog Checker

Verifies that every file path referenced in `assets.ron` catalogs exists on disk.
Optionally reports unreferenced files in `assets/shared/`.

No extra dependencies — stdlib only. Run from the repo root.

## Usage

```bash
# Check all projects for broken references
python tools/asset_checker/check.py

# Also report orphaned files in assets/shared/
python tools/asset_checker/check.py --orphans

# Check a single project
python tools/asset_checker/check.py --project custom_materials

# Verbose: show every checked path
python tools/asset_checker/check.py --verbose
```

## What it checks

- Every quoted string in `assets.ron` files that ends in a known asset extension
  (`.glb`, `.gltf`, `.png`, `.jpg`, `.jpeg`, `.webp`, `.hdr`, `.wav`, `.ogg`, `.mp3`, `.wgsl`)
- Paths are resolved relative to the `assets/` directory (Bevy asset root)
- `#Fragment` suffixes (e.g. `#Scene0`) are stripped before resolving

## Orphan exclusions

Files skipped during orphan scanning (even if unreferenced):
- `*.json` — texture manifests
- `*.avif` — source preview images
- `*-preview.png` — PNG previews converted from AVIF
- `*.md` — documentation

## When to run

Run after: renaming or moving asset files, editing any `assets.ron`, or adding new
files to `assets/shared/` that should be catalogued. Exits with code 1 if any
missing references are found.
