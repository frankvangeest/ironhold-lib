---
name: scene-ambient-light-pattern
description: Scene lighting.ambient was a dead AmbientLight entity until prerelease_render_fixes phase A (GlobalAmbientLight resource); sRGB-linearised colour x brightness math shows most authored scenes get far darker; global_environment EnvironmentMapLight is the SAME dead-entity bug (no IBL fill anywhere); scene-scoped lighting resources leak
metadata:
  type: project
---

Until `prerelease_render_fixes` phase A (plan 5217c95, 2026-10-12; post-impl review 2026-10-13), `apply_lighting_v2`
spawned an `AmbientLight` entity, which in Bevy 0.18 is a per-camera override (`#[require(Camera)]`), so every scene
rendered with Bevy's GlobalAmbientLight default (white, 80). Phase A: `scene_ambient_light()` helper, always inserted on
Replace (reset when unauthored), either-field rule, both shadow-map resources reset too. Overlays never call it.

**Effective-ambient math to reuse:** colour goes through `Color::srgba`, so (0.2, 0.25, 0.35) linearises to
~(0.03, 0.05, 0.10). Luminance ratio vs old white x 80 at implementation time: quick_scene 4%, 3rd_person main and
terrain_demo ~5.5%, primitive_world start_menu ~2%, camera_modes/entity_logic ~8%, local_coop rooms 7-11%, menus with no
brightness (0.3,0.3,0.35)x150 ~14%, stats_demo 17%, primitive_world main 22%, particles main 25%; brighter: particles2
~122%, integration_tests terrain_test ~130%. Re-tuned in phase A: foliage_demo (0.7,0.75,0.85)x80 ~52%, custom_materials
(0.6,0.7,1.0)x40 ~23%, blank_project (0.8,0.85,1.0)x80 ~70%.

**Same dead-entity class, still open after phase A:** `ProjectConfig.global_environment` spawns `EnvironmentMapLight` on a
bare entity (scene_loader.rs end of apply_lighting_v2). Bevy 0.18 extracts view env maps only `With<Camera3d>`
(bevy_pbr light_probe/mod.rs ~L325) and probes need `LightProbe`, so IBL does nothing in all 17 projects that set it. It
means there is NO fill besides ambient: the darkening numbers above are unmitigated. docs/20 "Global environment
lighting" and README "fallback environment" claim it works. Fix = put it on the camera(s) (6+ camera spawn sites, see
[[camera-modes-v1-pattern]]) or a global LightProbe.

**Scene-scoped resource leak class:** any new scene-scoped resource must reset on Replace when unauthored (phase A did
this for GlobalAmbientLight + both shadow maps). Stale doc: schema/scene_v2.rs says point shadow "Bevy default: 512";
Bevy 0.18 is 1024 (docs/20 is right).

**lighting_validate (2026-10-23):** `SceneLightingV2::ambient_problem`/`shadow_map_size_problem`/`problems` in
schema/scene_v2.rs, shared by CLI `invalid_lighting` (hard error) and loader warn+fallback (diagnostic twin pattern,
[[diagnostic-only-feature-pattern]]). Ambient 0 and large brightness accepted; shadow 64..=8192 pow2. Point-shadow
"512" doc staleness is now fixed. **Still unchecked and a PANIC, not a cosmetic issue:** `CascadeShadowConfigBuilder::build`
(bevy_light-0.18.0 cascade.rs ~L96-120) asserts `num_cascades > 0`, `maximum_distance > minimum_distance (0.1)`,
`overlap_proportion in [0,1)` -- so RON `num_cascades: 0`, `shadow_distance: <=0.1/NaN`, `cascade_overlap: 1.0` crash the
app, and docs/20 + schema doc say overlap range "0.0–1.0" (1.0 documented-valid but panics). Bevy default max distance is
150 (schema doc's "~1000 native/~100 WebGL" is stale). Non-panicking unclamped: directional/point intensity, range, radius,
global_environment.intensity. Overlay-only scenes: validate has an overlay-only scene detector
(`overlay_scene_bindings_ignored`, validate.rs strict section) that lighting could reuse.

**Unit wording:** Bevy ambient brightness is cd/m^2 (a colour multiplier), not lux.

**How to apply:** any future lighting/day-night/EnvState change touches the same resource; check reset-on-Replace,
brightness-without-colour handling, re-baseline scope, and whether a lighting component is on an entity Bevy actually
reads (camera / LightProbe / resource). Related: [[web-host-page-contract]].
