# Assets

## Art style

All shared assets follow a **stylized hand-painted** direction. Before adding or modifying assets, read [`docs/05_art_style.md`](../docs/05_art_style.md). Key points:

- Hand-painted look: exaggerated detail, strong edge contrast, painted-in lighting.
- Details must read clearly at reduced size — no fine noise that disappears in-game.
- Controlled, intentional color palette (warm organics, cool stone/metal, earned accent color).
- Photorealistic or contemporary-style textures do not belong in `shared/` unless clearly flagged as non-stylized outliers.

## Structure

```
assets/
  shared/                 ← reusable across all projects
    shaders/              ← custom WGSL shaders (prefix: custom_*)
    textures/             ← see subfolder conventions below
    terrain/              ← shared terrain layers (grass, rock, dirt, snow, splatmap)
    audio/                ← music, UI sounds, footstep packs, ambient effects
    models/               ← shared GLB models (creatures, props)
  projects/{name}/        ← project-specific assets; not shared
    {name}.project.ron
    scenes/
    assets.ron
    prefabs/
    logic/
    terrain/              ← project-specific heightmap (heightmap.png + heightmap.json manifest)
```

### Texture subfolder conventions

| Folder | Type | Contents |
|---|---|---|
| `{Name}_SD/` | **PBR set** | Full PBR material set — basecolor, normal, roughness, AO, height (one folder per material) |
| `particles/` | Texture | Particle billboard sprites (e.g. kenney-particle-pack) |
| `noise/` | Texture | Seamlessly tiling greyscale utility textures for WGSL shaders (Perlin, Voronoi, blue noise …) |
| `decals/` | Texture | Projected decal textures (rings, impact marks, splats) |
| `foliage/` | Texture | Leaf alpha brush-stroke textures for the stylized foliage system |
| `ui/` | Texture | UI icons, atlas sheets, and HUD elements |
| `skybox/` | Texture | Skybox cubemaps and HDRIs |

The `_SD` suffix is the naming convention for **all** PBR material sets. Any folder that does not end in `_SD` is a non-PBR texture category. The asset browser (`assets.html`) uses this distinction to show a "PBR Set" badge vs a "Texture" badge.

Do not dump new textures at the `textures/` root. Place them in the matching subfolder; create the subfolder with a `.gitkeep` if none exists yet. Delete the `.gitkeep` once real files land.

## Shared vs project-specific

Put an asset in `shared/` only if it is genuinely reusable across unrelated projects. A texture used by one scene, a sound effect tied to one mechanic, or a model built for one character belongs in the project folder.

## Texture descriptions

Visual descriptions of all shared textures live in `shared/textures/texture-descriptions.md`. Update it when adding or removing textures.

## Tools

**After editing any `assets.ron` or moving/renaming files**, run the asset checker to catch broken references:

```bash
python tools/asset_checker/check.py
```

**After adding, removing, or renaming any asset files**, regenerate the asset browser manifest so `assets.html` stays current:

```bash
python tools/build_asset_manifest.py
```

This writes `assets_manifest.json` at the repo root. The asset browser (`assets.html`) reads this file to populate its model, texture, and audio listings. Commit the updated manifest alongside any asset changes.

**Noise textures and terrain heightmaps** are generated with the texture tool — see `tools/texture_gen/CLAUDE.md` for noise types and heightmap parameters. Each project's heightmap lives at `projects/{name}/terrain/heightmap.png` and must have a `.json` manifest alongside it.

**AVIF preview images** in `assets/shared/models/avif/` can be converted to PNG with:

```bash
python tools/avif2png/convert.py assets/shared/models/avif/
```

## Shader authoring

See [`docs/25_custom_shaders.md`](../docs/25_custom_shaders.md) for the WGSL binding contract, uniform packing rules, and step-by-step authoring guide.
