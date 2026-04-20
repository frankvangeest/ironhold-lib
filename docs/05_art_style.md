# Art Style Guide

> **Doc type:** Reference
>
> **Scope:** Shared assets and all projects built on this engine. Individual projects may extend or specialize this style, but should not contradict it without good reason.

---

## Philosophy

This engine targets web builds first. That means real-time constraints are tight: no HDR pipeline, no bloom, moderate polygon budgets, and texture memory that must be kept reasonable for browser delivery. The chosen art style is **stylized hand-painted** — it turns these constraints into strengths rather than fighting them.

A stylized hand-painted style:

- Reads clearly at lower resolutions and on smaller screens.
- Communicates depth and material through painted detail rather than shader complexity.
- Ages well — it does not depend on cutting-edge rendering techniques to look intentional.
- Allows the same asset to work at a wide range of distances without LOD transitions feeling jarring.
- Keeps texture sizes smaller without visible quality loss, since smooth gradients compress well.

All shared assets should serve this direction. Projects that intentionally diverge (e.g. a realistic horror game, a UI-only tool) may do so, but the shared pool is not the right home for their assets.

---

## Visual characteristics

**Chunky silhouettes.** Shapes are read from distance first. Details are exaggerated to survive scaling — oversized rivets, thick mortar lines, pronounced roof edges. Fine noise that only reads at close range is avoided.

**Painted lighting.** Shadows, highlights, and ambient occlusion are partially baked into the albedo/basecolor, not entirely left to the renderer. A surface should look approximately right even under flat lighting.

**Edge contrast.** The boundary between two materials or surfaces is always clearly legible. Thin, uncertain edges are replaced with confident, slightly exaggerated transitions.

**Controlled saturation.** Colors are saturated but not garish. Each material has a dominant hue family (warm brown woods, cool gray stone, earthy ochre thatch). Grays are rarely neutral — they lean warm or cool to feel intentional.

**Readable noise.** Surface texture, grain, and damage patterns are simplified into large, clear strokes rather than photorealistic noise. A plank grain becomes a sweeping painted line. A chip or nick becomes a clear silhouette shape.

**Consistent scale.** Tile sizes across all shared textures should feel like they belong to the same world. Bricks and cobblestones should feel roughly the same real-world size relative to each other. Mismatched scale between adjacent surfaces is one of the fastest ways to break style coherence.

---

## Palette guidance

- **Warms dominate organic materials** — wood, thatch, bark, earth use brown, ochre, amber, sienna families.
- **Cools anchor stone and metal** — stone floors, cliff rocks, metal plates use blue-gray, slate, charcoal families.
- **Accent color is earned** — moss green, rust orange, and trim gold appear as accent patches, not dominant fills.
- **Avoid pure black or pure white** in albedo. Shadows bottom out at a dark, slightly warm or cool version of the surface color; highlights top out at a tinted off-white.
- **Specular / roughness** should be kept matte-to-semi-matte for most organic and stone materials. Hard specular is reserved for metal and polished stone.

---

## Textures

A shared texture fits the style if:

1. The basecolor has visible hand-painted detail — gradients, highlights, or damage brushstrokes.
2. Details read clearly at 50% of the intended display size.
3. It uses the dominant hue families described above (or justifies its own hue clearly).
4. It tiles without a visible seam or repetition pattern at 2–3 tiles per surface.

Photorealistic textures — those built from scanned photographs with minimal stylization — should not be added to the shared pool. They will always look out of place next to hand-painted neighbors.

---

## Shaders and custom materials

Custom shaders should reinforce, not fight, the hand-painted look:

- **Unlit + additive (`alpha_mode: Add`)** for emissive effects (energy fields, glows, particles). Values stay in [0, 1] — no HDR overflow.
- **Fresnel effects** should use smooth, readable transitions rather than tight, specular-like rims.
- **Animated shaders** (pulse, dissolve) should animate slowly enough to read intentionally — fast strobing breaks the calm, painterly mood.
- **Avoid photorealistic effects** — subsurface scattering simulations, physically accurate BRDF sheen, or screen-space effects that depend on full HDR. These do not belong in the shared shader library.

---

## What to avoid

- Photorealistic normal maps with high-frequency noise baked from a photoscan.
- Neutral mid-gray as a dominant surface color — everything should lean warm or cool.
- Texture sets that only look good with specular highlights or in specific lighting — stylized textures should read in flat, directional, and point-lit scenes equally.
- Modern or contemporary materials (subway tiles, particle board, industrial grating) unless the project explicitly targets a non-fantasy setting.
- Fine detail that only resolves at very close range — it disappears in-game and wastes texel budget.

---

## Evaluating new shared assets

Before adding a texture, model, or shader to `assets/shared/`, ask:

1. **Does it read at half size?** Scale the basecolor down to 50%. If the character is lost, the detail is too fine.
2. **Does it belong to the same world as the existing stylized textures?** Hold it next to `Stylized_Stone_Floor_010` or `Stylized_Wood_Planks_003`. Does it feel like the same artist made it?
3. **Is it general enough?** Shared assets should be reusable across unrelated projects. A very specific prop texture (e.g. one character's skin) belongs in a project folder, not shared.
4. **Does it stay within budget?** Prefer 512×512 for tileable textures. 1024×1024 is acceptable for hero assets or large tileable surfaces. 2048+ requires justification.
