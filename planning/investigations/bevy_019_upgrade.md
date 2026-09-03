# Investigation: Bevy 0.19 Upgrade Feasibility

_Investigated at `5d7261d` (2026-06-23)_

## Verdict

**Do not upgrade now. Wait for the dependency tree to resolve, then upgrade.**

Gate: `bevy_rapier3d` with Bevy 0.19 support (current latest: v0.33 targets 0.18). Two additional deps also need releases before the tree resolves.

---

## Blockers (hard — upgrade cannot proceed without these)

| Dependency | Current | Status |
|---|---|---|
| `bevy_rapier3d` | 0.33 (Bevy 0.18) | **No 0.19 release confirmed.** Last CHANGELOG entry (v0.33.0, March 2026) targets 0.18. |
| `bevy_framepace` | 0.21 | Used in native runner. Must ship a Bevy 0.19 compatible release. |
| `bevy_common_assets` | git-pinned (`ae3243d`) | RON loaders depend on it. Must ship a compatible release before the git pin can be resolved. |

**Why not switch to Avian physics instead?** Avian 0.7 supports Bevy 0.19, but this is a separate, larger migration — not a shortcut. The project's `enhanced-determinism` feature pin on Rapier signals a correctness requirement that would need independent evaluation for Avian. Treating a physics engine swap as a Bevy version bump workaround is the wrong reason for a physics swap.

---

## Real breaking changes (once the dep tree resolves)

### Largest bucket: Text migration (~4–7 days total)

`TextFont.font_size` type changes from `f32` to `FontSize::Px(f32)`. Grep shows **~120 occurrences** of `font_size` across the codebase — the depth-scale system in `world_label_screen_pos_system` mutates font_size on every WorldLabel entity every frame. Every direct write is a compile error. Mechanical, but high volume.

| Old | New |
|---|---|
| `text_font.font_size = 14.0` | `text_font.font_size = FontSize::Px(14.0)` |
| `TextFont { font: handle, font_size: 14.0, ..default() }` | `TextFont { font: handle.into(), font_size: FontSize::Px(14.0), ..default() }` |
| `Handle<Font>` field | `FontSource` (wraps handle, family name, or category) |
| `TextLayout::new_with_justify(...)` | `TextLayout::justify(...)` |

### SceneRoot rename

`SceneRoot` → `WorldAssetRoot`, `Scene` → `WorldAsset`, `DynamicScene` → `DynamicWorld`. Used in `scene_loader.rs` for GLB actor spawning. Compiler-guided find-and-replace.

### Material import paths

`SpecializedMeshPipelineError`, `AlphaMode`, `OpaqueRendererMethod`, `MaterialProperties` moved from `bevy_pbr` to the new `bevy_material` crate. Affects all 6 custom material files:
- `capabilities/custom_material.rs`
- `capabilities/terrain_material.rs`
- `capabilities/flame_material.rs`
- `capabilities/foliage.rs`
- `capabilities/particle_renderer.rs`
- `capabilities/stat_radar.rs`

Compiler-guided import updates.

### MeshPipelineKey specialization

`MeshPipelineKey::from_primitive_topology(topology)` → `MeshPipelineKey::from_primitive_topology_and_strip_index(topology, index_format)`. Affects custom material specialization in the 4 material files that implement `SpecializedMeshPipeline`.

### Custom SystemParam validation timing

`SpawnParams` and `SceneV2Params` in `runtime/scene_manager/mod.rs` use `#[derive(SystemParam)]`. Validation now occurs at data-fetch time rather than setup. Need to audit `init_state` and `get_param` implementations for correctness under the new timing.

### Resources-as-Components ECS change

Resources are now stored as ECS components on a hidden singleton entity. Known implications for this project:
- `ResMut<R>` requires `R: Resource<Mutability = Mutable>` — affects any generic system that takes `ResMut<R>` with a type parameter
- `Components::resource_id()` → `Components::component_id()` — affects any reflection/introspection code
- `ReflectResource` → `ReflectComponent` — affects reflected resource types

**Confirmed non-issue** (architect verified against the actual code): the project never calls `World::clear_entities()` directly. The scene reload path uses targeted `LevelEntity` despawns in `action_executor.rs`. The resource-clearing behavior change does not affect this project.

---

## Silent risks (won't show up as compile errors)

### 16-byte uniform alignment in custom materials — WASM only

The project's 6 custom materials use `AsBindGroup` with manually-padded uniform structs. If 0.19 tightens WebGPU alignment validation, improperly aligned uniforms will silently produce garbage rendering in Chrome/Firefox WASM builds (no Rust compile error). Each material's uniform struct must be verified: every `f32` field following a `Vec3` needs explicit padding to reach 16-byte alignment.

Files to audit: `custom_material.rs`, `terrain_material.rs`, `flame_material.rs`, `foliage.rs`, `particle_renderer.rs`, `stat_radar.rs`.

### Binary size pressure

Current WASM binary: **90.7 MB** against a 100 MB GitHub Pages hard limit (warn threshold: 95 MB). A major Bevy version bump with rendering refactors can increase binary size. The 0.19 render-graph → ECS-systems rewrite in particular may add code. Must check `ls -lh pkg/ironhold_web_bg.wasm` immediately after the release build — if ≥ 95 MB, size work is required before push.

### Pipeline warmup variant drift

`pipeline_warmup_system` in `lib.rs` pre-warms the render pipeline by spawning invisible Mesh3d entities. If 0.19 changes the set of pipeline variants that need warming (new `MeshPipelineKey` fields, new EarlyPostProcess vs PostProcess split), the warmup may miss variants, causing a visible first-frame stall on WASM. Verify WASM first-frame behavior visually after upgrade.

### Task drop now cancels in WASM

`AsyncComputeTaskPool` task dropping now cancels the task instead of detaching it. No current usage affected (terrain chunking not yet implemented), but any future async task that currently assumes detach-on-drop will need explicit `Task::detach()`.

---

## Confirmed non-issues (do not re-investigate)

The initial audit flagged several items that were verified as absent from the codebase:

| Flagged item | Verdict |
|---|---|
| `World::clear_entities()` resource-clearing behavior | Not used — reload path uses targeted `LevelEntity` despawns |
| `AnimationTargetId` algorithm change | Not used — animations keyed by string name via `AnimationGraph`/`AnimationNodeIndex` |
| `Ref<T>.clone()` now returns `Ref<T>` | `Ref<T>` not used in hot paths |
| `Skybox { image }` → `Option` | No skybox in any current project |
| `WgpuSettingsPriority::Compatibility` rename | Not referenced |

---

## Effort estimate (when blockers are cleared)

| Category | Effort |
|---|---|
| Text migration (~120 `font_size` + `FontSource`) | 1.5 days |
| SceneRoot → WorldAssetRoot rename | 0.5 day |
| Material import paths (6 files) | 0.5 day |
| MeshPipelineKey specialization (4 material files) | 0.5 day |
| SystemParam validation audit (2 params) | 0.5 day |
| Resources-as-Components audit | 0.5 day |
| WASM testing (alignment, warmup, binary size) | 1.0 day |
| **Total** | **~4–7 days** (wall-clock ~1.5 weeks including test cycles) |

---

## Next steps

1. **Watch for bevy_rapier3d announcing 0.19 support** — check the [CHANGELOG](https://github.com/dimforge/bevy_rapier/blob/master/CHANGELOG.md) and Dimforge Discord.
2. **Watch bevy_framepace and bevy_common_assets** for 0.19 releases.
3. **When all three dep blockers clear**: create a feature branch, apply changes in order: SceneRoot rename → material imports → MeshPipelineKey → text migration → SystemParam → Resources-as-Components → WASM test.
4. **Before merging**: run full WASM release build, check binary size, visually verify first-frame behavior in Chrome and Firefox.

Backlog entry: `## Icebox › Engine / Runtime — Bevy 0.19 upgrade`.
