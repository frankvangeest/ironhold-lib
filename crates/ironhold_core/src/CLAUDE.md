# ironhold_core — Rust Source Rules

## The Message → Interpreter → Action → Executor pipeline

This is the single most important architectural rule. All game behaviour flows through this pipeline:

```
Capability emits Message  →  Interpreter matches rules  →  ActionQueue  →  Executor runs Action
```

**Stages:**
1. **Message** — a capability detects something (collision, button press, scene event) and emits a typed message: `UiEvent::ButtonPressed`, `SceneEvent::Ready`, etc.
2. **Interpreter** — `fsm_interpreter_system` (or `message_interpreter_system` for rule-file projects) reads messages and matches them against the loaded `state_machine.ron` / `rules.ron`. Matching rules push `Action` values onto `ActionQueue`.
3. **ActionQueue** — a FIFO `VecDeque<Action>`. Push order equals execution order. Exit actions are pushed before entry actions.
4. **Executor** — `action_executor_system` drains the queue and dispatches each `Action` (LoadScene, Despawn, PlaySound, SetVariable, IncrementVariable, etc.).

### Rules for new capabilities

**Physics sensors emit messages, not actions.**  
A sensor that detects player overlap must emit `GameEvent::Trigger("entity.collected:{id}")` — it must NOT push `Action::Despawn` or `Action::PlaySound` directly to `ActionQueue`. This keeps the sensor dumb and the behaviour configurable in RON.

**Choose the right message type for each source:**
| Source | Message type | Event name format |
|---|---|---|
| UI widget (button, slider) | `UiEvent::ButtonPressed(trigger)` | `"ui.button_pressed:{trigger}"` |
| Physics sensor / gameplay logic | `GameEvent::Trigger(name)` | `"{name}"` as-is (caller namespaces it) |
| Scene lifecycle | `SceneEvent::Ready/Loaded/…` | `"scene.ready:{scene}"` etc. |

**Never push to `ActionQueue` from a capability system.**  
The only code that should push to `ActionQueue` is the interpreter systems. If you find yourself adding `ResMut<ActionQueue>` to a physics or gameplay system, stop — emit a `GameEvent` or `UiEvent` instead and handle the response in `state_machine.ron`.

**Hardwired actions prevent data-driven behaviour.**  
If you push actions directly from Rust, the behaviour is locked in code. You can't add health rewards, score, extra effects, or conditions without recompiling. If you go through the pipeline, all of that can be added by editing RON.

### Adding new actions

1. Add the variant to `schema/actions.rs` with a doc comment.
2. Add a `match` arm to `action_executor_system` in `runtime/scene_manager/action_executor.rs`.
3. Add `#[derive(Deserialize)]` — it's already derived on `Action`; just ensure serde can deserialize the inner types.
4. Document the new action in this file if it has non-obvious semantics.

### RON action syntax — struct vs tuple variants

The RON syntax for an action depends on how its variant is declared in `schema/actions.rs`:

- **Struct variant** (`SpawnEffect { key, entity, position }`) → **named-field** RON:
  ```ron
  SpawnEffect(key: "hit_spark", entity: "{self}")
  ```
- **Tuple variant** (`SetVariable(String, String)`, `IncrementVariable(String, i32)`) → **positional** RON:
  ```ron
  SetVariable("score", "0")
  IncrementVariable("score", 1)
  ```

Using named fields on a tuple variant (`SetVariable(key: "score", value: "0")`) is a RON parse error. When in doubt, check the variant definition in `schema/actions.rs`.

### Conditions on rules

The only runtime condition available to the rules engine is `LogicState` — a single named string (e.g. `"playing"`, `"hp_low"`). Rules with a `when:` field only fire while the FSM is in a matching named state. To add conditions:
- Have a gameplay system call `Action::EnterState("hp_low")` when HP drops below threshold.
- Gate the conditional rule with `when: "hp_low"` in `state_machine.ron`.

Do not add a general condition system to the interpreter unless the above pattern is genuinely insufficient.

---

## Before coding

No assets should be hardcoded in the runtime. All assets should be defined in the `assets/projects/{name}/assets.ron` file. Audio catalog keys (not file paths) are passed to `Action::PlaySound`; the executor resolves the path.
When making code changes to the ironhold_core make sure we are using the code workflow properly.

---

## Composite and nested prefab spawning

`runtime/scene_manager/scene_loader.rs` contains a free function `spawn_primitive_children` that handles both **inline primitive children** and **nested prefab references** (`ChildPrimitiveDef.prefab`). It takes a `ChildSpawnCtx` struct (split-out asset refs from `SceneMaterialParams` plus the GLB-dispatch refs) and recurses into referenced prefabs via the `PrefabCatalog`.

When a nested prefab reference is resolved, the spawner dispatches on `nested_prefab.kind`:
- **`Primitive` with `children`** — spawns an anchor entity, recurses into children (existing path).
- **`Primitive` with no `children`** — spawns an anchor + one mesh child using `build_primitive_mesh` (reads `nested_prefab.shape`).
- **`Actor` / `Prop`** — calls `spawn_prefab_instance` with the resolved GLB model path; the returned entity is parented directly under the composite parent at the child `offset`/`rotation_euler_deg`/`scale`.

**`ChildSpawnCtx<'a>`** fields: `meshes`, `standard`, `built_mats`, `custom_mats`, `primitive_default_color` (material/mesh refs) plus `asset_server`, `model_spawner`, `fixes`, `asset_catalog`, `project_root` (needed for GLB dispatch).

- Cycle detection and depth limit (8 levels) are enforced inside `spawn_primitive_children`.
- Cycle detection at **load time** is in `PrefabCatalog::validate()` (DFS via `prefab_has_cycle()`).
- All child-spawning code must go through `spawn_primitive_children` — do **not** duplicate the mesh/material dispatch match arms. The two call sites are: composite non-player prefabs and player cosmetic children.
- Transform composition is **multiplicative** (standard Bevy hierarchy). Non-uniform scale on parent anchors causes shearing in rotated children — document this in RON comments when relevant.

---

## Entity FSM (per-entity behavior)

Per-entity behavior uses the same `StateMachineAsset` schema as the global FSM. Behavior files live in `assets/projects/{name}/behaviors/` by convention.

**`{self}` substitution** — in behavior files, `{self}` in any event pattern or action target string is replaced at runtime with the entity's spawn ID. This makes behavior files reusable across multiple instances of the same prefab.

**The interpreter chain** (all in `Update`, chained):
1. `message_interpreter_system` — global rules.ron
2. `fsm_interpreter_system` — global state_machine.ron
3. `entity_fsm_interpreter_system` — per-entity .behavior.ron
4. `action_executor_system`

**Never bypass the pipeline from entity behavior.** Entry/exit actions in `.behavior.ron` push to the global `ActionQueue` — they go through the same executor as all other actions. Do not add `Commands` access to the entity FSM interpreter.

**Supported `{self}` targets in actions:**
- `Despawn("{self}")` → `Despawn("entity_id")`
- `PlayAnimationOn { target: "{self}", clip: "..." }` → target becomes the entity's ID
- `EmitEvent("event:{self}")` → event name with `{self}` filled in
- `Spawn { prefab: "...", id: "{self}_child" }` → id with `{self}` filled in
- `ModifyStat(key: "{self}.health", delta: ...)` → key becomes `"entity_id.health"` (routes to StatMap)
- `SetStat(key: "{self}.mana", value: ...)` → key becomes `"entity_id.mana"`
- `ShowDamagePopup(entity: "{self}", amount: -25.0)` → entity becomes the entity's ID
- `SetEntityVisible(entity: "{self}", visible: false)` → entity becomes the entity's ID
- `EmitEventAfterDelay(event: "entity.respawned:{self}", delay_secs: 15.0)` → event name with `{self}` filled in
- `SpawnEffect(key: "hit_spark", entity: "{self}")` → entity becomes the entity's ID (burst spawns at that entity's position)
- `ResetToSpawn("{self}")` → entity ID becomes the entity's spawn ID; teleports NPC to its `NpcAgent.origin` and zeros velocity

**`{target}` substitution** — in global rules.ron, state_machine.ron, and behavior files, `{target}` in any action field is replaced with the current `CurrentTarget` spawn ID. If `CurrentTarget` is `None`, the literal `"{target}"` is left as-is (action will likely no-op gracefully). The substitution runs in all three interpreter systems before pushing to `ActionQueue`. Supported action fields: same as `{self}` above (key, entity, event, id, spawn_point).

**`target.*` events** — emitted by the targeting capability (`capabilities/targeting.rs`; set
`click_selectable: true` or `targetable: true` on `PrefabDef`). Selection is **screen-space
proximity** (project each candidate to the screen via `camera.world_to_viewport`, pick the
nearest to the cursor) — NOT mesh raycasting, which raycasts bind-pose geometry and misses
animated/skinned GLB characters. Tab-cycle is nearest-first by world distance.
- `target.clicked:{id}` / `target.changed:{id}` / `target.changed` — new target selected
- `target.cleared` — target cleared (click on empty space, `ClearTarget` action, or `LoadScene`)

The capability also writes three `GameVariables` for UI labels (bind whichever you need):
`target_display` (`"<prefab> <id>"`), `target_name` (prefab key), `target_id` (spawn id).
Entities carry a `PrefabKey` component (catalog key) alongside `SpawnId` (instance id) to
support this.

**New capabilities for entity logic:**

**Behavior on composite primitive prefabs** — the `behavior` field works on ALL prefab kinds, including `kind: Primitive` prefabs with a non-empty `children` list. Both the single-mesh primitive path and the composite (multi-child) path in `scene_loader.rs` attach `PendingBehavior`.

`TriggerZone` — set `trigger_zone: (radius: 2.0)` on a `PrefabDef`. A Rapier sphere sensor is spawned. Works on **all prefab kinds**: single-mesh primitives, composite primitives (those with `model: ""` and a non-empty `children` list), and GLB actor/prop prefabs. Emits:
- `GameEvent::Trigger("entity.entered:{id}")` on player enter
- `GameEvent::Trigger("entity.exited:{id}")` on player exit

`Interactable` — set `interactable: (radius: 2.5)` on a `PrefabDef`. No collider needed. Emits:
- `GameEvent::Trigger("entity.interacted:{id}")` when player is within `radius` metres and presses the interact key (configured via `inputs.interact` in the player prefab, default `"KeyF"`)

`interactable_system` runs in `Update` before the interpreter chain (`.before(message_interpreter_system)`). `trigger_zone_system` runs in `FixedUpdate`.

---

## WebGPU 16-byte alignment
Custom GPU-bound structs (e.g., `TerrainMaterial`) **must** use 16-byte aligned uniform buffer layouts. Violating this causes `BUFFER_BINDINGS_NOT_16_BYTE_ALIGNED` panics in web builds. Verify `AsBindGroup` mappings distinguish Uniform vs. Storage buffers per Bevy 0.18 expectations.
- Use `Vec4` (16 bytes) for all uniform fields; never bind a bare `f32`, `Vec2`, or `Vec3`.
- `CustomMaterialUniforms` (4 × Vec4 = 64 bytes) and `TerrainMaterial.uv_scale` (Vec4 padded) already comply — keep them that way.

## WGSL is the first-class shader language
All shaders in this project are authored in WGSL. WGSL is the native language of WebGPU and runs identically on desktop (wgpu) and browser (WebGPU) — zero transpilation cost and consistent output on all platforms.

**When writing or reviewing shader code:**
- Shared (reusable) shaders → `assets/shared/shaders/`, named `custom_*.wgsl`.
- Project-specific shaders → `assets/projects/{name}/shaders/`.
- All custom fragment shaders must declare the full `CustomMaterial` binding contract (see `docs/25_custom_shaders.md`). Missing bindings cause WebGPU validation errors, not panics.
- `TonyMcMapface` and `BlenderFilmic` tonemapping are excluded because they require a LUT texture. Do not add LUT-dependent shaders.
- `CustomMaterial` currently overrides the **fragment shader only**. Vertex shader override is planned but not yet implemented — do not attempt to swap the vertex shader via `specialize()`.
- Always test WGSL changes in a web build (`python test_web.py`). WebGPU validates binding interfaces strictly; native wgpu is more permissive and will not catch all errors.

See `docs/25_custom_shaders.md` for the full shader authoring guide.

## Physics & movement must use `FixedUpdate`
All player movement, physics processing, and camera-follow logic must run in `FixedUpdate`. Using `Update` for physics-driven movement causes stuttering.

## Terrain generation is async
Terrain mesh generation is compute-heavy. Always use Bevy's `AsyncComputeTaskPool` and poll `Task` components — never block the main thread.

## Inspector isolation
`bevy_egui` inspector and game UI are strictly separated. The inspector renders on its own camera/layer; never mix it with the main game UI camera. Data structs that should be visible in the inspector must be conditionally exposed using `#[cfg_attr(feature = "inspector", ...)]` attributes — see existing components for the pattern.

## Frame pacing and performance

**Native frame cap:** `bevy_framepace` (native only, `#[cfg(not(target_arch = "wasm32"))]`) caps the render loop at 60 fps to prevent vsync busy-wait inflating GPU utilisation numbers. Web builds are capped by `requestAnimationFrame` naturally.

**Unfocused throttle:** `WinitSettings` drops the focused mode to `Reactive { wait: 100ms }` (~10 fps) when the window loses focus, reducing GPU load in the background.

**Pipeline warmup:** `pipeline_warmup_system` adds `NoFrustumCulling` to all `Mesh3d` entities for 4 frames after each scene load (`PipelineWarmup(4)` inserted by `spawn_scene_v2`). On WASM, WebGPU pipeline compilation is synchronous and lazy — without warmup, moving the camera to reveal previously-culled entities triggers 300–2000 ms frame stalls.

**Change-detection discipline:** Systems that update render-affecting components every frame (font sizes, colours, visibility) must guard writes so Bevy's change detection only fires when the value actually changes. Unconditional writes to `Mut<T>` fields mark the component as changed every frame, which re-triggers downstream render work (text layout, glyph atlas uploads, material rebind). Pattern:
```rust
// BAD — triggers change detection even when value is identical
text_font.font_size = new_size;

// GOOD — only fires change detection when the value meaningfully differs
if (text_font.font_size - new_size).abs() >= 0.5 {
    text_font.font_size = new_size;
}
```
The same applies to `Visibility`, `Transform`, and any component read by the render pipeline.

## Audio

### Preloading
`preload_audio_system` fires on every `SceneEvent::Ready` and calls `asset_server.load::<AudioSource>()` for every entry in `LoadedAssetCatalog.audio`, storing the resulting handles in `LoadedAudioHandles`. This eliminates first-play I/O latency — the asset server cache is warm before the player can interact, so `Action::PlaySound` resolves instantly rather than blocking on file I/O. The handles in `LoadedAudioHandles` must stay alive (i.e. not be dropped) to prevent the asset server from evicting the audio between scene loads.

On each new `SceneEvent::Ready` the resource is cleared and repopulated, so scene transitions always reflect the current catalog without accumulating stale handles.

### Audio file authoring
Short SFX (jumps, pickups, UI clicks) must have **no leading silence** in the audio file. Any silence baked into the file at export time is played back verbatim, adding perceived delay on top of any engine latency. Trim the start of the file in your audio editor before exporting.

Use WAV for all short SFX — it is uncompressed PCM with zero decode overhead. OGG/Vorbis and MP3 incur a decoder initialisation cost that is especially noticeable on first play in WASM. Reserve compressed formats for long-form audio (background music, ambient loops) where the file-size saving is worth the decode cost.

## Spawning: standard entity metadata

**`tag_spawned_entity` (in `runtime/scene_manager/mod.rs`) is the single source of truth for the
metadata every addressable spawned entity gets.** Every spawn site routes through it — GLB
actor/prop, single-mesh primitive, composite primitive, foliage root, both player paths, and
dynamic `Action::Spawn`. It always inserts `SpawnId` + `PrefabKey` + `LevelEntity` and registers
the entity in `SpawnRegistry`; it inserts the `ClickSelectable`/`Targetable` markers per the
prefab flags (players pass `false`). Player-specific components (CharacterController, physics,
camera) stay at the call site.

Do **not** hand-insert `SpawnId`/`PrefabKey`/`LevelEntity` or call `spawn_registry.entities.insert`
at a spawn site — call `tag_spawned_entity`. The 5-way divergence this replaced caused real bugs
(GLB actors missing `SpawnId`, the GLB player missing `SpeedMultiplier`/`SpawnId`, dynamic spawns
missing `PrefabKey`/`LevelEntity`). Adding a new "every entity gets X" field means editing the
helper once, not every site. `PrefabKey` (catalog key, e.g. `"enemy_orc_melee"`) is distinct from
`SpawnId` (instance id, e.g. `"orc_01"`).

## Dynamic spawning

### Spawn queue
`Action::Spawn` does **not** call `spawn_prefab_instance` inline. Instead it pushes a `QueuedSpawn` struct (pre-resolved: prefab def, model path, transform, spawn ID, project root) onto `PendingEntitySpawns`. `drain_spawn_queue_system` runs at the end of the interpreter chain and processes at most `SPAWNS_PER_FRAME = 2` entries per frame.

This caps wave-spawn WebGPU pipeline-compile stalls. On WASM, every new mesh+material combination causes a synchronous `device.createRenderPipeline()` call on first render (~100–300 ms each). Limiting to 2 spawns per frame keeps the per-frame stall under ~600 ms instead of seconds for large waves.

For single-entity spawns the queue is transparent: action_executor pushes, drain_spawn_queue processes, all within the same `app.update()` call.

`PendingEntitySpawns` is cleared on `Action::LoadScene` so no orphaned spawns execute after a scene transition.

### Component parity with scene-placed entities

Dynamically spawned entities (via `Action::Spawn`) receive the same prefab-driven components as scene-placed entities:

- **`motion`** — inserted inside `spawn_prefab_instance` (GLB path), so any prefab with `motion:` gets rotation/bob automatically on dynamic spawn.
- **`stat_label` / `world_stat_bar`** — `drain_spawn_queue_system` pushes a `DynamicStatUiEntry` to `DynamicStatUiQueue` for each spawn whose prefab declares these widgets. `drain_dynamic_stat_ui_system` (runs in the same chained set, one slot after `drain_spawn_queue_system`) drains the queue and spawns the label/bar entities. The net result is a one-frame deferral — imperceptible in practice.
- **`interactable` / `trigger_zone` / `colliders` / `stat_templates` / `behavior`** — all inserted inside `spawn_prefab_instance` for both scene and dynamic paths.

**Known limitation:** `depth_scale: Some(true)` on a `StatLabelDef` or `WorldStatBarDef` is silently ignored for dynamic spawns — `depth_scale` is always `None` because no scene context is available at queue time. See `planning/claude_suggestions.md` for the planned fix (store active scene's `label_depth_scale` in a resource).

### GLB preloading
`Action::PreloadPrefab(key)` takes a prefab key, resolves it to a model path, calls `asset_server.load::<Scene>()`, and stores the `Handle<Scene>` in `PreloadedGlbHandles`. This prevents the ~1–2 s WASM stall caused by HTTP fetch + GLTF decode on first spawn of an uncached GLB.

**Usage pattern:** fire `PreloadPrefab` on `scene.ready` (before the player can interact) so the asset is decoded during the natural loading pause.

```ron
// logic/rules.ron
( on: "scene.ready:main", do_actions: [ PreloadPrefab("enemy_orc_melee") ] ),
```

`PreloadedGlbHandles` is cleared on `Action::LoadScene` alongside `PreloadedScenes`. The handles must stay alive (not be dropped) to keep the decoded GLB in the asset server cache between the preload and the first spawn.

### Particle pipeline warmup
`Action::SpawnEffect` uses GPU-compiled render pipelines. WebGPU compiles a pipeline for each material+blend combination the first time it is drawn — this stall (~300–1000 ms) happens synchronously on WASM. Pool group entities (`NoFrustumCulling` + `LevelEntity`) are created lazily on first use, so the existing `pipeline_warmup_system` cannot pre-warm them on its own.

**v2 pool renderer pipeline variants** (each compiles separately):

1. **Additive particles** — `StandardMaterial + AlphaMode::Add` (sphere-like quads, sprites without UV animation)
2. **Blend particles** — `StandardMaterial + AlphaMode::Blend` (smoke, cloud, soft auras)
3. **Flame distort particles** — `PoolFlameMaterial + AlphaMode::Add` (UV distort/scroll — campfire, torches)

**Fix:** fire a `SpawnEffect` for each variant you use during scene load. The pool group entity is created and its pipeline compiled on the first update after the effect lands in the pool. Since the group entity gets `NoFrustumCulling`, the warmup system's 4-frame pass then pre-warms subsequent frames.

```ron
// logic/state_machine.ron — playing state entry_actions
SpawnEffect(key: "hit_spark",     position: Some((0.0, -100.0, 0.0))),  // warms additive pipeline
SpawnEffect(key: "campfire_smoke",position: Some((0.0, -100.0, 0.0))),  // warms blend pipeline
SpawnEffect(key: "campfire_body", position: Some((0.0, -100.0, 0.0))),  // warms PoolFlameMaterial pipeline
```

Place these alongside `PreloadScene` / `PreloadPrefab` calls so they fire during the natural loading pause, before the player can interact.

**Budget footgun**: warmup `SpawnEffect` calls at `y=-100` are real particle allocations and consume `ParticleBudget`. In scenes with a tight budget (e.g. `particle_budget: 100`), 3–4 warmup effects can each fire their full `particle_count` against the cap. Either use low-count effects for warmup, place warmup calls on `scene.ready` before continuous emitters fill the pool, or account for warmup cost when sizing the budget.
