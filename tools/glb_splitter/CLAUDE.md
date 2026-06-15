# tools/glb_splitter

Splits a monolithic GLB into a mesh-only file and one or more animation-group files.
Prerequisite for the multi-source animations feature — produce the split files here,
then reference them via `animation_sources` in an `AnimationPolicy.ron`.

## Install

```bash
pip install pygltflib
```

## Usage

```bash
# List animations and their auto-detected prefixes
python tools/glb_splitter/split.py assets/shared/models/characters/hero.glb --list

# Auto-group by name prefix  (locomotion_walk, locomotion_run → "locomotion"; combat_attack → "combat")
python tools/glb_splitter/split.py assets/shared/models/characters/hero.glb --by-prefix

# Explicit groups
python tools/glb_splitter/split.py assets/shared/models/characters/hero.glb \
    --group locomotion walk,run,idle,jump \
    --group combat attack,dodge,die

# Write to a specific output directory
python tools/glb_splitter/split.py hero.glb --by-prefix --out-dir assets/shared/models/characters/

# Skip generating the mesh-only file (if you only need new animation groups)
python tools/glb_splitter/split.py hero.glb --by-prefix --no-mesh
```

## Output files

Given input `hero.glb`:

| File | Contents |
|---|---|
| `hero_mesh.glb` | Geometry + materials + skeleton, **no animations** |
| `hero_locomotion.glb` | Skeleton + locomotion clips, **no mesh/material data** |
| `hero_combat.glb` | Skeleton + combat clips, **no mesh/material data** |

Animation-group files carry the full skeleton (joint names intact) so Bevy can bind
clips to the model's `AnimationGraph` by name. Mesh geometry is stripped; the files
are typically 5–20× smaller than the source GLB.

## Blender-exported names

Blender prepends `ObjectName|` to animation names on export (e.g. `Armature|walk`).
The splitter strips this prefix automatically for both `--by-prefix` grouping and
`--group` clip matching — you can pass either `walk` or `Armature|walk`.

## Workflow with multi-source animations

After splitting:

1. Add all output files to `assets.ron`:
   ```ron
   models: {
       "hero_mesh":       (path: "shared/models/characters/hero_mesh.glb"),
       "hero_locomotion": (path: "shared/models/characters/hero_locomotion.glb"),
       "hero_combat":     (path: "shared/models/characters/hero_combat.glb"),
   }
   ```

2. Reference them in the character's `AnimationPolicy.ron`:
   ```ron
   // prefabs/animation/hero.ron
   model_key: "hero_mesh",
   animation_sources: ["hero_locomotion", "hero_combat"],
   named_animations: { ... }
   ```

   _(Requires the multi-source animations runtime feature — currently Queued.)_

## Test file

`assets/shared/models/characters/character-animations.glb` — 123 clips, 8 MB.
Good reference for testing both `--by-prefix` and `--group` modes.

```bash
python tools/glb_splitter/split.py assets/shared/models/characters/character-animations.glb --list
python tools/glb_splitter/split.py assets/shared/models/characters/character-animations.glb --by-prefix --out-dir /tmp/split_test
```

## Known limitations (v1)

- The mesh-only GLB retains animation byte data in its binary buffer (the JSON
  animation array is stripped but the buffer is not compacted). File size is unchanged
  from the source. A future `--compact-mesh` flag will address this.
- Sparse accessors are not handled; inputs with sparse accessor data will produce
  incorrect output. Standard Blender / game-tool exports do not use sparse accessors.
- Assumes a single embedded buffer (standard GLB). Multi-buffer GLTF files are not
  supported.
