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
4. **Executor** — `action_executor_system` drains the queue and dispatches each `Action` (LoadScene, Despawn, PlaySound, AddScore, etc.).

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

### Conditions on rules

The only runtime condition available to the rules engine is `LogicState` — a single named string (e.g. `"playing"`, `"hp_low"`). Rules with a `when:` field only fire while the FSM is in a matching named state. To add conditions:
- Have a gameplay system call `Action::EnterState("hp_low")` when HP drops below threshold.
- Gate the conditional rule with `when: "hp_low"` in `state_machine.ron`.

Do not add a general condition system to the interpreter unless the above pattern is genuinely insufficient.

---

## Before coding

No assets should be hardcoded in the runtime. All assets should be defined in the `assets/projects/{name}/assets.ron` file. Audio catalog keys (not file paths) are passed to `Action::PlaySound`; the executor resolves the path.

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
