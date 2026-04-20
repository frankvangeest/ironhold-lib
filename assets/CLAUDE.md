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
    textures/             ← tileable PBR and stylized texture sets + noise utilities
    audio/                ← music, UI sounds, footstep packs, ambient effects
    models/               ← shared GLB models (creatures, props)
  projects/{name}/        ← project-specific assets; not shared
    {name}.project.ron
    scenes/
    assets.ron
    prefabs/
    logic/
```

## Shared vs project-specific

Put an asset in `shared/` only if it is genuinely reusable across unrelated projects. A texture used by one scene, a sound effect tied to one mechanic, or a model built for one character belongs in the project folder.

## Texture descriptions

Visual descriptions of all shared textures live in `shared/textures/texture-descriptions.md`. Update it when adding or removing textures.

## Shader authoring

See [`docs/25_custom_shaders.md`](../docs/25_custom_shaders.md) for the WGSL binding contract, uniform packing rules, and step-by-step authoring guide.
