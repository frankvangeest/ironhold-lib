# Iconsheet Converter

Converts variable-cell-size icon atlas images (RGB, solid background) to a uniform
8×8 RGBA grid at 512×512 px (64 px per cell), ready for use as a `InventoryPanel`
`icon_sheet` in Ironhold.

## When to use

Whenever a new iconsheet arrives in the same format as `iconsheet-items-01-backup.png`:
- RGB image (no transparency)
- Icons on a solid, uniformly dark background
- Approximately 8×8 icons but with non-uniform cell widths/heights

## Quick start

```bash
# Always run from repo root
python tools/iconsheet_converter/convert.py \
    assets/shared/ui/iconsheet-items-01-backup.png \
    assets/shared/ui/iconsheet-items-01.png
```

## Arguments

| Argument | Default | Meaning |
|---|---|---|
| `source` | — | Source PNG (RGB, variable grid) |
| `output` | — | Output PNG path |
| `--core-thresh` | 130 | Luma threshold for detecting icon pixels (vs background) |
| `--alpha-low` | 105 | Luma at which alpha begins rising from 0 |
| `--alpha-high` | 160 | Luma at which alpha reaches 255 |
| `--margin` | 10 | Extra pixels on each side of the p75 bounding box |

## Algorithm notes

1. **Boundary detection** — projects content pixels (luma > 100) onto X and Y axes,
   then finds valley midpoints between columns/rows. Works with variable cell widths.

2. **Pass 1 (core detection)** — for each cell, detects pixels above `--core-thresh`
   to find the icon's bounding box centre and dimensions.

3. **Uniform crop** — uses the 75th-percentile of core bounding-box max-dimensions
   plus 2× `--margin` as the uniform crop size. Using p75 (not max) avoids one
   outlier inflating the crop and making all icons appear small.

4. **Alpha-before-resize** — converts to RGBA *before* LANCZOS resize:
   - Background pixels below `alpha-low` → fully transparent
   - Smooth ramp from `alpha-low` to `alpha-high`
   - Icon centres above `alpha-high` → fully opaque
   - RGB is always white throughout
   
   This ensures LANCZOS only varies the alpha channel; mixing white × transparent
   never produces grey halos (as it would if alpha were applied after resizing
   a dark-background image).

5. **OOB crop handling** — when the crop square extends outside the source image
   (icons near edges), the out-of-bounds region is filled with transparent white.

## Tuning thresholds

If the output has **checkerboard background showing through**, raise `--alpha-low`
(currently 105). This lifts the point at which background pixels become transparent.

If icons have **transparent holes or edges that are too clipped**, lower `--core-thresh`
(currently 130) to include more border pixels, or lower `--alpha-low` to let
more fringe pixels have partial alpha.

If icons appear **consistently too small** compared to source, lower `--margin`
(currently 10) or check whether `--core-thresh` is too high (small detected bbox).

If icons appear **blurry or the scale varies**, check that the source has no stray
high-luma pixels far outside the icon centre that inflate the bounding box.

## Dependencies

- `Pillow` (PIL) — `pip install Pillow`
- `numpy` — `pip install numpy`

Both are already available in the standard project Python environment.
