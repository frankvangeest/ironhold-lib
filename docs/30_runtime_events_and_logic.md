# Runtime: Events and Logic

> **Doc type:** Design Doc (vision)
>
> **Status legend:**
> - ✅ **Implemented** — exists in code today
> - 🧪 **Prototype / Partial** — exists but incomplete or unstable
> - 🧭 **Planned** — intended design; not implemented yet

## Status
🧭 Planned (with a growing ✅ implemented subset)

## Goal
Provide a single, cross-platform logic model that is **driven by data** and does not require recompilation for most gameplay iteration. The runtime should be consistent between native and web/WASM.

## Design overview
Ironhold’s runtime model is built around three layers:

1. **Messages (events)** 🧭  
   Standardized, engine-level messages representing “something happened” (input, UI, triggers, scene lifecycle, animation markers).
2. **Actions** 🧭  
   Discrete, explicit commands representing “do something” (load scene, quit, play animation, spawn entity, set variable, etc.).
3. **Execution** 🧭  
   A controlled executor that applies actions in a predictable order, enabling testing, determinism strategies, and clear debugging.

### Why separate messages and actions?
- Messages are **observations** (facts): a button was pressed, an input action fired, a trigger entered.
- Actions are **commands** (intent): load a scene, quit the game, play a sound, set a state variable.

This separation helps:
- keep data-defined logic declarative and testable 🧭
- support deterministic simulation by controlling when/how actions apply 🧭
- decouple UI/input from gameplay logic 🧭

## Implementation snapshot (today)
This section is factual and reflects what exists right now.

- ✅ A robust action layer exists: `ActionQueue` plus actions such as `LoadScene(String)`, `Quit`, `Log`, `Spawn`, `PlayAnimation`, `PlaySound`, `PlayMusicLoop`, `StopMusic`, `SetVolume`, `PreloadScene`, `EmitEvent`, `SetVariable`, `IncrementVariable`, and more.
- ✅ UI events exist (`UiEvent`) and are emitted by UI button interaction and key bindings; button `action` strings have the `"ui."` prefix stripped before firing (e.g. `action: "ui.dance"` → `UiEvent::ButtonPressed("dance")`).
- ✅ Gameplay events exist (`GameEvent::Trigger(String)`) and are emitted by capabilities (physics sensors, etc.); the trigger name is used as-is in the logic pipeline.
- ✅ Input messages (`InputActionMessage`) decouple raw input from gameplay logic (point-to-point, not through the pipeline).
- ✅ Scene lifecycle events (`SceneEvent`) are emitted during loading transitions.
- ✅ A **FSM interpreter** maps events to actions and state transitions using a `StateMachineAsset` loaded from `logic/state_machine.ron`. `global_on` fires from any state without changing it; a state's `on:` list fires only while that state is active; `transitions` drive state changes and run the destination state's `entry_actions` plus the source state's `exit_actions`. A flat file (no `states`/`transitions`, just `global_on`) is a fully valid, minimal project logic file.
- ✅ An **entity FSM interpreter** (`entity_fsm_interpreter_system`) runs the same `StateMachineAsset` format per entity. Entities with a `behavior` path on their `PrefabDef` load an independent FSM; `{self}` in event patterns and action targets is substituted with the entity's spawn ID at runtime, making behavior files reusable across instances. See [Entity FSM section](#entity-fsm-beta-04) below.
- ✅ An action executor applies actions; notably:
  - `LoadScene(path)` loads a scene asset and transitions to `LoadingScene`.
  - `LoadSceneOverlay(path)` / `UnloadOverlay` load/unload overlay scenes (e.g. pause menu).
  - `Quit` requests app exit (writes `AppExit::Success`).
  - `Spawn { prefab, id, position, spawn_point, yaw_deg }` / `Despawn(id)` spawn/remove prefab instances by ID; spawn is frame-paced (max 2/frame) to cap WASM pipeline-compile stalls.
  - `PreloadPrefab(key)` loads a prefab's GLB early (fire on `scene.ready`) to prevent the first-spawn decode stall on WASM.
  - `PlayAnimation(clip)` plays an animation on available controllers.
  - `PlayAnimationOn { target, clip, start_at_fraction, freeze }` plays an animation on a specific entity by spawn ID; `start_at_fraction` (0.0–1.0) seeks into the clip's duration before playback, `freeze` pauses it there instead of continuing — see [Spawn-already-posed pattern](20_data_formats.md) in `docs/20_data_formats.md`.
  - `EmitEvent(name)` emits a `GameEvent::Trigger`; `{self}` is substituted in behavior contexts.
  - `PlaySound(key: k)` / `PlayMusicLoop(key: k)` / `StopMusic` control audio; optional `volume: f32` multiplies the per-entry catalog volume.
  - `SetVolume(pct)` sets global volume (0–100).
  - `PreloadScene(path)` / `PreloadPrefab(key)` warm the asset cache for scenes and prefab GLBs respectively.
  - `Log(msg)` emits an `info!` log line.
- ✅ `LogicState` resource tracks the current named state (default `""`). A state's own `on:` bindings only fire while `LogicState` matches that state's name; `global_on` bindings fire regardless. FSM transitions update `LogicState` directly in the interpreter — there is no separate action to force a state change; see `docs/20_data_formats.md`'s "Removed: rules.ron" callout for why and what to use instead (`EmitEvent` + a transition).
- ✅ `DebugState` resource exposes `last_action`, `app_state`, `scene`, `frame`, `logic_state`, `score`, `window_size`, and `ui_scale` for observability and browser-based testing.

## Event model (planned)
We standardize runtime messages so content can bind to them consistently.

### Core message categories

#### 1) InputAction ✅
Abstract input actions (not raw keys/buttons):
- `input.move` (vector2)
- `input.look` (vector2)
- `input.jump` (pressed/released)

**Why:** input mappings vary by platform/device, but gameplay should consume stable action names.

#### 2) UiEvent 🧭
UI interactions and higher-level UI events:
- `ui.button_pressed` (id)
- `ui.quit_requested`
- `ui.menu_opened` (name)

**Why:** keep UI wiring declarative; bind UI events to gameplay actions.

#### 3) SceneEvent ✅
Scene lifecycle events fire in this order:

| Event name | When it fires | Use for |
|-----------|---------------|---------|
| `scene.requested:<stem>` | Load has been requested (asset not yet read) | Show a loading indicator |
| `scene.loaded:<stem>` | RON asset deserialized; entities **not yet spawned** | Pre-spawn setup (e.g. set state, queue audio) |
| `scene.ready:<stem>` | All entities spawned and ready | Start gameplay logic, trigger transitions |
| `scene.unloading:<stem>` | Before a full scene replace (not overlays) | Teardown (e.g. stop music, save state) |

`<stem>` is the filename without `.scene.ron` (e.g. `"main"` for `scenes/main.scene.ron`).

**Why:** data-defined flows (menus → loading → gameplay) need stable hooks at each stage.

#### 4) GameEvent / Trigger ✅
Physics sensors and gameplay capabilities emit named triggers via `GameEvent::Trigger(String)`.
The name is used as-is in the rules pipeline — the caller is responsible for namespacing:
- `"player.jumped"` — emitted by `CharacterController` on every successful jump ✅
- `"player.attack_missed"` — pressed the interact key with no `Interactable` in range at all. Does **not** fire for a gated interactable the player is in range of but lacks the item for — that press still "landed" on a real entity (`entity.interact_blocked:<id>` fires instead), it just didn't succeed ✅
- `"entity.collected:<id>"` — collectible sensor overlap ✅
- `"entity.entered:<id>"` — trigger zone entry (Rapier sensor; `FixedUpdate`) ✅
- `"entity.exited:<id>"` — trigger zone exit (Rapier sensor; `FixedUpdate`) ✅
- `"entity.interacted:<id>"` — player within radius + pressed the interact key (default: `"KeyF"`; override via `inputs.interact` on the player prefab) ✅
- `"entity.interact_blocked:<id>"` — player within radius + pressed the interact key, but `interactable.requires_item` names an `items.ron` key not in `PlayerInventory`; fires instead of `entity.interacted:<id>` (never both). `PlayerInventory` is a single shared resource, not per-player, so in a local co-op scene any one player carrying the item unlocks it for every player. Event matching is exact-id-only (no wildcard) same as `entity.interacted:<id>` — for many gated doors sharing one prefab, a `.behavior.ron` with `event: "entity.interact_blocked:{self}"` scales better than one `global_on:` rule per door ✅
- `"container.opened:<id>"` — emitted by `Action::OpenContainer(id)` after the `ContainerPanel` UI is shown for the given entity's `Inventory` ✅
- `"container.closed"` — emitted by `Action::CloseContainer` (no entity id — only one container can be open at a time) ✅
- `"container.looted:<id>"` — emitted by `Action::TakeAllFromContainer` once every item has been transferred out of the given entity's `Inventory`. **Does not fire if the container was already empty** — `TakeAllFromContainer` early-returns before emitting when there is nothing to transfer, so an empty container is indistinguishable from "never looted" to anything listening for this event ✅
- `"entity.attacked:<id>"` — you emit this yourself from a skill slot's `do_actions`, next to the `ModifyStat` (`EmitEvent("entity.attacked:{target}")`, see `3rd_person_game_demo`). It makes an NPC with `on_player_near: Chase | Interact` enter the `Investigating` state (it walks toward the attacker's last-known position); `Flee` and `Alert` NPCs ignore it. The event names only the NPC that was hit, not who hit it. In co-op the NPC walks toward whichever player is standing **nearest to the NPC** when the hit lands, which is not necessarily the shooter: if player 2 hits an orc from range while player 1 stands closer, the orc investigates player 1. Two players at exactly the same distance: the spawn `id` that sorts first alphabetically wins (so `player10` comes before `player2`). If the NPC is already gone (for example despawned by the same hit), no NPC reacts, but any binding you wrote on the event still fires ✅
- `"npc.player_spotted:<id>"` — NPC transitioned from Alerted to Chase after detecting player ✅
- `"npc.player_reached:<id>"` — NPC reached the player's position (entered Interact) ✅
- `"npc.player_lost:<id>"` — NPC lost visual contact with player and began Investigating ✅
- `"npc.investigating:<id>"` — NPC entered Investigating state (walking toward last-known attacker position); fires on hit from range, on Chase LOS loss, and on hit while returning ✅
- `"npc.investigation_failed:<id>"` — NPC gave up investigating (timeout elapsed or arrived at destination without spotting player); transitions to Return ✅
- `"collision.hit:<id>"` — impact event 🧭
- `"target.clicked:<id>"` — player left-clicked a `click_selectable: true` entity (screen-space proximity) ✅
- `"target.changed:<id>"` — fires only when that specific entity becomes the target; use for per-entity reactions (e.g. `target.changed:boss_01` → start a boss healthbar) ✅
- `"target.changed"` — fires on every selection change; pair with `{target}` in `do_actions` for generic feedback (e.g. a `global_on` binding: `( event: "target.changed", do_actions: [ ShowFloatingText(entity: "{target}", text: "Selected!") ] )`) ✅
- `"target.cleared"` — `CurrentTarget` was cleared (click on empty space, `ClearTarget` action, `LoadScene`, or the targeted entity becoming hidden/despawned) ✅
- The targeting capability also writes the `target_display` / `target_name` / `target_id` `GameVariables` on every change — bind a `Label` to one of these for a HUD target frame (no rule wiring needed; they go blank whenever 2+ players are present, so use the per-viewport `target_hud:` block described in [20_data_formats.md](20_data_formats.md) instead). ✅
- `"audio.muted"` — emitted by `ToggleMute` when transitioning to muted ✅
- `"audio.unmuted"` — emitted by `ToggleMute` when transitioning to unmuted ✅
- `"game.paused"` — emitted when the game becomes paused (`PauseGame`/`TogglePause` took effect); only on a real change, so a second `PauseGame` emits nothing ✅
- `"cursor.lock_lost"` — a first-person camera with `capture_cursor: true` had the mouse captured and the browser took it back (the player pressed Escape, which the page cannot see as a key press). Web only (a native build never loses the grab this way). Emitted once per loss, and the engine shows the cursor again; with no rule bound to it the game keeps running with mouse look off until the next click. Typical rule: `( event: "cursor.lock_lost", do_actions: [ PauseGame ] )`, with the menu itself opened by a `game.paused` rule so that two pause requests in one frame cannot open it twice. See `docs/20_data_formats.md`, "Control schemes" ✅
- `"game.resumed"` — emitted when the game stops being paused (`ResumeGame`/`TogglePause`, or the automatic resume on a Replace-mode `LoadScene`) ✅
- `"audio.volume_changed"` — emitted by `SetVolume` after the active fraction changes ✅ (the audio system also keeps the `audio_volume_percent` `GameVariables` entry in sync — bind a `Label` to it, no rule needed; mute is not auto-written)
- `"dialogue.started:{npc_id}"` — dialogue panel opened for the NPC with the given spawn ID; emitted by `Action::StartDialogue` ✅
- `"dialogue.ended:{dialogue_path}"` — dialogue panel closed; payload is the project-relative `.dialogue.ron` path; emitted by `Action::EndDialogue` and cleared on `LoadScene` ✅
- `"inventory.added:{entity}:{item_key}:{count}"` — items successfully added to the named entity's inventory (or `"player"` for `PlayerInventory`); emitted by `Action::AddItem` ✅
- `"inventory.full:{entity}"` — `AddItem` found no room; all slots occupied; emitted alongside the partial-add event when some but not all items fit ✅
- `"inventory.removed:{entity}:{item_key}:{count}"` — items removed from an inventory; `count` is the actual amount removed; emitted by `Action::RemoveItem` ✅
- `"inventory.transferred:{from}:{to}:{item_key}"` — items moved between two inventories; emitted by `Action::TransferItem` ✅
- `"intent.slot.{n}:{entity}"` — emitted by the action bar when slot `n` is activated (key, `gamepad_key` or click) and passes all checks and passes all checks (cooldown, cost, target); fires before the slot's `do_actions` are committed; a matching rule's `do_actions` replace the slot's built-in ones; if no rule matches the slot fires as normal ✅
- `"action_bar.pressed:{key}"` — slot activated (key, `gamepad_key` or click) and passed all gate checks; fires immediately before the interpreter runs, even if a rule later cancels the intent; use for telemetry or UI feedback that should fire unconditionally ✅
- `"action_bar.activated:{key}"` — slot `do_actions` committed (fires only when no rule suppressed the intent); cooldown starts at the same time; use to react to confirmed ability execution ✅
- `"action_bar.on_cooldown:{key}"` — slot activated (key, `gamepad_key` or click) while on cooldown ✅
- `"action_bar.insufficient_resource:{key}"` — slot activated (key, `gamepad_key` or click) but cost stat too low ✅
- `"action_bar.no_target:{key}"` — `{target}` used in `do_actions` but no target is selected ✅

**Why:** drive scripted logic without bespoke code; keeps capabilities decoupled from the rules they trigger.

#### 5) AnimationMarker 🧭
Animation timeline markers:
- `anim.marker` (entity, marker_name)

**Why:** enable animation-driven gameplay events without code changes.

### Event naming rules (planned)
- Events are referenced by **strings** (e.g. `"ui.start"`, `"scene.loaded"`). 🧭
- Names should be **stable** and **namespaced**.
- Payload schemas should be documented and versioned.

## Action model
Actions represent explicit operations the runtime can execute.

### Action categories

#### Scene actions 🧭
- `LoadScene(path)` ✅
- `UnloadScene(name)` 🧭

#### App/system actions ✅
- `Quit` ✅
- `Log(message)` ✅

#### Entity actions
- `Spawn(asset_path)` ✅
- `Despawn(entity)` 🧭
- `SetTransform(entity, transform)` 🧭

#### Animation/audio actions
- `PlayAnimation(clip_id)` ✅ — plays a named animation on all animated entities by semantic ID (see AnimationPolicy); cannot seek/freeze (see `PlayAnimationOn` below)
- `PlayAnimationOn { target, clip, start_at_fraction, freeze }` ✅ — plays a clip on one entity by spawn ID (`{self}`/`{target}` substituted). `start_at_fraction` (0.0–1.0, a fraction of the clip's *duration*, not seconds) seeks to that point before playback starts; `freeze: true` pauses it there (or at `0.0` if `start_at_fraction` is omitted) instead of continuing. Demo: `assets/projects/dynamic_animation_control/`. See the "Spawn-already-posed pattern" in `docs/20_data_formats.md`.
- `PlaySound(key: audio_key)` ✅ — plays a sound by `AssetCatalog` audio key; fire-and-forget (entity despawns on completion); warns and no-ops for unsupported formats (`.wav`, `.ogg`, `.mp3` supported) or missing catalog keys; optional `volume: f32` (0.0–1.0, default 1.0) multiplies the per-entry catalog volume
- `PlayMusicLoop(key: audio_key)` ✅ — starts a looping background music track by `AssetCatalog` audio key; stops any currently playing music; optional `volume: f32` (0.0–1.0, default 1.0) multiplies the per-entry catalog volume
- `StopMusic` ✅ — stops the current background music
- `SetVolume(pct)` ✅ — sets global audio volume 0–100 (percent); scales against the project's `max_volume` ceiling so `SetVolume(100)` equals `max_volume`, not 1.0; updates the engine-written `audio_volume_percent` variable (bind a `Label` to it); emits `audio.volume_changed`
- `ToggleMute` ✅ — toggles muted state; muting emits `audio.muted`, unmuting restores the previous volume and emits `audio.unmuted`

#### State/variables actions
- `EmitEvent(name)` ✅ — fires a `GameEvent::Trigger(name)`; the idiomatic way to drive a state change is to emit an event here and add a `transitions` entry that matches it (see `docs/20_data_formats.md`'s "Removed: rules.ron" callout for the worked example) — there is no direct "set state" action
- `SetVariable(key, value)` ✅ — writes a named string value into `GameVariables`; readable by data-bound UI labels; `DebugState.score` is derived from the `"score"` key
- `IncrementVariable(key, delta)` ✅ — parses the variable as `i32` and adds the delta; missing or unparseable values default to `0`

#### Stat actions ✅
- `ModifyStat(key: "key", delta: -25.0)` — adds `delta` to a stat, clamped to `[min, max]`. Negative delta resets the regen cooldown. **Dot-routing:** `"spawn_id.stat_name"` targets that entity's `StatMap` component; a key with no dot targets the global `LoadedStats` resource. In behavior files, `{self}` in `key` is substituted with the entity's spawn ID (e.g. `ModifyStat(key: "{self}.health", delta: -35.0)`).
- `SetStat(key: "key", value: 100.0)` — sets a stat to an absolute value, clamped to `[min, max]`. Decreasing the value resets the regen cooldown. Same dot-routing and `{self}` substitution as `ModifyStat`.

After either action executes, the engine checks the stat thresholds in the same frame, and any false→true crossing reaches your rules at the start of the **next** frame (see [Same-frame event order](#same-frame-event-order)).

**Global stats** (defined in `stats.ron`, persisted in `LoadedStats`): addressed without a dot, e.g. `"player_health"`.  
**Instance stats** (defined in `stat_templates` on a `PrefabDef`, stored in `StatMap` component): addressed with a dot, e.g. `"goblin_01.health"`.

See `docs/20_data_formats.md` — `stats.ron` and `Instance stats (stat_templates)` sections for the full schema and `ThresholdCondition` variants.

#### Entity visibility / timing actions ✅
- `ShowDamagePopup { entity, amount }` ✅ — spawns a floating `+N` / `-N` world-space label above the entity with the given spawn ID; style (font size, duration, rise speed, colours) configured via `damage_popup_style` in `.project.ron`; `{self}` is substituted in behavior contexts
- `SetEntityVisible { entity, visible }` ✅ — shows or hides a spawned entity by spawn ID; entity stays in ECS (stats and behavior FSM keep running); world-space labels tracking the entity auto-hide; `{self}` substituted in behavior contexts
- `EmitEventAfterDelay { event, delay_secs }` ✅ — fires `GameEvent::Trigger(event)` after `delay_secs` seconds; cleared on `LoadScene`; `{self}` substituted in behavior contexts

#### UI actions
- `SetUiVisible { id, visible }` ✅ — shows or hides a scene `ui:` node by its `id` (a `Group` hides its subtree and takes no space while hidden); every node with that id, base scene and overlay alike; no match is a silent no-op; resets on `LoadScene`; not for panels or the `ActionBar` (validate rejects them). See `docs/20_data_formats.md#setuivisible`

### Action semantics (planned)
- Actions are executed in a defined order per tick/frame.
- Actions should be **idempotent** where reasonable.
- Action execution should be observable for debugging and replay.

## Project logic: `state_machine.ron`
The heart of data-driven behavior is `logic/state_machine.ron`, which maps incoming messages to actions.

### Bindings, and the flat-first workflow
A **binding** is "when event X happens, run actions Y" — a `( event: "...", do_actions: [...] )` pair. Start with a flat file: just `global_on`, no `states`/`transitions` at all. Add states only once you actually need modes (menu vs. playing vs. paused).

```ron
// logic/state_machine.ron — flat
(
    schema_version: 1,
    global_on: [
        ( event: "scene.ready:main", do_actions: [ PlayMusicLoop(key: "bg_music") ] ),
        ( event: "entity.collected:coin_01", do_actions: [ IncrementVariable("score", 1) ] ),
    ],
)
```

Once you need modes, a menu → playing → paused flow could look like this:

```ron
// logic/state_machine.ron — with states/transitions
(
    schema_version: 1,
    initial_state: "menu",
    states: [
        (
            name: "menu",
            on: [
                // Loading the scene lives here, on the state's own on: binding, not on the
                // transition below (transitions carry no actions — see the note after this
                // example) and not on "playing"'s entry_actions (which would re-run every time
                // "playing" is re-entered, e.g. on unpause — see the initial_state gotcha below
                // for the general version of this trap).
                ( event: "ui.button_pressed:start_game", do_actions: [ LoadScene("scenes/main.scene.ron") ] ),
                ( event: "ui.button_pressed:quit", do_actions: [ Quit ] ),
            ],
        ),
        ( name: "playing" ),
        (
            name: "paused",
            // "paused" is only ever entered/exited via the two toggle_pause transitions below,
            // so these fire exactly once per pause/unpause — no re-fire trap here.
            // PauseGame/ResumeGame are a pair: PauseGame on entry, ResumeGame on THIS state's exit,
            // so every way out of "paused" resumes the game (see "Pausing the game" below).
            entry_actions: [ LoadSceneOverlay("scenes/pause.scene.ron"), PauseGame ],
            exit_actions: [ UnloadOverlay, ResumeGame ],
        ),
    ],
    transitions: [
        ( from: "menu", on: "ui.button_pressed:start_game", to: "playing" ),
        ( from: "playing", on: "ui.button_pressed:toggle_pause", to: "paused" ),
        ( from: "paused", on: "ui.button_pressed:toggle_pause", to: "playing" ),
    ],
)
```

**Transitions carry no actions of their own** — `FsmTransition` has only `from`/`on`/`to` fields
(`deny_unknown_fields` rejects a `do_actions:` on one outright). Put a state change's side effects
on the **destination state's `entry_actions`** and/or the **source state's `exit_actions`**
instead, as `paused` does above — or on the state's own `on:` binding for the same event the
transition matches, as `menu` does above, when the action should fire once regardless of which
state the event happens to also advance you to.

### Pausing the game

A pause menu is a state that opens an overlay **and** holds the simulation. Put `PauseGame` in the state's `entry_actions`
and `ResumeGame` in the **same state's** `exit_actions`, right next to `LoadSceneOverlay`/`UnloadOverlay` (the `paused` state
in the example above). Whatever transition leaves the state — Resume, the pause key, the player dying, a later "main menu"
button — resumes the game, so the pair can never get out of step.

| Action | Effect |
|---|---|
| `PauseGame` | Hold the simulation. A **request**: the engine emits `game.paused` only when your pause takes effect. A no-op if already paused (there is no counting: one `ResumeGame` always undoes it). |
| `ResumeGame` | Release the pause. A no-op if not paused. |
| `TogglePause` | Flip it. For flat projects that only use `global_on`; inside `entry_actions`/`exit_actions` use the pair above instead, because entering a state while already paused would *un*pause. |

A flat project (no states) pauses like this:

```ron
global_on: [
    ( event: "ui.button_pressed:toggle_pause", do_actions: [ ToggleOverlay("scenes/pause.scene.ron"), TogglePause ] ),
    ( event: "ui.button_pressed:resume",       do_actions: [ UnloadOverlay, ResumeGame ] ),
],
```

This is second-class on purpose: `ToggleOverlay` closes *any* open overlay, so with a map overlay open the pause key closes the
map but still pauses, and the overlay and the pause drift apart. Prefer a `paused` state, which cannot get out of step.
**An action-bar slot with `do_actions: [TogglePause]` can pause but never resume** (the action bar is frozen while paused):
bind the pause key in `global_key_bindings` (or a pad button in `global_claimed_gamepad_bindings`) instead.

**What freezes while paused:** physics, the player, NPCs and monster AI, motion (`rotate`/`bob`), particles, animations,
cooldowns, `EmitEventAfterDelay` and `SetDespawnTimer` timers, stat regeneration and over-time effects, camera movement
(including first-person and flycam mouse look), and the interact / loot / targeting / action-bar inputs. **What keeps
running:** the UI (buttons, the overlay), your rules (`global_on`, state `on:` bindings), scene loading, and audio. The
pause lands on the frame *after* the action runs.

Things to know when you author one:

- **A Replace-mode `LoadScene` always un-pauses.** If the paused state's Retry/"main menu" button loads a scene, the new scene
  starts running even if you forgot `ResumeGame`. (`LoadSceneOverlay`/`ToggleOverlay`/`UnloadOverlay` never resume.) The
  load always wins: `[LoadScene("x"), PauseGame]` in one list ends with the new scene **running**, because the scene spawns
  a few frames after both actions ran and releases the pause then. To start a scene paused, run `PauseGame` from that
  scene's `scene.ready:<scene>` rule.
- **"Main menu" from the pause menu.** A Replace-mode `LoadScene` un-pauses, but the FSM stays in `paused` until the new
  scene's `scene.ready` transition moves it, so give that scene an any-state transition
  (`( on: "scene.ready:start_menu", to: "menu" )`). Otherwise `playing`'s rules never fire again and the pause key acts
  strangely.
- **Your rules and some UI keep running.** The UI is not paused: the overlay's full-screen backdrop blocks clicks on the HUD
  underneath, but a key in `global_key_bindings` bound to a gameplay trigger, a rule in `global_on`, and any panel drawn above
  the backdrop (inventory, shop, chest) still work. Bind gameplay triggers in the `playing` state's `on:` (not `global_on`),
  and close open panels on entry (`CloseInventory`, `CloseShop`, `CloseContainer`) as the demos do.
- **Do not put run-start actions in `playing`'s `entry_actions`.** Unpausing re-enters `playing`, so a health/score reset or a
  `PlayMusicLoop` there would fire on every unpause. Put them in a `scene.ready:<scene>` rule (it fires once per run and never
  for an overlay), as `primitive_world` and `3rd_person_game_demo` do.
- **Death while paused.** Threshold events (`stat.player_health.depleted`) arrive once and never re-fire. Handle them in
  `global_on`, or give the transition a `from: "paused"` row too, so a death that lands on the pause frame is not dropped.
- **Do not arm a timer from the pause menu.** An `EmitEventAfterDelay` armed in a paused state never fires until the game is
  resumed, so a "fade, then `LoadScene`" quit button would hang. Run `ironhold validate --strict`: it warns about this and about the
  other pause mistakes (a paused state with no `ResumeGame` in its `exit_actions`, `TogglePause` in an entry/exit list, ...).
- **Audio keeps playing.** Do not use `ToggleMute` (it un-mutes a player who had muted) or `StopMusic` followed by
  `PlayMusicLoop` (the track restarts) in the pause entry/exit. React to `game.paused`/`game.resumed` if you want to duck.
- **Same frame:** a keyboard pause and a joined pad's pause button pressed in the same frame produce two `toggle_pause` events and
  cancel out (the once-per-frame dedupe is per binding map); see [Same-frame event order](#same-frame-event-order).
- **Keys and pads.** Bind a pause key in `global_key_bindings` (`"Escape": "toggle_pause"`; add `"KeyP"` too, since a browser
  can swallow Escape in fullscreen). A joined gamepad player's own buttons never reach `global_unclaimed_gamepad_bindings`,
  so use `global_claimed_gamepad_bindings: { "Start": "toggle_pause" }` (project file). Per-scene bindings in an *overlay*
  scene are ignored (the engine logs a warning): bind keys in the project file or the base scene.
- **Local co-op:** one clock, so any player pauses everyone, and the overlay is one screen-space UI across the whole window
  (it sits across the seams of a split-screen grid). A player can still hot-join while paused and spawns frozen.
- **Online play (later):** `PauseGame` may only open your menu there (the host decides whether the simulation stops). Build your
  pause UI around the `game.paused`/`game.resumed` events, not around the action itself, and your content needs no changes.

### Per-event evaluation order
For each event, in this order:
1. `global_on` — fires regardless of state, never changes state.
2. The current state's own `on:` list — fires only while that state is active, never changes state.
3. The first matching `transitions` entry — changes state, running the source state's `exit_actions` then the destination state's `entry_actions`.

Only the first matching transition fires per event; all matching `global_on`/in-state bindings fire. When several events arrive in the
same frame, see [Same-frame event order](#same-frame-event-order) for the order they are read in.

### Same-frame event order

**Design so the order does not matter.** When several things happen in the same frame the engine always handles them in the
same order (on the same build and in the browser), but your logic is easiest to read, and to keep working, if it does not depend
on it. The interpreter takes the **first matching** transition for each event and applies it immediately, so the order can change
where the state machine ends up. (One thing is *not* fixed yet: the order **between entities**, for example two behaviors
reacting in the same frame or two coins collected in the same physics tick, still follows the engine's internal entity order.)

**What the rules read in a frame, in order**

1. **Key presses, gamepad buttons, UI buttons** (all UI events are read before any game event):
   key presses (`global_key_bindings` and `scene_key_bindings`) first, then gamepad triggers (a joined player's
   `global_claimed_gamepad_bindings`, then the unclaimed-pad join trigger), then buttons and icon buttons.
2. **Game events**:
   1. *carried over from the previous frame*: `action_bar.activated:*`, the events your actions emitted (`EmitEvent`, combat,
      `container.*`, `inventory.*`, `dialogue.*`, `audio.*`), then `game.paused` / `game.resumed`, then stat **threshold**
      crossings (`stat.*.depleted`, `.low`, ...). Thresholds and the pause events are always read one frame after the action
      that caused them;
   2. *physics-tick events*: player movement, collectibles, trigger zones, NPC reactions (none while paused);
   3. *this frame*: clicking an entity to target it and Tab, the action bar, the interact key, delayed events
      (`EmitEventAfterDelay`), and last a modifier expiring (regeneration emits no event of its own; a threshold it crosses
      is read next frame, see 2.1).
3. **Scene events** last: last frame's `scene.requested` / `scene.unloading`, then this frame's `scene.loaded` / `scene.ready`.

**A scene load runs first in the frame.** Keys and buttons work on the very first frame of the new scene (they use the new scene's
key bindings and UI, and a pause you left open is already released; a Replace-mode `LoadScene` does this, an overlay never
does), but timers, regeneration, cooldowns and camera smoothing start counting one frame later. A key pressed on the load frame
is read *before* that frame's `scene.ready`, so it is matched against the old state.

**What order your actions run in** (the order actions are queued is the order they execute):

1. a spawned entity's behavior starting, and a `SetDespawnTimer` running out;
2. dialogue (`StartDialogue` on interact, and a chosen dialogue choice's actions);
3. your `state_machine.ron` rules, then each entity's own `.behavior.ron`;
4. the action bar's own slot actions.

So a slot's built-in `do_actions` run *after* every `state_machine.ron` reaction to the same frame, and dialogue actions run
*before* them. A skill key and the pause key pressed in the same frame still fire the skill: the pause lands after the slot's
actions were queued, and the input gate only closes the next frame.

**Worked examples**

- *Esc and the Resume button in the same frame* (different triggers, as in the demos): Esc is read first, so it un-pauses, and the
  Resume transition (which only matches while paused) no longer applies. If a key and a button share **one** trigger and both fire in
  the same frame they cancel each other out (rare: give overlay buttons their own trigger, like `resume`; a keyboard pause and a
  joined pad's pause button pressed together do the same).
- *Damage in one frame, Esc in the next:* Esc is read first (UI events come before game events), so the state machine is already
  `paused` when `stat.player_health.depleted` arrives. A transition written `from: "playing"` then silently misses the death. Handle
  must-not-lose events in `global_on`, or give the transition a `from: "paused"` row too (`primitive_world` does).
- *Esc and a `playing`-scoped click in the same frame:* if Esc moves the state machine to `paused` first, the click's rule (written
  in `playing`'s `on:`) does not match. That is usually what you want (the pause wins). A reaction in `global_on` would still fire,
  and also while paused, which is why [Pausing the game](#pausing-the-game) says to keep gameplay triggers in `playing`'s `on:`.
- *One key bound in `global_key_bindings` and also on an action-bar slot:* the key binding's rule always reacts first (a key is read
  before the slot's events), and the slot's built-in actions run after it.

**Rules that follow:** timers, stat thresholds, NPC and physics events arrive in *any* state (the menu, game over, a dialogue
state, and `paused` for events produced just before the pause), so handle them in `global_on` or author the transition from
every state they can land in. The action bar's resource check sees the previous frame's stat regeneration (one frame, about
16 ms). An NPC's AI sees its own `stat.*.health.depleted` in the same frame your rules do, and the AI goes first. Particle
effects are cleared once, when a scene is requested; an old scene's emitters may keep spawning until the new scene appears, so do
not `SpawnEffect` in the same list as `LoadScene`: use the new scene's `scene.ready:<scene>` rule.

See also: [Per-event evaluation order](#per-event-evaluation-order), [Pausing the game](#pausing-the-game), and in
[20_data_formats.md](20_data_formats.md) the key bindings, the action bar's same-frame slot order and the `stats.ron` order notes.

### The `initial_state` entry-actions gotcha
**`entry_actions` on the `initial_state` do *not* run at boot.** Entry actions only run when a **transition** enters a state — the state you start in was never "entered" by a transition, so its `entry_actions` are silently skipped. If you follow "start flat, add states later" and move boot-time work (like `PlayMusicLoop` on scene-ready) into your first state's `entry_actions`, expecting it to fire immediately, it won't. Keep boot-time work in a `global_on` `scene.ready:<scene>` binding instead, or restructure so the initial state is only reached via an explicit transition from a dedicated boot state.

### Changing state from a condition, not a direct call
There is no "set state" action (see `docs/20_data_formats.md`'s "Removed: rules.ron" callout for why). To change state from a condition — a stat threshold, a dialogue choice, anything — emit a `GameEvent` and add a `transitions` entry that matches it:

```ron
do_actions: [ EmitEvent("quest.started") ],   // wherever the condition is detected
```
```ron
transitions: [ ( from: "exploring", on: "quest.started", to: "on_quest" ) ],
```

See `docs/20_data_formats.md`'s callout for the full worked example, including why this avoids a silent-desync bug class the old direct-state-change action had.

### The `on:` field means three different things
Old `rules.ron` muscle memory (`( on: "x", do_actions: [...] )`) produces a plausible-looking `state_machine.ron` file that fails to parse. In the current format:
- A **state's** `on:` is a *list of bindings* (each with its own `event:` field).
- A **transition's** `on:` is an *event-name string*.
- A **binding** itself uses `event:`, never `on:`.

See `docs/20_data_formats.md`'s "Removed: rules.ron" callout for the wrong-vs-right side-by-side.

## Execution model (planned)

### Interpreter 🧭
Transforms messages into actions using the rule set for the current scene/project.

### Executor 🧭
Applies actions to the world. Key design points:
- The executor is the **single place** where side effects happen.
- The executor can be made deterministic/fixed-step later.

### Ordering & determinism notes 🧭
- For determinism, prefer a **fixed tick** for gameplay actions.
- Separate deterministic gameplay actions from non-deterministic presentation effects.
- The order of same-frame events and actions is fixed and documented: see [Same-frame event order](#same-frame-event-order).

(See `docs/dev/40_determinism_and_networking.md` for design notes.)

## Milestone mapping (suggested)

- **Milestone 0.1 + 0.2 (implemented)** ✅
  - ✅ `UiEvent::ButtonPressed` emitted by UI buttons; mapped to actions via data-defined rules
  - ✅ Full action set: `LoadScene`, `Quit`, `Log`, `Spawn`, `PlayAnimation`, `PlaySound`, `SetVariable`, `IncrementVariable`
  - ✅ `InputAction` abstraction (`Move`, `Turn`, `Look`, `Jump`, `Run`)
  - ✅ Scene lifecycle events: `SceneEvent::{Requested, Loaded, Ready, Unloading}`
  - ✅ Data-defined project logic wired to interpreter + executor (originally two dialects, later consolidated onto `logic/state_machine.ron` alone — see `docs/20_data_formats.md`'s migration callout)
  - ✅ `DebugState` resource for runtime observability (`frame`, `app_state`, `last_action`, `scene`, `logic_state`, `score`, `window_size`, `ui_scale`)
  - ✅ `GameEvent::Trigger(String)` for physics sensors and gameplay capabilities

- **Milestone: Rule bindings v2** 🧭
  - Conditions/filters on rules (entity tags, state variables, scene)
  - Parameter flow from event payload into actions

- **Milestone: Trigger / Collision events** 🟡
  - `GameEvent::Trigger` and collectible sensors implemented ✅
  - Zone enter/exit events (`zone.entered:<id>`) 🧭

- **Milestone: Deterministic core hooks** 🧭
  - Fixed tick loop for gameplay
  - Replayable input stream

## Non-goals (for now)
- Fully deterministic rendering/audio 🧭
- A complete visual scripting system 🧭

## Appendix: Implemented subset (today)

### Messages ✅
- `UiEvent::ButtonPressed(String)` — emitted by UI buttons, key bindings, and unclaimed-gamepad bindings (see `docs/20_data_formats.md`'s "Gamepad-triggered hot join"); flows into the pipeline as `"ui.button_pressed:{trigger}"`
- `GameEvent::Trigger(String)` — emitted by gameplay capabilities (physics sensors, etc.); name is used as-is in the pipeline (e.g. `"entity.collected:coin_01"`)
- `SceneEvent::{Requested(String), Loaded(String), Ready(String), Unloading(String)}` — scene lifecycle
- `InputAction::{Move(Vec2), Turn(f32), Look(Vec2), Jump(bool), Run(bool)}` — abstract input (point-to-point, not pipeline)
- `InputActionMessage { entity: Entity, action: InputAction }` — input bound to a specific entity (point-to-point, not pipeline)

### Actions ✅
- `LoadScene(String)` — loads a `.scene.ron` and transitions to `LoadingScene`. `ironhold_cli validate` checks this path exists on disk (same for `LoadSceneOverlay` and `PreloadScene`/`ToggleOverlay` below)
- `LoadSceneOverlay(String)` — loads a `.scene.ron` as an overlay (e.g. pause menu); automatically spawns a transparent full-screen backdrop beneath the overlay UI that blocks pointer events — base-scene buttons are not clickable through the overlay
- `UnloadOverlay` — despawns all overlay entities (including the backdrop)
- `ToggleOverlay(String)` — opens overlay if none is active, closes if one is; `ironhold_cli validate` checks this path exists on disk too
- `Quit` — writes `AppExit::Success`
- `Log(String)` — emits an `info!` log line
- `Spawn { prefab, id, position, spawn_point, yaw_deg }` — enqueues a prefab spawn (processed max 2/frame by `drain_spawn_queue_system`); `id` auto-generated if omitted, and supports `{self}`/`{target}` substitution plus `{new_id}` — a fresh counter value resolved at spawn time so a repeated `{self}` (e.g. a monster that always respawns under the same id) doesn't force the same derived id every time; see the `{new_id}` substitution note below for what it does and doesn't guarantee; `position: (x,y,z)` sets an explicit world position; `spawn_point: "name"` looks up a named point from the scene's `spawn_points` map; defaults to world origin when neither is given; `yaw_deg: f` rotates around the Y axis in degrees
- `PreloadPrefab(String)` — loads a prefab's GLB model and stores the handle in `PreloadedGlbHandles`; fire on `scene.ready` to eliminate the WASM GLB-decode stall on first spawn (handles cleared on `LoadScene`)
- `PreloadGlb(String)` — the model-catalog equivalent of `PreloadPrefab`: takes a key from `assets.ron` `models:` (use it for animation-source GLBs that have no prefab entry); the handle is stored and cleared the same way
- `Despawn(String)` — removes a previously spawned entity by its spawn ID
- `PlayAnimation(String)` — plays an animation by semantic ID (see AnimationPolicy)
- `PlayAnimationOn { target, clip, start_at_fraction, freeze }` — plays a clip on one entity by spawn ID; `start_at_fraction`/`freeze` seek/pause into the clip (see the Animation/audio actions list above)
- `PlaySound { key, volume }` — fire-and-forget audio by catalog key; `volume` (0.0–1.0, default 1.0) multiplies the per-entry catalog volume; warns for unsupported formats or missing keys
- `PlayMusicLoop { key, volume }` — starts a looping background music track by catalog key; `volume` (0.0–1.0, default 1.0) multiplies the per-entry catalog volume
- `StopMusic` — stops the current background music
- `SetVolume(u8)` — sets global audio volume 0–100
- `PreloadScene(String)` — warms the asset cache for a `.scene.ron` before it is needed; use on `scene.ready` so a subsequent `LoadScene` resolves without a loading pause
- `PreloadPrefab(String)` — loads a prefab's GLB model and stores the handle in `PreloadedGlbHandles`; fire on `scene.ready` to eliminate the WASM GLB-decode stall on first spawn
- `PreloadGlb(String)` — the model-catalog equivalent of `PreloadPrefab`: takes a key from `assets.ron` `models:` (use it for animation-source GLBs that have no prefab entry); the handle is stored and cleared the same way
- `SetVariable(String, String)` — writes a named string value into `GameVariables`; readable by data-bound UI labels; `DebugState.score` is derived from the `"score"` key
- `IncrementVariable(String, i32)` — parses the variable as `i32` and adds the delta; missing or unparseable values default to `0`
- `ModifyStat { key, delta }` — adds `delta` to a stat, clamped to `[min, max]`; negative delta resets regen cooldown. **Dot-routing:** `"spawn_id.stat_name"` → entity `StatMap`; no dot → global `LoadedStats`. `{self}` in `key` is substituted in behavior contexts.
- `SetStat { key, value }` — sets a stat to an absolute value, clamped to `[min, max]`; decreasing resets regen cooldown. Same dot-routing and `{self}` substitution as `ModifyStat`.
- `ShowDamagePopup { entity, amount }` — spawns a floating `+N` / `-N` world-space label above the entity with the given spawn ID; style configured via `damage_popup_style` in `.project.ron`; `{self}` substituted in behavior contexts
- `ShowFloatingText { entity, text, offset? }` — spawns a floating text label (warm yellow) above the entity; rises and fades like `ShowDamagePopup`; use for status messages, selection feedback, etc.; `{self}` and `{target}` substituted in both `entity` and `text`. Optional `offset: (x, y, z)` overrides the default spawn height from `damage_popup_style.spawn_offset` — set a different Y to prevent two simultaneous texts from stacking at the same position (e.g. ability name fires at default height, kill confirmation fires at `offset: (0.0, -0.02, 0.0)`)
- `SetEntityVisible { entity, visible }` — shows or hides a spawned entity by spawn ID; entity stays in ECS (stats and behavior FSM keep running); world-space labels tracking the entity auto-hide; `{self}` substituted in behavior contexts
- `SetUiVisible { id, visible }` — shows or hides a scene `ui:` node (or `Group` subtree) by its `id`; every node with that id, base scene and overlay alike; unknown id is a silent no-op; not for panels or the `ActionBar` (validate rejects them); no `{self}`/`{target}` substitution
- `EmitEventAfterDelay { event, delay_secs }` — fires `GameEvent::Trigger(event)` after `delay_secs` seconds; cleared on `LoadScene` so no delayed events survive scene transitions; `{self}` substituted in behavior contexts
- `SpawnEffect { key, position, entity }` — bursts a particle effect from `AssetCatalog.effects`; `entity` (spawn ID) takes precedence over `position` (world coords); the effect def `offset` is added to the resolved position; `{self}` substituted in behavior contexts; silently skips on unknown key or unresolvable entity; see `docs/20_data_formats.md` for `EffectDef` fields. **WASM:** sphere particles and flame sprite particles (`uv_distort > 0` or `uv_scroll_speed > 0`) compile separate WebGPU pipelines — fire one warmup burst per variant on `scene.ready` at `position: Some((0.0, -100.0, 0.0))` to pre-compile both pipelines before the player can interact.
- `SetTarget(String)` — sets `CurrentTarget` to the given spawn ID; emits `target.changed:{id}` and `target.changed`; `{target}` in subsequent rule actions resolves to this ID
- `ClearTarget` — clears `CurrentTarget`; emits `target.cleared`; also cleared automatically on `LoadScene`
- `AddItem { entity, item_key, count? }` — adds `count` (default 1) of `item_key` to the named entity's inventory; `entity: "player"` targets `PlayerInventory` (the **reserved keyword** `"player"` — distinct from any entity's spawn id, even if the player character's spawn id is `"player_01"`); any other string routes to a container entity's `Inventory` by spawn ID. Emits `inventory.added:{entity}:{key}:{n}` on success, `inventory.full:{entity}` when no slot is available. `{self}` and `{target}` substituted in `entity`.
- `RemoveItem { entity, item_key, count? }` — removes up to `count` (default 1) of `item_key`; emits `inventory.removed:{entity}:{key}:{n}` with the actual count removed. `{self}` and `{target}` substituted in `entity`.
- `TransferItem { from, to, item_key, count? }` — removes `count` of `item_key` from `from` and adds it to `to` (both support `"player"` or a spawn ID). Emits `inventory.transferred:{from}:{to}:{key}`. `{self}` and `{target}` substituted in both `from` and `to`.
- `OpenInventory` — makes the scene's `InventoryPanel` visible (guarded by change detection). Targets by node **type**, not by `id` — there can be at most one `InventoryPanel` per scene. The `id` field on the panel is currently decorative (available for future index-based targeting).
- `CloseInventory` — hides the `InventoryPanel` (equality-guarded; no-op if already hidden)
- `ToggleInventory` — checks current `InventoryPanel` visibility and flips it: shows if hidden, hides if visible. Use this for a single key/button that both opens and closes the panel.
- `OpenShop(merchant_id)` — resolves the merchant's `MerchantDef` via the `SpawnRegistry` + `PrefabKey`, makes the `ShopPanel` visible, and repopulates its stock list. Targets by node **type** — at most one `ShopPanel` per scene. `{self}` and `{target}` substituted in the merchant ID. Warns if no merchant entity or no `ShopPanel` UI node is present.
- `CloseShop` — hides the `ShopPanel` (equality-guarded; no-op if already hidden)
- `PauseGame` / `ResumeGame` / `TogglePause` — hold / release / flip the simulation (see [Pausing the game](#pausing-the-game)). `PauseGame` is a request: `game.paused` is emitted only when the game becomes paused, and `game.resumed` when it stops being paused (including the automatic resume on a Replace-mode `LoadScene`). No-ops if already in that state.

### Infrastructure ✅
- `ActionQueue` — FIFO queue processed each frame by `action_executor_system` (push order equals execution order)
- `LogicState` resource — tracks the current named state (default `""`); checked by `fsm_interpreter_system` (the per-entity `entity_fsm_interpreter_system` tracks its own state independently, via `EntityFsmState`)
- `DebugState` resource — tracks `frame`, `app_state`, `last_action`, `scene`, `logic_state`, `score`, `window_size`, `ui_scale`; serialised to DOM on WASM for browser testing
- `StateMachineAsset` loaded from `logic/state_machine.ron` via `fsm_interpreter_system`; every project uses this — `state_machine_path` in the project config points to it

### FSM asset schema (`logic/state_machine.ron`) ✅

Use `logic/state_machine.ron` for **gameplay state machines** — managing named logic states with explicit transitions, entry/exit actions, and per-state event binding.

**Best for:**
- Gameplay states (playing, paused, menus as states)
- Systems requiring entry/exit actions when transitioning between states
- Explicit state transitions with `from:`/`to:` guards
- `global_on:` events that fire from any state without changing state
- Per-state `on:` bindings that fire while in that state

**Schema pattern:**
```ron
(
    schema_version: 1,
    initial_state: "menu",   // sets LogicState on load; no entry actions fired at startup

    global_on: [
        // Fires from any state; does not change state.
        ( event: "ui.button_pressed:debug_reload", do_actions: [ Log("reload") ] ),
    ],

    states: [
        // Every name used as initial_state, or as a transition's from/to, must be declared here
        // -- StateMachineAsset::validate() rejects initial_state: "menu" otherwise, even with no
        // entry/exit actions of its own.
        ( name: "menu" ),
        (
            name: "playing",
            // Queued on EVERY entry to this state, including each unpause: keep run-start actions (music, stat resets,
            // preloads) out of here and in a `scene.ready:<scene>` rule instead (see "Pausing the game").
            entry_actions: [ SetVariable("status", "playing") ],   // queued when entering this state
            exit_actions:  [ SetVariable("status", "left playing") ],  // queued when leaving this state
            on: [
                // In-state bindings: fire while in "playing", do not change state.
                ( event: "ui.button_pressed:dance", do_actions: [ PlayAnimation("dance") ] ),
            ],
        ),
        (
            name: "paused",
            entry_actions: [ LoadSceneOverlay("scenes/pause.scene.ron"), PauseGame ],
            exit_actions:  [ UnloadOverlay, ResumeGame ],
            on: [],
        ),
    ],

    transitions: [
        // Omit `from` to match any current state.
        ( on: "scene.ready:main", to: "playing" ),

        // Explicit from/to.
        ( from: "playing", on: "ui.button_pressed:toggle_pause", to: "paused" ),
        ( from: "paused",  on: "ui.button_pressed:toggle_pause", to: "playing" ),
    ],
)
```

**Execution order per transition:** exit actions → state change → entry actions.
The engine handles this automatically — there is no action an author writes to force a state change; a `transitions` entry matching the current event is the only thing that advances `LogicState`.

> New Messages or Actions must update `docs/STATUS.md` (Engine ABI section), this appendix, and `docs/20_data_formats.md` with an authoring example.

---

## Entity FSM (Beta 0.4)

### Overview

Each entity can run its own independent `StateMachineAsset` loaded from a `.behavior.ron` file. The behavior file uses the same format as `logic/state_machine.ron` (states, transitions, entry/exit actions, global_on). The key difference is the `{self}` placeholder, which is substituted with the entity's spawn ID at runtime.

### Authoring

Add `behavior` (and optionally `interactable` or `trigger_zone`) to a `PrefabDef`:

```ron
// prefabs/prefabs.ron
"collectible_box": (
  kind: "primitive",
  model: "Cuboid",
  behavior: "behaviors/collectible_box.behavior.ron",
  interactable: ( radius: 2.5 ),
  primitive: ( size: (0.8, 0.8, 0.8), color: (0.9, 0.7, 0.2) ),
),
```

The behavior file uses `{self}` as a placeholder for the entity's spawn ID:

```ron
// behaviors/collectible_box.behavior.ron
(
  schema_version: 1,
  initial_state: "idle",
  global_on: [],
  states: [
    ( name: "idle",      entry_actions: [],                                   exit_actions: [], on: [] ),
    ( name: "collected", entry_actions: [ PlaySound(key: "score"), Despawn("{self}") ], exit_actions: [], on: [] ),
  ],
  transitions: [
    ( from: "idle", on: "entity.interacted:{self}", to: "collected" ),
  ],
)
```

When two boxes `box_01` and `box_02` share this file, interacting with `box_01` fires `entity.interacted:box_01`, which only matches `box_01`'s transition (pattern `entity.interacted:{self}` → `entity.interacted:box_01`). `box_02` is not affected.

### `{self}` substitution rules

`{self}` is replaced with the entity's spawn ID in:
- Transition `on` patterns
- In-state and `global_on` event patterns
- `Despawn("{self}")` → `Despawn("box_01")`
- `PlayAnimationOn { target: "{self}", clip: "open" }` → `target: "box_01"`
- `EmitEvent("door.opened:{self}")` → `"door.opened:box_01"`
- `Spawn { prefab: "...", id: "{self}_debris_{new_id}" }` → id `"box_01_debris_1"`, `"box_01_debris_2"`, ... — a fixed `id: "{self}_debris"` alone would reuse the same literal every time `box_01`'s own id doesn't change, colliding with the previous debris spawn; see `{new_id}` substitution below
- `ModifyStat(key: "{self}.health", delta: -35.0)` → `key: "box_01.health"` (routes to that entity's `StatMap`)
- `SetStat(key: "{self}.mana", value: 0.0)` → `key: "box_01.mana"`
- `ShowDamagePopup(entity: "{self}", amount: -25.0)` → `entity: "box_01"`
- `SetEntityVisible(entity: "{self}", visible: false)` → `entity: "box_01"`
- `EmitEventAfterDelay(event: "entity.respawned:{self}", delay_secs: 15.0)` → `event: "entity.respawned:box_01"`

Note: inside a dialogue choice's `do_actions`, `{self}` resolves to the NPC's spawn ID (the same fields as in
behavior files, including `Spawn`'s `id`, `spawn_point` and `at_entity`), but `{target}` is **not** resolved there yet.
Project-level `state_machine.ron` has no `{self}` to resolve at all — there is no "self" at the project level, only `{target}`.

### `{new_id}` substitution

Only supported inside `Action::Spawn`'s `id` field. Resolves to a fresh, monotonically increasing
counter value at spawn time — use it to avoid reusing the same literal id across repeated spawns
from the same source, e.g. a monster whose corpse should get its own id every death even though
the monster itself always respawns under the same stable id:

```ron
Spawn(prefab: "zombie_corpse", id: "{self}_corpse_{new_id}")
```

- Guarantees no collision against other `{new_id}`/auto-generated ids **within the current
  scene** — it does *not* guard against colliding with a hand-authored literal id of the same
  shape (e.g. `id: "crate_{new_id}"` can still collide with a scene-placed entity literally named
  `"crate_1"`). A collision either way logs a `warn!`.
- An id built with `{new_id}` can't be referenced by another RON file afterward — only that
  entity's own behavior file (`{self}`) or the current target (`{target}`) can address it. Use it
  only for entities that manage their own lifetime (e.g. despawn themselves on a timer).
- Resets to 0 on `LoadScene`, same as the auto-generated fallback it shares a counter with — safe,
  since no spawned entity survives a scene transition anyway.

### New capabilities

| Capability | PrefabDef field | Emitted event | Notes |
|---|---|---|---|
| `CharacterController` | `components.movement` | `player.jumped` | Emitted on every jump; bind sound/effect in `state_machine.ron` |
| `TriggerZone` | `trigger_zone: ( radius: 2.0 )` | `entity.entered:{id}` / `entity.exited:{id}` | Rapier sphere sensor; runs in `FixedUpdate`. Ghost collider — excluded from player ground detection, so it can never be stood on and never suppresses grounding on the real floor beneath/near it |
| `Interactable` | `interactable: ( radius: 2.5 )` | `entity.interacted:{id}` | Player within radius + presses interact key (`inputs.interact`, default `"KeyF"`); runs in `Update` |
| `Interactable` (with `requires_item`) | `interactable: ( radius: 2.5, requires_item: "old_key" )` | `entity.interact_blocked:{id}` | Same trigger as above, but the player's `PlayerInventory` lacks `requires_item`'s key — fires instead of `entity.interacted:{id}` |

### Respawn pattern (hide + delayed re-emit)

When an entity should reappear after a delay — training dummies, respawning enemies, timed obstacles — prefer the **hide + delay + restore** pattern over `Despawn` + `Spawn`. The entity stays in the ECS the entire time, preserving its spawn ID, `StatMap`, and world-space position.

```ron
// behaviors/attack_dummy.behavior.ron
// Demonstrates a complete respawn loop in two FSM states.
(
  schema_version: 1,
  initial_state: "alive",
  global_on: [],
  states: [
    (
      name: "alive",
      // Reset health and restore visibility on every entry (including first spawn).
      entry_actions: [
        SetStat(key: "{self}.health", value: 100.0),
        SetEntityVisible(entity: "{self}", visible: true),
      ],
      exit_actions: [],
      on: [
        ( event: "entity.interacted:{self}", do_actions: [
            ModifyStat(key: "{self}.health", delta: -25.0),
            PlaySound(key: "swing_miss"),
            ShowDamagePopup(entity: "{self}", amount: -25.0),
        ]),
      ],
    ),
    (
      name: "dead",
      entry_actions: [
        SetEntityVisible(entity: "{self}", visible: false),
        EmitEventAfterDelay(event: "entity.respawned:{self}", delay_secs: 15.0),
      ],
      exit_actions: [],
      on: [],
    ),
  ],
  transitions: [
    ( from: Some("alive"), on: "stat.{self}.health.depleted", to: "dead" ),
    ( from: Some("dead"),  on: "entity.respawned:{self}",    to: "alive" ),
  ],
)
```

**Key notes:**
- The health reset (`SetStat`) happens before the visibility restore (`SetEntityVisible`) because entry actions are queued FIFO — HP is guaranteed to be full by the time the entity becomes interactable again.
- `EmitEventAfterDelay` is cleared on `LoadScene`, so the delayed respawn event is silently cancelled if the player loads a different scene before the 15 seconds elapse.
- Only labels attached via `stat_label` / `world_stat_bar` auto-hide when the entity is hidden. Screen-space UI labels driven by `SetVariable` continue to display.

**When to use this pattern vs. `Despawn` + `Spawn`:**

| Situation | Use |
|-----------|-----|
| Entity has persistent state (stats, position, spawn ID) to preserve | hide + delay + restore |
| Entity is a one-shot consumable (coin, key) that should not reappear | `Despawn` |
| Entity is dynamically cloned from a template each time | `Despawn` + `Spawn` |
| A killed enemy should be lootable, then decay faster once looted than an unlooted one | `Despawn` + `Spawn(..., at_entity: "{self}")` into a separate corpse entity — see "Lootable corpse (loot-on-death)" below |

### Lootable corpse (loot-on-death)

*(`planning/features/monster_corpse_loot.md` v2 — supersedes an earlier same-entity design that
put `interactable:`/`inventory:` directly on the monster prefab and used `SetEntityVisible` +
`entity.respawned` for revival. That approach could not support a respawn timer independent of the
corpse's own loot-decay timer — see "Why not the same entity?" below — so it was replaced with the
separate-entity approach documented here.)*

A monster's corpse is a **separate entity**, not the monster hidden in place. On death, the
monster plays its death animation, spawns a dedicated `{monster}_corpse` prop entity at its exact
death position, and despawns itself. Looting and decay are entirely the corpse's own concern; the
monster's own respawn (a fixed delay, independent of whether the corpse is ever looted) is handled
by a global rule that no longer depends on the original entity existing.

```ron
// behaviors/enemy_zombie.behavior.ron — "dead" state (excerpt)
(
  name: "dead",
  entry_actions: [
    // ...death animation, effects...
    EmitEventAfterDelay(event: "zombie.swap_to_corpse:{self}", delay_secs: 3.0), // matches death anim length
    EmitEventAfterDelay(event: "monster.respawn:{self}",       delay_secs: 30.0),
  ],
  on: [
    (
      event: "zombie.swap_to_corpse:{self}",
      do_actions: [
        Spawn(prefab: "zombie_corpse", id: "{self}_corpse_{new_id}", at_entity: "{self}"),
        Despawn("{self}"),
      ],
    ),
  ],
),
```

`Spawn`'s `at_entity` field (see `docs/20_data_formats.md`) resolves the corpse's full transform —
position, rotation, and scale — from the monster's own live `GlobalTransform` at the moment the
swap fires, so the corpse always appears exactly where the monster died, even though monsters
patrol and their death position varies run to run. A fixed `position:`/`spawn_point:` would not
work here for that reason.

**Each corpse's id includes `{new_id}`** (see "`{new_id}` substitution" above), a fresh,
monotonically increasing suffix appended at the moment the `Spawn` actually
executes — so `zombie_01`'s corpses are `zombie_01_corpse_7`, `zombie_01_corpse_12`, and so on,
never the same literal twice. This means repeated kills of the same monster slot let every corpse
coexist and decay/be looted independently, instead of an earlier design where the id was a fixed,
reused `"{monster}_corpse"` literal and an older, still-decaying corpse had to be pre-emptively
despawned to avoid two corpses colliding on the same id. **A `SetDespawnTimer` (see below), not
`EmitEventAfterDelay`, is still what despawns a corpse** — not because of an id-collision risk
(unique ids make that impossible now), but because `SetDespawnTimer` is simply the right tool for
a per-entity decay timer: it's a component living directly on the corpse entity, so it Just Works
for any number of simultaneously-decaying corpses with zero extra event-name bookkeeping, where
`EmitEventAfterDelay` would need you to manually keep each corpse's decay event globally unique
yourself.

**Global respawn rule** (`logic/state_machine.ron`'s top-level `global_on:`, not `"playing"`'s
own state-scoped `on:` list):

```ron
global_on: [
  ( event: "monster.respawn:zombie_01", do_actions: [
      Spawn(prefab: "enemy_zombie", id: "zombie_01", spawn_point: "zombie_01_spawn", yaw_deg: 200.0)
  ] ),
  // ...one rule per monster slot...
],
```

This **must** be `global_on`, not a state-scoped rule. A real pause freezes the timer, but it still
fires while the game is in any other non-`"playing"` state (the menu, a scene swap) — a state-scoped
`on:` handler simply never matches in that case, silently and permanently losing that monster's
respawn for the rest of the session.
`global_on` fires "regardless of state, no state change," so it always catches the event. The event
name is a single, monster-type-agnostic `monster.respawn:{id}` convention (not
`zombie.respawn:{id}`/`snake.respawn:{id}`/etc.) specifically so a future 4th monster type is a
copy-paste of one rule line, not a new per-type event name to keep in sync everywhere.

**The corpse prefab** carries the fields a static chest would, plus a shared behavior file and its
own animation policy (see "Spawn-already-posed pattern" in `docs/20_data_formats.md`):

```ron
// prefabs/prefabs.ron — zombie_corpse (excerpt)
"zombie_corpse": (
  kind: Prop,
  animation_policy: "prefabs/animation/corpse_policy_zombie.ron",
  interactable: (radius: 2.0, hint_text: "Loot"),
  inventory: (
    max_slots: 6,
    initial_items: [
      (item_key: "gold_coin", count: 15),
      (item_key: "health_potion", count: 1),
    ],
  ),
  behavior: "behaviors/lootable_corpse.behavior.ron",
  // no trigger_zone, no colliders — see the note below
),
```

**`animation_policy` points at a corpse-specific policy file, not the live monster's full one.**
`corpse_policy_zombie.ron` sets `base.idle`/`walk`/`run`/`jump_loop` all to the death clip and
keeps only the `death` override — so any resolver fallback (no active override, a missing node
index, a graph-validation failure) still lands on the death pose instead of a standing idle loop.
Without `animation_policy` at all, a corpse prop renders in raw bind/rest pose — not lying dead —
since nothing ever drives its skeleton.

**Adding a corpse for a new monster type — the complete list:**
1. `prefabs/animation/corpse_policy_{monster}.ron` — copy an existing one; set all four
   `base.idle`/`walk`/`run`/`jump_loop` and the single `death` override to *this* model's death
   clip name (check it with `ironhold inspect glb`). Only list `animation_sources` if this
   model's death clip lives in an external animation-pack GLB, like the zombie's does — the
   snake/spider ones need none, since their clip is embedded in their own model GLB.
2. `prefabs/prefabs.ron` — a `{monster}_corpse` prefab: `kind: Prop`, the monster's own `model`,
   `animation_policy:` pointing at step 1, `interactable:`, `inventory:`, and
   `behavior: "behaviors/lootable_corpse.behavior.ron"` (shared — do not copy or edit it).
3. `behaviors/enemy_{monster}.behavior.ron` — the `dead` state's two `EmitEventAfterDelay` lines
   and the `swap_to_corpse` handler's Spawn → Despawn pair (`id: "{self}_corpse_{new_id}"`, see the
   zombie example above).
4. `scenes/main.scene.ron` — a `spawn_points:` entry per instance, matching that entity's own
   placement/`yaw_deg`.
5. `logic/state_machine.ron`'s `global_on:` — one `monster.respawn:{literal_id}` rule per
   instance (see the Global respawn rule example above).
6. `logic/state_machine.ron`'s `entry_actions` — a `PreloadPrefab("{monster}_corpse")` line, or
   that type's first death stalls the browser while its corpse GLB decodes.

You do **not** need to touch `lootable_corpse.behavior.ron` itself — every field in it is
`{self}`-relative, so it already covers any new monster type with zero changes.

**Deliberately no `trigger_zone`.** The corpse prop needs none: the loot panel is closed with its
own close button. The consequence is just that there's no
auto-close-on-walk-away — the player closes the loot panel with its own close button instead
(already globally wired via `ui.button_pressed:close_container`, the same as `chest_01`'s panel).

**`behaviors/lootable_corpse.behavior.ron` is shared by every monster's corpse prefab** — every
field in it is `{self}`-relative, so `zombie_corpse`/`snake_corpse`/`spider_corpse`/any future one
point at the same file with no changes needed:

```ron
(
  schema_version: 1,
  initial_state: "fresh",
  states: [
    (
      name: "fresh",
      entry_actions: [
        // Jump straight to the final frame of the death clip and hold it there, instead of
        // playing the whole clip from scratch (the player already watched it play out on the
        // now-despawned live monster). See docs/20_data_formats.md's "Spawn-already-posed
        // pattern". Fired from here, not the monster's own death sequence, because the corpse
        // entity doesn't exist yet at the point the monster's Spawn(...) action runs.
        PlayAnimationOn(target: "{self}", clip: "death", start_at_fraction: 1.0, freeze: true),
        SetDespawnTimer(entity: "{self}", delay_secs: 600.0),
      ],
      on: [ ( event: "entity.interacted:{self}", do_actions: [ OpenContainer("{self}") ] ) ],
    ),
    (
      name: "looted",
      entry_actions: [
        CloseContainer,                                        // close before anything else
        SetDespawnTimer(entity: "{self}", delay_secs: 20.0),
      ],
      on: [],
    ),
  ],
  transitions: [ ( from: "fresh", on: "container.looted:{self}", to: "looted" ) ],
)
```

`SetDespawnTimer(entity, delay_secs)` arms a self-contained despawn on the named entity — after
`delay_secs` of game time (frozen while the game is paused) it despawns automatically, no event round-trip and no `on:` handler
needed to react to it (unlike `EmitEventAfterDelay`, which needs a paired `on:` handler and, for a
timer whose target's id might later be reused by an unrelated entity, needs careful ownership to
avoid firing against the wrong one — see `crates/ironhold_core/src/capabilities/despawn_timer.rs`).
Re-arming it (calling it again on the same entity, as the `fresh` → `looted` transition does)
overwrites the previous countdown rather than stacking.

**Why not the same entity? (superseded v1 approach)** An earlier version of this feature put
`interactable:`/`inventory:` directly on the monster prefab and split its `dead` state into
`dead_full`/`dead_looted`, using `SetEntityVisible` to hide the corpse and `entity.respawned` to
revive it in place. This worked until the feature's actual requirement — the corpse persists for
up to 10 minutes independent of when the monster respawns (a fixed 30s after death) — because
a same-entity design has no way to keep the monster "dead" (so its own corpse state machine can
keep tracking loot/decay) while also being independently "alive" (so it can be fought again). The
two timers are only independent if they belong to two different entities, hence the redesign above.

**Key notes:**
- `container.looted:{id}` only fires when `TakeAllFromContainer` actually transfers at least one
  item — a corpse spawned with no loot never reaches `looted` via this path, and just decays on
  `fresh`'s own 600s ambient timer instead. This is expected, not a bug: "empty" and "never looted"
  are indistinguishable to this pattern.
- `CloseContainer` in `looted`'s entry actions runs before `SetDespawnTimer` — entry actions are
  queued FIFO, so the panel is never left open and bound to a corpse that's about to disappear.
- **Opening a container while one is already open no longer double-counts `panels_open`.**
  `interactable_system` fires `entity.interacted` for *every* interactable within radius on one
  keypress, not just the nearest — with two corpses near each other, one interact press could queue
  two `OpenContainer` calls in the same frame. `Action::OpenContainer` only increments `panels_open`
  when no container was already open, since the single `ContainerPanel` UI can only ever show one
  container at a time regardless of how many `OpenContainer` calls land — without this guard, a
  second call while one was already open would over-increment a counter that only ever gets
  decremented once per `CloseContainer`, permanently suppressing interact/collectible-pickup/
  tab-targeting (all gated on `panels_open == 0`) until the next `LoadScene`. The same guard applies
  to `OpenShop`.
- **`Action::Despawn` closes the container panel if the despawned entity is the one currently
  open.** Without this, a corpse decaying while its own loot panel is open would leave a ghost
  panel bound to a gone entity and `panels_open` stuck above 0, permanently blocking
  interact/pickup/tab-targeting.
- **The player's target selection clears when the targeted entity is genuinely despawned, not just
  hidden.** `target_auto_clear_system` originally only checked `Visibility::Hidden` on an entity
  still present in `SpawnRegistry` — correct for the old hide-in-place revival, but a real
  `Despawn` (this feature's death sequence) removes the entity from the registry outright. Treating
  "not found in the registry" the same as "hidden" prevents a stale target from silently surviving
  until the same id is reused by that slot's next respawn, up to 30s later.
- An orphaned `stat_label`/`world_stat_bar` widget (one still tracking a despawned entity) is
  cleaned up automatically by `stat_widget_cleanup_system` (mirrors the pre-existing
  `nameplate_cleanup_system` pattern) — without it, `Action::Despawn` alone left every stat-bar
  update system logging a "stat_key not found" warning every frame for the gone entity.
- **Corpses no longer collide on id.** Each corpse spawns under a fresh, unique
  `"{self}_corpse_{new_id}"` id (see "`{new_id}` substitution" above) — if the same monster slot
  dies again while its previous corpse is still on screen, both corpses simply coexist and decay/
  are lootable independently, rather than the newer one replacing the older.
- See `crates/ironhold_core/src/capabilities/inventory.rs` for `Inventory`/`ContainerPanel`,
  `crates/ironhold_core/src/capabilities/despawn_timer.rs` for `SetDespawnTimer`'s implementation,
  and `assets/projects/3rd_person_game_demo/behaviors/{enemy_zombie,lootable_corpse}.behavior.ron`
  for the full files.

### Multi-scene navigation via portal prefabs

A walkthrough portal between two scenes is the standard pattern for multi-scene projects. The full setup uses a composite `TriggerZone` prefab and a `LoadScene` binding:

**1. Define the portal prefab** (`prefabs/prefabs.ron`):
```ron
"portal_to_arena": (
  kind: "primitive",
  model: "",
  trigger_zone: (radius: 1.5),
  children: [
    // arch pillars, lintel, glow disc — visual only
    ( offset: (-0.8, 1.5, 0.0), primitive: Cuboid(0.3, 3.0, 0.3), color: (0.3, 0.3, 0.4) ),
    ( offset: ( 0.8, 1.5, 0.0), primitive: Cuboid(0.3, 3.0, 0.3), color: (0.3, 0.3, 0.4) ),
    ( offset: ( 0.0, 3.1, 0.0), primitive: Cuboid(2.0, 0.3, 0.3), color: (0.3, 0.3, 0.4) ),
    ( offset: ( 0.0, 1.5, 0.0), primitive: Cylinder(height: 2.8, radius: 0.75),
      color: (0.5, 0.3, 1.0), alpha: 0.35, alpha_mode: Blend ),
  ],
),
```

**2. Place it in the scene** (`scenes/main.scene.ron`):
```ron
( id: "portal_to_arena", prefab: "portal_to_arena",
  transform: ( translation: (0.0, 0.0, -20.0), ... ) ),
```

**3. Wire the binding** (`logic/state_machine.ron`):
```ron
( event: "entity.entered:portal_to_arena", do_actions: [ LoadScene("scenes/arena.scene.ron") ] ),
```
(as a `global_on` entry, or inside a state's `on:` list if the portal should only work in one mode)

The `TriggerZone` radius should cover the gate opening but not the frame — 1.0–1.5 m works for a standard doorway width. The trigger fires `entity.entered:{id}` on player overlap; `LoadScene` then transitions immediately.

**Quality and warmup on scene entry**: if the destination scene uses `SetParticleQuality`, include it in the `scene.ready:{name}` binding alongside any warmup `SpawnEffect` calls (see [Warming up particle pipelines](20_data_formats.md#warming-up-particle-pipelines-web-builds)) — quality persists across `LoadScene` and must be reset explicitly per scene.
