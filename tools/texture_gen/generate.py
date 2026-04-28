#!/usr/bin/env python3
"""
Seamless procedural texture generator.

Usage:
    python generate.py --type perlin  [--size 512] [--res 8]   [--output path.png]
    python generate.py --type fbm     [--size 512] [--res 4] [--octaves 6] [--output path.png]
    python generate.py --type value   [--size 512] [--res 16]  [--output path.png]
    python generate.py --type voronoi [--size 512] [--cells 12] [--output path.png]
    python generate.py --type gabor   [--size 512] [--kernels 2000] [--freq 0.08]
                                      [--angle 45] [--sigma 12]   [--output path.png]
    python generate.py --type blue    [--size 64]  [--sigma 1.9]   [--output path.png]

All types produce a seamless (tileable) greyscale PNG.
Add --normal-map to convert the greyscale output to a tangent-space RGB normal map.
Omit --output to auto-name based on type and parameters.
Add --show to open the image after generation (requires a display).
"""

import argparse
import datetime
import json
import os
import sys
import math
from pathlib import Path

import numpy as np
from PIL import Image


# ---------------------------------------------------------------------------
# Generators
# ---------------------------------------------------------------------------

def _perlin_raw(size: int, res: int) -> np.ndarray:
    """Raw seamless Perlin noise as float, output range approximately [-1, 1]."""
    d = size / res
    grid = np.mgrid[0:size, 0:size] / d

    angles = 2 * np.pi * np.random.rand(res, res)
    gradients = np.stack((np.cos(angles), np.sin(angles)), axis=-1)

    def grad(i, j):
        return gradients[i % res, j % res]

    x0 = grid[0].astype(int)
    y0 = grid[1].astype(int)
    xf = grid[0] - x0
    yf = grid[1] - y0

    def fade(t):
        return 6*t**5 - 15*t**4 + 10*t**3

    n00 = np.sum(np.stack([xf,   yf  ], 0) * grad(x0,   y0  ).transpose(2,0,1), 0)
    n10 = np.sum(np.stack([xf-1, yf  ], 0) * grad(x0+1, y0  ).transpose(2,0,1), 0)
    n01 = np.sum(np.stack([xf,   yf-1], 0) * grad(x0,   y0+1).transpose(2,0,1), 0)
    n11 = np.sum(np.stack([xf-1, yf-1], 0) * grad(x0+1, y0+1).transpose(2,0,1), 0)

    u, v = fade(xf), fade(yf)
    return (n00 + u*(n10-n00)) + v*((n01 + u*(n11-n01)) - (n00 + u*(n10-n00)))


def gen_perlin(size: int, res: int) -> np.ndarray:
    """Seamless single-octave Perlin noise."""
    return _normalize(_perlin_raw(size, res))


def gen_fbm(size: int, res: int, octaves: int,
            persistence: float = 0.5, lacunarity: float = 2.0) -> np.ndarray:
    """Fractal Brownian Motion — stacked Perlin octaves for natural multi-scale detail."""
    result = np.zeros((size, size), dtype=float)
    amplitude = 1.0
    total_amplitude = 0.0
    cur_res = res
    for _ in range(octaves):
        result += amplitude * _perlin_raw(size, cur_res)
        total_amplitude += amplitude
        amplitude *= persistence
        cur_res = max(1, round(cur_res * lacunarity))
    return _normalize(result)


def gen_value(size: int, res: int) -> np.ndarray:
    """Seamless value noise via periodic bilinear interpolation."""
    grid = np.random.rand(res, res)
    x, y = np.meshgrid(
        np.linspace(0, res, size, endpoint=False),
        np.linspace(0, res, size, endpoint=False),
    )
    x0, y0 = x.astype(int), y.astype(int)
    x1, y1 = (x0+1) % res, (y0+1) % res
    xf, yf = x - x0, y - y0

    def fade(t):
        return t*t*t*(t*(t*6 - 15) + 10)

    u, v = fade(xf), fade(yf)
    v00, v10 = grid[y0, x0], grid[y0, x1]
    v01, v11 = grid[y1, x0], grid[y1, x1]
    noise = (v00 + u*(v10-v00)) + v*((v01 + u*(v11-v01)) - (v00 + u*(v10-v00)))
    return _normalize(noise)


def gen_voronoi(size: int, cells: int) -> np.ndarray:
    """Seamless Voronoi / Worley noise via tiled seed points."""
    cell_size = size / cells
    seeds = np.random.rand(cells, cells, 2)

    x, y = np.meshgrid(np.arange(size), np.arange(size))
    coords = np.stack([x, y], axis=-1).astype(float)
    min_d2 = np.full((size, size), float(size*size*4))

    for ti in range(-1, 2):
        for tj in range(-1, 2):
            shift = np.array([ti * size, tj * size], dtype=float)
            for cx in range(cells):
                for cy in range(cells):
                    p = (np.array([cx, cy]) + seeds[cy, cx]) * cell_size + shift
                    d2 = np.sum((coords - p) ** 2, axis=-1)
                    min_d2 = np.minimum(min_d2, d2)

    return _normalize(np.sqrt(min_d2))


def gen_gabor(size: int, kernels: int, freq: float, angle_deg: float, sigma: float) -> np.ndarray:
    """Seamless Gabor noise via randomly splatted oriented Gaussian–cosine kernels."""
    theta = math.radians(angle_deg)
    noise = np.zeros((size, size), dtype=float)

    k_size = int(sigma * 6)
    if k_size % 2 == 0:
        k_size += 1
    half_k = k_size // 2

    kx, ky = np.meshgrid(np.arange(k_size) - half_k, np.arange(k_size) - half_k)
    gaussian = np.exp(-(kx**2 + ky**2) / (2 * sigma**2))
    cosine = np.cos(2 * np.pi * freq * (kx * math.cos(theta) + ky * math.sin(theta)))
    stamp = gaussian * cosine

    print(f"Splatting {kernels} Gabor kernels…", flush=True)
    px_all = np.random.randint(0, size, kernels)
    py_all = np.random.randint(0, size, kernels)
    weights = np.random.uniform(-1, 1, kernels)

    for n in range(kernels):
        for iy in range(k_size):
            ty = (py_all[n] + iy - half_k) % size
            for ix in range(k_size):
                tx = (px_all[n] + ix - half_k) % size
                noise[ty, tx] += weights[n] * stamp[iy, ix]

    return _normalize(noise)


def gen_blue(size: int, sigma: float) -> np.ndarray:
    """Seamless blue noise via the Void-and-Cluster algorithm."""
    try:
        from scipy.ndimage import gaussian_filter
    except ImportError:
        print("scipy is required for blue noise. Install with: pip install scipy", file=sys.stderr)
        sys.exit(1)

    total = size * size
    points = np.zeros((size, size), dtype=bool)
    rank_map = np.zeros((size, size), dtype=int)

    iy, ix = np.random.randint(0, size, 2)
    points[iy, ix] = True

    print(f"Generating {size}×{size} blue noise (slow for large sizes)…", flush=True)
    milestone = max(1, total // 10)

    for i in range(total):
        density = gaussian_filter(points.astype(float), sigma=sigma, mode='wrap')
        masked = np.where(points, np.inf, density)
        idx = np.argmin(masked)
        ry, rx = np.unravel_index(idx, (size, size))
        points[ry, rx] = True
        rank_map[ry, rx] = i
        if i % milestone == 0:
            print(f"  {i * 100 // total}%", flush=True)

    return (rank_map / (total - 1) * 255).astype(np.uint8)


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def _normalize(arr: np.ndarray) -> np.ndarray:
    lo, hi = arr.min(), arr.max()
    if hi == lo:
        return np.zeros_like(arr, dtype=np.uint8)
    return ((arr - lo) / (hi - lo) * 255).astype(np.uint8)


def _to_normal_map(height: np.ndarray, strength: float = 1.0) -> np.ndarray:
    """Convert an 8-bit greyscale height map to a tangent-space RGB normal map (OpenGL convention)."""
    h = height.astype(float) / 255.0
    # Finite-difference gradient with torus wrapping keeps the map seamless
    dx = (np.roll(h, -1, axis=1) - np.roll(h, 1, axis=1)) * strength
    dy = (np.roll(h, -1, axis=0) - np.roll(h, 1, axis=0)) * strength
    nx, ny, nz = -dx, -dy, np.ones_like(dx)
    length = np.sqrt(nx**2 + ny**2 + nz**2)
    nx, ny, nz = nx / length, ny / length, nz / length
    r = ((nx + 1.0) * 0.5 * 255).astype(np.uint8)
    g = ((ny + 1.0) * 0.5 * 255).astype(np.uint8)
    b = ((nz + 1.0) * 0.5 * 255).astype(np.uint8)
    return np.stack([r, g, b], axis=-1)


def _auto_name(args) -> str:
    t = args.type
    s = args.size
    nm = "-nm" if getattr(args, "normal_map", False) else ""
    if t == "perlin":
        return f"noise-perlin-{s}-r{args.res}{nm}.png"
    if t == "fbm":
        return f"noise-fbm-{s}-r{args.res}-o{args.octaves}{nm}.png"
    if t == "value":
        return f"noise-value-{s}-r{args.res}{nm}.png"
    if t == "voronoi":
        return f"noise-voronoi-{s}-c{args.cells}{nm}.png"
    if t == "gabor":
        freq_str = f"{args.freq:.2f}".replace(".", "")
        return f"noise-gabor-{s}-f{freq_str}-a{round(args.angle)}-s{round(args.sigma)}{nm}.png"
    if t == "blue":
        return f"noise-blue-{s}{nm}.png"
    return f"noise-{t}-{s}{nm}.png"


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------

# ---------------------------------------------------------------------------
# Manifest helpers
# ---------------------------------------------------------------------------

_MANIFEST_KEYS = [
    "type", "size", "res", "cells", "kernels", "freq", "angle", "sigma",
    "octaves", "persistence", "lacunarity", "normal_map", "strength", "seed",
]


def _write_manifest(args, out: Path):
    manifest = {k: getattr(args, k) for k in _MANIFEST_KEYS}
    manifest["output"] = str(out)
    manifest["generated"] = datetime.datetime.now(datetime.timezone.utc).isoformat()
    manifest_path = out.with_suffix(".json")
    manifest_path.write_text(json.dumps(manifest, indent=2))
    print(f"Manifest: {manifest_path}")


def _apply_manifest(manifest_path: str, args):
    """Overlay manifest values onto args, leaving CLI-supplied --output and --show untouched."""
    data = json.loads(Path(manifest_path).read_text())
    for key in _MANIFEST_KEYS:
        if key in data:
            setattr(args, key, data[key])
    # Use the manifest's output path only if the user didn't supply --output
    if args.output is None and "output" in data:
        args.output = data["output"]


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------

def main():
    p = argparse.ArgumentParser(
        description="Generate a seamless (tileable) greyscale PNG texture.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__,
    )
    p.add_argument("--type",          choices=["perlin", "fbm", "value", "voronoi", "gabor", "blue"],
                                      help="Noise algorithm to use")
    p.add_argument("--size",          type=int,   default=512,  help="Image width/height in pixels (default: 512)")
    p.add_argument("--res",           type=int,   default=8,    help="perlin/fbm/value: base grid resolution (default: 8)")
    p.add_argument("--octaves",       type=int,   default=6,    help="fbm: number of octaves (default: 6)")
    p.add_argument("--persistence",   type=float, default=0.5,  help="fbm: amplitude decay per octave (default: 0.5)")
    p.add_argument("--lacunarity",    type=float, default=2.0,  help="fbm: frequency multiplier per octave (default: 2.0)")
    p.add_argument("--cells",         type=int,   default=12,   help="voronoi: number of seed cells (default: 12)")
    p.add_argument("--kernels",       type=int,   default=2000, help="gabor: number of kernels (default: 2000)")
    p.add_argument("--freq",          type=float, default=0.08, help="gabor: wave frequency (default: 0.08)")
    p.add_argument("--angle",         type=float, default=45.0, help="gabor: orientation in degrees (default: 45)")
    p.add_argument("--sigma",         type=float, default=12.0, help="gabor: kernel spread / blue: cluster radius (default: 12 / 1.9)")
    p.add_argument("--normal-map",    action="store_true",      help="Convert greyscale output to a tangent-space RGB normal map")
    p.add_argument("--strength",      type=float, default=4.0,  help="normal-map: bump strength / contrast (default: 4.0)")
    p.add_argument("--output",        type=str,   default=None, help="Output path (default: auto-named in cwd)")
    p.add_argument("--seed",          type=int,   default=None, help="Random seed for reproducibility")
    p.add_argument("--manifest",      action="store_true",      help="Write a JSON manifest alongside the image (auto-records the seed)")
    p.add_argument("--from-manifest", type=str,   default=None, metavar="JSON",
                                      help="Regenerate from a manifest file (restores all params and seed)")
    p.add_argument("--show",          action="store_true",      help="Open the image after saving (requires a display)")
    args = p.parse_args()

    # Load manifest first so CLI flags can still override individual values
    if args.from_manifest:
        _apply_manifest(args.from_manifest, args)
        args.manifest = True  # always write manifest when regenerating from one

    if args.type is None:
        p.error("--type is required (or supply --from-manifest)")

    # Resolve seed: explicit > manifest-loaded > auto-generate when --manifest requested
    if args.seed is None and args.manifest:
        args.seed = int.from_bytes(os.urandom(4), "big")

    if args.seed is not None:
        np.random.seed(args.seed)

    # Blue noise sigma default differs from gabor
    if args.type == "blue" and args.sigma == 12.0:
        args.sigma = 1.9

    print(f"Generating {args.type} noise ({args.size}×{args.size})…", flush=True)

    if args.type == "perlin":
        data = gen_perlin(args.size, args.res)
    elif args.type == "fbm":
        data = gen_fbm(args.size, args.res, args.octaves, args.persistence, args.lacunarity)
    elif args.type == "value":
        data = gen_value(args.size, args.res)
    elif args.type == "voronoi":
        data = gen_voronoi(args.size, args.cells)
    elif args.type == "gabor":
        data = gen_gabor(args.size, args.kernels, args.freq, args.angle, args.sigma)
    elif args.type == "blue":
        data = gen_blue(args.size, args.sigma)

    if args.normal_map:
        print(f"Converting to normal map (strength={args.strength})…", flush=True)
        data = _to_normal_map(data, args.strength)

    out = Path(args.output) if args.output else Path(_auto_name(args))
    out.parent.mkdir(parents=True, exist_ok=True)

    img = Image.fromarray(data)
    img.save(out)
    print(f"Saved: {out}")

    if args.manifest:
        _write_manifest(args, out)

    if args.show:
        img.show()


if __name__ == "__main__":
    main()
