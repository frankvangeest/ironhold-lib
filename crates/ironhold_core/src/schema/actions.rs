use bevy::prelude::*;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub enum Action {
    LoadScene(String),
    Quit,
    Log(String),
    /// Spawn a prefab by ID.
    /// - `id` — optional stable handle for later `Despawn`; auto-generated if omitted.
    /// - `position` — explicit world-space position `(x, y, z)`; takes precedence over `spawn_point`.
    /// - `spawn_point` — name of a spawn point defined in the scene's `spawn_points` map.
    ///   If neither `position` nor `spawn_point` is given, the entity spawns at the world origin.
    /// - `yaw_deg` — optional Y-axis rotation in degrees (0 = model default facing, 90 = 90° clockwise).
    ///   Covers N/S/E/W compass orientations. Defaults to 0 if omitted.
    Spawn {
        prefab: String,
        #[serde(default)]
        id: Option<String>,
        #[serde(default)]
        position: Option<(f32, f32, f32)>,
        #[serde(default)]
        spawn_point: Option<String>,
        #[serde(default)]
        yaw_deg: Option<f32>,
    },
    /// Despawn a previously spawned entity by the ID used in Spawn.
    Despawn(String),
    PlayAnimation(String),
    /// Play a one-shot sound effect by catalog key.
    /// `volume` (0.0–1.0) multiplies the entry's catalog volume. Defaults to 1.0 when omitted.
    PlaySound {
        key: String,
        #[serde(default = "default_action_volume")]
        volume: f32,
    },
    /// Play an audio file in a loop as background music. Stops any currently playing music.
    /// `volume` (0.0–1.0) multiplies the entry's catalog volume. Defaults to 1.0 when omitted.
    PlayMusicLoop {
        key: String,
        #[serde(default = "default_action_volume")]
        volume: f32,
    },
    /// Stop the currently playing background music.
    StopMusic,
    /// Load a scene on top of the current one without unloading the game world.
    /// Only the UI section of the overlay scene is spawned; 3D entities are ignored.
    /// Entities are tagged OverlayEntity and removed by UnloadOverlay or any full LoadScene.
    /// A transparent full-screen backdrop is automatically spawned beneath the overlay content,
    /// blocking pointer events so base-scene buttons are not clickable through the overlay.
    LoadSceneOverlay(String),
    /// Remove all OverlayEntity entities (dismiss the current overlay).
    UnloadOverlay,
    /// If an overlay is currently active: unload it. Otherwise: load the given path as an overlay.
    /// Use this for ESC-style toggles so the same key/button opens and closes the overlay.
    ToggleOverlay(String),
    /// Set global audio volume. Value is 0–100 (percent).
    /// Scales against the project's `max_volume` ceiling — `SetVolume(100)` equals `max_volume`, not 1.0.
    /// Emits the `audio.volume_changed` pipeline event.
    SetVolume(u8),
    /// Toggle the global mute state.
    /// Muting sets `GlobalVolume` to 0 and emits `audio.muted`.
    /// Unmuting restores `GlobalVolume` to `active_fraction * max_volume` and emits `audio.unmuted`.
    ToggleMute,
    /// Re-emit the current audio mute state as a pipeline event without changing it.
    /// Emits `audio.muted` if currently muted, `audio.unmuted` if not.
    /// Use in state `entry_actions` alongside a `global_on` bridge to ensure bound labels
    /// reflect the true state on every state entry — including the first project load where
    /// no toggle has yet fired.
    SyncAudioState,
    /// Toggle the local player's own nameplate visibility as a runtime preference, independent
    /// of the scene-authored `NameplateOptionsDef.show_player_nameplate` default. Flips
    /// `PlayerNameplatePreference` and emits `nameplate.own_shown` / `nameplate.own_hidden`.
    /// Has no effect on NPC/prop nameplates (`show_nameplates`/`faction_filter`), and is
    /// overridden by an explicit per-prefab `nameplate: Some(true)`/`Some(false)` on the
    /// player prefab, same precedence as `show_player_nameplate` itself.
    ToggleOwnNameplate,
    /// Pre-load a scene asset into the cache so it's ready instantly when first needed.
    /// Takes a project-relative path to a `.scene.ron`. Does not spawn or transition; purely
    /// warms the cache so a subsequent `LoadScene` resolves instantly.
    PreloadScene(String),
    /// Pre-load a prefab's GLB model so the first `Spawn` of that prefab doesn't block the
    /// game loop with asset decode on the WASM main thread. Takes a prefab key (as defined in
    /// `prefabs.ron`). Fire on `scene.ready:{name}` so the GLB is warm before the player
    /// can trigger a spawn. Does not create any visible entity.
    PreloadPrefab(String),
    /// Pre-load a model catalog GLB so the GLTF file (and all animation clips inside it) is
    /// decoded before it is first needed. Takes a model catalog key (as defined in `assets.ron`
    /// under `models:`). Especially useful for animation-source GLBs that have no prefab entry.
    /// Stores the handle in `PreloadedGlbHandles` alongside `PreloadPrefab` handles; cleared on
    /// `LoadScene`. Does not create any visible entity.
    PreloadGlb(String),
    /// Transition the interpreter to a named logic state.
    /// Rules with a matching `when` field become active; rules in other states are suppressed.
    /// Use an empty string `""` to return to the stateless (always-fire) default.
    EnterState(String),
    /// Set a named runtime variable to a string value.
    /// The value is stored in `GameVariables` and readable by data-bound UI labels.
    /// Example: `SetVariable("level", "2")` or `SetVariable("player_name", "Hero")`.
    SetVariable(String, String),
    /// Add (or subtract if negative) a numeric delta to a named variable.
    /// The variable is parsed as `i32`; missing or unparseable values default to `0`.
    /// Example: `IncrementVariable("score", 10)` awards 10 points;
    ///          `IncrementVariable("score", -5)` deducts 5.
    IncrementVariable(String, i32),
    /// Play an animation clip on a specific entity identified by its spawn ID.
    /// Use `target: "{self}"` inside behavior files — the entity FSM interpreter
    /// substitutes `{self}` with the entity's spawn ID before queuing the action.
    PlayAnimationOn {
        /// Spawn ID of the target entity, or `"{self}"` inside behavior files.
        target: String,
        /// Name of the animation clip to play.
        clip: String,
    },
    /// Emit a `GameEvent::Trigger` with the given name.
    /// Inside behavior files, `{self}` in the event name is replaced with the entity's
    /// spawn ID before the event is written, allowing reusable behavior-driven signals.
    EmitEvent(String),
    /// Add `delta` to a named stat (defined in `stats.ron`). Clamps to `[min, max]`.
    /// Negative delta reduces the stat and resets the regen cooldown.
    /// Example: `ModifyStat(key: "health", delta: -25.0)`.
    ModifyStat {
        key: String,
        delta: f32,
    },
    /// Set a named stat to an absolute value (defined in `stats.ron`). Clamps to `[min, max]`.
    /// Example: `SetStat(key: "health", value: 100.0)`.
    SetStat {
        key: String,
        value: f32,
    },
    /// Apply a named modifier template (defined in `stats.ron`) to its target stat.
    /// Multiple applications stack according to the modifier's `stack_rule`.
    /// Timed modifiers expire automatically; permanent ones persist until `RemoveModifier`.
    /// Example: `ApplyModifier(modifier_key: "speed_boost")`.
    ApplyModifier {
        modifier_key: String,
    },
    /// Remove all active instances of a named modifier from its target stat.
    /// Emits `stat.modifier.removed:{modifier_key}` when at least one instance was removed.
    /// No-op if the modifier is not currently active.
    /// Example: `RemoveModifier(modifier_key: "poison")`.
    RemoveModifier {
        modifier_key: String,
    },
    /// Spawn a floating damage number above a named entity.
    /// Positive `amount` renders in green (healing); negative in red (damage).
    /// The number rises ~1.5 m over 1.2 s then despawns automatically.
    /// Inside behavior files, `{self}` in `entity` is resolved to the entity's spawn ID.
    /// Example: `ShowDamagePopup(entity: "{self}", amount: -25.0)`.
    ShowDamagePopup {
        entity: String,
        amount: f32,
    },
    /// Show a floating text label above a named entity.
    /// The label rises and fades using the same animation as `ShowDamagePopup`.
    /// Colour is warm yellow; use `ShowDamagePopup` when you need green/red numeric feedback.
    /// Inside behavior files, `{self}` in both `entity` and `text` is resolved to the spawn ID.
    /// `offset` overrides `DamagePopupStyle.spawn_offset` when set — use to avoid stacking.
    /// Example: `ShowFloatingText(entity: "player_01", text: "Speed Boost!", offset: (0.0, 2.5, 0.0))`.
    ShowFloatingText {
        entity: String,
        text: String,
        #[serde(default)]
        offset: Option<(f32, f32, f32)>,
    },
    /// Show or hide a spawned entity by its ID.
    /// `visible: true` restores the entity; `visible: false` hides it (entity remains in ECS).
    /// World labels (health bars, stat labels) tracking the entity are hidden automatically.
    /// Inside behavior files, `{self}` in `entity` is resolved to the entity's spawn ID.
    /// Example: `SetEntityVisible(entity: "{self}", visible: false)`.
    SetEntityVisible {
        entity: String,
        visible: bool,
    },
    /// Emit a `GameEvent::Trigger` with the given name after a delay (in seconds).
    /// The event is buffered in `DelayedEventQueue` and fired by `tick_delayed_events_system`.
    /// Cleared on `Action::LoadScene` so no stale events fire after a scene transition.
    /// Inside behavior files, `{self}` in `event` is resolved to the entity's spawn ID.
    /// Example: `EmitEventAfterDelay(event: "entity.respawning:{self}", delay_secs: 15.0)`.
    EmitEventAfterDelay {
        event: String,
        delay_secs: f32,
    },
    /// Spawn a named particle burst effect defined in `AssetCatalog.effects`.
    /// Position resolution precedence:
    ///   1. `entity` (spawn ID, resolved via `SpawnRegistry` → `GlobalTransform`) + `EffectDef.offset`
    ///   2. `position` (explicit world coords) + `EffectDef.offset`
    ///   3. Neither given → no-op with a warning logged
    /// If both `entity` and `position` are given, `entity` wins and a warning is logged.
    /// Inside behavior files, `{self}` in `entity` is substituted with the entity's spawn ID.
    /// Example: `SpawnEffect(key: "hit_spark", entity: "{self}")`.
    SpawnEffect {
        key: String,
        #[serde(default)]
        position: Option<(f32, f32, f32)>,
        #[serde(default)]
        entity: Option<String>,
    },
    /// Spawn a flat textured quad on the ground plane. Used for AoE circles, cast indicators,
    /// impact splats, and persistent debuff zones.
    ///
    /// - `key` — decal catalog key defined in `assets.ron decals` map.
    /// - `entity` — if set, the decal XZ position tracks this entity each frame.
    ///   Use `"{self}"` in behavior files. Mutually exclusive with `position`; `entity` wins.
    /// - `position` — explicit world-space origin `(x, y, z)`. The y component is ignored;
    ///   decals always float at y=0.02 above the ground.
    /// - `radius` — decal radius in metres (scales the quad uniformly in XZ).
    /// - `duration_secs` — lifetime in seconds before the decal despawns.
    /// - `color` — RGBA tint `(r, g, b, a)` in linear 0–1 range. Defaults to opaque white.
    /// - `pulse_speed` — cycles per second for opacity heartbeat. 0.0 = no pulse.
    ///
    /// Example:
    /// ```ron
    /// ProjectDecal(key: "aoe_fire_circle", entity: "boss_01", radius: 3.0,
    ///              duration_secs: 5.0, color: (1.0, 0.4, 0.1, 0.7), pulse_speed: 0.8)
    /// ```
    ProjectDecal {
        key: String,
        #[serde(default)]
        entity: Option<String>,
        #[serde(default)]
        position: Option<(f32, f32, f32)>,
        radius: f32,
        duration_secs: f32,
        #[serde(default = "default_decal_color")]
        color: (f32, f32, f32, f32),
        #[serde(default)]
        pulse_speed: f32,
    },
    /// Set the global particle quality level. Scales particle counts for all subsequent
    /// `SpawnEffect` calls. Does not affect already-spawned particles.
    /// `High` (default) = full count; `Minimal` = 0.25× count, minimum 1 per layer.
    /// Persists across scene transitions — call again to restore full quality.
    /// Example: `SetParticleQuality(Low)`.
    SetParticleQuality(crate::schema::catalog::QualityLevel),
    /// Set `CurrentTarget` to the given spawn ID and emit `target.changed:{id}`.
    /// Cleared automatically on `LoadScene`. Use `ClearTarget` to remove the selection.
    /// Example: `SetTarget("enemy_01")`.
    SetTarget(String),
    /// Clear `CurrentTarget` and emit `target.cleared`.
    /// Also cleared automatically on `LoadScene`.
    ClearTarget,
    /// Instantly teleport a spawned NPC entity back to its scene-placed origin and zero its velocity.
    /// Call before `SetEntityVisible(visible: true)` so the entity appears at its spawn point
    /// rather than wherever it died. Warns and no-ops for non-NPC entities (requires `NpcAgent`).
    /// Inside behavior files, `{self}` is substituted with the entity's spawn ID.
    /// Example: `ResetToSpawn("{self}")`
    ResetToSpawn(String),
    /// Apply a procedural position shake to the active orbit camera for `duration_secs` seconds.
    /// `intensity` is the peak displacement in world-space metres (typically 0.05–0.3).
    /// Re-triggering while a shake is active restarts it with the new parameters.
    /// No-op (with a warning) in scenes that use a flycam instead of an orbit camera.
    /// Example: `CameraShake(duration_secs: 0.4, intensity: 0.15)`
    CameraShake {
        duration_secs: f32,
        intensity: f32,
    },
    /// Open the dialogue panel and begin playing a `.dialogue.ron` conversation.
    /// `npc_id` is the spawn ID of the NPC entity; `dialogue_path` is the project-relative path
    /// to the `.dialogue.ron` file. The dialogue_tick_system handles this via the
    /// auto-wire path (entity.interacted:{id} on entities with `PrefabDef.dialogue` set) but
    /// designers can also fire it directly from rules.ron or state_machine.ron.
    StartDialogue {
        npc_id: String,
        dialogue_path: String,
    },
    /// Advance the current dialogue to the next node.
    /// No-op when no dialogue is active or when the current node has unresolved choices.
    AdvanceDialogue,
    /// Close the current dialogue panel immediately.
    /// No-op when no dialogue is active. Emits `dialogue.ended:{path}` into the pipeline.
    EndDialogue,
    /// Add items to an entity's inventory.
    /// `entity: "player"` routes to `PlayerInventory` (persistent across scenes).
    /// Any other value routes to the entity's `Inventory` component by spawn ID.
    /// Emits `inventory.added:{entity}:{item_key}:{count}` on success.
    /// Emits `inventory.full:{entity}` when the inventory has no space.
    AddItem {
        entity: String,
        item_key: String,
        #[serde(default = "one_u32")]
        count: u32,
    },
    /// Remove items from an entity's inventory. Removes all held if count exceeds what's held.
    /// `entity: "player"` targets `PlayerInventory`; any other value targets the entity's `Inventory`.
    /// Emits `inventory.removed:{entity}:{item_key}:{actual_count}` on success.
    RemoveItem {
        entity: String,
        item_key: String,
        #[serde(default = "one_u32")]
        count: u32,
    },
    /// Transfer items from one entity's inventory to another.
    /// `from` / `to` each accept `"player"` or a spawn ID.
    /// Emits `inventory.transferred:{from}:{to}:{item_key}` on success.
    TransferItem {
        from: String,
        to: String,
        item_key: String,
        #[serde(default = "one_u32")]
        count: u32,
    },
    /// Show the `InventoryPanel` UI node for the player's inventory.
    OpenInventory,
    /// Hide the `InventoryPanel` UI node.
    CloseInventory,
    /// Toggle the `InventoryPanel`: show it if hidden, hide it if visible.
    ToggleInventory,
    /// Populate the `ShopPanel` UI node with the given merchant's stock and show it.
    /// The argument is the merchant entity's spawn ID.
    OpenShop(String),
    /// Hide the `ShopPanel` UI node.
    CloseShop,
    /// Buy one unit of `item_key` from the currently open shop.
    /// Deducts `buy_price` from the player's currency stat and adds the item to the player's
    /// inventory. No-ops with a warning if no shop is open, the item is not in stock, or
    /// the player cannot afford it. Emits `item.bought:{item_key}` on success.
    BuyItem(String),
    /// Show the `ContainerPanel` UI node populated with the given entity's inventory.
    /// The argument is the container entity's spawn ID (e.g. `"chest_01"`).
    /// Emits `container.opened:{entity_id}` on success.
    OpenContainer(String),
    /// Hide the `ContainerPanel` UI node and clear the active container.
    CloseContainer,
    /// Transfer all items from the currently open container to the player's inventory.
    /// No-op if no container is open. Emits `container.looted:{entity_id}` on success.
    TakeAllFromContainer,
    /// Spawn a new player into an already-`Grid`-split local co-op scene at runtime, growing
    /// the split-screen camera layout live (up to `MAX_SPLIT_PLAYERS`) — no scene reload,
    /// existing players/cameras are completely untouched. The joiner's prefab is resolved from
    /// the scene's `join_prefab_keys[next_slot]` (0-based, `next_slot` is the absolute slot
    /// number — same numbering as `PlayerIndex`); its spawn position from
    /// `spawn_points["player_{next_slot + 1}_start"]` (1-based, matching every other
    /// `player_N_start` key in this project — falling back to the primary player's current
    /// position plus a small offset if that key is absent). No-ops with a `warn!` if the scene
    /// isn't currently `Grid`-split, is already at `MAX_SPLIT_PLAYERS`, or has no
    /// `join_prefab_keys` entry for the next slot. Emits `coop.lobby_full` when the join brings
    /// the count to the cap.
    /// Typically bound via `scene_key_bindings: {"KeyG": "join"}` (avoid a key any join-target
    /// prefab's own `inputs:` already binds — see the docs) and a rule
    /// `ui.button_pressed:join -> Action::JoinPlayer`. See
    /// `planning/features/local_coop_hot_join_leave.md`.
    JoinPlayer,
}

fn default_action_volume() -> f32 { 1.0 }
fn default_decal_color() -> (f32, f32, f32, f32) { (1.0, 1.0, 1.0, 1.0) }
fn one_u32() -> u32 { 1 }
