# Asset Catalog Checker

Verifies that every file path referenced in `assets.ron` catalogs exists on disk AND
matches the real on-disk case/separators — `Path.exists()` is case-insensitive on
Windows, so a mis-cased reference previously validated clean here while still 404ing
over the case-sensitive HTTP path a real WASM/browser build serves from. Also checked
by `ironhold_cli validate` (see `docs/60_contributing.md`'s "Checks performed" list) —
this tool remains useful as a fast, no-build, `assets.ron`-only spot-check.
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
- Each existing reference is also walked component-by-component against the real
  on-disk directory listing; a byte-exact match is preferred, falling back to a
  case-insensitive one only to report what the *real* casing is
- A backslash-separated reference (e.g. `"Scenes\\Main.scene.ron"`) is flagged too —
  Windows resolves it locally, but the web build serves assets over HTTP, which only
  understands `/`

**Narrower than `ironhold_cli validate`, in one respect:** this tool only scans `assets.ron`
files (regex over quoted strings ending in `ASSET_EXTS`, which does not include `.ktx2`), so
it cannot see `MaterialDef`'s texture/shader/splatmap paths inside `materials:` blocks that use
a different extension family (none currently do), a relocated `asset_catalog`, or
`ProjectConfig.global_environment`'s IBL paths / a scene's `terrain:` paths — `validate` checks
all of those too. Conversely, this tool reports the exact **line number** of a bad reference,
which `validate` structurally cannot (RON line/column info doesn't survive parsing into typed
structs). Reach for `validate` first; use this one for a quick `assets.ron`-only spot-check or
when line numbers matter.

## Orphan exclusions

Files skipped during orphan scanning (even if unreferenced):
- `*.json` — texture manifests
- `*.avif` — source preview images
- `*-preview.png` — PNG previews converted from AVIF
- `*.md` — documentation

## When to run

Run after: renaming or moving asset files, editing any `assets.ron`, or adding new
files to `assets/shared/` that should be catalogued. Exits with code 1 if any
missing references or case mismatches are found.
