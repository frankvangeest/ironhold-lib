---
name: scene-prefab-boundary
description: Where ironhold's scene/prefab responsibility split diverges from canonical game-engine model, and the recommended direction
metadata:
  type: project
---

Design advisory on what "scene" vs "prefab" mean in ironhold vs general engines. See [[core-architectural-decisions]].

## Current state (as of 2026-06)
- **Scene** (`GameSceneV2`, schema/scene_v2.rs) = placed-instance manifest + per-scene environment + screen-space UI. `SceneEntityDef` is thin: `{ id, prefab, transform, label? }` — **no per-instance override channel**.
- **Prefab** (`PrefabDef`, schema/catalog.rs) = archetype/template (~30 fields) carrying 5 concern-classes: composition, behavior, presentation, physics, and a `components: PrefabComponents` grab-bag.

## The divergences from Unity/Unreal/Godot/Bevy canon
1. **No per-instance overrides on SceneEntityDef** — the defining feature of the canonical prefab/scene split is absent. Forces prefab forks. Evidence in repo: `shrine_*` ×4 (identical geometry, diff orb color+hint), `attack_dummy`/`attack_dummy_ascii` (diff only in world_stat_bar.style), `chest_01`/`chest_02`.
2. **Camera/input/player-singleton config lives on the prefab** — `PrefabComponents.camera`/`flycam`/`inputs` are read only for tagged player/flycam prefabs. These are GameMode/level-layer concerns everywhere else. Can't reuse a character prefab with a different camera without forking.
3. **`tags: Vec<String>` is overloaded** — `"player"`/`"flycam"`/`"collectable"` are magic strings that drive control flow in scene_loader.rs (TAG_PLAYER/TAG_FLYCAM/TAG_COLLECTABLE). Asymmetric: interactable/trigger_zone/targetable/npc are typed fields, but those three are stringly-typed.
4. **Composite prefabs duplicate scene-graph placement** — `ChildPrimitiveDef` (offset/rot/scale/material/nested prefab ref) is a second placement schema parallel to SceneEntityDef; they drift (ChildPrimitiveDef nests prefabs, SceneEntityDef doesn't).

## Recommended direction (value-to-disruption order)
1. **Per-instance overrides on SceneEntityDef** — start additive (`material: Option<String>`, `motion: Option<MotionDef>`, etc., `#[serde(default)]`, NO version bump). General `overrides:` block later (needs feature file + likely GameSceneV2 v3).
2. **Promote magic tags to typed fields** — `collectable: bool` (additive, free); typed player role / relocate player-designation to scene/project layer (keep tag fallback to avoid version bump).
3. **Move camera/input toward scene/project layer** — highest correctness, highest disruption. Interim: let scene override prefab camera via the #1 override channel.
4. **Unify SceneEntityDef + ChildPrimitiveDef** into one nestable placement primitive (Godot model) — design-only for now; freeze divergence.

**How to apply:** When advising on scene/prefab schema changes, push toward this split. Favor additive Option fields over version bumps. The override channel (#1) is the linchpin — most other improvements layer on it. Watch for new prefab forks in projects as the signal that overrides are overdue.

## Consumer-side debt observed
scene_loader.rs has the NpcAgent-construction block copy-pasted between the composite-prefab arm (~line 302) and single-mesh arm (~line 544). Refactor target once PrefabDef slims down.
