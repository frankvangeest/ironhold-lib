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

**Per-player targeting (Phase 1, `planning/features/per_player_split_screen_targeting.md`)** —
each player entity carries its own `PlayerTarget(Option<String>)` component
(`capabilities/player.rs`), inserted in `entity_spawner.rs::spawn_player_entity_core`'s shared
post-model-source-dispatch code — as of `player_model_source_unification.md` v1 this covers both
GLB and primitive players spawned via the immediate scene-load path (see "Player-construction
sites" below). `CurrentTarget` (`capabilities/action_bar.rs`) was deliberately kept as a resource
rather than deleted — it is now "the primary player's `PlayerTarget`, mirrored". The **primary
player** is whichever player entity has `PlayerIndex(0)` or no `PlayerIndex` at all (see
"Player-construction sites" for when the latter is still reachable). `{target}` substitution above, and any `rules.ron`-
overridden slot intent's `do_actions` (see Phase 2 below), keep reading `CurrentTarget` exactly as
before this feature — those two paths only ever resolve against the primary player. A non-primary
player's `PlayerTarget` drives their own visual feedback (ring, per-viewport HUD readout) *and*,
as of Phase 2, their own action bar's slots — but never `rules.ron`/`state_machine.ron`/behavior
actions fired outside the action bar, nor a rule that overrides a slot's intent event. This is a
documented scope boundary, not a bug.

**Per-player action bars (Phase 2, `planning/features/per_player_split_screen_targeting.md`)** —
`ActionBarDef.owner_player: Option<u32>` (`#[serde(default)]`), copied onto `ActionSlotUi` at scene
load, scopes a bar's slots to whichever player entity carries `PlayerIndex(owner_player)`; `None`
(or `Some(0)`) means the primary player, same definition as above. **This did *not* need player
identity threaded through the Message → Interpreter → Action → Executor pipeline** — the original
Phase 1 speculation about that (see `planning/claude_suggestions.md` ▸ Camera) turned out to be
wrong once `action_bar_input_system` was re-read: the action bar already calls `rewrite_target`
itself, locally, before anything reaches `ActionQueue`, so `{target}` is already a concrete entity
ID by the time the interpreter chain sees it. `action_bar_input_system` was rewritten from a single
`find`+`return` (which silently dropped one player's press if 2+ slots fired the same frame) to a
loop over **every** slot whose resolved key is `just_pressed`; for each, `owns_slot(owner_player,
player_index)` resolves the acting player, and that player's own `PlayerTarget` — not the global
`CurrentTarget` — drives the `{target}` rewrite, the no-target gate, and the
`intent.slot.*:{player_id}` event's player id. For the primary player this is a no-op in practice
(`PlayerTarget` is already kept in lockstep with `CurrentTarget` for the primary player). **The
`cost:`/`SlotCost` check/deduct is now per-player too** (`planning/features/per_player_stat_pools.md`):
it resolves against the acting player's own `StatMap` first — populated from `PlayerConfig.
stat_templates`, forwarded from `PrefabDef.stat_templates` exactly like any NPC/prop prefab, and
inserted by `spawn_player_entity_core` — falling back to the single shared `LoadedStats` resource
only when that player's prefab declares no matching `stat_templates` entry (see
`docs/20_data_formats.md`'s `SlotCost` section). The check and the deferred deduct action's key are
resolved **once** per firing slot (`resolve_cost_source` in `action_bar.rs`) and reused for both,
rather than independently re-resolved, so the two can never disagree about which pool a slot's cost
hits. A scene-load `warn!` (`scene_loader.rs::warn_missing_player_stat_templates`) plus an
`ironhold_cli validate` error (`missing_player_stat_template`) both flag the one likely-mistake
case: an `owner_player`-scoped bar's `cost.stat` isn't among that player's *own* declared
`stat_templates`, even though the player clearly opted into a per-player pool by declaring some.
Declaring **no** `stat_templates` at all is never flagged — that's the ordinary, unchanged global
fallback every single-player project (and any bar that doesn't opt in) still gets. **What remains
out of scope**: a `rules.ron` rule that intercepts a non-primary player's slot intent still resolves
its own replacement `do_actions`' `{target}` via the interpreter against `CurrentTarget` (the
primary player), not the firing player's `PlayerTarget` — only the slot's *own* built-in
`do_actions` (bypassed when a rule takes over) get the per-owning-player resolution. Two bars
sharing a slot key get both a scene-load `warn!` (`scene_loader.rs::warn_cross_bar_duplicate_keys`,
scene-wide, unlike the pre-existing per-bar-only check) and an `ironhold_cli validate` error
(`cross_bar_duplicate_key`), since `CooldownMap`/`PendingIntentActions`/`HandledIntentSlots` are
still keyed by the literal slot key string alone, scene-wide. Both detectors key their "same bar
vs. different bar" check by positional index, not `ActionBar.id` — nothing enforces `id`
uniqueness, so comparing by `id` would misclassify a real cross-bar collision if two bars happened
to share one (system-architect finding, plan-review).

**`action_bar_input_system` now hard-depends on every `CharacterController` entity also carrying
`PlayerTarget`** — its player-resolution query widened from `Query<&SpawnId, With<CharacterController>>`
to `Query<(&SpawnId, &PlayerTarget, Option<&PlayerIndex>), With<CharacterController>>`, so a player
entity missing `PlayerTarget` silently drops out of the match and that player's entire action bar
never fires (not just a missing ring/HUD, as a `PlayerTarget` omission would have meant before this
phase). Every player-construction site already inserts `PlayerTarget` (see the four-site inventory
above), so this holds today — but check it again against both spawn paths if a future change ever
touches player-entity construction.

**Gamepad-routed action-bar slots (`planning/features/done/gamepad_action_bar_slots.md`)** —
`ActionSlotDef.gamepad_key: Option<String>` (parsed via `InputMap::parse_gamepad_button` at scene
load into `ActionSlotUi.resolved_gamepad_button`, same call site as `resolved_key`) lets a slot
also fire from gamepad, alongside its existing keyboard `key`. The two devices resolve
**differently** — keyboard is shared hardware (`key` fires from the one global
`ButtonInput<KeyCode>` regardless of `owner_player`, unchanged pre-existing behavior); a gamepad is
not shared the same way, so `gamepad_key` only fires from the **owning player's own**
`InputMap.gamepad_index`, resolved via the same `resolve_gamepad(sorted_gamepads, index)` helper
every other gamepad-consuming system uses (`runtime::input::resolve_gamepad`). `action_bar_input_
system`'s query widened again to include `&CharacterController` (for `gamepad_index`) and a new
`Query<(Entity, &Gamepad)>`, sorted once per system call. The fast-path skip (`!keyboard_fired &&
resolved_gamepad_button.is_none()`) preserves the exact perf profile and cooldown-event behavior of
every keyboard-only slot — the gamepad check, and the owning-player lookup it requires, only ever
run for slots that actually declare `gamepad_key`. The keyboard cooldown-gate-before-player-lookup
ordering is preserved byte-for-byte; a second, symmetric cooldown check runs after player
resolution to cover a gamepad-only fire (which couldn't be checked earlier, since it needs the
owning player's own `gamepad_index`) — the two checks are mutually exclusive on `keyboard_fired`, so
neither double-emits. `action_bar_visual_system` is untouched (cost/cooldown-driven only, no input
reads, `owns_slot`'s signature unchanged). Collision detection needs a **second, separately-scoped**
pass distinct from the existing scene-wide keyboard check: `gamepad_key` isn't part of the
intent/cooldown pipeline's key space at all, so the risk isn't cross-bar pipeline entanglement —
it's a same-player double-fire (one physical press activating 2 slots for the same player). Both
`scene_loader.rs::warn_same_player_gamepad_duplicate_slots` and `ironhold_cli validate`'s matching
check key by `(owner_player.unwrap_or(0), GamepadButton)` — the same "`None`/`Some(0)` both mean the
primary player" normalization `owns_slot`/`warn_missing_player_stat_templates` already use — so two
*different* players sharing a button name (each has their own physical pad) is correctly not
flagged.

**`target.*` events** — emitted by the targeting capability (`capabilities/targeting.rs`; set
`click_selectable: true` or `targetable: true` on `PrefabDef`). Selection is **screen-space
proximity** (project each candidate to the screen via `camera.world_to_viewport`, pick the
nearest to the cursor) — NOT mesh raycasting, which raycasts bind-pose geometry and misses
animated/skinned GLB characters. Tab-cycle is nearest-first by world distance, and now processes
**every player independently each frame** — each `CharacterController`'s own `InputMap.target_next`
key press only ever changes that player's own `PlayerTarget`. `click_select_system`'s
viewport-aware camera resolution (see the split-screen section below) additionally maps the
resolved camera to its **owning player** via `OrbitCamera.target`, falling back to the primary
player for camera modes with no single owner (`PartyOrbitCamera`, the no-player default camera) —
one physical mouse can still only act for one player per click, an accepted, unavoidable
limitation.
`select_aim_height: f32` (default `1.0`) on `PrefabDef` controls how many metres above the entity
origin the click-projection aim point sits. Set lower for ground-hugging creatures (e.g. `0.4` for
a snake, `0.6` for a spider) so the selectable zone aligns with the visible body.
- `target.clicked:{id}` / `target.changed:{id}` / `target.changed` — new target selected
  (`target.changed*` only fires for the primary player — see "Per-player targeting" above)
- `target.cleared` — target cleared (click on empty space, `ClearTarget` action, or `LoadScene`;
  same primary-player-only gate)

The capability also writes three `GameVariables` for UI labels (bind whichever you need):
`target_display` (`"<prefab> <id>"`), `target_name` (prefab key), `target_id` (spawn id).
Entities carry a `PrefabKey` component (catalog key) alongside `SpawnId` (instance id) to
support this. **These three vars go blank whenever 2+ players are present** (computed via a plain
`CharacterController` entity count, not gated on real split-screen camera state) — there is no
single meaningful "the" target across independent players; use the new per-viewport `target_hud:`
scene block (`docs/20_data_formats.md`) instead for a 2+ player scene's readout. Single-player
scenes are unaffected — the vars keep populating exactly as before this feature.

**Target indicator** (`capabilities/target_indicator.rs`) — a ground-ring decal that tracks the
selected entity, now **one independent ring per player**. Activated via `target_indicator:` in
scene RON (references a `decals:` catalog key). `target_indicator_system` runs in `Update`,
watches `LoadedTargetIndicator` (mesh-cache rebuild) and each player's `Changed<PlayerTarget>`
(spawn/despawn that player's own ring only), and manages one `TrackingTarget` entity per player.
`TrackingTarget` now carries both `target: Entity` (the tracked world entity) and
`owner: Entity` (the player entity whose ring this is), instead of just the tracked entity — the
`owner` field is what lets one player's target change despawn/respawn only their own ring without
touching any other player's. The indicator is tagged `LevelEntity` and does NOT go through the
action pipeline (it is a pure cosmetic side-effect of the target state).

Ring colour is resolved per-target at target-switch time via three-tier precedence **only in a
single-player scene**:
1. `PrefabDef.indicator_color` — direct RGBA override on the prefab (highest priority)
2. `PrefabDef.indicator_category` — string key looked up in `TargetIndicatorDef.named_colors`
3. `TargetIndicatorDef.color` — scene-level fallback

**Whenever 2+ players are present, every ring is tinted by the fixed `PLAYER_LABEL_COLORS`
palette instead** (same palette the split-screen "P{n}" corner HUD label uses, see
`capabilities/camera.rs`) — the per-target precedence above is overridden entirely, so it's
visually obvious whose ring belongs to whom. If two players target the same entity, both rings
render, coincident, each in its own player's colour; there is no deduplication. This is a
deliberate design decision (`planning/features/per_player_split_screen_targeting.md`), not an
oversight — a per-target colour would make it impossible to tell whose ring is whose once two
players can each select something different.

The system reads the target entity's `PrefabKey` component, looks up the prefab in `LoadedPrefabCatalog`,
and applies the precedence chain (single-player only). Material handles are memoised by resolved `[u32;4]` colour bits —
alternating between two targets/players of the same resolved colour creates no new `StandardMaterial`. The mesh handle
(radius-driven, colour-independent) is a single cached `Local`; both caches clear on scene change.

**Per-viewport target HUD readout** (`capabilities/camera.rs`'s `target_hud_spawn_system`/
`target_hud_update_system`) — opt-in via the new `GameSceneV2.target_hud: Option<TargetHudDef>`
scene field (`docs/20_data_formats.md`). Mirrors the existing `split_viewport_player_label_spawn_
system`/`_update_system` pattern exactly: one `Text` entity per `SplitViewportSlot` camera
(`Added<SplitViewportSlot>`-triggered spawn, only when the scene authors a `target_hud:` block),
kept in sync every frame with that camera's owning player's `PlayerTarget` (via
`OrbitCamera.target`) and the camera's live `Camera.viewport`/`is_active`. Anchored bottom-left
(the corner label is top-right) so the two never collide. `target_hud_update_system` is chained
`.after(split_screen_viewport_system)` in `lib.rs`, same ordering guarantee as the corner-label
update system, so there's no stale-frame risk across a `dynamic` split's merge/split transition.
`TargetHudDisplay` (`Full`/`NameOnly`/`IdOnly`) controls which of prefab/id/name the readout shows.

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

## Despawning: prefer `try_despawn()` when an entity may already be gone

If a system's own logic — not just concurrent/external interference — can plausibly queue two
`commands.entity(e).despawn()` calls for the same entity in one run (e.g. two independent code
paths in the same system both deciding "this entity should go away" from the same query snapshot,
since `Commands` are deferred and don't remove the entity from that snapshot until the system
finishes), use **`try_despawn()`** instead of `despawn()` for at least the second/later call site.
`despawn()` on an already-despawned entity logs a warning via Bevy's default `warn` error handler
(harmless — the generation check prevents any actual corruption — but noisy); `try_despawn()`
silently no-ops in that case. Found via a real bug: `target_indicator_system` iterated one query
snapshot in two separate passes (dead-target cleanup, then owner-retarget replacement), and a ring
hit by both in the same frame got queued for despawn twice. Fixed there with an explicit
per-frame `HashSet<Entity>` dedup guard (chosen for clarity at that call site), but `try_despawn()`
is the lower-ceremony default for new code hitting this same shape — reach for it first unless you
specifically need to know whether a despawn actually happened (`try_despawn()`'s return type is
`EntityCommands`, not a bool, so if the call site needs that signal, keep the explicit dedup guard
instead). Sibling instances in `action_executor.rs`'s `StopMusic`/`PlayMusicLoop`, duplicate
`Action::Despawn`, and `UnloadOverlay`/`ToggleOverlay` are now fixed with `try_despawn()` too —
the `UnloadOverlay`/`ToggleOverlay` pair (plus `scene_loader.rs`'s `LevelEntity`/overlay teardown
sweeps) turned out to be more than theoretical: recursive `despawn()` on a widget anchor (e.g. a
`Pixel`-style `world_stat_bar` or nameplate, both of which attach their own children as separate
`LevelEntity`-tagged siblings via `add_child`) kills those children too, so the sweep's later
iteration over an already-recursively-despawned child hits the exact same warning — confirmed via
a real console warning during `local_coop_hot_join_leave.md`'s playtest (unrelated to that feature
itself; just the first playtest to chain enough scene loads through split-screen-widget-bearing
rooms to surface it).

**Tagging a widget's own children with `LevelEntity`/`OverlayEntity` (redundant with recursive
despawn) is deliberate, not an oversight — don't "clean it up".** The teardown sweeps above are
the only thing that removes these entities on a scene/overlay transition; if a future refactor
ever changes them from a flat query-and-despawn-everything sweep to something that walks only
root/anchor entities and relies on Bevy's recursion for the rest (a `Without<ChildOf>` filter, for
instance), an untagged child would leak the moment its actual parent-despawn path changed for any
reason. Keeping every level of the hierarchy self-tagged means the sweep is correct regardless of
which entity in a tree gets despawned first or by what mechanism — `try_despawn()` (per this
section) is what makes that safe to keep, not a workaround to eventually design away.

### Player-construction sites

Any feature that changes player spawning (local co-op, character select, respawn, possession)
must account for all of these or players diverge silently. Before
`player_model_source_unification.md` v1, this was **four** genuinely separate sites — one of
them (the primitive/capsule path) bypassed `PlayerConfig` entirely and silently lacked
`PlayerIndex`/material override/`StatMap`. v1 collapsed that gap for the common case; what
remains is:

1. **Unified scene-load collector** — `scene_loader.rs` builds `player_configs: Vec<PlayerConfig>`
   from every scene entity whose prefab has `tags: ["player"]`, **for both GLB (`kind: Actor`) and
   primitive (`kind: Primitive`) prefabs**, via the shared `assemble_player_config()` helper
   (`entity_spawner.rs`) — dispatched on `prefab.kind == PrefabKind::Primitive`, not on
   `shape`/`children` presence. This is the only collector now; the old separate
   primitive-collector-plus-inline-spawn path is gone for the non-terrain case.
2. **Dynamic spawn** — `action_executor.rs`'s `Action::Spawn` handler assembles a `PlayerConfig`
   for a `tags: ["player"]` prefab (the character-select flow). **GLB-only in practice**: a
   primitive-shaped player prefab has no `model` key (empty string), so the
   `asset_catalog.models.get(&prefab_def.model)` lookup fails and rejects it with a `warn!` before
   `assemble_player_config` is ever reached — v3-deferred, not a v1 regression (primitive players
   never worked here).
3. **Terrain-deferred spawn** — `spawn_delayed_players_system` via `PendingPlayerConfig`
   (`Vec<PlayerConfig>`). Also **GLB-only in practice**: a primitive player prefab combined with
   `scene.terrain: Some(...)` gets a scene-load `warn!` and an `ironhold_cli validate` error
   (`unsupported_primitive_player_on_terrain`) instead of spawning — v3-deferred, since the
   built-materials map/mesh-asset access primitive body construction needs isn't yet threaded
   through this resource-poor path (see `planning/features/player_model_source_unification.md`'s
   v3 section).
4. **Shared spawn functions** — `entity_spawner.rs`'s `spawn_player_entity` (single player, own
   `OrbitCamera`), `spawn_players_and_camera` (1+ players; 2+ share one camera or split-screen),
   and `spawn_player_when_terrain_ready`. All three call the private `spawn_player_entity_core`,
   which dispatches body construction on `PlayerConfig.model_source: PlayerModelSource` (`Glb(key)`
   or `Primitive { shape, params, children }`) — everything **after** that dispatch (physics
   bundle, `tag_spawned_entity`, `PlayerIndex`/`PlayerOwnership`/`PlayerTarget`, `StatMap`, stat
   widgets, nameplate) is now shared, unconditional code for both model sources, not a GLB-only
   path. Only site 1 passes a real `PrimitivePlayerCtx` (mesh/material assets, prefab catalog,
   built-materials map) so the `Primitive` arm can actually build a body; sites 2 and 3 pass `None`
   and would panic if they ever reached the `Primitive` arm — which they can't, since both reject
   primitive-shaped prefabs earlier (see above).
5. **Hot-join spawn** (`local_coop_hot_join_leave.md`) — `action_executor.rs`'s `Action::JoinPlayer`
   arm assembles a `PlayerConfig` via the same `assemble_player_config()` helper as site 2, then
   overrides `PlayerIndex` to the target slot and pushes a `QueuedSpawn` with `is_hot_join: true`.
   `drain_spawn_queue_system`'s `is_hot_join` branch calls `spawn_player_entity_core` directly
   (camera-less, `PrimitivePlayerCtx: None` — **GLB-only**, same reasoning as site 2) followed by
   `spawn_split_camera_for_player` (a thin wrapper factored out of `spawn_players_and_camera`'s
   `Grid` loop, adding just `SplitViewportSlot`/`Camera.order`), then increments
   `ActiveSplitSlotCount` by one. Scoped to `Grid`-split scenes only — see the doc comment on
   `ActiveSplitSlotCount` below, which this site resolves. When triggered by a gamepad
   (`gamepad_hot_join.md`), this site also overrides `player_config.inputs.gamepad_index` to the
   pressing pad — see "Gamepad-triggered hot join" above.

Because `PlayerIndex`, `PlayerTarget`, `StatMap` (when `stat_templates` is non-empty), stat
widgets, and material override are now inserted in the shared post-dispatch code rather than
per-model-source, **a new "every player gets X" component only needs adding in one place**
(`spawn_player_entity_core`, after the model-source match) instead of being checked against
multiple divergent spawn paths — this is the exact class of bug the old four-site inventory above
existed to flag, and the risk surface for it is now much smaller. What still needs checking
against multiple paths: whether a *new* `PlayerConfig`/`PrefabDef` field is forwarded correctly in
`assemble_player_config`'s two call sites (1 and 2 above), and whether a fix belongs in the
model-source-dispatch match (body-construction-specific) vs. the shared post-match code
(everything else).

Note `PlayerIndex` can still be entirely absent from an entity in principle — "primary player" is
defined as "`PlayerIndex(0)` **or no `PlayerIndex` at all**" (`capabilities/targeting.rs::
is_primary_player`) — but as of v1 this is no longer reachable via any *spawning* primitive
player; it's only meaningful for the v3-deferred terrain/character-select paths, which don't spawn
primitive players at all yet (sites 2/3 above reject them outright, they don't spawn one without a
`PlayerIndex`).

**`stat_label`/`world_stat_bar` on players** (`planning/features/player_stat_widgets.md`) —
players get the exact same floating-widget mechanism NPCs/props/`Action::Spawn` entities use,
routed through the existing `DynamicStatUiQueue`/`drain_dynamic_stat_ui_system` rather than a
player-specific spawn path: `spawn_player_entity_core` pushes a `DynamicStatUiEntry` (with
`{self}` already resolved against that player's own spawn ID) when
`PlayerConfig.stat_label`/`.world_stat_bar` is set — for both GLB and primitive players as of v1,
since this push happens in the shared post-dispatch code, not per-model-source. The actual
`Text2d`/`Mesh2d` entity-spawning logic lives in
`capabilities/stat_display.rs::spawn_stat_label_widget`/`spawn_world_stat_bar_widget`. Since
`spawn_scene_v2` is already at Bevy's 16-top-level-param `SystemParam` ceiling, `DynamicStatUiQueue`
is bundled into the existing `SceneV2Params` struct rather than added as a bare param.

**`PrefabDef.material` does NOT automatically apply via the generic spawn path — it needs the
player path's own insertion, and both model sources need it.** `spawn_prefab_instance` (the
generic Actor/Prop/NPC path) reads `prefab.material` and inserts `PendingMaterialOverride`;
`spawn_player_entity_core` is completely separate and needs its own insertion via
`PlayerConfig.material: Option<String>`, forwarded by `assemble_player_config`. Before v1 this only
worked for GLB players (the primitive/capsule path bypassed `PlayerConfig` and this insertion
entirely — this bit Stage 6's local co-op 4-way split during playtest, for the GLB case). v1 fixed
it for primitive players too, since the insertion is now in the shared post-dispatch code. **Any
future `PrefabDef` field meant to affect rendering/visuals must be checked against the player path
in addition to the generic one** — same class of bug the site inventory above exists to prevent.

**Deliberately NOT unified in v1** (kept as pre-existing behavioral divergence, not a bug):
collider sizing (GLB derives it from `movement`-config-driven capsule dimensions; primitive derives
it from the prefab's own `shape`/`params`) and the zero-`Friction` component (primitive players get
one, preventing catching on cube edges; GLB players don't) — see
`planning/features/player_model_source_unification.md`'s v2 section for the open question on
whether to reconcile the latter.

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
  This was a deliberate architecture-review fix during planning: `Grid` was built as the
  foundation for hot-join (`local_coop_hot_join_leave.md`, now implemented) — deriving the count
  live would silently reflow the grid on any mid-transition entity churn, whereas a stored,
  explicitly-written count doesn't. `drain_spawn_queue_system`'s `Action::JoinPlayer` branch is now
  the second writer of this resource (alongside `spawn_players_and_camera` at scene load) —
  incrementing it by one per successful hot-join rather than recomputing from a live query is what
  makes that safe. Layout:
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

**Keyboard camera look (`per_player_camera_look_controls.md`, shipped)** closes the gap this leaves
— `orbit_button: "None"` only disables *mouse* orbit; each player can still independently turn
their own camera via `InputMap.look_left`/`look_right`/`look_up`/`look_down`, pre-resolved once at
spawn onto `OrbitCamera.look_left_key`/etc. (mirroring how `orbit_lmb`/`orbit_rmb` are already
pre-resolved rather than re-parsed every frame) and applied in `camera_orbit_system` independently
of the mouse `orbit_active` gate. `CameraConfig.look_speed` (rad/sec, default 2.0) is the shared
rate dial for this — deliberately not `orbit_speed`, which is tuned as a mouse-pixel-delta
multiplier and would be far too slow reused as a keyboard-hold rate. Pitch direction is pinned to
match the existing mouse convention (`look_up` increases `pitch` toward `max_pitch`, i.e. mirrors
"mouse down" in this codebase's convention, not a literal "up = sky" reading) — see the regression
test asserting direction, not just clamp bounds. `PartyOrbitCamera` deliberately has no equivalent
— it's shared by every player at once, with no single owner to attribute a look binding to.
Designer-facing docs (RON fields, the demo scheme table) live in `docs/20_data_formats.md`'s
"Keyboard camera look" note — this paragraph is the implementation-side summary only.

**Known limitation:** `Action::CameraShake` only queries `With<OrbitCamera>`
(`SceneStateParams::orbit_cameras` in `scene_manager/mod.rs`), so it silently no-ops on a scene
using `PartyOrbitCamera` — but **does** fire on both cameras in a `split` scene, since those are
real `OrbitCamera`s. This is an intentional consequence of split-screen using real per-player
cameras, not an oversight to fix.

**`world_label_screen_pos_system` (`lib.rs`) is viewport-aware** (fixed — this was the root cause
of "Portal room-name labels render static and mis-positioned in every split-screen room"; see
`planning/features/world_label_split_screen_positioning.md`). It queries every active `Camera3d`
(`camera.is_active`, not `.single()`) and, per `WorldLabel`, picks the `WorldLabelRank`-th
(default 0 when the component is absent) active camera whose own `logical_viewport_rect()`
actually contains the point's `world_to_viewport()` projection — deterministic order:
`SplitViewportSlot` index first (cameras with no slot, e.g. `PartyOrbitCamera`, sort last),
tie-broken by `Entity`. This fixes positioning for every `WorldLabel` consumer at once (room
labels, entity labels, stat labels, damage popups, nameplate anchors), since they all share this
one system.

**Scene-level `world_labels:` (portal room-name labels) duplicate across simultaneously-visible
split viewports** (2026-07-10 playtest amendment — Frank found the single-camera-per-label
behavior above showed a label vanishing from a fixed split screen's *other*, still-fully-rendered
viewport whenever a portal became visible in both at once). `scene_loader.rs`'s `world_labels:`
spawn loop now spawns `MAX_SPLIT_PLAYERS` (4) sibling entities per authored label — ranks 0..3,
via the `WorldLabelRank(u8)` component — instead of just one. Each sibling independently binds to
a different active-camera priority in `world_label_screen_pos_system`'s selection above, so up to
4 simultaneously-visible active split viewports each get their own correctly-positioned,
independently-hideable copy. **Extended to `stat_label` and `Ascii`-style `world_stat_bar` in
Phase 4** (`planning/features/split_screen_camera_followups.md`) — same rank-duplication
pattern, at both spawn sites (`scene_loader.rs`'s scene-load loops and
`drain_dynamic_stat_ui_system`'s `Action::Spawn`/wave-spawn path), but gated on the loading scene
actually being split-screen (`player_configs.first().camera.split.is_some()` at scene-load time,
or `ActiveSplitScreen`/`DynamicSplitConfig` at runtime) — unlike `world_labels:`/`label:`, which
duplicate unconditionally. The gate exists because these widgets are rewritten every frame by
`stat_label_update_system`/`world_stat_bar_update_system` regardless of `Visibility`, so
unconditional duplication would be pure per-frame overhead in every ordinary (non-split) scene;
ordinary scenes get exactly 1 entity per widget, unchanged. **Extended to `ShowDamagePopup`/
`ShowFloatingText` in Phase 2 of `per_player_split_screen_targeting.md`** — same gate
(`action_executor.rs`'s `Action::ShowDamagePopup`/`ShowFloatingText` handlers read
`SceneStateParams.active_split`/`dynamic_split`), needed so a damage popup or floating text shows
in whichever viewport the target is actually visible in, not just the single highest-priority
active camera regardless of which player's action triggered it — surfaced during that phase's
playtest (a damage popup consistently appeared in player 1's viewport even when player 2 was the
one dealing the hit). **Extended to `Pixel`-style `world_stat_bar` in
`pixel_world_stat_bar_split_screen_duplication.md`** — `spawn_world_stat_bar_widget`'s `Pixel`
arm now duplicates its whole anchor+children hierarchy per rank exactly like the `Ascii` arm
already did (border/background mesh+material handles are registered once and cloned across
ranks; the fill is created fresh per rank). **`Icon`-style `world_stat_bar` built in with
day-one split-screen support** (`world_icon_stat_bar.md`) — its arm uses the same per-rank anchor
pattern from the start (texture + `TextureAtlasLayout` registered once and cloned across
ranks/cells; each `Sprite` cell created fresh, matching Pixel's fill-sharing precedent).
**`Textured`-style `world_stat_bar` also built in with day-one split-screen support**
(`world_textured_stat_bar.md`) — a 9-sliced continuous fill bar cropped from one shared
`texture_sheet` via a static `Sprite.rect` per layer (no `TextureAtlasLayout` needed, unlike
`Icon`, since each layer only ever draws one fixed sub-rect). The one `Handle<Image>` and the
`TextureSlicer`/`SpriteImageMode` are registered once and cloned across both layers and every
rank; the empty/track layer is static (`bg_color`-tinted once at spawn), only the fill layer's
`custom_size`/`color` update per frame via `world_textured_bar_update_system`
(`WorldTexturedBarFillMarker`), mirroring `world_pixel_bar_update_system`'s translation math and
change-detection guards exactly. Replaced the `Icon` hearts bar on `3rd_person_game_demo`'s
`player_male`/`player_female` as its playtest demo. **Damage popups and nameplate anchors remain
single-instance** (no `WorldLabelRank`, implicit rank 0 = highest-priority camera only) — the same
multi-viewport gap still applies to them; extend the same pattern to a given consumer's spawn site
only if a real project need surfaces.

**`particle_renderer.rs`'s billboard orientation is now viewport-aware** (fixed — Phase 1 of
`planning/features/split_screen_camera_followups.md`). `rebuild_pool_meshes_system` used to call
`camera_q.single()` with no `is_active` filter at all, so it fell back to unconditional world-axis
billboarding (`Vec3::X`/`Vec3::Y`) in *every* split-screen project, not just when 2 cameras were
simultaneously active — the widest-reaching of the four sites below. It now filters `is_active` and
picks the highest-priority active camera via the new shared
`capabilities::camera::camera_priority_key(entity, slot)` helper (same `SplitViewportSlot`-then-
`Entity` deterministic order as `world_label_screen_pos_system`, which was refactored to call the
same helper instead of inlining its own copy). **Known, accepted limitation**: with 2
simultaneously active split cameras at different angles, particles still only billboard correctly
toward the one picked camera — true per-viewport-correct billboarding would need duplicate
particle meshes per viewport, out of scope for this fix.

**`targeting.rs`'s click-to-select is now viewport-aware** (fixed — Phase 2 of
`planning/features/split_screen_camera_followups.md`). `click_select_system` used to pick the
first active `Camera3d` via `.find(|c| c.is_active)`, ignoring where the cursor actually was — a
click in player 2's viewport could silently be evaluated against player 1's camera. It now filters
to active cameras whose `logical_viewport_rect()` contains the cursor position before running the
nearest-entity search, using the same shared `camera_priority_key` comparator as Phase 1 to break
ties (cursor exactly on a shared viewport boundary) deterministically. **Known, minor behavior
change**: a click in a screen region no active camera's viewport covers (e.g. a dead grid quadrant
per Stage 6's 3-player 2×2 case) now does nothing, whereas the old arbitrary-camera pick used to
fall through to "clicked empty space" and clear `CurrentTarget`. Invisible in ordinary
single-camera scenes (a full-window viewport covers every in-window cursor position).

**`nameplate.rs`'s distance-culling is now viewport-aware via store-and-read** (fixed — Phase 3 of
`planning/features/split_screen_camera_followups.md`). `nameplate_visibility_system` used to call
`camera_q.single()` with no `is_active` filter at all, so it silently no-op'd whenever 2+
`Camera3d` entities existed *at all* — not just when 2+ were simultaneously active (a merged
dynamic split with one inactive sibling camera hit this too). It no longer queries cameras
directly: `world_label_screen_pos_system` (which already selects one active camera per
`WorldLabel` each frame, containment-tested) now also stashes that camera's distance onto a new
`NameplateCameraDistance(Option<f32>)` component on the nameplate anchor — `None` on every
early-return path (tracked entity gone/hidden, or no qualifying camera this frame),
`Some(distance)` on the success path. `nameplate_visibility_system` reads that stashed value
instead of independently re-selecting a camera, guaranteeing the two systems can never disagree on
which camera is authoritative for a given anchor's position and visibility. An anchor with no
stashed distance (off every active viewport) is treated as out-of-range (hidden), matching the
prior no-op contract for "no qualifying camera." **Known, minor behavior change**: the culling
distance is now measured from the anchor's actual world position (tracked entity origin +
`NameplateOptionsDef.offset`, default `(0, 2.4, 0)` — the point that's actually drawn on screen)
against the viewport-selected camera, instead of the old `.single()` path's entity-origin-to-only-camera
distance. Arguably more correct (culling the point that's actually rendered), and the difference is
sub-metre at normal `max_distance` scales, but it is a real change near the boundary. **Accepted
limitation** (unchanged from before this fix): nameplate anchors remain single-instance (Phase 4
does not extend `WorldLabelRank` to them), so an entity's nameplate still shows in **at most one**
simultaneously-visible split viewport, never duplicated across two.

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
specific gamepad *in addition to* the keyboard (additive, not a replacement — see the doc comment
on the field itself). Bevy has no built-in numeric gamepad index (each
connected pad is its own `Gamepad` entity); `resolve_gamepad` (`runtime/input.rs`, `pub(crate)`)
takes an already-sorted-by-`Entity::index()` slice built once per system per frame and resolves
`gamepad_index` against it — every gamepad-consuming system (`input_translator_system`,
`tab_targeting_system`, `interactable_system`, `camera_orbit_system`, and — as of
`gamepad_action_bar_slots.md` — `action_bar_input_system`) builds its own sorted slice once and
calls this shared resolver, rather than re-sorting per player. `gamepad_index: 0` means "whichever
gamepad connected first this session," not a hardware-guaranteed slot.

Button/axis mapping is now fully RON-configurable via `InputMap` — `gamepad_jump`/`gamepad_run`/
`gamepad_interact`/`gamepad_target_next: String` (parsed via `InputMap::parse_gamepad_button`,
mirroring `parse_key`'s validation seam — an unrecognized name `warn!`s and no-ops rather than
crashing) and `gamepad_deadzone: f32`, all defaulting to the same values every scene had hardcoded
before this field existed (`South`/`East`/`West`/`North`/`0.15`). Left stick moves/strafes, right
stick X turns, right stick Y drives camera pitch (via `OrbitCamera.gamepad_index`/
`gamepad_deadzone`, pre-resolved at spawn from the player's own `InputMap` — same
spawn-time-resolution pattern as `look_left_key`/etc.) — independent of the keyboard's
`strafe_mouse_button` toggle (that only exists to disambiguate A/D on one keyboard; a gamepad
already has separate sticks). `gamepad_interact`/`gamepad_target_next` fold into
`interactable_system`'s/`tab_targeting_system`'s existing per-player `keyboard || gamepad`
boolean, so both work in local co-op, not just single-player — no gamepad path exists for camera
*yaw* (right-stick-X already drives character turning), a permanent, deliberate keyboard/gamepad
parity gap, not an oversight (see `docs/20_data_formats.md`).

**Gamepad-triggered hot join** (`gamepad_hot_join.md`) adds a second, *global* gamepad-binding
surface alongside the per-player `InputMap.gamepad_*` fields above — `ProjectGamepadBindings`/
`LoadedGamepadBindings` (`runtime/scene_manager/mod.rs`), populated from
`ProjectConfig.global_unclaimed_gamepad_bindings`/`GameSceneV2.scene_unclaimed_gamepad_bindings` at exactly the three
sites `ProjectKeyBindings`/`LoadedKeyBindings` already use (two in `project_loader.rs`, one in
`scene_loader.rs`), same per-key overlay semantics. `unclaimed_gamepad_trigger_system`
(`runtime/input.rs`, `.before(message_interpreter_system)`) checks these bindings only against
gamepads **not** already claimed by a live player's `InputMap.gamepad_index` or by an undrained
`is_hot_join` entry in `PendingEntitySpawns` — on a `just_pressed` match (no separate "live
signal" prefilter: a phantom/dead duplicate pad, see the troubleshooting note above, never
produces that edge on anything) it emits the usual `UiEvent::ButtonPressed` **and** writes the
matched gamepad's `Entity` into a new `PendingJoinGamepad(Option<Entity>)` resource — at most one
pad captured per frame (deterministic: lowest `Entity::index()`-sorted), reset to `None`
unconditionally at the top of every run so a non-join gamepad trigger (e.g. a pause button) can
never leave a stale pad identity for a later frame's keyboard-triggered join to inherit.
`Action::JoinPlayer`'s executor arm (site 5 in "Player-construction sites" below) `.take()`s this
resource after resolving the joiner's `PlayerConfig` and, if set, overrides
`player_config.inputs.gamepad_index` to that pad's sorted index — translated via the same
sorted-by-`Entity::index()` convention `resolve_gamepad` uses — instead of whatever
`join_prefab_keys` statically authored. A keyboard-triggered join sees the resource already `None`
and is unaffected. This override does **not** disable the joiner's keyboard scheme — gamepad and
keyboard inputs are read additively (`||`), never exclusively, everywhere in this file's gamepad
routing above.

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

- **`stat_label` / `world_stat_bar` depth scaling** — `LoadedLabelDepthScale(Option<LabelDepthScaleDef>)` stores the active scene's `label_depth_scale` block (populated in `spawn_scene_v2` alongside `ActiveTonemapping`). `drain_dynamic_stat_ui_system` calls `resolve_label_depth_scale(res.0.as_ref(), None)` — the exact same call scene-placed stat labels/bars make (`scene_loader.rs:1021`/`:1037`) — so a wave-spawned enemy's stat label/bar shrinks with distance identically to a scene-placed one. Note there is no per-widget override field on `StatLabelDef`/`WorldStatBarDef` (unlike `WorldLabelDef`/`EntityLabelDef`, which do have `depth_scale: Option<bool>`) — stat widgets always simply inherit the scene setting. `style: Pixel` world stat bars remain excluded from depth scaling either way (pre-existing, documented limitation — the anchor entity's `depth_scale` is deliberately left `None`).

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
SpawnEffect(key: "hit_spark",     position: (0.0, -100.0, 0.0)),  // warms additive pipeline
SpawnEffect(key: "campfire_smoke",position: (0.0, -100.0, 0.0)),  // warms blend pipeline
SpawnEffect(key: "campfire_body", position: (0.0, -100.0, 0.0)),  // warms PoolFlameMaterial pipeline
```

Place these alongside `PreloadScene` / `PreloadPrefab` calls so they fire during the natural loading pause, before the player can interact.

**Budget footgun**: warmup `SpawnEffect` calls at `y=-100` are real particle allocations and consume `ParticleBudget`. In scenes with a tight budget (e.g. `particle_budget: 100`), 3–4 warmup effects can each fire their full `particle_count` against the cap. Either use low-count effects for warmup, place warmup calls on `scene.ready` before continuous emitters fill the pool, or account for warmup cost when sizing the budget.
