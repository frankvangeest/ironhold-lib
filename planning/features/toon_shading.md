# Feature: Toon / Cel Shading (3-tone, 4-tone, 5-tone)

_Status: Icebox_
_Planned at: `b384785` (2026-04-29)_

## What

A family of stylized shading shaders for use with `CustomMaterial`. Instead of a smooth
lighting gradient, the surface is divided into discrete flat-colour bands based on the
N·L dot product (surface normal vs. primary directional light direction):

| Variant | Bands | Typical use |
|---|---|---|
| 3-tone | shadow / base / light | Classic cel-shaded look, most common |
| 4-tone | shadow / mid-shadow / base / highlight | Richer form without a ramp |
| 5-tone | shadow / dark / base / light / highlight | Full stylized gradient; requires ramp texture (see below) |

All three are pure fragment shaders — no new Rust types, no new RON schema fields.
They plug directly into the existing `CustomMaterial` pipeline.

## Why

All current materials are either PBR (smooth gradient) or unlit (no shading at all).
Stylized games typically want discrete light bands — it is the single most-requested
look for the `CustomMaterial` showcase. The 3- and 4-tone variants fit completely within
the existing uniform budget and can be shipped as WGSL files only.

## Approach

### Light direction

All three shaders need the primary directional light direction. Bevy exposes this through
`bevy_pbr::mesh_view_bindings`:

```wgsl
#import bevy_pbr::mesh_view_bindings::lights

// In fragment():
let n_dot_l = clamp(dot(normalize(normal), lights.directional_lights[0].direction_to_light), 0.0, 1.0);
```

If `lights.n_directional_lights == 0u`, fall back to a fixed upward direction
`vec3(0.0, 1.0, 0.0)` so the shader degrades gracefully in unlit scenes.

Use `pbr_input_from_vertex_output(mesh, is_front, false)` to get the correct
world-space normal (handles double-sided, normal maps).

### Uniform packing — 3-tone and 4-tone

Each tone is encoded as `(r, g, b, threshold)` in one `vec4`, where `threshold` is the
**upper** N·L boundary for that tone. The last tone's threshold is unused (always wins).

```
3-tone  (uses params_0..2, params_3 free):
  params_0 = (shadow.rgb,  t_shadow_to_base)    e.g. 0.30
  params_1 = (base.rgb,    t_base_to_light)     e.g. 0.65
  params_2 = (light.rgb,   —unused—)

4-tone  (uses all 4 params slots):
  params_0 = (shadow.rgb,      t01)
  params_1 = (mid_shadow.rgb,  t12)
  params_2 = (base.rgb,        t23)
  params_3 = (highlight.rgb,   —unused—)
```

Shader logic (3-tone example):

```wgsl
if n_dot_l < material.params_0.a { return vec4(material.params_0.rgb, 1.0); }
if n_dot_l < material.params_1.a { return vec4(material.params_1.rgb, 1.0); }
return vec4(material.params_2.rgb, 1.0);
```

### Uniform capacity constraint — why 5-tone needs a different approach

5 tones require at minimum 5 × RGB + 4 thresholds = **19 floats**. The current
`CustomMaterialUniforms` block is exactly 4 × vec4 = **16 floats**. There is no way to
fit 5 directly-encoded tones without exceeding the budget.

### 5-tone: ramp texture approach

A single `custom_toon_ramp.wgsl` shader replaces the inline colour encoding with a
1D lookup texture. The tone palette is a tiny PNG strip (e.g. 16×1 px); N·L is used as
the UV X coordinate.

```
texture_0   ← 1D ramp: left = shadow, right = highlight (author in any image editor)
params_0.x  ← contrast: sharpens the band edges (1.0 = hard, 0.0 = smooth gradient)
```

Sampling:

```wgsl
let u = pow(n_dot_l, max(material.params_0.x, 0.01));
let tone = textureSample(texture_0, sampler_0, vec2(u, 0.5));
return vec4(tone.rgb, 1.0);
```

RON authoring:

```ron
"hero_toon": (
  kind: Custom((
    shader: "shared/shaders/custom_toon_ramp.wgsl",
    textures: {
      "texture_0": "projects/my_game/textures/hero_toon_ramp.png",
    },
    floats: {
      "contrast": 6.0,   // higher = harder band edges
    },
  )),
),
```

The ramp texture approach naturally supports any tone count. Artists can design their
palette visually in any image editor and iterate without touching RON.

### Double-sided compatibility

All three shaders must handle `@builtin(front_facing)` so they work correctly when
`double_sided: true` is set on the prefab. Use `pbr_input_from_vertex_output` to
get the already-flipped normal — this is already how `custom_pbr.wgsl` works.

### Files to create

| File | Description |
|---|---|
| `assets/shared/shaders/custom_toon_3.wgsl` | 3-tone cel shader |
| `assets/shared/shaders/custom_toon_4.wgsl` | 4-tone cel shader |
| `assets/shared/shaders/custom_toon_ramp.wgsl` | Ramp-texture toon shader (5+ tones) |
| `assets/projects/entity_logic_demo/textures/` or a shared demo ramp | Example ramp PNG for the custom_materials showcase |

No new Rust code. No new RON schema fields. No new Bevy material types.

### Showcase in `custom_materials`

Add entries to the `custom_materials` project (one row each):

```ron
"toon_3":    custom_toon_3.wgsl     — classic 3-tone, sphere
"toon_4":    custom_toon_4.wgsl     — 4-tone, sphere
"toon_ramp": custom_toon_ramp.wgsl  — ramp-based, sphere, uses a shared demo ramp PNG
```

## Open questions

- **Ambient contribution**: pure N·L toon ignores ambient light, leaving shadowed sides
  fully flat. Options: add a minimum brightness floor (a float param), or blend in a
  fraction of the ambient color. Decide during implementation.
- **Multiple lights**: the current design reads only `directional_lights[0]`. Scenes with
  multiple directional lights (e.g. a fill light + key light) will show only one. If this
  becomes a problem, a follow-up could accumulate N·L across all directional lights before
  quantizing.
- **Ramp texture format**: should the shared ramp be authored as a PNG strip (fast to
  iterate) or a gradient? A 1×16 PNG strip with hard edges gives true discrete bands; a
  smooth gradient gives painterly shading. Either works — document both in the shader header.
- **Outline rendering**: a back-face-expanded outline mesh or post-process edge pass is the
  natural companion to toon shading. This is out of scope for this feature but worth
  planning as a follow-up in the icebox.

## Acceptance criteria

- `custom_toon_3.wgsl` and `custom_toon_4.wgsl` are usable from RON with only `colors`
  entries; no new Rust code required.
- Assigning `double_sided: true` to a toon-shaded prefab renders correctly (no normal
  artefacts on back faces).
- `custom_toon_ramp.wgsl` samples `texture_0` at the N·L UV coordinate and renders
  visibly discrete bands when given a hard-edge ramp texture.
- All three shaders appear in the `custom_materials` showcase scene with labels.
- All existing tests pass.
