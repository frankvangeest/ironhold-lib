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

### Color field convention

All color tuples read from RON must be passed to `Color::srgba(r, g, b, a)` or `Color::srgb(r, g, b)` — **never** `Color::linear_rgba` or `Color::linear_rgb`. RON color values are authored as sRGB (same as CSS / image editors); Bevy linearises internally. Using `linear_rgba` on designer-authored values makes colors appear washed out. This applies to every color field: UI backgrounds, icon tints, stat bars, particles, lights, primitives — all of them.

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
- `AddItem(entity: "{self}", item_key: "potion")` → entity becomes the entity's ID (routes to that entity's `Inventory` component)
- `RemoveItem(entity: "{self}", item_key: "key_01")` → entity becomes the entity's ID
- `TransferItem(from: "{self}", to: "player", item_key: "loot")` → both `from` and `to` are substituted independently
- `OpenShop("{self}")` → the merchant ID becomes the entity's spawn ID (looks up that entity's `MerchantDef`)

**`{target}` substitution** — in global rules.ron, state_machine.ron, and behavior files, `{target}` in any action field is replaced with the current `CurrentTarget` spawn ID. If `CurrentTarget` is `None`, the literal `"{target}"` is left as-is (action will likely no-op gracefully). The substitution runs in all three interpreter systems before pushing to `ActionQueue`. Supported action fields: same as `{self}` above (key, entity, event, id, spawn_point).

**`target.*` events** — emitted by the targeting capability (`capabilities/targeting.rs`; set
`click_selectable: true` or `targetable: true` on `PrefabDef`). Selection is **screen-space
proximity** (project each candidate to the screen via `camera.world_to_viewport`, pick the
nearest to the cursor) — NOT mesh raycasting, which raycasts bind-pose geometry and misses
animated/skinned GLB characters. Tab-cycle is nearest-first by world distance.
`select_aim_height: f32` (default `1.0`) on `PrefabDef` controls how many metres above the entity
origin the click-projection aim point sits. Set lower for ground-hugging creatures (e.g. `0.4` for
a snake, `0.6` for a spider) so the selectable zone aligns with the visible body.
- `target.clicked:{id}` / `target.changed:{id}` / `target.changed` — new target selected
- `target.cleared` — target cleared (click on empty space, `ClearTarget` action, or `LoadScene`)

The capability also writes three `GameVariables` for UI labels (bind whichever you need):
`target_display` (`"<prefab> <id>"`), `target_name` (prefab key), `target_id` (spawn id).
Entities carry a `PrefabKey` component (catalog key) alongside `SpawnId` (instance id) to
support this.

**Target indicator** (`capabilities/target_indicator.rs`) — a ground-ring decal that tracks the
selected entity. Activated via `target_indicator:` in scene RON (references a `decals:` catalog key).
The `target_indicator_system` runs in `Update`, watches `CurrentTarget` and `LoadedTargetIndicator`
via change detection, and manages one `TrackingTarget` entity. The indicator is tagged `LevelEntity`
and does NOT go through the action pipeline (it is a pure cosmetic side-effect of the target state).

Ring colour is resolved per-target at target-switch time via three-tier precedence:
1. `PrefabDef.indicator_color` — direct RGBA override on the prefab (highest priority)
2. `PrefabDef.indicator_category` — string key looked up in `TargetIndicatorDef.named_colors`
3. `TargetIndicatorDef.color` — scene-level fallback

The system reads the target entity's `PrefabKey` component, looks up the prefab in `LoadedPrefabCatalog`,
and applies the precedence chain. Material handles are memoised by resolved `[u32;4]` colour bits —
alternating between two targets of the same colour creates no new `StandardMaterial`. The mesh handle
(radius-driven, colour-independent) is a single cached `Local`; both caches clear on scene change.

**New capabilities for entity logic:**

**Behavior on composite primitive prefabs** — the `behavior` field works on ALL prefab kinds, including `kind: Primitive` prefabs with a non-empty `children` list. Both the single-mesh primitive path and the composite (multi-child) path in `scene_loader.rs` attach `PendingBehavior`.

`TriggerZone` — set `trigger_zone: (radius: 2.0)` on a `PrefabDef`. A Rapier sphere sensor is spawned. Works on **all prefab kinds**: single-mesh primitives, composite primitives (those with `model: ""` and a non-empty `children` list), and GLB actor/prop prefabs. Emits:
- `GameEvent::Trigger("entity.entered:{id}")` on player enter
- `GameEvent::Trigger("entity.exited:{id}")` on player exit

`Interactable` — set `interactable: (radius: 2.5)` on a `PrefabDef`. No collider needed. Emits:
- `GameEvent::Trigger("entity.interacted:{id}")` when player is within `radius` metres and presses the interact key (configured via `inputs.interact` in the player prefab, default `"KeyF"`)

`interactable_system` runs in `Update` before the interpreter chain (`.before(message_interpreter_system)`). `trigger_zone_system` runs in `FixedUpdate`.

### Dialogue system (`capabilities/dialogue.rs`)

**`DialoguePath(String)` component** — inserted by the scene loader on entities whose `PrefabDef.dialogue` is set. `dialogue_tick_system` reads `entity.interacted:{id}` events and matches them against `DialoguePath` entities to auto-fire `Action::StartDialogue`.

**`ActiveDialogue` resource** — tracks the current conversation: `npc_id`, `dialogue_path`, `current_node_index`, `auto_advance_timer`, `handle: Option<Handle<DialogueDef>>`, `last_rendered_node`. Cleared on `EndDialogue` and `LoadScene`.

**System ordering**: `dialogue_tick_system` runs `.after(button_system).after(interactable_system).before(message_interpreter_system)`.

**Auto-advance guard**: `advance_delay_secs` only applies when `node.choices.is_empty()`. Nodes with choices never auto-advance.

**`do_actions` substitution**: `{self}` in choice `do_actions` is replaced with `active.npc_id` by `substitute_self_in_action()` before pushing to `ActionQueue`. Mirrors behavior-file substitution so dialogue choices and behaviors behave consistently.

**Pipeline events emitted:**
- `dialogue.started:{npc_id}` — fired by `Action::StartDialogue` in the executor
- `dialogue.ended:{dialogue_path}` — fired by `Action::EndDialogue` and on `LoadScene`

**`portrait` field**: reserved in `DialogueNodeDef`, not yet rendered. Mark as not-implemented in authoring docs.

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

**Engine-internal shaders (capability-owned) must be embedded at build time, not runtime-loaded:**
- Engine capabilities that own their own `Material`/`UiMaterial` types (`FoliageMaterial`, `FlameParticleMaterial`, `PoolFlameMaterial`, `RadarMaterial`, `TerrainMaterial`) embed their shaders via `include_str!()` in a `Startup` system and register them at a stable `Handle<Shader>` using `uuid_handle!()`.
- The `Material`/`UiMaterial` impl returns `ShaderRef::Handle(HANDLE)`, never a `"shared/shaders/..."` path string.
- Do NOT use a `ShaderRef` path string for engine-owned shaders — it creates a runtime file dependency that breaks projects that do not ship `assets/shared/`. See `terrain.rs` + `terrain_material.rs` for the canonical pattern.
- The `CustomMaterial` system is the only place where path-based `ShaderRef` strings are acceptable, because there the path is explicitly designer-authored in `assets.ron`.
- Similarly, never fabricate asset paths in code as catalog fallbacks (e.g. `format!("shared/textures/{}.png", key)`). If a catalog key is missing, warn once and use a 1×1 white fallback or skip the entity — never construct a `shared/` path silently.

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
actor/prop, single-mesh primitive, composite primitive, foliage root, every player spawn path
(see the four-site inventory below), and dynamic `Action::Spawn`. It always inserts `SpawnId` +
`PrefabKey` + `LevelEntity` and registers the entity in `SpawnRegistry`; it inserts the
`ClickSelectable`/`Targetable` markers per the prefab flags (players pass `false`).
Player-specific components (CharacterController, physics, camera) stay at the call site.

Do **not** hand-insert `SpawnId`/`PrefabKey`/`LevelEntity` or call `spawn_registry.entities.insert`
at a spawn site — call `tag_spawned_entity`. The 5-way divergence this replaced caused real bugs
(GLB actors missing `SpawnId`, the GLB player missing `SpeedMultiplier`/`SpawnId`, dynamic spawns
missing `PrefabKey`/`LevelEntity`). Adding a new "every entity gets X" field means editing the
helper once, not every site. `PrefabKey` (catalog key, e.g. `"enemy_orc_melee"`) is distinct from
`SpawnId` (instance id, e.g. `"orc_01"`).

### The four player-construction sites

Any feature that changes player spawning (local co-op, character select, respawn, possession)
must account for all four or players diverge silently:

1. **GLB collector** — `scene_loader.rs` builds `player_configs: Vec<PlayerConfig>` from every
   scene entity whose prefab has `tags: ["player"]`.
2. **Primitive collector + inline spawn** — a separate `primitive_player` path in
   `scene_loader.rs` builds its own `CharacterController` + `OrbitCamera` directly and does
   **not** go through `PlayerConfig` at all. Single-player only — local co-op (`player_index`,
   the shared party camera) does not extend to primitive/capsule players. A capsule-based demo
   would need a separate change to support 2+ players.
3. **Dynamic spawn** — `action_executor.rs`'s `Action::Spawn` handler assembles a `PlayerConfig`
   for a `tags: ["player"]` prefab (the character-select flow).
4. **Shared GLB spawn functions** — `entity_spawner.rs`'s `spawn_player_entity` (single player,
   own `OrbitCamera`) and `spawn_players_and_camera` (1+ players; 2+ share one camera). Both
   call the private `spawn_player_entity_core` to avoid duplicating the model/physics/metadata
   setup. The terrain-delayed path (`PendingPlayerConfig`, now `Vec<PlayerConfig>`) also routes
   through `spawn_players_and_camera`.

Sites 1 and 3 both hand-assemble a `PlayerConfig` — routed through the shared
`assemble_player_config()` helper (`entity_spawner.rs`) so a new `PlayerConfig` field (e.g.
`player_index`) only needs adding in one place. `spawn_player_entity_core` inserts
`player_config.player_index` as a queryable `PlayerIndex(u32)` component
(`capabilities/player.rs`) on every GLB player entity — no system reads it yet (input routing
uses `gamepad_index`, camera targeting uses scene entity order), but it's a real ECS fact a
future per-player system (nameplate/HUD labeling) can query without another schema pass.

**`PrefabDef.material` does NOT automatically apply to players — this bit Stage 6's local co-op
4-way split during playtest.** `spawn_prefab_instance` (the generic Actor/Prop/NPC path) reads
`prefab.material` and inserts `PendingMaterialOverride`; `spawn_player_entity_core` (the player
path) is completely separate and did not, and `PlayerConfig` didn't even carry the field. Fixed by
adding `PlayerConfig.material: Option<String>`, forwarding it in `assemble_player_config` (so all
three sites above get it for free), and inserting `PendingMaterialOverride` in
`spawn_player_entity_core` exactly like the generic path does. **Any future `PrefabDef` field
meant to affect rendering/visuals must be checked against both spawn paths, not just the generic
one** — this is the same class of bug the four-site inventory above exists to prevent.

### Local co-op: shared camera, split-screen, gamepad routing, view-box clamp

**`PartyOrbitCamera`** (`capabilities/camera.rs`) is a sibling to `OrbitCamera`, not a
replacement — single-player scenes are untouched. When a scene has 2+ `tags: ["player"]`
entities, `spawn_players_and_camera` reads the **first** player's `CameraConfig.party:
Option<PartyZoomDef>` and `CameraConfig.split: Option<SplitScreenDef>` as the explicit switches
(mutually exclusive — if both are set, `split` wins and a warning is logged):
- `party` set → spawns one `PartyOrbitCamera` framing the midpoint of all players; radius is
  `clamp(max_pairwise_separation + zoom_margin, min_radius, max_radius)`, recomputed every frame
  by `party_camera_follow_system`. `PartyZoomDef.allow_manual_zoom` (default `false`) controls
  whether scroll-wheel still nudges the derived radius via an accumulated offset.
- `split` set → spawns one **real `OrbitCamera` per player** (not `PartyOrbitCamera`), each
  tagged `SplitViewportSlot(u32)` (which cell it owns — slot index = spawn order, i.e. entity
  order in the scene's `entities:` list). `split_screen_viewport_system` recomputes every
  `SplitViewportSlot` camera's `Camera.viewport` every frame from `Window::physical_size()`
  (physical pixels already — no manual `scale_factor()` multiplication needed, unlike a naive
  `width()`/`height()` read) and `ActiveSplitScreen`'s orientation (`SplitOrientation::Vertical`
  splits left/right, `Horizontal` splits top/bottom, `Grid` computes an N-cell grid — see below).
  Split-screen orientation lives in the `ActiveSplitScreen` resource (mirrors `ActiveViewBox`/
  `LoadedTargetIndicator` — populated by `spawn_players_and_camera`, cleared on `LoadScene`),
  **not** on `OrbitCamera` or `SplitViewportSlot` — this keeps split-screen state out of the
  camera components so the planned `camera_modes` unification doesn't have to untangle it later.
  `Vertical`/`Horizontal` are always exactly 2-way (`.take(2)` in `spawn_players_and_camera`'s
  `split` branch); only `Grid` (Stage 6) unlocks N-way, `.take(slot_count)` where
  `slot_count = entities.len().min(MAX_SPLIT_PLAYERS)` (`MAX_SPLIT_PLAYERS = 4`, `camera.rs`) —
  a `Grid` scene with more players than the cap spawns the extras cameraless, same as what
  already happened pre-Stage-6 if a 3rd player existed in a `Vertical`/`Horizontal` scene.
- `Grid` orientation (Stage 6) → `split_screen_viewport_system` reads a separate resource,
  **`ActiveSplitSlotCount(Option<u32>)`** (populated once by `spawn_players_and_camera` at scene
  load, cleared on `LoadScene` — mirrors `DynamicSplitConfig`'s exact write-once lifecycle), for
  its player count, rather than counting `SplitViewportSlot` cameras live in the query each frame.
  This was a deliberate architecture-review fix during planning: since `Grid` is meant as the
  foundation for a future hot-join/leave feature, deriving the count live would silently reflow
  the grid on any mid-transition entity churn — a stored, explicitly-written count doesn't. Layout:
  `cols = ceil(sqrt(count))`, `rows = ceil(count / cols)`, cell assigned row-major by `slot.0`
  (`row = slot.0 / cols`, `col = slot.0 % cols`); the last row/column absorbs the remainder on an
  odd window dimension, same pattern as `Vertical`/`Horizontal`. `count == 3` leaves one grid cell
  (slot `3` of a 2×2 grid) with no camera — renders as clear color, not a special-cased 3-pane
  layout. `Grid` does **not** support `split.dynamic` — dynamic merge/split stays `Vertical`/
  `Horizontal`-only.
- `split.dynamic: Option<DynamicSplitDef>` set (Stage 5) → the view starts **merged** (its own
  internal `PartyOrbitCamera`, tuned by `DynamicSplitDef.merged_zoom_margin`/
  `merged_allow_manual_zoom` — mirrors `PartyZoomDef`'s two fields, self-contained specifically so
  dynamic split doesn't also require authoring a `party:` block alongside `split:`) and
  auto-splits into the two per-player `OrbitCamera`s once `split_distance` is exceeded, merging
  back below `merge_distance` (hysteresis — the gap prevents flicker right at one boundary).
  `dynamic_split_screen_system` (`capabilities/camera.rs`) runs every frame, `.after(
  party_camera_follow_system)` and `.before(split_screen_viewport_system)` (see the `.chain()` in
  `lib.rs`), and decides merged-vs-split purely by toggling `Camera.is_active` on the
  already-spawned party/split cameras — **it never spawns or despawns cameras**; all three exist
  for the scene's lifetime, so there is no pop/snap on transition since inactive cameras keep
  tracking their targets the whole time (`camera_orbit_system`/`party_camera_follow_system` don't
  gate on `is_active`). The split axis (`Vertical` vs `Horizontal`) is chosen automatically from
  `abs(dx)` vs `abs(dz)` between the two players only at the merged→split transition instant, then
  held fixed for that whole split period — `SplitScreenDef.orientation` becomes a rare tie-break
  hint (used only when dx/dz are exactly equal) rather than the authored axis. **Unlike the
  fixed-orientation case, `ActiveSplitScreen` is continuously rewritten while dynamic mode is
  active** — every merge/split transition updates it — rather than being write-once at scene load;
  `DynamicSplitConfig` (the static per-scene tuning resource, populated once at scene load like
  `ActiveSplitScreen` itself) is what stays write-once.
- Neither set → logs a warning and falls back to a single `OrbitCamera` targeting only the
  first player. Never silently spawns one `OrbitCamera` per player without split-screen viewports
  — that would mean two cameras fighting for the same full-window viewport with no RON-visible
  symptom.

Later players' `camera.party`/`camera.split` fields are ignored entirely — only the first
player-tagged scene entity's config is read for those two switches. Scene entity order in
`entities:` therefore matters for local co-op. **Split-screen is the one case where every
player's OTHER camera fields still matter** — each split-screen player gets a real `OrbitCamera`
built from their own `camera` block (offset, `zoom_speed`, `orbit_button`, etc.), not just the
first player's. A shared mouse would otherwise rotate/zoom every split-screen camera identically
(`camera_orbit_system` reads mouse input once per system call, applying the same delta to every
`OrbitCamera` in its query) — split-screen scenes disable manual control per-camera instead via
RON: `zoom_speed: 0.0` (scroll × 0 has no effect) and `orbit_button: "None"` (new
`parse_orbit_button` arm returning `(false, false)`, no warning — distinct from an actually
unrecognized string, which warns and defaults to `"Either"`).

**Known limitation:** `Action::CameraShake` only queries `With<OrbitCamera>`
(`SceneStateParams::orbit_cameras` in `scene_manager/mod.rs`), so it silently no-ops on a scene
using `PartyOrbitCamera` — but **does** fire on both cameras in a `split` scene, since those are
real `OrbitCamera`s. This is an intentional consequence of split-screen using real per-player
cameras, not an oversight to fix.

**Other known limitations, introduced by split-screen having 2 real `Camera3d` entities per
scene** (none affect `local_coop_demo`, which uses none of these features, but matter for any
future project combining split-screen with them): `world_label_screen_pos_system` (`lib.rs`),
`nameplate.rs`'s distance-culling, `particle_renderer.rs`'s billboard orientation, and
`targeting.rs`'s click-to-select all assume one `Camera3d` exists. None panic (graceful
`.single()`-as-`Result` or `.iter().find(...)` patterns), but they silently no-op or arbitrarily
pick one of the two cameras rather than being viewport-aware. See `planning/claude_suggestions.md`
▸ Camera for the exact line references.

**Split-screen player HUD labels** (`capabilities/camera.rs`) — the first real consumer of
`PlayerIndex` (see above). `split_viewport_player_label_spawn_system` reacts to
`Added<SplitViewportSlot>` (mirroring `nameplate_setup_system`'s `Added<NameplateTag>` idiom, so
no per-frame "does a label already exist" scan is needed) and spawns a standalone (unparented) UI
`Text` node — a corner "P{n}" label — for any split camera whose `OrbitCamera.target` carries a
`PlayerIndex`. The camera entity gets tagged `SplitScreenPlayerLabel` + `LinkedPlayerLabel(Entity)`
pointing at the UI entity. `split_viewport_player_label_update_system` (`.after(
split_screen_viewport_system)` in `lib.rs`'s `.chain()`) keeps the label's `Node.left`/`top`
synced to that camera's live `Camera.viewport` (physical pixels ÷ `window.scale_factor()` →
logical, top-right anchored so it never collides with a room's top-left `room_hint` title) and its
`Visibility` synced to `Camera.is_active` — the ordering guarantees no stale frame across a
`dynamic` split's merge/split transition. Label text reads `PlayerIndex`, not scene entity/spawn
order; label color comes from a fixed `PLAYER_LABEL_COLORS` palette, not from `PrefabDef.material`
— see `docs/20_data_formats.md`'s "Split-screen player HUD labels" section for the designer-facing
version of both notes. The label is a valid standalone UI root because it resolves against the
same full-window `Camera2d` every RON UI label already uses (see the "Adding a new asset
project"/UI conventions elsewhere in this file) — `IsDefaultUiCamera` being commented out on that
camera does not change this in practice, but a future refactor of that setup should re-verify it.

**Gamepad routing** — `InputMap.gamepad_index: Option<usize>` lets a player prefab bind to a
specific gamepad instead of the keyboard. Bevy has no built-in numeric gamepad index (each
connected pad is its own `Gamepad` entity); `input_translator_system` (`runtime/input.rs`) sorts
connected gamepads by entity index, so `gamepad_index: 0` means "whichever gamepad connected
first this session," not a hardware-guaranteed slot. Left stick moves/strafes, right stick X
turns, South button jumps, East button toggles run — independent of the keyboard's
`strafe_mouse_button` toggle (that only exists to disambiguate A/D on one keyboard; a gamepad
already has separate sticks).

**View-box clamp** — `GameSceneV2.max_view_box: Option<(f32, f32, f32, f32)>` (`min_x, min_z,
max_x, max_z`) is read into the `ActiveViewBox` resource on scene load (cleared on `LoadScene`,
same pattern as `LoadedTargetIndicator`). `player_view_box_clamp_system` (`capabilities/player.rs`,
`FixedUpdate`, after `player_movement_system`) clamps every `CharacterController`'s XZ position
into the box (Y/jump untouched) and zeroes the clamped axis's `Velocity.linvel` — without the
velocity zero, Rapier keeps re-integrating the outward velocity every tick and the player jitters
against the edge instead of stopping cleanly.

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
- **`NameplateTag`** — inserted at spawn time when `should_insert_nameplate(prefab.nameplate, show)` returns true (`scene_manager/mod.rs`, beside `tag_spawned_entity`): `nameplate: Some(false)` always suppresses; otherwise `show` or an explicit `nameplate: Some(true)` opt-in enables it. **`show` differs by entity type** — NPCs/props use `scene.show_nameplates`/`nameplate_config.enabled`; `Player`-tagged entities (see below) use the independent `show_player_nameplate` / `nameplate_config.player_enabled` instead, never `show_nameplates`. This is the single source of truth for all 6 nameplate-gating call sites (`scene_loader.rs` ×5 — 4 NPC/prop + 1 primitive-player, `entity_spawner.rs` ×1, `action_executor.rs` ×1 for the character-select dynamic player spawn) — do not re-inline the predicate at a new call site. `nameplate_setup_system` queries `Added<NameplateTag>` every `Update` frame and spawns the anchor + `Text2d` + pixel bar quads for any newly-tagged entity — scene-placed entities, dynamically spawned actors, and wave-spawned enemies all use the same path; it also re-checks `player_enabled` vs `enabled` per-entity (via `Option<&Player>`) since the tag alone doesn't carry which toggle governs it. Bar fills are driven by the existing `world_pixel_bar_update_system`. `NameplateSceneConfig` (resource) is populated from `GameSceneV2` on each scene load and cleared on `LoadScene`. `nameplate_visibility_system`'s `faction_filter` (HostileOnly/FriendlyOnly/All) is an NPC/prop-only categorization — `Player` entities bypass it entirely (same treatment as a `Some(true)` override: distance-only), since faction hostility doesn't apply to "should I see my own name."
- **`Player` / `PlayerOwnership`** (`capabilities/player.rs`) — marker inserted unconditionally wherever a player entity is spawned (`spawn_player_entity` for GLB, inline in `scene_loader.rs` for the primitive-player path). `PlayerOwnership::{Local, Remote}` is always `Local` today (no multiplayer exists yet) — reserved so nameplate/UI/camera systems can distinguish "me" from "other players" once Beta 0.6 (LAN co-op) lands, without another schema pass. See `planning/features/player_nameplate_visibility.md`.
- **`Action::ToggleOwnNameplate`** (v2 of the above) — flips `PlayerNameplatePreference` (a `Resource`, `init_resource`'d alongside `NameplateSceneConfig`), emitting `nameplate.own_shown`/`nameplate.own_hidden`. Consumed only by `nameplate_visibility_system`'s per-frame `Player`-entity branch — `nameplate_setup_system`'s spawn-time gate is untouched, since toggling only needs to flip `Visibility`, not insert/remove `NameplateTag`. An explicit per-prefab `nameplate: Some(true)`/`Some(false)` on the player prefab always wins over this preference, same precedence as `show_player_nameplate`. Re-seeded from `show_player_nameplate` on every scene load in `scene_loader.rs` (does NOT persist across scene transitions — a deliberate simplicity choice, matching `player_enabled`'s own per-scene-authored behavior rather than `AudioState`'s session-persistent pattern).
- **`interactable` / `trigger_zone` / `colliders` / `stat_templates` / `behavior`** — all inserted inside `spawn_prefab_instance` for both scene and dynamic paths.

**Known limitation:** `depth_scale: Some(true)` on a `StatLabelDef` or `WorldStatBarDef` is silently ignored for dynamic spawns — `depth_scale` is always `None` because no scene context is available at queue time. See `planning/claude_suggestions.md` for the planned fix (store active scene's `label_depth_scale` in a resource).

### GLB preloading
`Action::PreloadPrefab(key)` takes a prefab key, resolves it to a model path, calls `asset_server.load::<Scene>()`, and stores the `Handle<Scene>` in `PreloadedGlbHandles`. This prevents the ~1–2 s WASM stall caused by HTTP fetch + GLTF decode on first spawn of an uncached GLB.

`Action::PreloadGlb(key)` is the model-catalog equivalent — it takes a **model catalog key** (from `assets.ron` `models:`), looks up the path, and loads it as `Handle<Scene>`. Use this for animation-source GLBs that have no prefab entry (e.g. `"anim_magic"`, `"anim_zombie"`). Loading as `Scene` triggers the full GLTF loader, which also decodes all animation clips and stores them as sub-assets — so the `Handle<Gltf>` the animation system needs is warm in the cache. The handle is stored in `PreloadedGlbHandles` alongside `PreloadPrefab` handles.

**Usage pattern:** fire both in `entry_actions` of the playing state (before the player can interact) so all GLBs are decoded during the natural loading pause.

```ron
// logic/state_machine.ron — playing state entry_actions
PreloadGlb("anim_locomotion"),
PreloadGlb("anim_melee"),
PreloadGlb("anim_magic"),
PreloadGlb("anim_zombie"),
PreloadPrefab("enemy_orc_melee"),
```

`PreloadedGlbHandles` is cleared on `Action::LoadScene` alongside `PreloadedScenes`. The handles must stay alive (not be dropped) to keep the decoded GLB in the asset server cache between the preload and the first use.

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
