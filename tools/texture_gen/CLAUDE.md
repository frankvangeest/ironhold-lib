# Texture Generator

Generates seamless (tileable) greyscale PNGs for use as material texture inputs.
Normal map output (`--normal-map`) produces an RGB PNG in OpenGL tangent-space convention.

## Install

```bash
pip install -r tools/texture_gen/requirements.txt
```

## Noise types — when to use each

| Type | Best for | Key parameters |
|---|---|---|
| `fbm` | **Most useful** — organic multi-scale detail for terrain, rock, clouds, surfaces | `--res`, `--octaves` (4–8), `--persistence` |
| `perlin` | Soft single-scale variation; base layer for blending | `--res` (higher = finer detail) |
| `value` | Softer, blobby variation; cheaper alternative to perlin | `--res` |
| `voronoi` | Cell patterns, cracked earth, stone, scales, leather | `--cells` (fewer = larger cells) |
| `gabor` | Directional textures: wood grain, brushed metal, fabric | `--freq`, `--angle`, `--sigma`, `--kernels` |
| `blue` | Dithering base, stipple patterns, screen-space noise | `--sigma` — **slow above 128px** |

Add `--normal-map` to any type to convert the greyscale output to a tangent-space RGB normal map (OpenGL convention, suitable for Bevy PBR). Use `--strength` to control bump intensity (default 4.0).

## Usage

```bash
# FBM — best general-purpose noise (organic, multi-scale)
python tools/texture_gen/generate.py --type fbm --size 512 --res 4 --octaves 6 --manifest --output assets/shared/textures/noise/noise-fbm-512-r4-o6.png

# FBM as a detail normal map for PBR materials
python tools/texture_gen/generate.py --type fbm --size 512 --res 4 --octaves 6 --normal-map --strength 4.0 --manifest --output assets/shared/textures/noise/noise-fbm-512-r4-o6-nm.png

# Perlin — single-octave soft noise
python tools/texture_gen/generate.py --type perlin --size 512 --res 8 --manifest --output assets/shared/textures/noise/noise-perlin-512-r8.png

# Value — blobby, softer
python tools/texture_gen/generate.py --type value --size 512 --res 16 --manifest --output assets/shared/textures/noise/noise-value-512-r16.png

# Voronoi — cracked / cell pattern
python tools/texture_gen/generate.py --type voronoi --size 512 --cells 12 --manifest --output assets/shared/textures/noise/noise-voronoi-512-c12.png

# Gabor — wood grain (60 degrees)
python tools/texture_gen/generate.py --type gabor --size 512 --kernels 3000 --freq 0.08 --angle 60 --sigma 12 --manifest --output assets/shared/textures/noise/noise-gabor-512-f008-a60-s12.png

# Blue noise — small sizes only
python tools/texture_gen/generate.py --type blue --size 64 --sigma 1.9 --manifest --output assets/shared/textures/noise/noise-blue-64.png

# Regenerate identically from a recorded manifest
python tools/texture_gen/generate.py --from-manifest assets/shared/textures/noise/noise-fbm-512-r4-o6.json
```

Always use `--manifest` when generating textures for `assets/shared/textures/noise/` so the seed and parameters are recorded for later reproduction.

## Terrain heightmaps

Heightmaps are greyscale PNGs where pixel brightness encodes elevation. FBM is the
right choice: low base resolution gives large landscape features; high octave count
adds small ridge and rock detail. Each project should have its own heightmap with a
unique seed so terrain is distinct.

Recommended starting point for a 1024×1024 terrain heightmap:

```bash
python tools/texture_gen/generate.py \
  --type fbm --size 1024 --res 2 --octaves 8 --persistence 0.55 \
  --manifest \
  --output assets/projects/{name}/terrain/heightmap.png
```

- `--res 2` — very large base wavelength (continental-scale hills)
- `--octaves 8` — enough detail for ridgelines and micro-terrain
- `--persistence 0.55` — slightly more high-frequency energy than default (0.5) for rougher terrain
- Each project uses its own output path; manifests record the seed so the exact heightmap can be reproduced

Vary `--res` (1–4) to control the scale of the dominant landform, and `--persistence`
(0.45–0.65) to control how rough/smooth the surface feels.

## Performance notes

- `perlin`, `fbm`, `value`: fast at any size
- `voronoi`: slow above ~512 due to nested loops; 1024 takes ~60s; run without `run_in_background`
- `gabor`: scales with `--kernels`; 2000–4000 is a good range for 512px
- `blue`: **very slow** — O(n^4) in pixel count; use 64–128px and tile in the shader

## Typical output destination

Generated textures go in `assets/shared/textures/noise/`. Register them in the project's
`assets.ron` before referencing them in a material shader.
