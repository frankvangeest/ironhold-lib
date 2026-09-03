# Feature: Extend Pipeline Warmup to Cover Text2d and UI Pipelines

_Status: Draft_
_Planned at: `0ac570d` (2026-05-05)_

## What

Extend the existing `pipeline_warmup_system` (which currently submits phantom `Mesh3d` entities to force 3D render pipeline compilation at scene load) to also warm up the 2D and UI render pipelines used by `Text2d` labels and UI buttons. The warmup entities are spawned invisible, survive one frame so GPU compilation runs, then are despawned.

Game designers are unaffected — this is a runtime-only change with no RON schema impact.

---

## Why

The current warmup only covers 3D mesh pipelines. `Text2d` labels and Bevy UI nodes (`Button`, `Node`) use separate 2D render pipelines that are compiled lazily on first use. On WASM/WebGPU, lazy pipeline compilation causes a visible frame spike the first time any text or UI element appears. If a scene has labels or buttons, those pipelines compile during gameplay rather than during the loading phase, producing the same kind of stall the 3D warmup was designed to prevent.

This is especially relevant for:
- Depth-scaled world-space labels (used in `custom_materials` demo)
- UI overlays (used in `quick_scene`, `3rd_person_game_demo`, etc.)

---

## Approach

The existing `pipeline_warmup_system` in `capabilities/` spawns a minimal set of invisible entities that cover the render pipelines needed by real scene content. The same approach applies here:

1. **`Text2d` warmup entity** — spawn a `Text2d` with a blank string and `Visibility::Hidden` for one frame, then despawn. This triggers the glyph atlas + 2D text pipeline compilation.

2. **UI warmup entity** — spawn a minimal `Node`/`Button` hierarchy with `Visibility::Hidden` for one frame, then despawn. This triggers the UI batching pipeline.

Both warmup entities should be spawned at the same time as the existing 3D warmup (on scene load, before `InGame` state, or at the existing warmup trigger point). They despawn after one frame via the same lifecycle marker already used for 3D warmup.

Key questions to resolve during implementation:
- Does Bevy compile a single shared 2D pipeline for all `Text2d` regardless of font/size, or is each font a separate pipeline? (Likely one pipeline per material variant — a single warmup entity should suffice.)
- Does the UI pipeline vary by `Node` styling in a way that requires multiple warmup variants? (Likely not for basic usage.)

---

## Tasks

- [ ] Read and understand existing `pipeline_warmup_system` — locate the warmup entity lifecycle marker and despawn logic
- [ ] Spawn a hidden `Text2d` warmup entity alongside existing warmup entities
- [ ] Spawn a hidden UI `Node` warmup entity alongside existing warmup entities
- [ ] Verify despawn lifecycle covers the new entities
- [ ] Smoke-test on native: confirm no visual artefacts from warmup entities leaking into view
- [ ] Test on WASM: confirm first-frame text/UI spike is gone (or reduced) after warmup
- [ ] Docs: note the extended warmup in any performance section of `docs/`

---

## Open questions

- Does the 2D text pipeline vary per font asset, requiring one warmup entity per font used in the scene? If so, the warmup may need to use the actual fonts from the scene's asset catalog rather than a default font.
- Should the warmup entities use the real fonts/styles from the current scene (more complete coverage) or a single generic entity (simpler, lower risk)? Start generic; upgrade if the spike persists.

---

## Acceptance criteria

- Given a scene with `Text2d` labels, the first frame those labels are visible produces no measurable GPU pipeline-compile stall on WASM.
- Given a scene with UI buttons, the first frame the UI overlay appears produces no measurable GPU pipeline-compile stall on WASM.
- Given any scene, no phantom warmup entities are visible to the player at any point.
- Given a scene with no text or UI, the warmup entities spawn and despawn silently with no side effects.
