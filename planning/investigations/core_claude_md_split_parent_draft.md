# ironhold_core — Rust Source Rules

Crate-wide rules for `crates/ironhold_core/src/`, loaded whenever you touch a file here. Rules that only matter inside
one folder live in that folder's `CLAUDE.md` (`capabilities/`, `runtime/`, `runtime/scene_manager/`, `schema/`, and
`assets/` for shaders); long-form history and reference is in `docs/dev/` (the moved-sections index
`docs/dev/moved-sections-index.md` maps every old section of this file to its new home).

<!-- b:1 -->
## The Message → Interpreter → Action → Executor pipeline

This is the single most important architectural rule. All game behaviour flows through it:

```
Capability emits Message  →  Interpreter matches rules  →  ActionQueue  →  Executor runs Action
```

1. **Message** — a capability detects something and emits a typed message (`UiEvent::ButtonPressed`, `SceneEvent::Ready`, ...).
2. **Interpreter** — `fsm_interpreter_system` matches messages against the project's `state_machine.ron` (global_on, in-state
   bindings, transitions) and `entity_fsm_interpreter_system` does the same per entity against its `.behavior.ron`; matches push
   `Action`s onto `ActionQueue`.
3. **ActionQueue** — a FIFO `VecDeque<Action>`: push order is execution order, and exit actions are pushed before entry actions.
4. **Executor** — `action_executor_system` drains the queue and dispatches each `Action`.

**Physics sensors emit messages, not actions.** A sensor that detects overlap emits `GameEvent::Trigger("entity.collected:{id}")`;
it must NOT push `Action::Despawn` or `Action::PlaySound`. **Never push to `ActionQueue` from a capability system:** only the
interpreters do. If you are adding `ResMut<ActionQueue>` to a physics or gameplay system, stop, emit a `GameEvent`/`UiEvent`
and handle the response in `state_machine.ron`. Hardwired actions lock behaviour in code; going through the pipeline lets
designers add rewards, score and conditions in RON without recompiling.

| Source | Message type | Event name |
|---|---|---|
| UI widget | `UiEvent::ButtonPressed(trigger)` | `"ui.button_pressed:{trigger}"` |
| Physics sensor / gameplay logic | `GameEvent::Trigger(name)` | `"{name}"` as-is (the caller namespaces it) |
| Scene lifecycle | `SceneEvent::Ready/Loaded/…` | `"scene.ready:{scene}"` etc. |

<!-- b:81 -->
## No hardcoded assets

No asset is hardcoded in the runtime: every asset is defined in `assets/projects/{name}/assets.ron`, and audio catalog
keys (not file paths) are what `Action::PlaySound` takes. Never fabricate a path in code as a catalog fallback (for
example `format!("shared/textures/{}.png", key)`): if a key is missing, warn once and use a 1x1 white fallback or skip the
entity.

### Color field convention

Every color tuple read from RON goes through `Color::srgba(r, g, b, a)` / `Color::srgb(r, g, b)`, **never**
`Color::linear_rgba` / `Color::linear_rgb`: RON colors are authored as sRGB and Bevy linearises internally, so `linear_*`
makes designer colors look washed out. This applies to every color field (UI, icons, stat bars, particles, lights,
primitives).

<!-- b:108.rule -->
**`scene.ui` must be walked with `schema::scene_v2::walk_ui_nodes` (or `walk_ui_nodes_pathed`), never iterated flat:** a flat
loop silently misses every nested `ui:` node. Detail in `schema/CLAUDE.md`.

## `lib.rs` ordering edges are load-bearing

Many system-ordering edges in `lib.rs` exist only to prevent scheduling races, and `lib.rs` loads only this file.
Each group of them is marked with a `// load-bearing:` comment in `lib.rs`: **never remove, reorder or "simplify" an edge
that sits under one**, and give any new system that touches the same state an explicit edge against them (a passing test run does not prove
an ordering). The ones that cross folders: the interpreter chain `fsm_interpreter_system` → `entity_fsm_interpreter_system`
→ `flush_pending_intent_system` → `action_executor_system` → … → `drain_spawn_queue_system` → `drain_dynamic_stat_ui_system`;
`unclaimed_gamepad_trigger_system.before(fsm_interpreter_system)`; `gamepad_bind_system` before `input_translator_system`;
`interactable_system` and `dialogue_tick_system` before the interpreters; the targeting chain before `action_bar_input_system`;
`camera_blend_system` last of the camera systems and `world_label_screen_pos_system` after it; `nameplate_visibility_system`
after `world_label_screen_pos_system`; `target_hud_update_system` after `split_screen_viewport_system`; the audio-volume
mirror after `action_executor_system` and before `update_dynamic_labels_system`.

<!-- b:575.rule -->
`tick_delayed_events_system` (`lib.rs`) ticks on raw `Time` with **no pause gate**, so a delayed event (a monster respawn
timer) can fire while the game is not in `"playing"`; RON rules that must catch such an event belong in the top-level
`global_on:`, never in a state-scoped `on:` list.

<!-- b:706 -->
## WebGPU 16-byte alignment

Custom GPU-bound structs (for example `TerrainMaterial`) **must** use 16-byte aligned uniform layouts: violating it panics
in web builds with `BUFFER_BINDINGS_NOT_16_BYTE_ALIGNED`. Use `Vec4` for every uniform field and never bind a bare `f32`,
`Vec2` or `Vec3`; `CustomMaterialUniforms` (4 × Vec4) and `TerrainMaterial.uv_scale` (padded Vec4) already comply, so keep
them that way. Check that `AsBindGroup` mappings distinguish Uniform from Storage buffers (Bevy 0.18).

<!-- b:722 -->
**Engine-internal shaders must be embedded at build time, not runtime-loaded.** Capabilities that own a `Material`/
`UiMaterial` (`FoliageMaterial`, `FlameParticleMaterial`, `PoolFlameMaterial`, `RadarMaterial`, `TerrainMaterial`) embed their
shader with `include_str!()` in a `Startup` system, register it at a stable `Handle<Shader>` via `uuid_handle!()`, and return
`ShaderRef::Handle(HANDLE)`, **never** a `"shared/shaders/..."` path string (that is a runtime file dependency that breaks
projects without `assets/shared/`); see `terrain.rs` + `terrain_material.rs`. Path-based `ShaderRef` is acceptable only in
`CustomMaterial`, where the designer authors the path in `assets.ron`.

<!-- b:729 -->
See `docs/25_custom_shaders.md` for the full shader authoring guide (the WGSL rules are in `assets/CLAUDE.md`).

<!-- b:731 -->
## Physics & movement must use `FixedUpdate`

All player movement and physics processing must run in `FixedUpdate`; using `Update` for physics-driven movement causes
stuttering. Camera follow/blend and the animation pipeline deliberately stay in `Update` (rendering cadence, not physics);
see the comment on that `.chain()` in `lib.rs`.

<!-- b:734 -->
Rapier steps in `FixedUpdate` too (`capabilities/physics.rs`, `TimestepMode::Fixed` at `FIXED_TICK_RATE` = 64Hz,
`planning/features/deterministic_fixed_timestep.md`); render smoothing is deliberately **aliasing, no transform
interpolation** (`TimestepMode::Interpolated` would bring back the wall-clock coupling). Native framepace is capped to 64
but that does **not** remove multi-tick frames: vsync means the real present rate is the display's, and no common refresh
rate divides 64Hz (a 60Hz display still gets ~4 double-tick frames a second). A two-tick frame is routine, so the
`FixedUpdate` chain runs `bevy::transform::systems::mark_dirty_trees` right before Rapier's `SyncBackend`; without it a
second tick reads one-tick-stale `GlobalTransform` inside `FixedUpdate` itself (e.g. `player_movement_system`'s `ground_cast`).

<!-- b:750 -->
**An `Update`-scheduled system must never read `&GlobalTransform` of a physics-driven or camera-driven entity: use
`crate::utils::fresh_global_transform`.** `GlobalTransform` is one tick stale relative to the `Transform` an `Update` system
just wrote, and a fixed-tick body and a per-frame camera no longer lag at the same rate, so a multi-tick frame visibly pops
(world labels, nameplates, target rings, `fixed_camera_system`'s `look_at_entity`, tracked decals).

<!-- b:1023 -->
## Deterministic iteration order on gameplay paths

std `HashMap`/`HashSet` use a seeded hasher (random on native; on wasm32 derived from addresses), so iterating one gives a
different order across runs and platforms; any gameplay path whose outcome depends on that order is a replay-divergence
source and a single-player nondeterminism bug (`planning/investigations/hashmap_iteration_order_audit.md`). **Rules for new
code:** a map that is *iterated* on a gameplay path is a `BTreeMap`, an `IndexMap` (insertion order is part of the contract;
never `swap_remove` from it) or a sorted `Vec`; a `HashMap` is only for keyed lookup; never use `Entity` index/order as a
tie-break in simulation logic. `action_bar_input_system` buffers its `GameEvent`s and writes them stable-sorted by slot key
because `slots.iter()` follows archetype order. Prefer switching the container over annotating it whenever the order is
visible (validation errors, CLI output, events, log order); a fixed-hasher map is no way out, since its order still depends on
insertion history.

**The guard is `tests/determinism_lint.rs`** (D2). It scans every `.rs` file under `ironhold_core/src` (skipping only the
brace-matched body of a `#[cfg(test)] mod`) and fails on any line that types or constructs a hash container (`HashMap<`,
`HashMap::`, `HashSet<`, `HashSet::`, or an identifier ending in them such as `FxHashMap`) unless the line carries
`// det: lookup-only` (keyed lookup/insert/remove/contains only) or `// det: order-independent` (iterated, but the result
provably cannot depend on order: a pure `retain`, a merge into an unordered collection, work sorted before use; say why in a
few words). **A marker is a claim about every use of the container:** if you add iteration to a `lookup-only` map, re-check
the claim and relabel it or switch the type (iteration that only orders `warn!` lines is `order-independent (log order
only)`). Aliases are never allowed (`use ... HashMap as X`, `type X = HashMap<..>` are flagged even with a marker). Not
covered: hash containers owned by external types (e.g. bevy's `Gltf::named_animations`), the CLI crate and query/entity order.
`clippy.toml` mirrors the rule as editor feedback only; the test is the gate. The four D1 sites are pinned by
`tests/same_frame_order_tests.rs`.

<!-- b:1030 -->
## Inspector isolation

The `bevy_egui` inspector and the game UI are strictly separate: the inspector renders on its own camera/layer, never mixed
with the main game UI camera. A data struct that should show in the inspector is exposed with
`#[cfg_attr(feature = "inspector", ...)]`; follow the existing components.

<!-- b:1033 -->
## Frame pacing and performance

Native only (`#[cfg(not(target_arch = "wasm32"))]`): `bevy_framepace` caps the render loop at `FIXED_TICK_RATE` to stop vsync
busy-wait inflating GPU numbers, and `WinitSettings` drops an unfocused window to `Reactive { wait: 100ms }`. Web is paced
by `requestAnimationFrame`. **`pipeline_warmup_system` adds `NoFrustumCulling` to every `Mesh3d` for 4 frames after each scene
load** (`PipelineWarmup(4)`, inserted by `spawn_scene_v2`): on WASM, WebGPU pipeline compilation is synchronous and lazy, and
without it revealing a previously-culled entity stalls the frame 300-2000 ms.

<!-- b:1041 -->
**Change-detection discipline:** a system that updates a render-affecting component every frame (font size, colour,
visibility, transform) must guard the write so change detection fires only when the value actually changes; an unconditional
write to a `Mut<T>` field re-triggers downstream render work (text layout, glyph atlas upload, material rebind).
```rust
// BAD: triggers change detection even when the value is identical
text_font.font_size = new_size;
// GOOD: only fires when the value meaningfully differs
if (text_font.font_size - new_size).abs() >= 0.5 { text_font.font_size = new_size; }
```

<!-- b:1084 -->
## Despawning: prefer `try_despawn()` when an entity may already be gone

If a system can plausibly queue two `commands.entity(e).despawn()` calls for the same entity in one run (two paths in the same
system deciding from one query snapshot, since `Commands` are deferred), use **`try_despawn()`** for at least the later call:
`despawn()` on an already-despawned entity logs a warning (harmless, but noisy), `try_despawn()` silently no-ops. Keep an
explicit dedup guard instead only when the call site needs to know whether a despawn happened (`try_despawn()` returns
`EntityCommands`, not a bool). It is also what makes the teardown sweeps safe: recursive `despawn()` of a widget anchor (a
`Pixel` `world_stat_bar`, a nameplate) already kills its separately-tagged children, so a later sweep iteration hits the same
warning (seen in `action_executor.rs` `StopMusic`/`PlayMusicLoop`/`UnloadOverlay`/`ToggleOverlay` and `scene_loader.rs`).

<!-- b:1111 -->
**Tagging a widget's own children with `LevelEntity`/`OverlayEntity` is deliberate even though recursive despawn makes it
redundant: do not "clean it up".** The teardown sweeps are the only thing that removes these entities on a scene or overlay
transition; if they ever stop being a flat query-and-despawn-everything sweep (a `Without<ChildOf>` filter, say), an untagged
child would leak. Self-tagging every level keeps the sweep correct whichever entity in a tree is despawned first, and
`try_despawn()` is what makes that safe.
