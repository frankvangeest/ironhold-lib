# Models

GLB assets used across all projects. Before authoring or editing RON files that
reference a model, run the inspector to get exact node names, material names,
and animation clip names:

```bash
python tools/glb_inspector/inspect_glb.py assets/shared/models/<file>.glb
```

## What to look for

| RON field | Inspector section |
|---|---|
| `mesh_node_name` / node refs in scene | **Nodes** — `[mesh]` entries |
| `clip_name` in AnimationPolicy | **Animations** — name column |
| Material overrides | **Materials** — name column |

Never guess node or animation names — they must match the GLB exactly.

## Preview images

Each model has a `{model-name}-preview.png` and optionally a `{model-name}-preview.avif`
living next to its GLB. The AVIF is the committed compact source; the PNG is for AI
visibility and may be gitignored for models where an AVIF already exists.

**Generate previews for models that have none** (uses Blender headless):

```bash
# PNG only
python tools/glb_preview/preview.py assets/shared/models/props/

# PNG + AVIF (recommended for new models going into shared/)
python tools/glb_preview/preview.py assets/shared/models/props/ --avif
```

**After generating, always verify no previews are blank** (no Blender needed):

```bash
python tools/glb_preview/preview.py assets/shared/models/ --check
```

If any are listed as blank, the output prints the exact `--force` command to regenerate them. Common causes: model has no embedded material (fallback grey is applied automatically), model is extremely small-scale (clip plane issue — fixed in the script), or rigged character with armature inflating the bounding box (also fixed). Re-run `--check` after regenerating to confirm.

**Convert existing AVIF previews to PNG** (when PNG is missing or gitignored):

```bash
python tools/avif2png/convert.py assets/shared/models/props/
```

Skips existing files automatically; use `--force` to regenerate.
