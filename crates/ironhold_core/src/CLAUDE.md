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

**`{new_id}` substitution** — independent of `{self}`/`{target}`, and supported on `Spawn`'s
`id: Option<String>` only. (Not "the only `Option<String>` action field" — `Spawn.spawn_point`,
`SpawnEffect.entity`, and `ProjectDecal.entity` are also `Option<String>`, but each of those
*references* an existing name, so a freshly-minted id is meaningless there; `id` is the only field
where minting a new identity makes sense.) Resolves to a fresh, monotonically increasing counter
value (`SpawnRegistry.counter` — the same counter that already backs the auto-generated id used
when `id` is omitted entirely) — use it to compose an id that won't collide with an earlier spawn
from the same source, e.g. `Spawn(prefab: "...", id: "{self}_corpse_{new_id}")`, so a slot that
spawns the same corpse id (`{self}` is always the *same* literal for a given monster slot, since it
always respawns under its own stable id) more than once in a scene's lifetime doesn't reuse the
same literal. **This is not an absolute uniqueness guarantee** — it only guarantees no collision
among ids produced by `{new_id}`/the auto-generated fallback; a hand-authored literal id of the
same shape (e.g. a scene-placed entity literally named `"crate_1"`) can still collide with a
`{new_id}`-derived `"crate_1"`, since `SpawnRegistry.entities` is one flat namespace regardless of
how an id was derived. `action_executor.rs` warns if the resolved id is already registered (same
diagnostic that now also covers the pre-existing plain-literal-collision case), and separately
warns if the resolved id still contains a literal `{` (a typo'd `{new_id}`, or `{self}`/`{target}`
authored somewhere that doesn't resolve them — see below).

**The resolved id is not observable from other RON files.** Only the spawned entity's own
behavior file can reference it afterward (via `{self}`), or whatever currently holds
`CurrentTarget` (via `{target}`) — a literal `Despawn("thing_{new_id}")` typed into `rules.ron`
will never resolve, since `{new_id}` only exists as a token at `Action::Spawn` authoring time, not
as a value anything else can look up. Use `{new_id}` only for entities that manage their own
lifetime (despawn themselves, or are reached via `{target}`).

**`{self}` does not currently resolve inside a dialogue choice's `Spawn.id`** —
`capabilities/dialogue.rs`'s `substitute_self_in_action` has no `Action::Spawn` arm (falls through
to `other => other`), a pre-existing gap unrelated to `{new_id}`. `{new_id}` still resolves
correctly there regardless, since it's resolved later, at the executor — but don't rely on
`{self}` inside a dialogue-authored `Spawn.id` until that gap is closed.

Unlike `{self}`/`{target}` — resolved by the interpreter systems (`message_interpreter.rs`) before
the action reaches `ActionQueue` — `{new_id}` is resolved by `action_executor.rs`'s `Action::Spawn`
arm itself, at the moment `id` is actually consumed; this is deliberate, not an inconsistency —
it's the only place with mutable access to the counter without threading a new resource through
every interpreter call site for a token exactly one field uses, and resolving downstream of every
interpreter is what makes it work uniformly regardless of which of the three interpreters (or
dialogue) queued the action. Repeated `{new_id}` occurrences in one `id` string all resolve to the
same counter value (one id per spawn, not one per occurrence). Resets to 0 on `LoadScene`, exactly
like the auto-generated fallback it shares a counter with — safe, since no entity from a prior
scene (however its id was derived) survives the `LevelEntity` teardown a `LoadScene` performs.

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
not shared the same way, so `gamepad_key` only fires from the **owning player's own** resolved
`BoundGamepad` (`bound.0.and_then(|e| gamepad_query.get(e).ok())` — see "Gamepad routing" below;
post-`gamepad_player_binding_hardening.md`, this is no longer a live positional `resolve_gamepad`
lookup). `action_bar_input_system`'s query widened again to include `&BoundGamepad` and a new
`Query<&Gamepad>`. The fast-path skip (`!keyboard_fired &&
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
resolved camera to its **owning player** via `CameraTargets`, falling back to the primary
player for camera modes with no single owner (`Party` mode, the no-player default camera) —
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

**Per-viewport ring visibility** (`SplitScreenDef.own_viewport_only`, `docs/20_data_formats.md`) —
opt-in restriction so a ring is only visible in its owner's own split viewport, instead of every
ring rendering in every viewport (the tinting above is unaffected either way; this only changes
*where* a ring renders). Built on Bevy `RenderLayers` — the first designer-facing feature to use
them; the only prior usage was `inspector.rs`'s feature-gated debug camera on reserved layer 31
(untouched by this feature). Layers 1–4 are reserved, one per split player, indexed identically to
`PLAYER_LABEL_COLORS`'s own scheme. `capabilities::camera::ring_layer_for_player(player_index)` and
`all_ring_layers()` are the sole owners of this arithmetic — every insertion site below calls one
of the two rather than re-deriving `1 + player_index % MAX_SPLIT_PLAYERS` by hand, so raising
`MAX_SPLIT_PLAYERS` can never desync one site from another. Components are only ever inserted when
`own_viewport_only == true` — zero `RenderLayers` footprint on any entity when it's `false` (the
default), verified by a regression test that default settings spawn zero `RenderLayers` components
anywhere.
- Each split `ActiveCameraMode::Orbit` — both the static `Grid`/`Vertical`/`Horizontal` loop and the
  `dynamic`-split loop (`entity_spawner.rs`'s `spawn_players_and_camera` and
  `spawn_split_camera_for_player`) — gets `RenderLayers::layer(0).with(ring_layer_for_player(
  player_index))`, keyed on `PlayerConfig.player_index`, not spawn/loop order — they can diverge
  when a scene authors player entities out of `player_index` order (see the reversed-order test);
  hot-join can NOT diverge here, since `Action::JoinPlayer` sets both `player_index` and the spawn
  slot to the same `next_slot` value.
- Each ring entity (`target_indicator_system`) gets `RenderLayers::layer(ring_layer_for_player(
  owner_player_index))` only — no layer 0, since a ring never needs to be "ordinary scene
  geometry."
- The shared `ActiveCameraMode::Party` (`spawn_party_orbit_camera`, `capabilities/camera.rs` — party
  mode's camera, also reused as `dynamic`-split's merged-state camera) gets `all_ring_layers()`
  when `own_viewport_only` is true — layer 0 plus every reserved ring layer, so the merged/party
  view still shows every player's ring. Leaving this camera componentless (implicit layer 0 only)
  would make it render **zero** rings the moment any ring restricts itself to a non-zero layer —
  this was caught during plan review, not by testing, and is the reason this camera needs its own
  explicit `RenderLayers` at all; treat it as an invariant, not an incidental detail, if this
  mechanism is ever extended.
- `TargetRingVisibilityMode` (`AllViewports` default / `OwnViewportOnly`,
  `runtime/scene_manager/mod.rs`) is the resolved runtime state `target_indicator_system` reads —
  `init_resource`'d in `lib.rs` so it's never missing, resolved by `spawn_players_and_camera` for
  every scene (including single-player/party-only), and reset to `AllViewports` on a full
  `Action::LoadScene` (`action_executor.rs`). `RenderLayers` is applied at ring-spawn time only,
  never re-applied to already-live rings — safe only because every write site to this resource is
  paired with a full `LevelEntity` teardown (rings carry `LevelEntity`), so no live ring can ever
  outlive the mode it was spawned under. A future mid-scene toggle (e.g. a settings menu) would
  need to re-tag every live `TrackingTarget` entity on an `is_changed()` branch — this resource does
  not do that today.
- Two collision/gap classes are warned rather than silently mishandled: `spawn_players_and_camera`
  warns when `own_viewport_only` is true and two players' `player_index` values collide under
  `% MAX_SPLIT_PLAYERS` (an out-of-range index, or a plain duplicate) — this would otherwise
  silently defeat the feature for that pair, unlike `PLAYER_LABEL_COLORS`' own harmless
  modulo-collision precedent (a cosmetic duplicate tint, not a broken guarantee). `drain_spawn_queue_system`
  warns when a non-hot-join `Action::Spawn` of a `tags: ["player"]` prefab lands in an
  `own_viewport_only` scene, since that path's dedicated full-window `ActiveCameraMode::Orbit` never gets a
  ring-visibility layer and so would see zero rings, not even its own.

> **`pipeline_warmup_system`'s `NoFrustumCulling` warmup pass does not touch `RenderLayers`** — it
> only inserts/removes `NoFrustumCulling` on `Mesh3d` entities for 4 frames after scene load
> (`lib.rs`). Benign today since rings reuse an already-warm ring material/mesh, but a future
> `RenderLayers` consumer added to this codebase should not assume warmup covers layer-restricted
> entities — it doesn't.

> **Bevy's directional/point-light visibility check intersects the *light's* `RenderLayers`
> (default layer 0) against each mesh's, not just camera-vs-mesh.** A mesh restricted to a non-zero
> layer only is therefore dropped from every layer-0 light's shadow pass. Harmless for rings today
> (`unlit: true`, and losing shadow-map membership is arguably desirable for a flat ground decal),
> but this reserved-layer scheme only works cleanly for unlit cosmetics — a future `RenderLayers`
> consumer restricting a *lit* prefab to a player layer would get an unexpectedly shadowless mesh.

Reader-facing entry points to trace this feature: `target_indicator_system`
(`capabilities/target_indicator.rs`) for the ring side, and `entity_spawner.rs`'s two split-camera
spawn sites (`spawn_players_and_camera`'s `Grid`/`Vertical`/`Horizontal` loop and
`spawn_split_camera_for_player`) plus `spawn_party_orbit_camera` (`capabilities/camera.rs`) for the
camera side.

**Per-viewport target HUD readout** (`capabilities/camera.rs`'s `target_hud_spawn_system`/
`target_hud_update_system`) — opt-in via the new `GameSceneV2.target_hud: Option<TargetHudDef>`
scene field (`docs/20_data_formats.md`). Mirrors the existing `split_viewport_player_label_spawn_
system`/`_update_system` pattern exactly: one `Text` entity per `SplitViewportSlot` camera
(`Added<SplitViewportSlot>`-triggered spawn, only when the scene authors a `target_hud:` block),
kept in sync every frame with that camera's owning player's `PlayerTarget` (via
`CameraTargets`) and the camera's live `Camera.viewport`/`is_active`. Anchored bottom-left
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

**Lootable corpse (loot-on-death), `planning/features/monster_corpse_loot.md`** — on death, a
monster despawns itself and is replaced by a separate, disposable corpse entity at the same
position/facing, so a fresh respawned monster and its still-lootable corpse can coexist as two
independent entities (v1 shipped a same-entity version first; this superseded it once a real
requirement — an unconditional fixed respawn delay, fully decoupled from how long the corpse
persists — made the same-entity model's inherent limitation a blocker, not just a documented
tradeoff). See `assets/projects/3rd_person_game_demo/behaviors/enemy_zombie.behavior.ron` +
`zombie_corpse`'s shared `behaviors/lootable_corpse.behavior.ron` for the full reference
implementation, and `docs/30_runtime_events_and_logic.md`'s "Lootable corpse (loot-on-death)"
section for the designer-facing walkthrough.

**The engine change this needed: `Action::Spawn.at_entity: Option<String>`** — resolves both
position and facing from a live entity's current `GlobalTransform`, via the same
`SpawnRegistry`-keyed lookup `SpawnEffect.entity` already uses. Necessary because these monsters
patrol, so the corpse's spawn transform can't be hardcoded in RON; it has to be read from wherever
the monster actually died. Precedence and substitution mirror `SpawnEffect.entity` exactly
(`{self}`/`{target}` supported at both `rewrite_self`/`rewrite_target` in `message_interpreter.rs`,
plus `action_bar.rs`'s `action_needs_target`) — but unlike `SpawnEffect`, `at_entity` resolves via
`GlobalTransform::compute_transform()` and so copies the source entity's full transform —
position, rotation, *and scale* — not just position, since it's meant to faithfully reproduce a
live entity's whole transform, not just place a particle burst. **Skips the
spawn with a warning, never falls back to the origin**, when the entity can't be resolved and no
`position`/`spawn_point` was also given as an explicit fallback — placing a dynamically-important
entity like a lootable corpse at the world origin would be worse than not spawning it at all
(`action_executor.rs`, mirroring `SpawnEffect`'s own "no entity or position resolved; skipping").
`Action::Spawn` already resolved its full `Transform` at executor time into `QueuedSpawn.transform`
before `drain_spawn_queue_system` ever reads it, so resolving `at_entity` there too means a
same-frame `Despawn("{self}")` immediately after can never race it.

**Corpse id collisions are now handled by giving every corpse a unique id, retrofitted from an
earlier structural-reuse design (`corpse_new_id_retrofit`, 2026-08-31 — see `planning/
claude_suggestions.md`'s resolved retrofit note for the full before/after).** Each monster behavior
file (`enemy_zombie`/`enemy_snake`/`enemy_spider.behavior.ron`) spawns its corpse with
`id: "{self}_corpse_{new_id}"` — the monotonic `{new_id}` token (see "Monotonic per-entity id
generation" below) — instead of the fixed, reused `"{self}_corpse"` literal the v2 design
originally shipped with. **Superseded, kept as a cautionary example of the failure mode this
retrofit closed:** the original design relied on the *live* monster always respawning under its
own stable id (`Spawn(..., id: "zombie_01", spawn_point: ...)` from a global rule — see below), so
`{self}` at any future death was always the same literal, and paired that with an idempotent
`Despawn("{self}_corpse")` immediately before every `Spawn(id: "{self}_corpse", ...)` call to
sacrifice a still-decaying earlier corpse rather than let two corpses collide on one id — a
deliberate, bounded tradeoff (`min(natural decay, time until this slot's next death)` instead of a
guaranteed full 10 minutes) rather than an unbounded id-collision bug. With ids now unique, no
Despawn-before-Spawn guard is needed at all: repeated kills of the same slot simply let every
corpse coexist and decay/be looted independently, closer to the actually-intended design.

**Corpse decay uses `Action::SetDespawnTimer`, not `EmitEventAfterDelay` — this is load-bearing,
not a style choice, though the specific bug it was chosen to avoid is now structurally impossible
regardless of mechanism (see above — ids can no longer collide across corpse generations at all).**
An earlier version of `lootable_corpse.behavior.ron` armed `EmitEventAfterDelay(event:
"corpse.decay:{self}", ...)` and handled it with an `on:` → `Despawn`. `debug-detective` proved
this unsafe under the *original* reused-id design specifically: a global, string-matched delayed
event has no owner, so a decay timer armed by an *older* corpse generation could still fire and
despawn a completely different, *newer* corpse that happened to share the same reused id — and
because every kill cycle left one more such stale timer in the global queue, this compounded over
extended play and eventually made a slot's loot permanently unobtainable, not just "corpse decays a
little early." `SetDespawnTimer` (`capabilities/despawn_timer.rs`) fixes this by construction: it's
a `DespawnTimer` component living directly on the target entity (modeled on the existing
`DamagePopup`/`damage_popup_system` self-despawn pattern), ticked by `despawn_timer_system` and
removed automatically when its entity despawns for any reason — a stale timer can never reach a
different, later entity, because there is no global registry of timers for it to leak through. Now
that ids are unique this specific hazard can't recur either way, but `SetDespawnTimer` remains the
right default for any per-entity decay timer regardless — no global event-name bookkeeping needed
to keep N simultaneously-decaying corpses from interfering with each other.
Prefer `SetDespawnTimer` over `EmitEventAfterDelay` + `Despawn` for **any** timer whose target's
spawn id might later be reused by an unrelated entity, not just this feature's corpses.

**`target_auto_clear_system` (`capabilities/targeting.rs`) clears on despawn, not just on
hidden.** It originally only checked `Visibility::Hidden` for an entity still present in
`SpawnRegistry` — correct for the engine's older hide-in-place revival pattern, but wrong once any
capability actually `Despawn`s a targeted entity (as this feature's death sequence does): the
entity is removed from the registry outright, so the old check never ran, and a player's stale
`PlayerTarget`/`CurrentTarget` selection silently survived until the same id was reused by that
slot's next respawn. Fixed (`debug-detective` finding) by treating "not found in `SpawnRegistry`"
the same as "hidden."

**`Action::Despawn` closes the container panel if the despawned entity is the one currently
open.** Without this, decaying a corpse whose loot panel is open at that moment leaves
`LoadedContainerUi.active_container` pointing at a gone entity and
`panels_open` stuck above 0 — the same permanently-blocked interact/pickup/tab-targeting symptom as
the `OpenContainer` double-count bug below, just reached from the opposite direction (`Despawn`
never closing, rather than `OpenContainer` over-opening). Fixed (`debug-detective` finding) by
running the same teardown `CloseContainer` does whenever `Action::Despawn`'s target matches the
open container.

**The dying entity's own per-entity behavior file cannot catch its own respawn timer, because it
won't exist anymore when that timer fires — and the catching rule must be pause-proof.** A
monster's `dead` state arms `EmitEventAfterDelay(event: "monster.respawn:{self}", delay_secs:
60.0)` *before* despawning itself (the delayed event is a plain `(f32, String)` entry in the global
`DelayedEventQueue`, entirely independent of the entity that armed it — see "Despawning" notes
elsewhere in this file). But once that entity is gone, `entity_fsm_interpreter_system` has no live
entity with that `SpawnId` left to match a per-entity `on:` handler against — the event needs a
**global** rule, one per scene-placed instance, keyed by the monster's *literal* scene id, exactly
the same convention `chest_01`'s own `entity.exited:chest_01 → CloseContainer` global rule already
uses. `spawn_point` (not `at_entity`) is used for this respawn `Spawn` — the replacement should
reappear at its original patrol spot, not wherever the previous instance happened to die.

These six rules **must live in `state_machine.ron`'s top-level `global_on:` block, not inside
`"playing"`'s own state-scoped `on:` list** (found by both `alignment-reviewer` and
`system-architect`, independently, during the final review pass — a critical bug, not a style
preference). `tick_delayed_events_system` ticks on raw `Time` with no pause-gate, so a monster's
30s respawn timer can fire while the game is in a non-`"playing"` state (e.g. paused); a
state-scoped `on:` handler simply never matches in that case, silently and permanently losing that
monster's respawn for the rest of the session. `global_on` fires "regardless of state, no state
change," so it always catches the event no matter what state the interpreter is in when it lands.
The event name is also unified to a single `monster.respawn:{id}` convention rather than
per-type (`zombie.respawn`/`snake.respawn`/`spider.respawn`) names, so adding a 4th monster type is
one copy-pasted rule line, not a new event-name convention to keep in sync everywhere.

**`Action::OpenContainer` guards against double-counting `panels_open`** (found by debug-detective
review during v1; still true and load-bearing here). `interactable_system` fires
`entity.interacted` for *every* interactable within radius on one keypress, not just the nearest —
two lootable corpses near each other can both queue `OpenContainer` in the same frame. The single
`ContainerPanel` UI can only ever show one container at a time regardless, so a second
`OpenContainer` while one is already open only re-targets `active_container` without incrementing
`panels_open` again — previously this over-incremented a counter that only ever gets decremented
once per `CloseContainer`, permanently suppressing interact/collectible-pickup/tab-targeting (all
gated on `panels_open == 0`, see `capabilities/inventory.rs`'s `LoadedInventoryUi` doc comment)
until the next `LoadScene`. General container-system fix, not specific to lootable corpses.

**Do NOT add `trigger_zone` to a prefab with an NPC/Dynamic rigid body** (found by debug-detective
review during v1) — a `trigger_zone` sensor gets no `ColliderMassProperties` override at spawn
(`entity_spawner.rs`'s `attach_prefab_features`), so its own volume-derived mass folds into the
*whole entity's* rigid-body mass on a Dynamic body, making it wildly heavier than intended and
effectively unpushable. Every prior `trigger_zone` usage was safe by accident — chests/merchants
are `Fixed`-body Props, where collider mass is irrelevant; the corpse prefabs here are also
`Fixed`-body Props (no `npc:` component), so this doesn't apply to them either, but it's why none
of the *monster* prefabs ever carried `trigger_zone`. Real, general engine bug tracked in
`planning/backlog.md`, not fixed here.

**Superseded from v1, kept here as a cautionary example — do not reintroduce for a same-entity
design:** the original same-entity version reused `ResetToSpawn` for revival, which meant
`Inventory` (a persistent component, unaffected by `ResetToSpawn`) had to be manually
cleared-then-refilled (`RemoveItem(..., count: 999)` then `AddItem(...)`) on every revival to avoid
either a permanently-empty or a doubled corpse — and a "looted → respawn sooner" state arming its
own faster respawn timer alongside the unlooted path's ambient one created a real stale-timer race
(system-architect finding): an entity revived early via the short timer, killed again, could then
be revived *again* prematurely when the first death's now-stale long timer finally fired against
the fresh second-death instance. Both problems are structural to reusing one entity across
multiple lifetimes with `DelayedEventQueue`'s "no cancellation" property — the current
separate-corpse-entity design sidesteps both by construction (fresh `Inventory` per `Spawn`, and
decay always ending in a real `Despawn` rather than a state transition a stale timer could
re-trigger).

### Animation resolver/playback pipeline (`capabilities/animation_resolver.rs` + `capabilities/animation.rs`)

Two-stage pipeline, chained back-to-back in `lib.rs`'s `Update` set:
`animation_resolver_system` (turns `LocomotionState` + queued `AnimationRequest`s into a single
`AnimationController.current`) → `animation_playback_system` (drives the real Bevy
`AnimationPlayer`/`AnimationTransitions` from that). **Field ownership is split between the two,
not fully owned by either** — despite the resolver's own doc comment saying it's the sole writer
of `current`, `animation.rs`'s missing-node-index recovery path also writes it (a last-resort
fallback to `base.idle`, not a normal write). The full split:

| Field | Owner | Notes |
|---|---|---|
| `current`, `transition_ms`, `should_loop` | resolver (write); playback (idle-fallback exception) | |
| `pending_seek` | resolver sets; playback clears | see below |
| `last_played`, `graph_initialized`, `node_indices`, `last_player_entity` | playback | |

**`AnimationController.pending_seek`** (`planning/features/dynamic_animation_control.md`) exists
because playback only re-triggers `transitions.play()` on `current != last_played` — a no-op for
"re-seek the *same* clip to a different fraction" (`PlayAnimationOn(..., start_at_fraction: ...)`
called twice in a row against an already-current clip). The resolver sets `pending_seek = true`
whenever it accepts a queued request that carries `start_at_fraction`/`freeze` — deliberately
**not** for an ordinary re-request with neither (e.g. rapid re-presses of a plain
`attack_light` override), which keeps its pre-existing behavior of not restarting the clip. Only
seek/freeze requests need the forced replay.

**A frozen (paused) clip must be resumed before the *next* clip plays, or it leaks forever.**
`AnimationTransitions::play`'s own fade-out guard explicitly skips creating a fade transition for
an outgoing clip that `is_paused()` — so a paused `ActiveAnimation` never decays out of
`AnimationPlayer.active_animations` on its own, and stays permanently blended at full weight
against whatever plays next. Invisible for an entity that only ever plays one clip in its
lifetime (a corpse), but immediately visible for one that cycles through several (the
`dynamic_animation_control` demo). `animation_playback_system` resumes `last_played`'s
`ActiveAnimation` (if paused) immediately before calling `transitions.play()` for the new clip —
this is not optional cleanup, it's required for `AnimationTransitions`' own fade-out mechanism to
engage at all.

**Seeking uses `ActiveAnimation::set_seek_time`, not `seek_to`.** `seek_to` intentionally replays
every animation event between the old and new time on the next update; a `0 → duration` jump
would replay a clip's entire event track. `set_seek_time` is the no-events variant — use it for
any future seek-like feature in this pipeline, even though no GLB in this project currently
declares animation events.

**Clip duration for a seek is resolved via the live `AnimationGraph`/`AnimationNodeType::Clip`,
not a separate `clip name → Handle<AnimationClip>` map.** `animation_playback_system` already has
the graph handle and node index in scope at the point it needs the duration; a second map keyed
by clip name would duplicate `node_indices`' key space and risks the exact "two maps built from
one source, allowed to desync" shape `tag_spawned_entity`'s own doc comment warns about.

**`ActiveOverride.seek_fraction`/`.frozen` are durable, not consumed-and-cleared on first
apply.** They must survive `animation.rs`'s documented GLTF-hierarchy-respawn recovery path (a
WASM-specific case: Bevy's `SceneSpawner` replaces the animated hierarchy after sub-assets finish
loading, forcing a second `transitions.play()` later in the entity's lifetime) — a one-shot seek
would silently un-freeze a frozen pose (e.g. a corpse) the moment that recovery path fires on the
web, which would be a hard-to-reproduce, web-only regression.

**Spawning an entity already posed mid-clip** (not just holding a death pose after a full
playthrough — see `docs/20_data_formats.md`'s "Spawn-already-posed pattern") needs its own
minimal `AnimationPolicy` file with `base.idle`/`walk`/`run`/`jump_loop` all pointing at the
target clip, not a reuse of a live character's full policy — see
`prefabs/animation/corpse_policy_zombie.ron`. Reusing the full policy leaves three independent
fallback paths (no active override, missing node index, graph validation failure) that all land
on `base.idle`, which for a "should look dead" entity is a strictly worse degraded state than for
a live one.

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

### Jump reset cannot rely on a ground-check edge (`planning/features/uphill_jump_lock.md`)

`player_movement_system`'s ground detection (`capabilities/player.rs`) is a fixed-reach downward
shape-cast (`collider_radius + ground_cast_length`), re-evaluated fresh every tick — **it cannot be
assumed to ever report `is_grounded = false`**. On a slope steep enough (~12°+ at shipped
defaults), the incline's rising surface closes the vertical gap a jump's Y-velocity opens *faster
than gravity does*, so the cast keeps reporting contact on every single tick, forever. The same
failure is reachable on perfectly flat ground too, with no slope involved: any player prefab whose
`jump`/`double_jump_height` apex doesn't clear `collider_radius + ground_cast_length` hits the
identical lock (a scene-load `warn!` — `scene_loader.rs::warn_jump_cannot_clear_ground_sensor` —
plus a matching `ironhold_cli validate` error, `jump_cannot_clear_ground_sensor`, flag this
misconfiguration).

**A slope-normal walkability gate is the first line of defense — added after real playtesting
found the mirror-image bug: an unbounded re-jump exploit while falling/sliding down a steep
decline.** The ground shape-cast's hit (`ShapeCastHit::details.normal1`, requesting
`compute_impact_geometry_on_penetration: true` in `ShapeCastOptions` so the normal is populated
even for the common near-zero-clearance resting case) is checked against
`CharacterController.max_walkable_slope_deg` (from `MovementConfig`, default 45°, matching Unity's
`CharacterController.slopeLimit` / Unreal's `WalkableFloorAngle` / Godot's `floor_max_angle`) —
**a surface steeper than that never counts as grounded at all**, regardless of proximity. Root
cause of the downhill bug: on any incline the sensor can't cleanly detach from (whether ascending
*or* descending), `velocity.linvel.y` is trivially `<= 0.0` for the entire descending portion, so
without this gate the tick/velocity/height reset logic below would re-arm `jumps_used` on *every*
tick once grace expires — an unbounded re-jump exploit that gets worse the longer the fall/slide
continues, exactly mirroring the original uphill lock but in the opposite direction. The gate fixes
this at the source rather than special-casing it in the reset logic: an unwalkably steep surface
is simply never "ground," so the character is correctly airborne/sliding on it and the whole reset
mechanism below never engages. **This does not replace the grace/velocity/liftoff-height mechanism
below for *walkable* slopes** (≤ `max_walkable_slope_deg`, e.g. an ordinary 12–30° hill) — a
walkable incline's contact is genuinely correct, continuous grounding while climbing or
descending it, so the sensor still can't cleanly detach there, and the bounded "pogo" cadence
tradeoff documented further down still applies to that range. See
`player_slope_jump_tests.rs::unwalkable_slope_does_not_allow_endless_rejump_while_sliding` and
`::walkable_slope_pogo_cadence_is_unaffected_by_the_slope_limit_check`.

Because of this, `jumps_used` is **not** reset on a `!was_grounded && is_grounded` edge (an
edge-triggered reset can starve permanently if the edge never fires). The landing *animation*
request (`jump_exit`) still fires on that real edge, unchanged from before this fix — a plain
fall (e.g. walking off a ledge, `jumps_used` already `0`) must still play the landing clip. The
`jumps_used` **reset** is a separate, level-gated check, re-evaluated every tick:

1. `CharacterController.jump_air_grace` — a `FixedUpdate` tick countdown, set at jump-fire time
   from `jump_air_grace_ticks()`, derived analytically from the jump's own velocity, `GRAVITY`
   (`scene_loader.rs`, `pub(crate)` specifically so this formula can share it), and the
   controller's own `collider_radius`/`ground_cast_length` — never a separate hand-tuned constant
   that could drift out of sync with a project's authored values. While `> 0`, a grounded reading
   isn't even considered as a possible landing.
2. Once grace hits `0`, the reset additionally requires **either** `velocity.linvel.y <= 0.0`
   (the jump's ballistic ascent has genuinely ended) **or** the entity having risen at least
   `collider_radius + ground_cast_length` above `CharacterController.jump_liftoff_y` (its Y
   position at jump-fire time) — proof the sensor's overlap with the liftoff pose can no longer
   explain a grounded reading.

**Both of those two extra checks are physical quantities, not clock-derived — this is deliberate,
not redundant with the tick counter.** `jump_air_grace` alone is not sufficient: it's counted in
`FixedUpdate` ticks, but Rapier's own physics stepping runs on `TimestepMode::Variable` in
`PostUpdate` (`capabilities/physics.rs`) — a *different*, framerate-coupled clock, not guaranteed
to advance in lockstep with `FixedUpdate`'s tick count. At a low enough real framerate (or one
`Time<Virtual>::max_delta`-clamped hitch), real elapsed physics time can lag behind what the tick
count assumes, so a tick-only grace could expire while the body is still genuinely rising — see
`player_slope_jump_tests.rs::grace_expiry_does_not_reset_early_when_real_physics_time_lags_ticks`,
which decouples the two clocks on purpose (`physics_dt != 1.0/64.0`) to prove this. The two checks
serve different terrain: `velocity.linvel.y <= 0.0` covers a jump whose ascent has genuinely ended
(flat ground, or a jump too short to ever clear the sensor); the liftoff-height check covers a
*continuously climbing* slope, where the contact solver keeps `linvel.y` pinned positive (matching
the climb rate) for as long as the player keeps walking uphill — `linvel.y <= 0.0` alone would
never fire there, but net height risen since the jump still grows the whole time.
`CharacterController.jump_liftoff_y`'s only clearing path is a successful reset; if a future
change ever removes the `velocity.linvel.y <= 0.0` half of that `||`, double check nothing can
leave a stale `jump_liftoff_y` behind after a non-jump position change (e.g. a teleport) — today
this is benign only because a teleport also zeroes `Velocity`, which the surviving `<= 0.0` clause
happens to catch.

**The `jumps_used`/`jump_liftoff_y` reset logic above intentionally never reads the coyote-buffered
`LocomotionState.is_grounded` — it reads a separate `raw_grounded` local, computed identically to
before coyote-time existed, in both the Rapier-context and no-physics branches.** An earlier version
of this fix (pre-coyote-time) forced `is_grounded = false` for a fixed window instead — rejected in
plan review because (1) `can_jump`'s airborne branch (`double_jump_enabled && jumps_used <
max_jumps`) would then go live immediately after a ground jump, letting a fast double-tap consume
the second jump at ground level instead of at a real airborne height, and (2) a fixed window's
safety margin is a function of authorable content (`jump` height, `collider_radius`,
`ground_cast_length`) and no single constant is safe across every project's authored values. If
touching this system again: `animation_resolver.rs`'s jump/land clip selection and `can_jump`'s
branch selection may legitimately read the coyote-buffered `loco.is_grounded` (that's the whole
point of the buffer — smoothing *feel*), but the `jumps_used` reset's grace/velocity/liftoff-height
gate must keep reading `raw_grounded`, never the buffered value.

**Real bug hit while adding coyote-time, kept here as the concrete cautionary example:** feeding the
coyote-buffered `loco.is_grounded` into the reset condition broke
`grace_expiry_does_not_reset_early_when_real_physics_time_lags_ticks` — `jumps_used` reset to `0`
while `linvel.y` was still clearly positive (~4.16). Root cause: right at the tick the sensor first
genuinely detaches, the coyote buffer keeps `loco.is_grounded` artificially `true` for several more
ticks; if the liftoff-height threshold (`risen_since_liftoff >= ground_sensor_reach()`) also happens
to cross in that same window — a check specifically designed to fire *while still rising*, for the
continuously-climbing-slope case — the reset fires prematurely on an ordinary flat-ground jump
nowhere near any slope. Two mechanisms, each correct for its own purpose, reintroduced exactly the
bug the other one already fixed. Fixed by the `raw_grounded`/`loco.is_grounded` split described
above — a debounce that smooths one consumer's *feel* must never leak into another consumer's
*correctness*-critical timing check.

### Coyote time — debounced grounding for uneven terrain (`planning/features/uphill_jump_lock.md`)

Playtesting `3rd_person_game_demo` surfaced a third, distinct problem from the two above: walking
over ordinary uneven terrain (bumps, small ledges, barely-there slope) made the character flicker
into the falling state constantly, even though no jump-lock or hover-exploit logic was involved —
just single-tick gaps in the raw ground shape-cast as the character crossed terrain irregularities
too small to be a real "leave the ground" event. Other engines solve exactly this with a debounce
buffer, universally nicknamed **coyote time** (Wile E. Coyote not falling until he looks down):
delay the grounded→airborne *transition* by a short window, refreshed every tick the sensor
genuinely reports contact.

`CharacterController.coyote_ticks_remaining` (a `FixedUpdate` tick countdown, refreshed to
`coyote_ticks(controller.coyote_time_secs)` every tick `raw_grounded` is `true`) buffers
`LocomotionState.is_grounded` — **and only that**: `raw_grounded` still exists as its own local,
computed once per tick before the buffer is applied, and is what the `jumps_used` reset logic reads
(see above). The buffer widens two things on purpose: how long the falling animation is suppressed
after a real-but-brief loss of contact, and how late a jump input can land after leaving a platform
edge and still fire (the classic "coyote time" forgiving-jump-timing benefit, not just an
anti-flicker fix). `MovementConfig.coyote_time_secs` (default `0.1`s) is a real designer-facing
tuning field, unlike `jump_air_grace` — see Q2 in the feature plan for why `jump_air_grace` is
deliberately *not* authorable while this is. `0.0`/negative both disable the buffer (negative is
flagged by `warn_negative_coyote_time_secs` + `ironhold_cli validate --strict`'s
`negative_coyote_time_secs`, since it's more likely a typo than an intentional "off").

**`can_jump`'s two branches read different grounded signals on purpose — this is not the same
mistake as feeding the buffer into the `jumps_used` reset, but it looks similar enough that three
independent post-implementation reviews all found the same real bug in the first version of this
fix.** The grounded (first-jump) branch is gated by `raw_grounded || (coyote_ticks_remaining > 0 &&
jumps_used == 0)` — deliberately buffered, since first-jump coyote-forgiveness is the whole point.
The airborne (double-jump) branch is reached only when that combined condition is false, and its own
`double_jump_enabled && jumps_used < max_jumps` check has **no** buffering in it at all — it depends
purely on the branch *not* having taken the grounded path, which for `jumps_used > 0` means purely
on `raw_grounded`. The first version of this fix instead gated the grounded branch on the fully
buffered `loco.is_grounded` with no `jumps_used == 0` qualifier — mutually exclusive with the
airborne branch, so for the entire coyote window after a real ground jump (`jumps_used == 1`),
*neither* branch was reachable: a double-jump press was silently swallowed until the buffer expired,
up to permanently for a large `coyote_time_secs`. If touching `can_jump` again: the coyote buffer
must only ever unlock a *first* jump (`jumps_used == 0`), never gate whether a *second* jump is
reachable — that must always come down to `raw_grounded` alone, exactly as if coyote-time didn't
exist.

**The ground shape-cast must exclude sensors, and must `normalize_or_zero()` a hit's normal before
using it — both found by a real playtest, not by the reviews.** `QueryFilter::new().exclude_rigid_
body(entity).exclude_sensors()` (`capabilities/player.rs`): without `.exclude_sensors()`, a nearby
prop's `trigger_zone` (a ghost `Collider::ball` + `Sensor` child, `entity_spawner.rs`'s
`attach_prefab_features`) could be swept by the ground cast just like real geometry. The cast ball
starts embedded in a large, nearby sensor sphere (the sensor's own radius, e.g. 2.5m for
`3rd_person_game_demo`'s chest, easily contains it from up to `radius + collider_radius` away), so
the resulting `time_of_impact == 0` "penetrating" hit beats the real floor's small-but-nonzero toi —
and the ball-in-ball EPA normal at that embedded position is radial, i.e. near-horizontal,
misclassified as an unwalkable wall by the slope-walkability gate above. Symptom: the player played
the falling animation while standing on ordinary flat ground, for as long as they stood within
range of *any* nearby `trigger_zone` prop. Matches the existing `.exclude_sensors()` precedent in
`capabilities/npc.rs`'s line-of-sight raycast — a sensor is a ghost collider by definition and must
never count as floor. Second, independently-found bug in the same code path: a penetrating hit's
`normal1` is not always unit length (measured ~0.52 for the ball-in-sensor case above) — a bare
`.dot(Vec3::Y).acos()` on that computes `acos(|n| * cos(theta))`, not the real angle theta, silently
biasing every penetrating-hit angle toward 90°. Fixed by `.normalize_or_zero()`-ing the normal
first; a fully degenerate (zero) result dots to 0 (90°, unwalkable), matching the existing
"no computable normal" treatment for a `details: None` hit. See
`crates/ironhold_core/tests/prop_ground_veto_tests.rs`.

**A solid (non-sensor) prop/wall pressed directly against the player could also veto the floor —
fixed by re-querying (`ground_cast`), not by touching `is_walkable_contact` (the walkability check
itself).** This feature's slope-walkability gate is what made a solid prop/wall's normal matter at
all: on `main`, the ground cast was proximity-only (`hit.is_some()`), so no collider's normal was
ever load-bearing. A prop tall enough to reach the cast ball's centre (feet + `collider_radius` +
skin ≈ 0.41m) has a penetrating (`time_of_impact == 0`) contact when the player stands pressed
against it — this always beats the real floor's non-zero toi, so `cast_shape` (which only ever
returns the single nearest hit) reported the wall instead of the floor, and the wall's
near-horizontal EPA normal then correctly-but-wrongly failed the walkability check. Silently
disabled jump entirely for any project with `double_jump_enabled: false` (every shipped project's
default) — not just a wrong animation, since `can_jump`'s only reachable branch then requires
`raw_grounded`.

Fixed by extracting the walkability check into `is_walkable_contact()` and the whole ground
shape-cast (origin lift, `ShapeCastOptions`, `QueryFilter`) into `ground_cast()` — both `pub fn`s in
`capabilities/player.rs`, shared by `player_movement_system` and its test probe
(`prop_ground_veto_tests.rs::probe()`, which now calls `ground_cast` directly instead of hand-
duplicating it, closing a real "the test and the engine can silently drift" risk a review flagged).
`ground_cast` re-queries in a bounded loop (`MAX_GROUND_CAST_CANDIDATES = 4`), excluding (via
`QueryFilter::predicate`, only attached once there's actually something to exclude) any hit that is
**both** not underfoot **and** not walkable, until an accepted candidate is found or every
candidate this tick is exhausted:

- **"Underfoot"** — the contact point (`ShapeCastHitDetails.witness1`, confirmed genuinely
  world-space for `cast_shape` despite a misleading doc comment inherited from parry — verified
  against `bevy_rapier3d-0.33.0/src/plugin/context/mod.rs`'s `RapierContext::cast_shape` doc
  comment, ~line 478) is at or below `feet_pos.y + collider_radius * 0.5`. A hit with no computable
  contact point (`details: None`) defaults to underfoot — its fate is decided by the walkability
  check either way, which itself treats a detail-less hit as unwalkable unless the `90.0` escape
  hatch is set.
- **Both conditions, not "not underfoot" alone**, is load-bearing — a first version of this fix
  rejected any non-underfoot hit unconditionally and was caught by system-architect review before
  landing: `collider_radius * 0.5` alone imposes a hidden `acos(1 - 0.5) = 60°` walkable-slope
  ceiling *independent of* `max_walkable_slope_deg` (both the tolerance and a slope's contact-height
  offset scale with `collider_radius`, so the ceiling angle doesn't depend on it) — a project
  authoring a steeper walkable slope, or a player merely spawned slightly inside geometry (a
  deep-penetration contact after a teleport/`at_entity` placement), would have had its own genuine
  floor contact wrongly excluded, turning a previously-grounded tick ungrounded. Requiring **both**
  conditions to fail before excluding makes the loop monotone — the underfoot check can only ever
  *rescue* a hit `is_walkable_contact` alone would have rejected (the wall case, the actual bug),
  never *reject* one `is_walkable_contact` alone would have accepted. This is also what makes the
  `max_walkable_slope_deg >= 90.0` escape hatch restore this project's exact pre-fix proximity-only
  behavior even after this loop was added: at `90.0` every hit is walkable, so the very first
  candidate is always accepted with zero exclusions.

**Known remaining gap, tracked as its own bug (`planning/backlog.md` ▸ Bugs), not covered by this
fix:** `QueryFilter::predicate` excludes by whole `Entity`, so a wall that's part of the *same*
collider entity as the walkable floor beneath it — any compound-collider prop with a tall-enough
component shape, not only a raised terrain edge carved into one `TriMesh` — still excludes both
together, reproducing the identical full-jump-lock symptom. No shipped project's compound colliders
currently combine a tall component with player-reachable floor geometry this way.

See `prop_ground_veto_tests.rs::solid_prop_taller_than_cast_ball_centre_no_longer_vetoes_when_pressed_against`
(and its `_on_trimesh_terrain` sibling) plus `player_slope_jump_tests.rs::walkable_slope_steeper_than_the_ground_cast_underfoot_tolerance_is_still_grounded`.

`jump_air_grace_ticks()` clamps its input velocity via `f32::max(0.0, vel)` (which also launders a
NaN velocity — from a negative/misconfigured `jump`/`double_jump_height` — to `0.0`) and floors
its result at 1 tick, so a near-zero or invalid jump height degrades to a bounded (if rapid)
re-arm rather than either a NaN-poisoned permanent lock or a same-tick event storm. The design-time
`warn!`/`ironhold_cli validate` check (below) should catch that authoring mistake before it ships;
this is defense-in-depth for if it doesn't.

One accepted, physically-unavoidable consequence: on a slope steep enough that the cast never
truthfully detaches, holding jump produces a bounded "pogo" cadence (roughly one re-jump per grace
window, ~0.26s at shipped defaults) rather than either the old permanent lock or an unbounded
hover exploit — see `player_slope_jump_tests.rs`'s cadence-bound test. Two secondary, accepted
consequences of that same tradeoff: a jump whose real airtime is shorter than the grace window
(e.g. landing on a nearby raised platform) plays its landing animation on the real edge but stays
un-rejumpable for the remainder of the window (bounded, ≤ the grace duration); and on a
never-detaching slope, `jump_exit` never fires at all (no real edge ever happens) while
`jump_enter` re-arms every pogo cycle, which can visibly pin the takeoff animation for as long as
the player holds jump uphill.

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
   `ActiveCameraMode::Orbit`), `spawn_players_and_camera` (1+ players; 2+ share one camera or split-screen),
   and `spawn_player_when_terrain_ready`. Only `spawn_players_and_camera` takes a `CameraSpawnMode`
   (`Spawn`/`Suppressed`) parameter — when a scene also has a `tags: ["flycam"]` entity, both its
   call sites (scene load, and `spawn_player_when_terrain_ready` reading the `SuppressPlayerCameras`
   resource) pass `Suppressed`, skipping every camera resource insert/spawn after the player-entity
   loop (see `planning/features/flycam_scene_conflicts.md`). `spawn_player_entity` (site 1, dynamic
   `Action::Spawn`/character-select) and the hot-join path (site 5 below) do **not** check
   `SuppressPlayerCameras` — a player dynamically spawned at runtime always gets its own camera,
   even in a scene that started in spectator mode; a known, documented limitation, not an oversight.
   All three call the private `spawn_player_entity_core`,
   which dispatches body construction on `PlayerConfig.model_source: PlayerModelSource` (`Glb(key)`
   or `Primitive { shape, params, children }`) — everything **after** that dispatch (physics
   bundle, `tag_spawned_entity`, `PlayerIndex`/`PlayerOwnership`/`PlayerTarget`/`BoundGamepad`,
   `StatMap`, stat widgets, nameplate) is now shared, unconditional code for both model sources, not
   a GLB-only path. `BoundGamepad(player_config.bound_gamepad)` is the one field here that isn't
   always `None` — see site 5 below. Only site 1 passes a real `PrimitivePlayerCtx` (mesh/material
   assets, prefab catalog, built-materials map) so the `Primitive` arm can actually build a body;
   sites 2 and 3 pass `None` and would panic if they ever reached the `Primitive` arm — which they
   can't, since both reject primitive-shaped prefabs earlier (see above).
5. **Hot-join spawn** (`local_coop_hot_join_leave.md`) — `action_executor.rs`'s `Action::JoinPlayer`
   arm assembles a `PlayerConfig` via the same `assemble_player_config()` helper as site 2, then
   overrides `PlayerIndex` to the target slot, sets `PlayerConfig.bound_gamepad` directly from any
   captured `PendingJoinGamepad` (see "Gamepad-triggered hot join" below), and pushes a
   `QueuedSpawn` with `is_hot_join: true`.
   `drain_spawn_queue_system`'s `is_hot_join` branch calls `spawn_player_entity_core` directly
   (camera-less, `PrimitivePlayerCtx: None` — **GLB-only**, same reasoning as site 2) followed by
   `spawn_split_camera_for_player` (a thin wrapper factored out of `spawn_players_and_camera`'s
   `Grid` loop, adding just `SplitViewportSlot`/`Camera.order`), then increments
   `ActiveSplitSlotCount` by one. Scoped to `Grid`-split scenes only — see the doc comment on
   `ActiveSplitSlotCount` below, which this site resolves. See "Gamepad-triggered hot join" above
   for the `bound_gamepad` hand-off `PlayerConfig.bound_gamepad` (set at line 560) feeds into.

Because `PlayerIndex`, `PlayerTarget`, `BoundGamepad`, `StatMap` (when `stat_templates` is
non-empty), stat widgets, and material override are now inserted in the shared post-dispatch code
rather than per-model-source, **a new "every player gets X" component only needs adding in one
place**
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
collider sizing — GLB derives it from `movement`-config-driven capsule dimensions; primitive
derives it from the prefab's own `shape`/`params`. This divergence is unrelated to Friction (below)
and stays.

**Unified in v2:** the `Friction` component (`entity_spawner.rs`'s `spawn_player_entity_core`) is
now inserted unconditionally for every player regardless of `model_source` — previously
primitive-only, leaving a GLB player's capsule at Rapier's default 0.5/`Average` friction while an
otherwise-identical primitive player got its own coefficient/`Min`. The coefficient is **`0.15`, not
`0.0`** — the two-scene playtest (room10 cube-edge + `quick_scene` hillside) that verified this
found `0.0` eliminated edge-catching but let an idle player creep downhill on sloped terrain
indefinitely (movement writes `velocity.linvel` directly each tick, so friction was never doing much
*while moving*; the risk was always specifically the idle case). `0.15` (still `combine_rule: Min`)
holds a slope while remaining low enough to avoid noticeably reintroducing edge-catching. `idle_drag`
(`MovementConfig`, `capabilities/player.rs`) bounds any residual creep further but cannot zero it on
its own (no grounded gate — pushing it very low also cancels air momentum after a jump), so it is a
secondary tuning knob, not the primary defense. No separate friction field was added to
`MovementConfig` — `0.15` is a fixed engine constant, not per-prefab-authorable (logged to
`planning/backlog.md`'s Icebox as a possible future physics-material field).

### Local co-op: shared camera, split-screen, gamepad routing, view-box clamp

**`ActiveCameraMode::Party`** (`capabilities/camera.rs`) is a sibling to `ActiveCameraMode::Orbit`, not a
replacement — single-player scenes are untouched. When a scene has 2+ `tags: ["player"]`
entities, `spawn_players_and_camera` reads the **first** player's `CameraConfig.party:
Option<PartyZoomDef>` and `CameraConfig.split: Option<SplitScreenDef>` as the explicit switches
(mutually exclusive — if both are set, `split` wins and a warning is logged):
- `party` set → spawns one `ActiveCameraMode::Party` framing the midpoint of all players; radius is
  `clamp(max_pairwise_separation + zoom_margin, min_radius, max_radius)`, recomputed every frame
  by `party_camera_follow_system`. `PartyZoomDef.allow_manual_zoom` (default `false`) controls
  whether scroll-wheel still nudges the derived radius via an accumulated offset.
- `split` set → spawns one **real `ActiveCameraMode::Orbit` per player** (not `ActiveCameraMode::Party`), each
  tagged `SplitViewportSlot(u32)` (which cell it owns — slot index = spawn order, i.e. entity
  order in the scene's `entities:` list). `split_screen_viewport_system` recomputes every
  `SplitViewportSlot` camera's `Camera.viewport` every frame from `Window::physical_size()`
  (physical pixels already — no manual `scale_factor()` multiplication needed, unlike a naive
  `width()`/`height()` read) and `ActiveSplitScreen`'s orientation (`SplitOrientation::Vertical`
  splits left/right, `Horizontal` splits top/bottom, `Grid` computes an N-cell grid — see below).
  Split-screen orientation lives in the `ActiveSplitScreen` resource (mirrors `ActiveViewBox`/
  `LoadedTargetIndicator` — populated by `spawn_players_and_camera`, cleared on `LoadScene`),
  **not** on `ActiveCameraMode` or `SplitViewportSlot` — this kept split-screen state out of the
  camera components, which is exactly why the `camera_modes.md` v1 unification didn't have to
  untangle it.
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
  internal `ActiveCameraMode::Party`, tuned by `DynamicSplitDef.merged_zoom_margin`/
  `merged_allow_manual_zoom` — mirrors `PartyZoomDef`'s two fields, self-contained specifically so
  dynamic split doesn't also require authoring a `party:` block alongside `split:`) and
  auto-splits into the two per-player Orbit-mode cameras once `split_distance` is exceeded, merging
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
- Neither set → logs a warning and falls back to a single `ActiveCameraMode::Orbit` targeting only the
  first player. Never silently spawns one `ActiveCameraMode::Orbit` per player without split-screen viewports
  — that would mean two cameras fighting for the same full-window viewport with no RON-visible
  symptom.

Later players' `camera.party`/`camera.split` fields are ignored entirely — only the first
player-tagged scene entity's config is read for those two switches. Scene entity order in
`entities:` therefore matters for local co-op. **Split-screen is the one case where every
player's OTHER camera fields still matter** — each split-screen player gets a real `ActiveCameraMode::Orbit`
built from their own `camera` block (offset, `zoom_speed`, `orbit_button`, etc.), not just the
first player's. A shared mouse would otherwise rotate/zoom every split-screen camera identically
(`camera_orbit_system` reads mouse input once per system call, applying the same delta to every
`ActiveCameraMode::Orbit` in its query) — split-screen scenes disable manual control per-camera instead via
RON: `zoom_speed: 0.0` (scroll × 0 has no effect) and `orbit_button: "None"` (new
`parse_orbit_button` arm returning `(false, false)`, no warning — distinct from an actually
unrecognized string, which warns and defaults to `"Either"`).

**`character_rotate_button` needs the same treatment, and was missed on every `local_coop_demo`
split prefab until `camera_modes.md` v1's playtest caught it live (added `character_rotate_button:
None` to all 15 split-screen camera blocks).** It's a *separate* switch from `orbit_button` —
`camera_orbit_system`'s `char_rotate` gate (`orbit.character_rotate_rmb && rmb`) fires independently
of `orbit_active`, defaults to `Some("Right")` (i.e. `character_rotate_rmb: true`) when omitted, and
rotates the **character**, not the camera's own yaw/pitch. Since it's gated only on the global RMB
state with no per-viewport cursor check (same limitation as `orbit_button`), an unset
`character_rotate_button` on a split-screen prefab spins *every* split player's character at once
from either viewport — confirmed via a live runtime diagnostic added temporarily to
`camera_orbit_system` during that playtest (compare `orbit_lmb`/`orbit_rmb`/`zoom_speed` logged at
spawn time vs. read live in the system — both matched and were correctly `false`/`false`/`0.0`,
which is what pointed at `character_rotate_button` as the actual unaccounted-for input). Any new
split-screen prefab must set `character_rotate_button: None` alongside `orbit_button: "None"` and
`zoom_speed: 0.0` — the docs (`docs/20_data_formats.md`) now state this as a three-field
requirement, not two.

**Keyboard camera look (`per_player_camera_look_controls.md`, shipped)** closes the gap this leaves
— `orbit_button: "None"` only disables *mouse* orbit; each player can still independently turn
their own camera via `InputMap.look_left`/`look_right`/`look_up`/`look_down`, pre-resolved once at
spawn onto `OrbitState.look_left_key`/etc. (mirroring how `orbit_lmb`/`orbit_rmb` are already
pre-resolved rather than re-parsed every frame) and applied in `camera_orbit_system` independently
of the mouse `orbit_active` gate. `CameraConfig.look_speed` (rad/sec, default 2.0) is the shared
rate dial for this — deliberately not `orbit_speed`, which is tuned as a mouse-pixel-delta
multiplier and would be far too slow reused as a keyboard-hold rate. Pitch direction is pinned to
match the existing mouse convention (`look_up` increases `pitch` toward `max_pitch`, i.e. mirrors
"mouse down" in this codebase's convention, not a literal "up = sky" reading) — see the regression
test asserting direction, not just clamp bounds. `ActiveCameraMode::Party` deliberately has no equivalent
— it's shared by every player at once, with no single owner to attribute a look binding to.
Designer-facing docs (RON fields, the demo scheme table) live in `docs/20_data_formats.md`'s
"Keyboard camera look" note — this paragraph is the implementation-side summary only.

**Fixed (`camera_modes.md` v1, was a known limitation before):** `Action::CameraShake`
(`SceneStateParams::orbit_cameras` in `scene_manager/mod.rs`) now queries `Or<(With<OrbitCameraMode>,
With<PartyCameraMode>)>`, so it fires correctly on a `party:` scene's shared camera too, not just
on both cameras in a `split` scene (which already worked, since those are real independent
`Orbit`-mode cameras). Deliberately still excludes `Fixed`/`FirstPerson`/`Flycam`'s markers — a
flycam scene must keep getting the explicit `warn!("no orbit camera in scene — shake ignored")`
instead of a silent overwrite, since `fly_camera_system` runs after the shake system and
unconditionally rewrites `Transform::rotation` every frame.

**`Action::SetCameraMode` / `camera_modes:` registry (`camera_modes.md` v2):** the runtime switch
lives in `action_executor.rs`; `entity_spawner.rs::apply_camera_mode` (`pub(crate)`) is its
switch-time analog of the spawn-time per-mode match arms in `spawn_active_camera_for_player` —
some deliberate duplication between the two, logged in `planning/claude_suggestions.md` rather than
unified in this pass (touching the shipped v1 spawn path for a v2-only feature). Three things worth
knowing before touching this code:
- `AuthoredCameraMode` (a camera's scene-authored starting mode, written once at spawn, never
  mutated) and `ActiveCameraMode` (the live, per-frame-mutable state) are deliberately two separate
  components — `SetCameraMode(mode: "default")` resolves against the former, everything else
  against `LoadedCameraModes` (the current scene's registry, inserted in `scene_loader.rs`'s
  Replace branch only, mirroring `LoadedSpawnPoints`).
- `CameraBlendState`/`camera_blend_system` blend the *rendered* `Transform`/FOV from a pre-switch
  snapshot toward whatever the *already-switched* mode's own per-frame system computes that same
  frame (Design A: let the new mode run unsuppressed, then interpolate on top) — it does **not**
  precompute a target pose itself, so it must run after every per-mode system in `lib.rs`'s
  `.chain()` (it's the last entry there, right before `animation_playback_system`). Player input to
  the new mode is not suppressed during the blend — a deliberate v2 simplification, see
  `planning/claude_suggestions.md` ▸ Camera.
- `CameraModeOverride` (zero-sized marker) is present on a camera only while it's under an explicit
  registry-preset switch (cleared by `mode: "default"`); `dynamic_split_screen_system` checks it
  per-camera and skips its automatic `is_active` merge/split toggle on any camera that has it, so a
  scripted override survives the dynamic split system fighting it every frame.

**`world_label_screen_pos_system` (`lib.rs`) is viewport-aware** (fixed — this was the root cause
of "Portal room-name labels render static and mis-positioned in every split-screen room"; see
`planning/features/world_label_split_screen_positioning.md`). It queries every active `Camera3d`
(`camera.is_active`, not `.single()`) and, per `WorldLabel`, picks the `WorldLabelRank`-th
(default 0 when the component is absent) active camera whose own `logical_viewport_rect()`
actually contains the point's `world_to_viewport()` projection — deterministic order:
`SplitViewportSlot` index first (cameras with no slot, e.g. `ActiveCameraMode::Party`, sort last),
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
`Text` node — a corner "P{n}" label — for any split camera whose `CameraTargets` carries a
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

**Gamepad routing (post-`gamepad_player_binding_hardening.md`)** — `InputMap.gamepad_index:
Option<usize>` lets a player prefab bind to a specific gamepad *in addition to* the keyboard
(additive, not a replacement — see the doc comment on the field itself), but it is only ever read
as a **one-time seed**, never a live positional lookup. `BoundGamepad(pub Option<Entity>)`
(`capabilities/player.rs`) is the actual source of truth every gamepad-consuming system reads:
`None` = "pending" (no seed authored, or the seed hasn't resolved to a live pad yet); `Some(entity)`
= "bound" — locked to that specific gamepad `Entity` for the player's whole lifetime (barring a
future hot-leave/rejoin). `gamepad_bind_system` (`runtime/input.rs`, `FixedUpdate`,
`.before(input_translator_system)`) is the system that *enforces* the invariant below — the only
other writer is the hot-join spawn site, which seeds `BoundGamepad` directly from an
already-known-good `Entity` at construction time (see "Gamepad-triggered hot join" below), not by
resolving a seed. `gamepad_bind_system` visits every player in one pass each tick (in ascending
`PlayerIndex` order for the pending ones, so which player wins a duplicated seed is deterministic,
not an accident of archetype/query order) and, for each still-pending player, attempts
`sorted_gamepads.get(seed)` (sorted by `Entity::index()`, built fresh each call) against a
`claimed: HashSet<Entity>` seeded from every already-bound player **and every undrained
`is_hot_join` spawn's own captured `bound_gamepad`** (a hot-joined player can sit in
`PendingEntitySpawns` for a frame or more before `drain_spawn_queue_system`'s rate limit lets it
through — without this half, a pending scene player could bind to the same pad in that window;
system-architect/debug-detective finding, post-implementation review) and grown as the same pass
binds new ones — a **hard invariant**: it will never bind a player to an `Entity` any other player
already holds, even across frames (the cross-time race this exists to close: pad B connects first
and binds to P1's seed 0; P2's seed 1 is out of range, stays pending; pad A connects later with a
*lower* `Entity::index()` than B, so the sorted slice becomes `[A, B]` — without the `claimed` check
P2's seed 1 would now resolve to B, already bound to P1). A displaced pending player just stays
pending — no auto-rebind to a different, now-free pad this session — and gets a one-shot `warn!`
(`GAMEPAD_DIAGNOSTIC_WARN_SECS = 3.0`) if the stuck state persists, same mechanism used to diagnose
a *bound* player whose `Gamepad` component has disappeared (disconnected); both timer `Local`s are
pruned each call against the live player set, so a player who despawns while stuck doesn't leak an
entry forever. `unclaimed_gamepad_trigger_system` (`Update`) additionally reserves the pad a
still-pending *live* player's seed is about to resolve to — needed because `FixedUpdate`'s
accumulator can tick zero times in a frame, so on the exact frame a pad first becomes visible (its
first press, also this system's join-trigger frame on the web) `gamepad_bind_system` may not have
run yet and the pad would otherwise look unclaimed for one frame (debug-detective finding). Once
bound, the four simple consumers (`input_translator_system`, `tab_targeting_system`,
`interactable_system`, `action_bar_input_system`) take `Option<&BoundGamepad>` (not required —
a required `&BoundGamepad` would silently drop any test-constructed player entity missing it out
of the *entire* query tuple, not just gamepad logic) and do a direct
`bound.and_then(|b| b.0).and_then(|e| gamepad_query.get(e).ok())` — no sorting, no re-deriving
position, immune to any other pad's connect/disconnect churn. `camera_orbit_system` has no player
`Entity` in its own query, so it resolves via a disjoint `bound_q: Query<&BoundGamepad>` looked up
through `CameraTargets`; a spawn-frozen positional `gamepad_index` copy on the camera itself was
never introduced, precisely to avoid it silently diverging from `BoundGamepad`. The old crate-shared
`resolve_gamepad` helper was removed — nothing needs a live
positional lookup anymore.

Button/axis mapping is fully RON-configurable via `InputMap` — `gamepad_jump`/`gamepad_run`/
`gamepad_interact`/`gamepad_target_next: String` (parsed via `InputMap::parse_gamepad_button`,
mirroring `parse_key`'s validation seam — an unrecognized name `warn!`s and no-ops rather than
crashing) and `gamepad_deadzone: f32`, all defaulting to the same values every scene had hardcoded
before this field existed (`South`/`East`/`West`/`North`/`0.15`). Left stick moves/strafes, right
stick X turns, right stick Y drives camera pitch — independent of the keyboard's
`strafe_mouse_button` toggle (that only exists to disambiguate A/D on one keyboard; a gamepad
already has separate sticks). `gamepad_interact`/`gamepad_target_next` fold into
`interactable_system`'s/`tab_targeting_system`'s existing per-player `keyboard || gamepad`
boolean, so both work in local co-op, not just single-player — no gamepad path exists for camera
*yaw* (right-stick-X already drives character turning), a permanent, deliberate keyboard/gamepad
parity gap, not an oversight (see `docs/20_data_formats.md`).

Two players authoring the same non-`None` `gamepad_index` **in the same scene's instantiated
`entities:` list** is caught by `scene_loader.rs::warn_duplicate_gamepad_index` (scene-load `warn!`)
plus a matching `ironhold_cli validate` hard error — deliberately scoped to instantiated players,
not the raw prefab catalog, since `local_coop_demo`'s catalog legitimately reuses `gamepad_index`
across different rooms' player variants that are never co-instantiated. Largely subsumed at
runtime by `gamepad_bind_system`'s `claimed` invariant above (the second player just stays
pending, never silently dual-controls), so this check is now purely explanatory/early-warning
rather than the only thing standing between a designer and broken dual-control.

**Gamepad-triggered hot join** (`gamepad_hot_join.md`) adds a second, *global* gamepad-binding
surface alongside the per-player `InputMap.gamepad_*` fields above — `ProjectGamepadBindings`/
`LoadedGamepadBindings` (`runtime/scene_manager/mod.rs`), populated from
`ProjectConfig.global_unclaimed_gamepad_bindings`/`GameSceneV2.scene_unclaimed_gamepad_bindings` at exactly the three
sites `ProjectKeyBindings`/`LoadedKeyBindings` already use (two in `project_loader.rs`, one in
`scene_loader.rs`), same per-key overlay semantics. `unclaimed_gamepad_trigger_system`
(`runtime/input.rs`, `.before(message_interpreter_system)`) checks these bindings only against
gamepads **not** already claimed by a live player's `BoundGamepad`, by an undrained `is_hot_join`
entry's own captured `PlayerConfig.bound_gamepad` in `PendingEntitySpawns`, or by a still-pending
live player's own seed resolving to that pad this same frame (the last case exists because
`gamepad_bind_system` — the only system that actually writes a *resolved* `BoundGamepad` — runs in
`FixedUpdate`, which may not tick this frame at all; without it a pad could look unclaimed for one
frame right as an authored player's seed is about to claim it, debug-detective finding). All three
are `Entity`-based, not the pre-hardening positional `HashSet<usize>` derived from live
`gamepad_index` — that set went stale the instant any pad connected/disconnected mid-session — on
a `just_pressed` match (no separate
"live signal" prefilter: a phantom/dead duplicate pad, see the troubleshooting note above, never
produces that edge on anything) it emits the usual `UiEvent::ButtonPressed` **and** writes the
matched gamepad's `Entity` into a new `PendingJoinGamepad(Option<Entity>)` resource — at most one
pad captured per frame (deterministic: lowest `Entity::index()`-sorted), reset to `None`
unconditionally at the top of every run so a non-join gamepad trigger (e.g. a pause button) can
never leave a stale pad identity for a later frame's keyboard-triggered join to inherit.
`Action::JoinPlayer`'s executor arm (site 5 in "Player-construction sites" below) `.take()`s this
resource after resolving the joiner's `PlayerConfig` and, if set, writes it directly into
`PlayerConfig.bound_gamepad` — no round-trip through `inputs.gamepad_index`/`resolve_gamepad` (the
pre-hardening design re-resolved a converted sorted index back to an `Entity` ≥1 frame later, a
window any pad churn could exploit to bind the wrong device). `spawn_player_entity_core` inserts
`BoundGamepad(player_config.bound_gamepad)` instead of always `None` for this one call path. A
keyboard-triggered join sees the resource already `None` and is unaffected. This override does
**not** disable the joiner's keyboard scheme — gamepad and keyboard inputs are read additively
(`||`), never exclusively, everywhere in this file's gamepad routing above.

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

- **`stat_label` / `world_stat_bar` / nameplate depth scaling** — `LoadedLabelDepthScale(Option<LabelDepthScaleDef>)` stores the active scene's `label_depth_scale` block (populated in `spawn_scene_v2` alongside `ActiveTonemapping`). `resolve_label_depth_scale` (`runtime/scene_manager/mod.rs`, beside `should_insert_nameplate`) is the single resolver every consumer calls — `drain_dynamic_stat_ui_system`, the scene-load `stat_label`/`world_stat_bar` spawn loops, and `nameplate_setup_system` (nameplates were previously hardcoded to `depth_scale: None` regardless of scene config — see `planning/backlog.md`'s "Nameplate/health-bar spacing looks wrong at the zoom extremes") — so a wave-spawned enemy's widgets, a scene-placed one, and a nameplate all shrink with distance identically. Note there is no per-widget override field on `StatLabelDef`/`WorldStatBarDef`/`NameplateOptionsDef` (unlike `WorldLabelDef`/`EntityLabelDef`, which do have `depth_scale: Option<bool>`) — these always simply inherit the scene setting. All four `world_stat_bar` styles (`Ascii`, `Pixel`, `Icon`, `Textured`) now scale — `Ascii` via the font-size branch of `world_label_screen_pos_system` (`lib.rs`), the other three (plus nameplates) via the anchor's own `Transform.scale` (XY only, Z untouched to avoid perturbing each anchor's own child z-layering), since their anchor entity carries no `TextFont` of its own (its `Mesh2d`/`Sprite` children do). Both branches share one `depth_scale_factor()` formula helper so the curve can't drift between them.
  **Validation coverage (`planning/features/label_depth_scale_validation.md`):**
  `default_label_ref_distance()` (`schema/scene_v2.rs`) is `20.0`, matching
  `entity_spawner::default_camera_config()`'s `max_radius` — the engine's own fallback `Orbit`
  camera, used whenever a player prefab authors neither `camera` nor `camera_mode` (that function
  is `pub`, not `pub(crate)`, specifically so `ironhold_cli`'s `validate.rs` can reuse the exact
  same numbers instead of duplicating them as a second, driftable copy). `resolve_label_depth_scale`
  silently clamps `min_scale` to `[0.0, 1.0]` on every call (a `> 1.0` value would otherwise pin
  every depth-scaled widget forever; negative is inert either way) — that clamp is unlogged since
  it's a per-widget spawn-time call site, not a place to log on every call; the one-time
  diagnostics live elsewhere: `ironhold_cli validate` reports an out-of-range `min_scale` as a hard
  error and a `reference_distance` far outside the scene's reachable camera range as a `--strict`
  warning; `scene_loader.rs`'s `warn_label_depth_scale_min_scale_out_of_range`/
  `warn_label_depth_scale_reference_distance` fire the matching scene-load `warn!`s (called from
  `spawn_scene_v2` alongside `warn_missing_player_stat_templates`) for a WASM-only designer with no
  `ironhold_cli` access. `CameraModeDef::radius_range()` (`schema/camera.rs`) is the single source
  of truth both the CLI and the runtime call for camera classification (`Orbit`/`Party`'s
  `min_radius`/`max_radius`, a `Follow` camera's fixed `offset.length()` as both bounds — `None` if
  that offset is ~zero-length, a degenerate Follow config that would otherwise collapse the band to
  `(0.0, 0.0)` and false-flag every positive `reference_distance` — `Fixed`/`FirstPerson`/`Flycam`
  skipped); it lives in `schema/camera.rs` rather than being duplicated per-crate since it's pure
  schema classification with no runtime dependency (the same reasoning that made
  `default_camera_config()` `pub`). Both checks also union `scene.join_prefab_keys` local-coop
  character-select variants, and both skip player-camera collection entirely when a
  `tags: ["flycam"]` entity is present (`SuppressPlayerCameras` means no player camera ever
  spawns in that scene — see "Player-construction sites" above).

  **The CLI and runtime checks still don't see identical camera sets, in either direction — this
  is a real, accepted asymmetry, not a bug.** The CLI additionally scans every project-wide
  `Action::Spawn` action for a player-tagged prefab, since a player is frequently spawned
  dynamically rather than scene-placed (`3rd_person_game_demo`'s own player, the original
  motivating example, is spawned entirely via `state_machine.ron`'s entry_actions) — the runtime
  `warn!` has no equivalent, since it only ever sees `player_configs`/`join_prefab_keys` already
  resolvable at the moment `spawn_scene_v2` runs. Two different failure shapes follow from this,
  not one: **(a)** a scene whose only players are dynamically spawned (like
  `3rd_person_game_demo`) gets zero reachable cameras at scene-load time, so the runtime check
  skips entirely — only `ironhold_cli validate --strict`'s wider action-scan catches a bad
  `reference_distance` there. **(b)** a scene mixing a scene-placed player with a separately,
  more-widely-ranged dynamically-spawned one could in principle have the runtime's narrower
  (scene-placed-only) band warn on a `reference_distance` that the CLI's wider (unioned) band
  accepts — i.e. `validate --strict` clean, console warns anyway. No shipped project currently has
  shape (b); shape (a) is exactly `3rd_person_game_demo`. Logged as a known, accepted scope
  boundary in `planning/claude_suggestions.md` rather than threading `LabelDepthScaleDef` through
  `spawn_player_entity_core`'s three call sites for one diagnostic warning.

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
