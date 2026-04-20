# primitive_world

A gameplay demo built entirely from Bevy primitive shapes — no imported GLB models.

## Rules

**All geometry in this project must be built from primitive shapes only.**
Accepted shapes: `Cuboid`, `Sphere`, `Cylinder`, `Capsule3d`, `Cone`, `ConicalFrustum`, `Torus`.

Prefabs may be composed of multiple primitives as children (e.g. a house built from a cuboid body + conical-frustum roof + cylinder chimney). What is not allowed is importing external model files (`.glb`, `.gltf`, etc.).

This constraint is intentional — the project demonstrates what is achievable with primitives and shared stylized textures alone, and serves as a baseline test environment that has zero dependency on external art assets.

## Textures

Scenery surfaces (buildings, fences, trees, rocks, bridge, path) use stylized hand-painted textures from `shared/textures/`. Character capsules (player, NPCs) keep flat colors so they remain visually distinct from the environment.

## Structure

```
primitive_world/
  primitive_world.project.ron   ← entry point
  scenes/                       ← scene files (start_menu, main, pause, map)
  assets.ron                    ← material and audio catalog
  prefabs/prefabs.ron           ← all primitive prefab definitions
  logic/                        ← rules and state machine
  audio/                        ← project-specific audio clips
```
