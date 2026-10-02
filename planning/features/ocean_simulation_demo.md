# Feature: Ocean Simulation Demo (Gerstner ocean, wind, buoyancy, boat, live parameters)

_Status: Draft_
_Planned at: `0eea7ab` (2026-10-02)_

Backlog: `### Ocean Simulation Demo` (8 items, requested 2026-09-30). Related, **referenced not absorbed**: `gameplay_fixed_tick_pipeline.md` (`SimTick`,
tick layout), `gameplay_pipeline_system_sets.md` (D3 `GameplaySet`/`EmitSet`), `day_night_cycle.md` (sun/ambient ownership), `lock_on_camera_mode.md`
(v1 `Follow.offset_space: Target` is the natural boat chase camera). Written autonomously while Frank was away; not plan-reviewed. **Nothing was
compiled or run** - every code claim is a file:line reading at `0eea7ab`; all numeric defaults and perf figures are starting points to measure.

## Phases

| Phase | Backlog item | Size | Status | Completed |
|---|---|---|---|---|
| v1 | Ocean surface: scene `ocean:` block, engine-owned `OceanMaterial` (vertex Gerstner), grid mesh, accumulated-phase clock | L | Queued | - |
| v2 | Wave height sampling API: pure-Rust `gerstner` mirror, `OceanSampler`, parity/self-consistency tests, debug probe | M | Queued | - |
| v3 | Wind field: `wind:` block, `Wind` resource, `SetWind`, `wind.gust`, optional wind-to-wave coupling (off) | M | Queued | - |
| v4 | Buoyancy / floating body: `PrefabDef.buoyant`, force/torque from per-point depth in `FixedUpdate` | L | Queued | - |
| v5 | Boat controller: `PrefabDef.boat`, throttle/rudder/sail, `SetBoatInput`, boat-as-player | L | Queued | - |
| v6 | Environment control: scalar `SetParam` registry, sun/ambient/fog (fog schema is new), presets | M-L | Queued | - |
| v7 | In-scene tweak panel: `Slider` UI node + live readouts (needs the UI pointer-capture fix first) | M | Queued | - |
| v8 | `ocean_demo` project: scenes, presets, scientific scene, composite-primitive boat, registration | M | Queued | - |

**Dependencies and independence.** v1 ships alone (a living sea for any scene). v2 needs v1 and is useful alone (bobbers, fish-spawn-on-surface, readouts). v3 is
independent of v1/v2 code (only its optional coupling touches the ocean) and can be built in a parallel worktree. v4 needs v2; v5 needs v4 + v3; v6 needs v1 + v3
(and registers keys for v4/v5 if they have shipped; if v6 lands first, v4/v5 each add their own keys); v7 needs the pointer-capture bug fixed, and is only *useful* with v6
(the widget itself is independent of the ocean); v8 needs all. Parallelism: {v1 -> v2 -> v4 -> v5}, {v3}, {v7 widget} can run in separate worktrees; **never compile in
two at once** (shared target dir, root `CLAUDE.md`). Merge hot spots: `schema/actions.rs`, `action_executor.rs`, `action_substitution.rs`, `lib.rs`, `scene_loader.rs`, `scene_v2.rs`, docs.

## What
A designer authors an ocean, wind, a floating boat and the lighting entirely in RON, and can change every value live (action, slider, or script) with numeric readouts -
for scene construction or simple, reproducible scientific modelling. Engine additions: an ocean surface drawn by vertex displacement (summed Gerstner waves, up to 8),
a CPU mirror of the same math for gameplay queries, a wind resource, buoyant rigid bodies, a boat controller, a scalar parameter registry (`SetParam`), sun/ambient/fog
control, and a slider widget. Nothing needs HDR, post-processing or multi-pass rendering.

## Why
The engine has no water that moves and no way for gameplay to know where the water is; `custom_water_stylized.wgsl` is fragment-only ("no vertex displacement", its own
header) so it cannot bob a body or silhouette a wave. The demo doubles as the first consumer of engine-owned vertex shaders, of per-tick Rapier *forces* (everything so far
sets `Velocity`/`ExternalImpulse`), and of a live-tunable parameter surface that the editor/inspector work (`live_project_editor.md`) will want anyway.
**Reflections stay out:** the parked "Water / reflective plane" item (`backlog.md:139`) is WASM-BLOCKED because it needs a reflection pass or screen-space sampling. Gerstner
needs neither - one forward pass, displacement computed per vertex. The result is *not* mirror water; it is shaded (fresnel-tinted sky colour, sun specular, foam) only.
That item stays Icebox-parked and is only partly superseded.

## Findings (verified against code at `0eea7ab`)
1. **`CustomMaterial` cannot do vertex displacement**: "overrides the fragment shader only ... do not attempt to swap the vertex shader via `specialize()`"
   (`crates/ironhold_core/src/CLAUDE.md:718`). So the backlog's "or a `CustomMaterial` WGSL variant" is ruled out; the ocean is an **engine-owned material** like
   `TerrainMaterial` (`terrain_material.rs:8-40`, shader `include_str!` at `terrain.rs:217-223`, handle via `uuid_handle!` `terrain_material.rs:8`) and `FoliageMaterial`, the
   latter being the precedent for a custom *vertex* stage (`foliage.wgsl` `@vertex`, `foliage.rs:16`) and for a per-frame sun uniform sync (`foliage.rs:183-196`).
   Engine-owned shaders must be embedded, never path strings (`CLAUDE.md:722-725`). Mesh/world matrix: `bevy_pbr::mesh_functions::get_world_from_local` as in foliage.
2. **Uniform alignment**: Vec4-only fields, no bare `f32`/`Vec2`/`Vec3` (`CLAUDE.md:705-708`; `TerrainMaterial.uv_scale` is a padded Vec4). `[Vec4; N]` arrays have stride 16
   and are fine; the Foliage struct-uniform pattern (`FoliageMaterialParams`, 5 Vec4) is the template.
3. **Pipeline warmup**: `pipeline_warmup_system` adds `NoFrustumCulling` to every `Mesh3d` for 4 frames after scene load (`CLAUDE.md:1034`); the ocean grid is a normal `Mesh3d`
   so its pipeline compiles in warmup. It also needs permanent `NoFrustumCulling` (displaced vertices leave the flat-grid AABB). Warmup does not touch `RenderLayers` (`:432`).
4. **No runtime setter exists** for sun/ambient/fog. `schema/actions.rs` has no light/fog/ocean/wind action (variant list read in full). Lights are spawned once from
   `SceneLightingV2` (`scene_loader.rs:2885-2921`: an `AmbientLight` entity only when `lighting.ambient` is set, one `DirectionalLight`; fields `scene_v2.rs:193-228`).
   **Fog does not exist at all**: `grep DistanceFog|FogSettings` over `src/` finds nothing, and `SceneLightingV2` has no fog field. `foliage_lighting_sync_system` already queries
   `With<DirectionalLight>` each frame (`foliage.rs:183`).
5. **No slider/text-input UI node.** `UiNodeDef` = Button, IconButton, Label, Rect, StatBar, StatSpread, StatRadar, ActionBar, DialoguePanel, Inventory/Shop/ContainerPanel
   (`scene_v2.rs:360-374`); the `### UI` backlog section has none (`ProgressBar`/`Panel` only). **Readouts are free**: `Label.bind` + `format` mirrors `GameVariables` every frame
   (`scene_v2.rs:596-603`, `lib.rs:72`, `GameVariables(HashMap<String,String>)`). Drag mechanics precedent: `draggable_windows.md` (cursor via `Window::cursor_position`, `targeting.rs`).
6. **Pointer capture bug is real and blocks sliders**: `backlog.md:21` - left mouse on any UI node still orbits the camera / strafes (`camera_orbit_system`,
   `input_translator_system` `input.rs:286` read raw `ButtonInput<MouseButton>`); candidate fix is a shared `UiPointerCaptured`. Dragging a slider would spin the camera.
7. **Physics**: Rapier steps in `FixedUpdate` at 64 Hz `TimestepMode::Fixed` (`physics.rs:12,40-63`); gameplay chain is `.before(PhysicsSet::SyncBackend)` (`lib.rs:312-322`).
   `ExternalForce` (vendored `bevy_rapier3d-0.33.0 dynamics/rigid_body.rs:310-343`) is *persistent* ("applied at each timestep"), unlike `ExternalImpulse`; so forces must be
   **overwritten every tick, including to zero**. `ExternalForce::at_point(force, point, com)` yields the torque. Dynamic bodies already get `Velocity`/`Damping`/`ExternalImpulse`
   (`entity_spawner.rs:316-327`, `:1121-1130`). A primitive prefab with `physics: true` is inserted as `RigidBody::Fixed` (`entity_spawner.rs:250`; race with `Dynamic` noted at `:1041`),
   so `buoyant` must insert `RigidBody::Dynamic` **after** that and win (test).
8. **Determinism rules**: transcendentals go through `libm` via `det_math` (`det_math.rs`: `sin`, `sin_cos`, `acos`, quat helpers; `libm = "0.2"` in `Cargo.toml:27`); the file
   states the residual that `glam` `Quat`/`Affine3A` ops are not bit-identical native vs WASM. There is **no `SimTick`** yet (grep). Plain `f32::sqrt` is IEEE-exact. Tests run exactly one
   tick per `app.update()` (`tests/support/mod.rs:79`, per the fixed-tick plan finding 6). `ExternalForce` writes must not iterate a `HashMap`.
9. **Input/intent layer**: movement input is typed `InputActionMessage{entity, InputAction::{Move,Turn,Look,Jump,Run}}` produced by `input_translator_system` for entities with a
   `CharacterController` (`messages.rs:31-43`, `input.rs:286-302`) - there is no `intent.move.*` event stream, only `intent.slot.*` (`action_bar.rs:260`). The backlog's `intent.boat.*` is
   therefore shorthand; the matching pattern is a typed boat input, see v5.
10. **Camera is spawned per player entity** (`CameraTargets(vec![player_entity])`, `entity_spawner.rs:1388,1604,1625`); the player path builds a capsule + `CharacterController` +
    `LockedAxes::ROTATION_LOCKED` (`entity_spawner.rs:1095-1130`). A boat that must be camera-followed and selectable as "the player" cannot reuse that path as-is (v5 risk).
11. **Action conventions**: `Action` is `deny_unknown_fields` (`actions.rs:5`); `rewrite_self`/`rewrite_target` are exhaustive with no wildcard (`action_substitution.rs:13,149`);
    `action_needs_target` (`action_bar.rs:407`), dialogue `substitute_self_in_action` (`dialogue.rs:320`, has a wildcard - check), CLI `query.rs` exhaustive name match (`:540`).
    **`ron_lint` forbids `Some(` in any project RON** (`tests/ron_lint.rs`) - `implicit_some` is on, so all examples below use `field: value`.
12. **No boat asset exists**: `assets/shared/models/props` (119 GLBs) has logs/firewood but no boat/ship/raft. The boat must be a composite primitive prefab
    (`ChildPrimitiveDef` cuboid/cone/cylinder children + `PrefabDef.colliders`, `catalog.rs:778,817,1608`), with an optional GLB hull swap later.

## Decisions (read first)
- **D-1 Engine-owned `OceanMaterial`, scene-level singleton `ocean:` block** (like `terrain:`, `scene_v2.rs:51-52`), not a `PrefabKind` (avoids touching every exhaustive `PrefabKind` match;
  one ocean per scene; the CPU sampler needs exactly one authority). Entities spawn with `LevelEntity` so scene unload cleans up.
- **D-2 The ocean's time is an accumulated per-wave phase, not `t`**. Each tick (FixedUpdate, constant `dt = 1/FIXED_TICK_RATE`) `phase_i += omega_i * dt * time_scale` in **f64**, wrapped
  mod 2pi, stored in `Ocean.phase[8]`. Shader and CPU both receive `phase_i` (cast to f32) and compute `theta_i = k_i (D_i . p) - phase_i`. Why: (a) no f32 time precision loss on long runs,
  (b) **changing a wavelength/speed live (`SetParam`, sliders, coupling) never teleports the waves** - the phase is continuous, (c) CPU/GPU parity depends only on shared inputs, (d) one
  clock for both = no wall-clock anywhere; `time_scale: 0` freezes the sea for screenshots/experiments. When `SimTick` lands it only stamps; this accumulator stays.
- **D-3 Determinism stance**: simulation-visible state (phases, wind, boat/buoyancy forces, readouts) is a pure function of authored params, parameter changes and tick count: `libm` via
  `det_math`, f64 accumulation, fixed iteration counts, `BTreeMap`/`Vec` only, no `rand`, no `Entity`-order tie-breaks. **Rendering is allowed to differ** (GPU `sin` ulp differences). The
  honest claim for the "scientific" scene is *reproducible on one machine and, up to the known Rapier/`Quat` residual (`det_math.rs` header), across machines* given the same parameter-change
  ticks. Until the fixed-tick plan moves the executor and delayed events onto the tick, parameter changes land on a frame-rate-dependent tick - documented limitation, not hidden.
- **D-4 Visual vs sim time**: the surface is drawn at the **last tick's phase, no overstep extrapolation**, matching the "accept aliasing, no transform interpolation" decision for bodies
  (`CLAUDE.md` Physics section). Boat and water stay mutually consistent (no floating-above-water jitter); cost is 64 Hz stepping of wave motion, 4 double-tick frames/s at 60 Hz. Revisit
  only if the playtest shows visible judder (then interpolate *both*).
- **D-5 One generic `SetParam { key, value }` instead of ~25 typed `Set*` actions** (backlog lists `SetOceanWave/SetSunDirection/...`). Scalar dotted keys (`ocean.wave.2.amplitude`,
  `sun.yaw_deg`, `wind.speed_mps`, `boat:{self}.thrust_max`) resolved by one `ParamKey::parse` shared by executor, sliders, presets and `ironhold_cli validate`. Each new typed action costs
  ~6 exhaustive-match edits (finding 11) and a CLI arm; a vec3 maps to scalar keys anyway because sliders are scalar. `SetWind` stays a typed action (backlog names it; it is also the one used
  before v6 exists). **Open question 1** if Frank prefers the typed set.
- **D-6 Sequencing before this batch**: **D2 (HashMap lint) must land first** - cheap, and otherwise every new file needs retrofits; **D1** only matters for `GameVariables`-style maps these
  phases do not iterate. **Fixed-tick Phase 1 (`SimTick` + timers) should land before v8's scientific scene** (delayed-event scripted experiments), not before v1-v7. D3 sets are *not* required
  (the sim set below is independent), but v7's slider system must be registered inside D3's `EmitSet::DirectActions` if D3 has shipped. The cross-platform harness is not a prerequisite;
  its tick-hash should later include `Ocean.phase` + boat transforms (one line in its Phase 4 hook).
- **D-7 Boat-as-player in v5, no mount/dismount**: the boat prefab carries `tags: ["player"]` so camera, targeting and `{player}` plumbing come for free; a character standing on a moving deck
  is out of scope (needs moving-platform support; see Out of scope).

## Approach

### v1 - Ocean surface
Schema (`schema/ocean.rs`, new; `GameSceneV2.ocean: Option<OceanDef>`, `deny_unknown_fields`, no `schema_version` bump: optional field):
```ron
ocean: (
    size: (400.0, 400.0), resolution: (192, 192),    // grid cells; validate: <= 256x256 (error above 320)
    sea_level: 0.0, follow: "player_boat",           // follow: optional spawn id; grid snaps to whole cells (pattern is world-space, no swimming)
    gravity: 9.81, time_scale: 1.0, start_phase_secs: 0.0,
    waves: [                                         // 1..=8; angle: 0 = -Z, 90 = +X, clockwise from above ("compass")
        (amplitude: 0.6, wavelength: 28.0, direction_deg: 20.0, steepness: 0.5),
        (amplitude: 0.25, wavelength: 11.0, direction_deg: 70.0, steepness: 0.4, speed: 3.0),  // speed: optional m/s, default sqrt(g/k)
    ],
    deep_color: (0.02, 0.12, 0.25), shallow_color: (0.10, 0.45, 0.55), color_depth_range: 3.0,
    foam: (threshold: 0.35, color: (0.95, 0.97, 1.0)), specular: (strength: 0.8, shininess: 96.0),
),
```
- `Q_i = steepness_i / (k_i * A_i * N)` (steepness 0..1 is the fraction of the loop-free limit; engine derives `Q`, so any authored value is loop-safe). Wave count `N` and per-wave constants are
  re-derived only when params change (`Ocean` resource holds `OceanParams` + `phase`; `Changed<Ocean>` guard).
- Math, shared verbatim by `ocean.wgsl` and `gerstner.rs`: `theta = k (D.p) - phase`; `p' = p + (sum Q A Dx cos(theta), sum A sin(theta), sum Q A Dz cos(theta))`;
  normal `n = normalize(-sum Dx k A cos(theta), 1 - sum Q k A sin(theta), -sum Dz k A cos(theta))`; foam factor from the `1 - sum Q k A sin(theta)` term (crest compression) against `foam.threshold` -
  a per-vertex scalar, no post-process. The vertex stage works from the **world-space** XZ of the undisplaced vertex, so the entity transform only supplies `sea_level`.
- `OceanMaterial` (`capabilities/ocean/material.rs`), `Material` with `fragment_shader`/`vertex_shader` = `OCEAN_SHADER_HANDLE` (`uuid_handle!`), shader embedded in a `Startup` system exactly like
  `setup_terrain_shader`; `enable_prepass() -> false`, `enable_shadows() -> false` (vendored `bevy_pbr-0.18.0 material.rs:185,191`; a prepass would not see the displacement) - verify no camera feature requires a prepass.
  Uniform (all Vec4, ~21 Vec4 = ~336 B): `waves_a: [Vec4; 8]` (dir.x, dir.z, k, A), `waves_b: [Vec4; 8]` (Q, omega, phase, active), `deep`, `shallow`, `foam` (rgb, threshold), `spec`
  (strength, shininess, color_depth_range, count), `sun_dir`, `sun_color`, `fog` (rgb, start/density packed in a second Vec4). Mesh has position/normal/uv only (normals come from the shader).
- Systems (Update, cosmetic, render rate): `ocean_spawn` (in `scene_loader` next to the terrain arm), `ocean_material_sync_system` (writes uniforms from `Ocean`, sun from the `DirectionalLight`
  like `foliage_lighting_sync_system`), `ocean_follow_system` (snap; reads the followed body with `utils::fresh_global_transform`, `utils.rs:44`, per the stale-transform rule). The clock is v2's
  FixedUpdate system but **v1 must ship it** (so a v1-only scene animates): `ocean_clock_system` in the sim set (see Schedule). Phase 1 includes it; v2 adds only the sampler.
- Perf (WASM) budget: default 192x192 = 36.9k verts / 73k tris, hard cap 256x256; <= 8 waves; per-vertex 8 sincos (acceptable on iGPU; measure with `wasm-perf-reviewer` and the diagnostics HUD);
  min wavelength validated `>= 4 * cell size` (else aliasing: CLI error). Uniform re-upload is ~340 B/frame, only on change. Pipeline: one extra material pipeline, warmed by `NoFrustumCulling` pass.
- Water `custom_water_stylized.wgsl` is untouched; docs explain when to use which.

### v2 - Wave height sampling API
- `capabilities/ocean/gerstner.rs`: **pure functions, no Bevy types** (so CLI and tests can use them): `forward(params, phases, x0, z0) -> (Vec3 displaced, Vec3 normal)` and
  `sample(params, phases, x, z) -> OceanSample { height, normal, displacement }`. Horizontal displacement means height at fixed world (x,z) needs the inverse; fixed-point iteration
  `p0 <- p - D(p0)` with a **constant 4 iterations** (no data-dependent exit - determinism and cost). All trig via `det_math::sin_cos`; add `det_math::powf` (libm) for v3.
- `OceanSampler` `SystemParam` (`Res<Ocean>`; `height_at`, `sample_at`); in FixedUpdate it sees this tick's phases (clock runs first), in Update the latest tick's - the same state the shader shows (D-4).
- **Parity**: a native test cannot execute WGSL. Strategy: (1) golden table of `forward()` outputs generated by an independent **f64 reference** in the test (different formulation: no precomputed `Q`),
  asserted within 1e-4 relative; (2) self-consistency `sample(forward(x0,z0).xz).height ~= forward(x0,z0).y` within 2e-3 across a seeded grid and all steepness values; (3) a test that parses
  `ocean.wgsl` and asserts the wave-array length constant equals `MAX_WAVES` (cheap guard against Rust/WGSL drift); (4) **manual GPU parity check in the web playtest**: `debug_probe: true` on `ocean:`
  draws a small emissive sphere at `sample_at(probe.x, probe.z)`; if it rides the crest on screen, CPU and shader agree. Never claim GPU bit-parity.
- Events/actions: none required. Optional `ironhold query ocean <project> --sample x,z --tick N` printing height/normal for a scene (cheap, supports the scientific use, shares `gerstner.rs`).

### v3 - Wind field
```ron
wind: (
    direction_deg: 270.0,                 // direction the wind blows TOWARD, compass (0 = -Z, 90 = +X); document loudly
    speed_mps: 8.0,                       // or beaufort: 5 (exactly one; table of mid-band speeds, validated)
    gust: (amplitude_mps: 3.0, period_secs: 7.0, seed: 42, event_threshold: 0.8),
    drives_waves: false,                  // optional coupling, OFF by default
    coupling: (reference_speed_mps: 8.0, amplitude_exponent: 2.0, wavelength_exponent: 2.0, align_direction: false, response_secs: 4.0),
),
```
- `Wind` resource `{direction, base_speed, gust_factor, speed, vec: Vec2}`; ticked by `wind_tick_system` in the sim set. Gust = normalised sum of three sines (periods `P, P/phi, P/phi^2`, phases from a
  seeded integer hash of `seed`, **no `rand`**), via `det_math::sin`. `wind.gust` is emitted as `GameEvent::Trigger("wind.gust")` on a rising crossing of `gust_factor >= event_threshold`
  (re-armed below `threshold - 0.1`); it is a `GameEvent` writer in FixedUpdate, ordered by the sim set's explicit chain (D-3), so it joins the stream deterministically.
- `Action::SetWind { direction_deg: Option<f32>, speed_mps: Option<f32> }` (instant; ramping is `response_secs`-style smoothing on the *effective* value only when asked - Open question 6).
- Coupling (v3 builds it, v1's `Ocean` exposes `effective_waves()`): effective amplitude/wavelength = authored x `(U/U_ref)^exponent` smoothed by a deterministic first-order lag; direction optionally
  aligned to wind. Authored values are never mutated, so with `drives_waves: false` every wave variable stays independently controllable (backlog requirement). It is a tunable scaling law, **not**
  a fetch-limited spectrum, and the docs say so. Wind also feeds particles/foliage later (resource is public).

### v4 - Buoyancy / floating body
```ron
"dinghy": (kind: Primitive, shape: Cuboid, primitive: (size: (1.6, 0.4, 4.0), color: (0.6, 0.4, 0.2), physics: true),
    buoyant: (mass_kg: 220.0, points: [(-0.7,-0.2,1.8), (0.7,-0.2,1.8), (-0.7,-0.2,-1.8), (0.7,-0.2,-1.8), (0.0,-0.2,0.0)],
              lift_ratio: 2.0, full_depth: 0.5, water_drag: 80.0, angular_drag: 60.0, air_drag: 2.0, center_of_mass_offset: (0.0,-0.15,0.0))),
```
- `PrefabDef.buoyant: Option<BuoyantDef>` (sibling of `motion`/`interactable`, `catalog.rs:781-795`). Spawn inserts `RigidBody::Dynamic`, `AdditionalMassProperties::Mass(mass_kg)`, `ExternalForce::default()`,
  `Velocity`, `Damping` and a `Buoyant` component; replaces the `Fixed` a `physics: true` primitive got (Finding 7). Missing `colliders`/primitive collider = validate error.
- `buoyancy_system` (FixedUpdate, sim set, after `ocean_clock`): for each `Buoyant` **sorted by `SpawnId`** (D4 corollary; never query order), for each local point (fixed `Vec` order): world point from the root
  `Transform` (the body is a root entity, so the stale-transform hazard does not apply; assert `Without<ChildOf>` in the query), `d = water_height - p.y`; `frac = clamp(d / full_depth, 0, 1)`;
  point force `= up * (lift_ratio * mass * g / n_points) * frac` along the **wave normal** blend (use world up in v4; normal-tilted lift is a tuning knob, Open question 4) minus `water_drag * frac * v_point`
  (`v_point = linvel + angvel x r`) - linear drag keeps the explicit integrator stable; plus `air_drag` when `frac == 0`. Forces/torques summed into one `ExternalForce` (via `at_point`), **overwritten every tick**.
  Angular drag = `-angular_drag * mean_frac * angvel` torque. Equilibrium is mean submerged fraction `1/lift_ratio` (author intuition: `2.0` = floats half-sunk).
- Stability: explicit force integration at dt = 1/64 s blows up when `dt * sqrt(k/m_pt) > ~1.5` (k = per-point force / `full_depth`). CLI `validate` computes that and the critical damping `2 sqrt(k m_pt)` and **warns**
  when `water_drag < 0.3 * critical` or the stiffness bound is exceeded. Playtest tunes defaults; every coefficient is a RON field and a v6 param key.
- Simplifications documented: drag acts on absolute velocity (no wave orbital velocity); no added mass, no slamming, no wave-drift force. No interaction with `player_movement`/NPC controllers: a `Buoyant` body never has
  `CharacterController`/`NpcAgent` (validate error); GLB hulls work the same (points are local space) but need an authored `colliders`/auto-collider. Sample cap: soft warn at **> 48 points/tick** total.
- Perf: per point `4 iterations x N waves` sincos; 8 points x 4 x 8 = 256 `sincosf`/tick (~tens of microseconds native; **unmeasured on WASM** - `wasm-perf-reviewer` + Tracy check, target < 0.3 ms/tick for one boat).

### v5 - Boat controller
```ron
boat: (thrust_max: 1800.0, thrust_point: (0.0,-0.2,2.0), rudder_authority: 900.0, rudder_min_speed: 0.5, keel_drag: 400.0, reverse_factor: 0.4,
       sail: (area: 6.0, height: 2.5, trim_deg: 30.0, trim_min_deg: 0.0, trim_max_deg: 90.0, lift_slope: 1.0, drag_base: 0.05, drag_k: 1.2, max_force: 2500.0, air_density: 1.2),
       input: (throttle_up: "W", throttle_down: "S", rudder_left: "A", rudder_right: "D", trim_in: "E", trim_out: "Q",
               gamepad_throttle_axis: "RightStickY", gamepad_rudder_axis: "LeftStickX")),
```
- `PrefabDef.boat: Option<BoatDef>`, **requires `buoyant`** (validate). `BoatInput { throttle (-1..1), rudder (-1..1), sail_trim_deg }` is a *level* component (not an edge), so the 0-/2-tick latch problem
  (fixed-tick plan finding 4) does not apply. Writers: `boat_input_system` (FixedUpdate; keyboard/gamepad per `input`, reusing the `BoundGamepad` pattern; absent `input` = script-only) and
  `Action::SetBoatInput { entity: String, throttle: Option<f32>, rudder: Option<f32>, sail_trim_deg: Option<f32> }` (RON/AI; `entity` is an address field: `{self}`/`{target}` substitution arms, `action_needs_target` arm, dialogue arm).
  This is the typed analogue of `InputActionMessage`; if v5 instead extends `InputAction` with `Throttle/Rudder/Trim`, the translator must stop requiring `CharacterController` - rejected as larger.
- `boat_forces_system` (after `buoyancy_system`, adds into the same `ExternalForce` so it is single-writer per tick: buoyancy zeroes+writes, boat *adds* - one system pair, explicit order): thrust along heading applied
  **at `thrust_point`** only while the thrust point is submerged (prop out of water = no thrust); rudder torque `rudder * rudder_authority * clamp(v_forward / rudder_min_speed, 0, 1)` about world Y; keel force `-keel_drag * v_lateral`.
  **Sail** (flat-plate model, no trig): apparent wind `w = wind.vec - v_boat.xz`; chord `c = rotate(heading, trim)` via `det_math::sin_cos`; `sin_a = cross(c, w_hat)`, `cos_a = dot(c, w_hat)`;
  `CL = lift_slope * 2 sin_a cos_a`, `CD = drag_base + drag_k sin_a^2`; `q = 0.5 * air_density * |w|^2 * area`; drag along `w_hat`, lift perpendicular on the side the sail is pushed to; clamped to `max_force`; applied at the sail centre
  (`height` above the origin) so heel torque falls out. Wind from v3 (`Wind` missing = zero wind, warn once).
- Boat-as-player (D-7): prefab `tags: ["player"]` + `boat` => spawner takes a **vessel path**: no `CharacterController`, no capsule, no `LockedAxes`; still creates `PlayerTarget`/`BoundGamepad`/camera
  (`entity_spawner.rs:1388`, `:1604`). **Spike first** (Task v5-1): the player path is ~100 lines of controller-specific inserts; if it cannot be split cleanly, fall back to an invisible player-anchor entity that
  the camera follows. Chase camera: Orbit works today; `Follow` with `offset_space: Target` (lock-on plan v1) is the better boat cam if shipped. Not required.
- Readout variables `boat.speed_mps`, `boat.heading_deg`, `boat.apparent_wind_mps`, `boat.height_at_hull` are published to `GameVariables` (render-rate system, only on change, cosmetic) for `Label.bind`.

### v6 - Environment control + parameter registry
- `capabilities/env_params.rs`: `ParamKey::parse(&str) -> Result<ParamKey, ParamError>` (pure; used by the executor, sliders, presets, CLI) and `get/set` through a `SystemParam` bundle over `Ocean`, `Wind`,
  lights, fog and `Boat/Buoyant` by spawn id. Keys: `ocean.{sea_level,time_scale,gravity,foam.threshold,spec.strength,...}`, `ocean.wave.{i}.{amplitude,wavelength,direction_deg,steepness,speed}`, `wind.{speed_mps,direction_deg,gust_amplitude_mps,gust_period_secs}`,
  `sun.{yaw_deg,pitch_deg,intensity,color.r|g|b}`, `ambient.{brightness,color.r|g|b}`, `fog.{start,end,density,color.r|g|b}`, `boat:<id>.{thrust_max,rudder_authority,...}`, `buoyant:<id>.{lift_ratio,water_drag,...}`.
  Values are clamped to the same ranges `validate` enforces (an out-of-range `SetParam` warns and clamps, never panics). Unknown key = `warn!` + no-op at runtime, **error in `validate`**.
- Actions: `SetParam { key: String, value: f32 }` (`{self}` allowed in `key`: arms in `rewrite_self`, `rewrite_target`, `substitute_self_in_action`, `action_needs_target`) and
  `ApplyParamPreset(String)`. Presets live on the scene: `param_presets: { "storm": { "wind.speed_mps": 18.0, "ocean.wave.0.amplitude": 1.8, "sun.intensity": 3000.0 }, ... }` (`BTreeMap`, applied in key order for determinism).
  `rewrite_self` for `ApplyParamPreset` is identity (no entity field) - listed explicitly, no wildcard.
- **Fog (new schema)**: `lighting.fog: (color: (0.6,0.7,0.8), mode: Linear(start: 40.0, end: 400.0))` (also `Exponential(density: 0.01)`); a system adds Bevy `DistanceFog` to every `Camera3d` from a `SceneFog`
  resource (forward-pass, no HDR, WebGPU-safe). `OceanMaterial` does **not** inherit PBR fog: it applies the same fog in its fragment stage from the `fog` uniform (task: verify `bevy_pbr::fog` import is usable
  from a custom fragment; else lerp manually). Optional `match_clear_color: true` sets the camera clear colour to the fog colour for a clean horizon (Open question 5).
- **Day/night coexistence**: `day_night_cycle.md` (Draft, `Update`, rewrites `DirectionalLight`/`AmbientLight` each frame) would overwrite manual values. Contract introduced here: resource `EnvOverrides { sun_color, sun_intensity, sun_dir, ambient_color, ambient_brightness, fog }`
  (bools). Any `SetParam sun.*`/`ambient.*` sets the matching flag; `ReleaseParamOverride(group)` action clears it; the day/night system must skip flagged channels (**manual override wins while set**, backlog). That plan needs a one-line amendment
  when accepted; no day/night code exists yet, so v6 only ships the resource + docs. `SetParam` with no light in the scene (`lighting.directional: None`) spawns nothing and warns.
- Missing light handling: `SetParam ambient.*` when no `AmbientLight` entity exists inserts one (the loader only spawns it if `lighting.ambient` is set, `scene_loader.rs:2884`).

### v7 - Slider panel
```ron
Slider((id: "wave0_amp", param: "ocean.wave.0.amplitude", min: 0.0, max: 2.0, step: 0.05, position: (20.0, 60.0), size: (240.0, 20.0),
        label: "Wave 0 amplitude: {}", decimals: 2)),
```
- `UiNodeDef::Slider(SliderDef)` (new arm in every exhaustive `UiNodeDef` match: `id()`, `size()`, loader, CLI). Built from a `Rect` track + knob; value shown through a bound `Label` string. **Not** a parallel inspector: the
  inspector (`inspector.rs`) is a dev tool behind a feature flag; this is an in-game, RON-authored widget.
- Drag: press on track sets `SliderDrag(entity)`; while the left button is held the value follows `window.cursor_position()` against the node's `ComputedNode` rect even outside the node (draggable-windows pattern);
  quantised to `step`; emitted at most once per frame and only when changed. **Prerequisite: land the `UiPointerCaptured` fix (`backlog.md:21`) first**, and make the slider press set it, else dragging orbits the camera.
- Pipeline conformance: the widget is a *capture* system (render-rate, `Interaction`/cursor). It sets `GameVariables["slider.<id>"]`, writes `UiEvent` `ui.slider_changed:<id>` (RON-bindable), and pushes
  `Action::SetParam{key: param, value}`. Pushing to `ActionQueue` from a capability is normally forbidden; this joins D3's explicit **direct pusher** group (`EmitSet::DirectActions`, alongside `despawn_timer_system`) so its
  order is fixed. If D3 has not shipped, register it `.before(fsm_interpreter_system)` like `interactable_system` (`lib.rs`). Alternative considered: interpreter `{value}` substitution (new generic mechanism) - rejected as larger.
- Slider reads its initial/current value from the registry `get` so presets and scripted `SetParam` move the knob (value sync system, change-guarded). `ExportParams` action writes current values as RON text to a
  `GameVariables` key and the log (no filesystem on WASM; user copies from a Label/console) - optional, last task of the phase.
- Layout: manual `position` per slider (no flex layout yet; `ui_flex_group.md` is Draft) - the demo scene will have a long, hand-positioned column; a `SliderGroup` helper is explicitly **not** in scope.

### v8 - `ocean_demo` project
Layout `assets/projects/ocean_demo/{ocean_demo.project.ron, assets.ron, scenes/{open_sea,calm,choppy,storm,sunset,scientific}.scene.ron, logic/state_machine.ron, prefabs/prefabs.ron, stats?}`.
- Boat: composite primitive `kind: Primitive` prefab (hull cuboid, bow cone, mast cylinder, sail thin cuboid, all `children:`), compound `colliders`, `buoyant` + `boat`; materials in `assets.ron`. GLB replacement later.
- Presets = `param_presets` + `ApplyParamPreset`; scene buttons and `scene_key_bindings` (1-4) fire them; `sunset` also sets `sun.*`, `fog.*`. Sliders panel (toggle with a key) for every wave, wind, sun, ambient, fog and boat key.
- `scientific.scene.ron`: `time_scale: 1.0`, fixed `wind.seed`, no gusts unless authored, a fixed `start_phase_secs`, a stationary reference boat (or none), a fixed camera, and `Label.bind` readouts: wave height at the boat, wind speed, apparent wind,
  boat speed, ocean clock phase. Header in the scene documents *exactly* what is reproducible (D-3).
- Registration steps (root `CLAUDE.md` "Adding a new asset project"): append `ocean_demo` to `test_web.py` `PROJECTS` (`test_web.py:51`); `python test_web.py --project ocean_demo --update-baselines --skip-build`
  (baseline per scene; **exclude** scenes whose water pose never converges - the screenshot settles after 120 frames - via `NON_DETERMINISTIC_SCENES` or freeze with `time_scale: 0` in a screenshot variant); `index.html` card;
  `ocean_demo/CLAUDE.md`/README per the other demos. Add the project to `tests/ron_validation.rs` round-trips.

## Schedule placement
New FixedUpdate system set `OceanSimSet`, chained, `.after(bevy::transform::systems::mark_dirty_trees)` (end of the existing movement chain, `lib.rs:312-322`) and `.before(PhysicsSet::SyncBackend)`:
`ocean_clock_system -> wind_tick_system -> boat_input_system -> buoyancy_system -> boat_forces_system`. Both forces write `ExternalForce` this tick, Rapier consumes them in the same tick's step. Under the fixed-tick plan Phase 3 this set becomes
**tick-layout step 5** (after `motion_system`, before Rapier) with no code change beyond its ordering edge; it does not use `Update`-written messages. Update (render rate, cosmetic, no sim state): `ocean_material_sync`, `ocean_follow`,
`env_fog_sync`, `readout_publish`, `slider_*` capture/sync. `SetParam`/`SetWind`/`SetBoatInput` are executor arms (`GameplaySet::Execute`); they write the same resources/components the sim set reads next tick. Wind `GameEvent` writer is inside
the chain, hence deterministic relative to `collectible`/`trigger_zone`/`npc_behavior`. Never `.before/.after(stat_effective_value_system)` (D3 rule).

## Out of scope (explicit)
Reflections/refraction/screen-space effects, depth-based shore blending, foam via post-process or particle foam trails, FFT/spectral oceans, wave-wave interaction, wake simulation, underwater rendering, buoyancy for terrain-intersecting shores, a character walking on a moving deck / mount-dismount,
multiple oceans per scene, GPU readback, replay recording of slider drags beyond the `SetParam` action stream, a sky/atmosphere model, a `SliderGroup`/layout system, ORDS-style export of params to a file.

## Tasks
**v1** - [ ] `schema/ocean.rs` (`OceanDef`, `WaveDef`, `FoamDef`, `SpecularDef`), `GameSceneV2.ocean` + `validate()` (wave count <= 8, steepness 0..1, wavelength >= 4*cell, resolution cap, `follow` id exists) - [ ] `capabilities/ocean/{mod,material,mesh,params}.rs`, `assets/shared/shaders/ocean.wgsl` (embedded; Vec4-only uniform) - [ ] `ocean_clock_system` (+ constants `OCEAN_MAX_WAVES`), loader arm, `LevelEntity`, `NoFrustumCulling` - [ ] verify no prepass/shadow requirement; verify web `NoFrustumCulling` warmup compiles the pipeline (no first-sight stall) - [ ] tests: schema round-trip (`ron_validation.rs`), clock phase accumulation (1 tick/update, wrap, `time_scale 0`), spawn/despawn on scene load, `assets_schema_version_regression` untouched - [ ] docs `docs/20_data_formats.md` (`ocean:` section), `docs/25_custom_shaders.md` (why not CustomMaterial), `src/CLAUDE.md` (ocean + sim set), `tests/CLAUDE.md` row - [ ] CLI: `validate` checks above, `stats` counts ocean waves/verts, `query ocean`; **`cargo check -p ironhold_cli`** - [ ] reviews (parallel): alignment, system-architect, debug-detective, ux-gamedesigner (schema+docs), **wasm-perf-reviewer** (vertex count, uniform size, warmup) - [ ] WASM dev build + playtest.
**v2** - [ ] `gerstner.rs` pure fns + `det_math::powf` - [ ] `OceanSampler` - [ ] tests (golden/f64 reference, self-consistency over steepness, WGSL `MAX_WAVES` guard, determinism: two identical runs bit-equal) - [ ] `debug_probe` field - [ ] docs + `query ocean --sample`; `cargo check -p ironhold_cli` - [ ] reviews incl. wasm-perf.
**v3** - [ ] `schema/wind.rs`, `Wind`, `wind_tick_system`, gust hash, `wind.gust`, `SetWind` (**all exhaustive arms**: `rewrite_self`, `rewrite_target`, dialogue, `action_needs_target` n/a, CLI `query.rs:540`) - [ ] coupling + off-by-default test - [ ] tests (seeded gust repeatability, event once per crossing, Beaufort table, coupling smoothing determinism) - [ ] docs `20_data_formats.md` + `30_runtime_events_and_logic.md` (`wind.gust`) - [ ] CLI validate (exclusivity of `speed_mps`/`beaufort`) - [ ] reviews.
**v4** - [ ] `BuoyantDef`, spawn path (Dynamic replaces Fixed; test the race), `buoyancy_system`, SpawnId-sorted loop, `ExternalForce` overwrite - [ ] tests (body at rest settles at predicted draft within tolerance on a flat `time_scale 0` sea, tilts on a slope, drag decays heave, forces zeroed when dry, two identical runs bit-equal, no `CharacterController` combo) - [ ] CLI stability warning + `cargo check -p ironhold_cli` - [ ] reviews incl. wasm-perf (sample count) - [ ] playtest tuning pass for defaults.
**v5** - [ ] vessel-spawn spike, `BoatDef`, `BoatInput`, systems, `SetBoatInput` (**all arms incl. `{self}`/`{target}`**) - [ ] tests (thrust only when submerged, rudder needs speed, sail force sign vs trim, script-only boat, determinism) - [ ] docs + CLI (`boat` requires `buoyant`) - [ ] reviews - [ ] playtest (keyboard + gamepad).
**v6** - [ ] `ParamKey`, registry, `SetParam`, `ApplyParamPreset`, `ReleaseParamOverride`, `param_presets`, `lighting.fog`, fog system, `EnvOverrides` - [ ] tests (every key set/get round-trip, clamping, unknown key, preset order, light override flags, fog added to cameras) - [ ] CLI validate (param keys in actions, presets, sliders) + `cargo check -p ironhold_cli` - [ ] day/night plan amendment note (Frank) - [ ] docs - [ ] reviews.
**v7** - [ ] confirm `UiPointerCaptured` fix shipped - [ ] `Slider` node + systems + events + value sync - [ ] tests (`ui_tests.rs`: drag maps cursor to value, step quantisation, outside-node drag, camera does not orbit while dragging, scripted `SetParam` moves the knob) - [ ] CLI/validate exhaustive `UiNodeDef` arms - [ ] docs - [ ] reviews (ux-gamedesigner important here).
**v8** - [ ] project files + registration + baselines + `index.html` card + `ron_validation` entry - [ ] `ironhold validate --strict` clean - [ ] `python test_web.py` (run on its own port when a manual `serve.py` is open) - [ ] reviews (game-world-designer optional) - [ ] playtest all scenes.

## Playtest checklist (per phase, abbreviated)
v1 calm sea animates, no cracks at steepness 1.0 and 8 waves, no pipeline hitch on first view (WASM), horizon and fog consistent. v2 probe sphere rides crests on screen (GPU/CPU agreement), including `steepness` 0.9. v3 `SetWind` changes sample readout; coupling off = waves unchanged.
v4 dinghy settles at the predicted draft, rolls in beam seas, does not explode at 144 fps or after a 250 ms stall (multi-tick frames). v5 W/S/A/D + gamepad, no thrust with the prop out of water, tack against wind. v6 every key reachable, sunset preset, override vs manual values.
v7 drag a slider while holding the mouse over the 3D view: camera must not orbit. v8 each preset loads; scientific scene shows identical readouts on two page loads at the same tick.

## Open questions (for Frank)
1. **Generic `SetParam` registry (D-5) vs the backlog's ~25 typed actions.** Recommendation: registry. It changes the backlog wording of item 6 and removes the typed `Set*` names - confirm.
2. **Boat as `tags: ["player"]` (D-7) vs mount/dismount a character.** Recommendation: boat-as-player now; mount/dismount is a separate feature needing moving-platform support.
3. **Land D2 first, and fixed-tick Phase 1 before v8's scientific scene?** Recommendation: yes to both (D-6); v1-v7 do not wait. Otherwise the "reproducible" claim in v8 is weakened to "same params, same sea".
4. **Lift direction**: world-up buoyancy (simple, stable) vs wave-normal-tilted lift (more lifelike, can destabilise). Recommendation: world-up, expose `normal_lift: 0..1` later.
5. **Horizon**: should `fog.color` optionally set the camera clear colour (`match_clear_color`), or do you want a real sky/background feature first?
6. **Wind/gust ramping**: instant `SetWind` only, or a `over_secs` ramp? Recommendation: instant plus the coupling's own smoothing; ramps are a Timeline-item concern.
7. **Wave direction convention** (compass: 0 = -Z, 90 = +X, clockwise) is chosen to match the engine's -Z forward; confirm, since it is applied to waves, wind and `sun.yaw_deg` and is expensive to change later.

## Acceptance criteria
- Given an `ocean:` block with N <= 8 waves, when the scene loads on native and WASM, then a displaced, shaded, fogged surface animates with no CPU mesh rebuild per frame, and the vertex count is <= 256x256.
- Given the same params and tick count, when `sample_at(x,z)` is called twice (or on two runs), then the result is bit-identical; and `sample_at` agrees with the CPU forward map within tolerance and with the on-screen probe visually.
- Given `drives_waves: false` (default), when `SetWind` is dispatched, then no wave parameter changes; with `true`, effective waves change smoothly and the authored values are untouched.
- Given a `buoyant` prefab on a flat frozen sea, when settled, then it floats at the draft implied by `lift_ratio` with `ExternalForce` zeroed when out of the water, and stays stable at 30/60/144 fps and after a 16-tick catch-up burst.
- Given a boat with `boat`, when W/D are held (or `SetBoatInput` is dispatched), then it accelerates, turns only while moving, and the sail produces force consistent with apparent wind.
- Given `SetParam` with a valid key, when dispatched or a slider is dragged, then the value changes live, is clamped, and an unknown key is a `validate` error and a runtime no-op warning; dragging a slider never orbits the camera.
- Given `ocean_demo`, when `ironhold validate --strict` and `cargo check -p ironhold_cli` run, then both are clean, all presets and the scientific scene load, and the project is registered in `test_web.py`, baselines and `index.html`.
