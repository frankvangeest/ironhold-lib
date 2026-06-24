---
name: bevy-019-upgrade
description: Assessment of the Bevy 0.18 to 0.19 upgrade — the real blockers, corrected audit, scope estimate, and the wait-don't-upgrade-now recommendation
metadata:
  type: project
---

Assessed 2026-06-25 (Bevy 0.19 just released). Recommendation: **do NOT upgrade now; ship nameplate system on 0.18; Icebox the upgrade gated on dependency readiness.**

**Why:** It's pure infrastructure churn with zero data-driven payoff, and 0.19 *just* released so the ecosystem deps aren't ready. The bar for interrupting feature work for a Bevy bump is high and not cleared today.

**How to apply:** When asked to revisit, the gate is dependency readiness, not "how much work." Don't start until rapier AND framepace AND bevy_common_assets all ship 0.19 releases.

## The real blocker = the whole ecosystem, not just rapier
Pinned (verified in Cargo.toml): `bevy 0.18.0`, `bevy_rapier3d 0.33` (with `enhanced-determinism` + `serde-serialize`), `bevy_framepace 0.21` (BOTH runners), `bevy-inspector-egui 0.36` (optional `inspector` feature, can disable temporarily), `bevy_common_assets` (git rev pin, RON loaders depend on it), `bevy_mesh 0.18` (separate pin, lockstep with bevy). Tree won't resolve until rapier+framepace+bevy_common_assets are all 0.19. rapier is the long pole; historically lags core Bevy by weeks.

**Avian-as-shortcut is a trap.** Switching physics engines to dodge the rapier wait conflates two migrations. Avian's cross-platform determinism differs and we use `enhanced-determinism` deliberately (matters for the networking roadmap). If ever switching to Avian: separate feature file, separate initiative, evaluated on determinism merits — never as a Bevy-bump unblocker.

## Audit items that DO NOT apply to this codebase (correct the next person)
Verified by grep against current src:
- `World::clear_entities()` clears-resources change → N/A. Scene reload does targeted `commands.entity(e).despawn()` over `LevelEntity`-tagged entities (action_executor.rs 71-226), never `clear_entities`. PlayerInventory/PlayerEquipment survive as untouched resources. The "silent regression" worry is unfounded for current code.
- `AnimationTargetId` recalc → N/A. animation.rs keys clips by string name via `gltf.named_animations` → `AnimationGraph` + `AnimationNodeIndex`. Zero AnimationTargetId usage. Bevy resolves targets internally on graph build.
- `Ref<T>.clone()` change → N/A. No `Ref<T>` in core.
- `Skybox`/`WgpuSettingsPriority` → N/A. Neither symbol in core.
- `SceneRoot`→`WorldAssetRoot` → audit claimed scene_loader uses SceneRoot; grep found ZERO in core. Likely already gone; verify but trivial.
- No `Component+Resource` double-derive exists → that specific compile error won't fire.

## Real touch points and scope (~4-7 focused person-days IF tree resolves; ~1-1.5 wk wall-clock)
- Text migration is the BIG bucket: **120 `font_size` occurrences** (audit said "15+ systems" — undercounted ~8x). `FontSize::Px`, `FontSource`, `TextLayout::justify`, `TextSection` getters. Hotspot: `world_label_screen_pos_system` per-frame font_size writes (lib.rs ~428) for depth-scale.
- Resources-as-Components: audit broad `Query<Entity>`/`Query<()>` for new IsResource conflict; add ResMut Mutable bounds. 20+ Resource derives but no double-derives.
- Material crate moves: `SpecializedMeshPipelineError`/`AlphaMode`/`MaterialProperties` → `bevy_material`. 7 files (custom_material, flame_material, foliage, particle_renderer, stat_radar, terrain_material, material_factory).
- 2 custom SystemParam (SpawnParams, SceneV2Params in scene_manager/mod.rs) — validation timing change, low risk.
- Camera HDR moves — verify `world_to_viewport` signature (used targeting.rs x2 + lib.rs ~428; backbone for nameplates/targeting).
- Testing native+WASM+WebGPU is 1.5-2 days, non-negotiable.

## Silent-risk surfaces (native tests WON'T catch — browser play-test mandatory)
- **16-byte alignment in 6 custom materials** — highest silent risk. If 0.19 changes AsBindGroup codegen, native renders fine but web panics BUFFER_BINDINGS_NOT_16_BYTE_ALIGNED. See [[fragile_modules]] / [[wasm_pitfalls]].
- **Pipeline warmup drift** — 3 hand-maintained variants (additive/blend/flame-distort). AlphaMode/specialization-key shifts could silently drop a variant → first-spawn WASM stall returns.
- **Binary size** — at 90.7 MB vs 100 MB hard wall. 0.19 crate-split is unpredictable; size-check release build immediately, have reduction fallback. Could force unbudgeted size work.
- **AsyncComputeTaskPool cancel-on-drop** (terrain.rs) — fast scene switch could leave terrain ungenerated. Latent footgun for chunked-terrain backlog.

## Upgrade order (when unblocked)
1. Bump versions, get tree to resolve/compile first. 2. Fix compile errors core-first (runners thin). 3. Text migration as one pass. 4. Material import moves. 5. `cargo check -p ironhold_cli` (schema-adjacent breaks query.rs silently). 6. Native integration tests. 7. WASM dev build + browser play-test ALL 6 materials + terrain + particles. 8. Screenshot baseline diff. 9. Release build + size check.
