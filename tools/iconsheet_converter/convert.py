"""
Iconsheet Converter — rebuilds a variable-grid icon atlas to a uniform 64-px grid.

Source format (same-source iconsheets):
  - RGB image (no alpha) with a solid background colour that is distinctly darker
    than the icon artwork.
  - Variable-sized cells: the grid lines are not pixel-perfect; cell boundaries are
    found via content-projection valleys.

Output:
  - RGBA PNG: 512 × 512 px, 8 × 8 grid, 64 × 64 px per cell.
  - White icons on transparent background.

Usage:
  python tools/iconsheet_converter/convert.py <source.png> <output.png>

  # Convert with custom thresholds
  python tools/iconsheet_converter/convert.py source.png iconsheet.png \
      --core-thresh 130 --alpha-low 105 --alpha-high 160 --margin 10

Algorithm (see CLAUDE.md for the design rationale):
  1. Project content pixels (luma > 100) onto X and Y axes.
  2. Find valley midpoints to detect the N−1 dividing lines between columns/rows.
  3. Pass 1  — for each cell, detect icon core (luma > CORE_THRESH) and record the
     bounding-box centre and dimensions.
  4. Uniform crop size — p75 of core bounding-box max-dims + 2 × MARGIN.
  5. Build full-image RGBA — smooth alpha ramp (ALPHA_LOW → ALPHA_HIGH) so
     background pixels fade to transparent; icon centres are fully opaque white.
  6. Pass 2  — crop a square of size CROP_SIZE centred on each icon core from the
     RGBA image, fill any out-of-bounds region with transparent white, then LANCZOS-
     resize to CELL_OUT × CELL_OUT and paste into the output grid.
"""

import sys
import argparse
import numpy as np
from PIL import Image


# ── Defaults ─────────────────────────────────────────────────────────────────

COLS       = 8
ROWS       = 8
CELL_OUT   = 64          # pixels per cell in the output grid

# Alpha extraction thresholds
CORE_THRESH  = 130       # luma threshold to classify a pixel as "icon core"
ALPHA_LOW    = 105       # luma at which alpha starts rising from 0
ALPHA_HIGH   = 160       # luma at which alpha is fully 255
MARGIN       = 10        # extra pixels added on each side of p75 core bbox


# ── Valley detection ──────────────────────────────────────────────────────────

def detect_bounds(profile: np.ndarray, n: int) -> list[tuple[int, int]]:
    """Return n (start, end) slice pairs from a 1-D content projection.

    Finds valleys in *profile* (pixels below 5 % of its max) and picks the
    N−1 deepest midpoints to divide the strip into N segments.
    """
    thresh = float(profile.max()) * 0.05
    low = profile < thresh

    valleys: list[int] = []
    in_valley = False
    v_start = 0
    for i, is_low in enumerate(low):
        if is_low and not in_valley:
            in_valley = True
            v_start = i
        elif not is_low and in_valley:
            in_valley = False
            valleys.append((v_start + i) // 2)

    size = len(profile)
    core_valleys = sorted(
        [v for v in valleys if size * 0.03 < v < size * 0.97]
    )[:n - 1]

    cuts = [0] + core_valleys + [size]
    return [(cuts[i], cuts[i + 1]) for i in range(len(cuts) - 1)]


# ── Core-bbox detection ───────────────────────────────────────────────────────

def core_bbox(cell: np.ndarray, thresh: int) -> tuple[int, int, int, int] | None:
    """Return (cx, cy, w, h) of the icon core pixels in *cell*, or None if empty."""
    luma = cell.astype(np.float32)
    mask = luma > thresh
    ys, xs = np.where(mask)
    if len(xs) == 0:
        return None
    x0, x1 = int(xs.min()), int(xs.max())
    y0, y1 = int(ys.min()), int(ys.max())
    cx = (x0 + x1) // 2
    cy = (y0 + y1) // 2
    return cx, cy, x1 - x0 + 1, y1 - y0 + 1


# ── Main conversion ───────────────────────────────────────────────────────────

def convert(src_path: str, out_path: str,
            core_thresh: int = CORE_THRESH,
            alpha_low: int   = ALPHA_LOW,
            alpha_high: int  = ALPHA_HIGH,
            margin: int      = MARGIN) -> None:

    src_img = Image.open(src_path).convert("RGB")
    src_arr = np.array(src_img, dtype=np.float32)
    luma_full = 0.299 * src_arr[:, :, 0] + 0.587 * src_arr[:, :, 1] + 0.114 * src_arr[:, :, 2]

    h, w = luma_full.shape

    # ── Step 1: detect cell boundaries ───────────────────────────────────────
    content_mask = (luma_full > 100).astype(np.float32)
    x_profile = content_mask.sum(axis=0)
    y_profile = content_mask.sum(axis=1)

    col_bounds = detect_bounds(x_profile, COLS)
    row_bounds = detect_bounds(y_profile, ROWS)

    # ── Step 2: Pass 1 — find icon core centre + size per cell ───────────────
    core_info: list[list[tuple | None]] = []
    all_maxdims: list[int] = []

    for r, (ry0, ry1) in enumerate(row_bounds):
        row_data = []
        for c, (cx0, cx1) in enumerate(col_bounds):
            cell_luma = luma_full[ry0:ry1, cx0:cx1]
            bbox = core_bbox(cell_luma, core_thresh)
            if bbox:
                local_cx, local_cy, bw, bh = bbox
                # Absolute centre in full image
                abs_cx = cx0 + local_cx
                abs_cy = ry0 + local_cy
                all_maxdims.append(max(bw, bh))
                row_data.append((abs_cx, abs_cy))
            else:
                row_data.append(None)
        core_info.append(row_data)

    # ── Step 3: uniform crop size (p75 of core maxdims + margin) ─────────────
    if all_maxdims:
        p75 = int(np.percentile(all_maxdims, 75))
    else:
        p75 = CELL_OUT
    half = p75 // 2 + margin
    crop_size = half * 2

    print(f"[convert] {COLS}x{ROWS} grid detected; core p75={p75}px -> crop={crop_size}px -> {CELL_OUT}px cells")

    # ── Step 4: build full-image RGBA (smooth alpha ramp) ────────────────────
    alpha_f = np.clip(
        (luma_full - alpha_low) * (255.0 / max(alpha_high - alpha_low, 1)),
        0.0, 255.0
    )
    rgba_arr = np.zeros((h, w, 4), dtype=np.uint8)
    rgba_arr[:, :, 0] = 255  # R = white
    rgba_arr[:, :, 1] = 255  # G = white
    rgba_arr[:, :, 2] = 255  # B = white
    rgba_arr[:, :, 3] = alpha_f.astype(np.uint8)
    rgba_img = Image.fromarray(rgba_arr, "RGBA")

    # ── Step 5: Pass 2 — crop → resize → paste into output grid ──────────────
    out_size = COLS * CELL_OUT
    out_img  = Image.new("RGBA", (out_size, out_size), (255, 255, 255, 0))

    for r in range(ROWS):
        for c in range(COLS):
            info = core_info[r][c]
            if info is None:
                # Empty cell: use cell centre from boundary bounds
                ry0, ry1 = row_bounds[r]
                cx0, cx1 = col_bounds[c]
                icx = (cx0 + cx1) // 2
                icy = (ry0 + ry1) // 2
            else:
                icx, icy = info

            # Crop centred on icon core (may go out-of-bounds — pad with transparent white)
            left   = icx - half
            top    = icy - half
            right  = left + crop_size
            bottom = top  + crop_size

            # Build the crop region with transparent-white padding for OOB
            crop_canvas = Image.new("RGBA", (crop_size, crop_size), (255, 255, 255, 0))
            # Clamped source rect
            src_x0 = max(left, 0)
            src_y0 = max(top,  0)
            src_x1 = min(right,  w)
            src_y1 = min(bottom, h)
            if src_x0 < src_x1 and src_y0 < src_y1:
                patch = rgba_img.crop((src_x0, src_y0, src_x1, src_y1))
                paste_x = src_x0 - left
                paste_y = src_y0 - top
                crop_canvas.paste(patch, (paste_x, paste_y))

            icon = crop_canvas.resize((CELL_OUT, CELL_OUT), Image.LANCZOS)
            out_img.paste(icon, (c * CELL_OUT, r * CELL_OUT))

    out_img.save(out_path)
    print(f"[convert] Saved {out_path}  ({out_size}×{out_size} RGBA)")


# ── CLI ───────────────────────────────────────────────────────────────────────

def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("source",  help="Source PNG (variable-grid, RGB, solid background)")
    parser.add_argument("output",  help="Output PNG path")
    parser.add_argument("--core-thresh",  type=int, default=CORE_THRESH,  help=f"Luma threshold for icon core detection (default {CORE_THRESH})")
    parser.add_argument("--alpha-low",    type=int, default=ALPHA_LOW,    help=f"Luma at which alpha starts rising (default {ALPHA_LOW})")
    parser.add_argument("--alpha-high",   type=int, default=ALPHA_HIGH,   help=f"Luma at which alpha reaches 255 (default {ALPHA_HIGH})")
    parser.add_argument("--margin",       type=int, default=MARGIN,       help=f"Extra pixels on each side of p75 bbox (default {MARGIN})")
    args = parser.parse_args()

    convert(
        args.source, args.output,
        core_thresh=args.core_thresh,
        alpha_low=args.alpha_low,
        alpha_high=args.alpha_high,
        margin=args.margin,
    )


if __name__ == "__main__":
    main()
