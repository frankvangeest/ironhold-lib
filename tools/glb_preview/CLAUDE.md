# GLB Preview Generator

Renders a 3/4-view 512×512 PNG preview for each `.glb` file using Blender headless
(EEVEE, transparent background, three-point lighting). Output is placed next to the
source GLB as `{model-name}-preview.png`.

No pip dependencies — uses Blender's built-in Python. Blender 4.x required.

## Blender path

Resolved in this order:
1. `--blender` CLI flag
2. `BLENDER_EXE` environment variable
3. `tools/glb_preview/blender_path.txt` (current machine path, not committed to git)

`blender_path.txt` is gitignored — each developer sets their own local path.

## Usage

```bash
# Single model — PNG only
python tools/glb_preview/preview.py assets/shared/models/props/anvil.glb

# Entire folder — PNG only (skips models that already have a preview)
python tools/glb_preview/preview.py assets/shared/models/props/

# PNG + AVIF (both written next to the GLB)
python tools/glb_preview/preview.py assets/shared/models/props/ --avif

# AVIF only — PNG rendered as intermediate then deleted (recommended for shared/)
python tools/glb_preview/preview.py assets/shared/models/props/ --avif-only

# Recursive — all GLBs under models/
python tools/glb_preview/preview.py assets/shared/models/

# Force regenerate all previews
python tools/glb_preview/preview.py assets/shared/models/props/ --force

# Custom size (default 512) or AVIF quality (default 80)
python tools/glb_preview/preview.py model.glb --size 256 --avif --avif-quality 90
```

When `--avif` is used, Blender renders the PNG first, then Pillow converts it to AVIF.
Both files are kept — the AVIF as the compact committed source, the PNG for AI visibility.

## Output convention

Previews are named `{model-stem}-preview.png` and live next to their GLB:

```
props/
  anvil.glb
  anvil-preview.png       ← generated here
```

## Performance

Each model takes ~5–15 seconds depending on complexity. For large batches, expect
a few minutes. Blender startup overhead is ~2s per model.
