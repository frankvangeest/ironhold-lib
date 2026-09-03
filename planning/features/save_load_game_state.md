# Feature: Save / Load Game State

_Status: Draft_
_Planned at: `fcc53aa` (2026-06-02)_

---

> ## Pre-implementation checklist
>
> - [ ] **Decide: which state is save-worthy.** Four candidates: (a) `GameVariables` — string map, serialises trivially; (b) `LoadedStats` — global stats (player health, mana, score); (c) per-entity `StatMap` — entity-local stats (goblin HP); (d) active modifiers on each `LiveStat`. Recommendation: **v1 saves (a) + (b) only.** Per-entity stats are tied to scene entities that respawn fresh on `LoadScene` — saving them requires a stable entity identity scheme (see next item). Active modifiers on global stats are part of `LiveStat` and included automatically when saving `LoadedStats`.
>
> - [ ] **Decide: cross-scene entity identity for per-entity stats.** Per-entity `StatMap` restoration is hard: entities are despawned and respawned on `LoadScene`; a saved `orc_01.health` must be reapplied after the entity respawns. The mechanism requires: (1) knowing which scene to reload; (2) a `PendingSaveRestore` resource that holds entity stat snapshots; (3) a post-scene-load hook that applies the snapshots by `SpawnId`. This is doable but adds significant complexity. **Defer per-entity stats to v2** — note it as a follow-up in the feature tasks. For v1, only global state is saved.
>
> - [ ] **Decide: save format.** Options: (a) RON — consistent with the rest of the project, human-readable, but needs the `ron` crate for serialisation (currently only used for deserialisation via `serde`); (b) JSON — `serde_json` crate, widely supported including WASM, one extra dependency. Recommendation: **JSON** — `serde_json` is WASM-compatible and already used by many Bevy projects; RON serialisation requires `ron::to_string()` which is less commonly tested for complex types.
>
> - [ ] **Decide: WASM storage backend.** `std::fs` is unavailable on WASM. Options: (a) `localStorage` via `web_sys::Storage` — synchronous, 5 MB limit, string values, no extra crate; (b) IndexedDB — async, larger limit, more complex. Recommendation: **`localStorage`** for v1 — sufficient for the state being saved (GameVariables + global stats are small); IndexedDB deferred. Key format: `"ironhold_save_{project_name}_{slot}"`.
>
> - [ ] **Decide: save slots.** Fixed 10 slots (0–9)? Unlimited named slots? Recommendation: **named string slots** — more flexible for designers (`"autosave"`, `"chapter_1"`, `"quicksave"`). The slot name is the file stem (native) or localStorage key suffix (WASM). Designers declare slot names in `do_actions`.
>
> - [ ] **Confirm: `serde` derives on `GameVariables` and `LiveStat`.** Both need `Serialize` added alongside the existing `Deserialize`. `LiveStat` has Vec fields (thresholds, modifiers) — confirm these all derive `Serialize` cleanly before starting.

---

## What

`SaveGame` / `LoadGame` actions that serialize `GameVariables` + global `LoadedStats` to disk (native) or `localStorage` (WASM). Designers wire saves to any pipeline event: scene transition, player death, checkpoint trigger, game completion.

v1 saves global state only (variables + global stats). Per-entity `StatMap` restoration (NPC health persistence across sessions) is explicitly deferred to a follow-up.

---

## Why

Without save/load, every session starts from scratch. `GameVariables` already stores scores, flags, and quest-adjacent state that designers want to persist. Global stats (player health, mana, currency) are the other half. These two together cover the minimum viable save for an RPG or action game.

Unblocks: persistent quest flags, unlockable areas, player progression across sessions.

---

## Save state schema

```rust
// The serialised representation. Not a Bevy component — a plain serialisable struct.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SaveState {
    pub schema_version: u32,        // version for forward-compatibility checks
    pub project_name: String,       // matches ProjectConfig.name
    pub scene_path: String,         // last loaded scene path (for LoadGame scene restore)
    pub variables: HashMap<String, String>,  // GameVariables snapshot
    pub global_stats: HashMap<String, SavedStat>,  // LoadedStats snapshot (global keys only)
    // Future v2: entity_stats: HashMap<String, HashMap<String, SavedStat>>
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SavedStat {
    pub current: f32,
    pub modifiers: Vec<SavedModifier>,  // active timed/permanent modifiers
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SavedModifier {
    pub key: String,
    pub remaining_secs: Option<f32>,  // None = permanent
}
```

---

## New actions (`schema/actions.rs`)

```ron
// Serialize current GameVariables + LoadedStats to slot "autosave" 
SaveGame(slot: "autosave")

// Load from slot "autosave", restore variables/stats, reload the saved scene
LoadGame(slot: "autosave")

// Delete a save slot
DeleteSave(slot: "autosave")
```

```rust
/// Serialise GameVariables + LoadedStats to a named save slot.
/// Native: writes to `{project_dir}/saves/{slot}.json`.
/// WASM: writes to localStorage key `"ironhold_save_{project}_{slot}"`.
/// No-op (with warning) if serialisation fails.
SaveGame { slot: String },

/// Restore GameVariables + LoadedStats from a named save slot, then reload the saved scene.
/// Emits `save.loaded:{slot}` before triggering LoadScene.
/// No-op (with warning) if the slot does not exist.
LoadGame { slot: String },

/// Delete a named save slot. No-op if it does not exist.
DeleteSave { slot: String },
```

---

## New pipeline events

```ron
save.written:{slot}      // save written successfully
save.load_failed:{slot}  // slot not found or deserialisation error
save.loaded:{slot}       // state restored successfully (fires before scene reloads)
```

---

## Runtime

### `SaveLoadState` resource (`capabilities/save_load.rs`)

```rust
#[derive(Resource, Default)]
pub struct SaveLoadState {
    /// Set by the project loader on scene transition — tracks which scene is active.
    pub current_scene_path: String,
    pub project_name: String,
}
```

### `save_game_system` (triggered by `Action::SaveGame`)

```rust
fn execute_save(slot: &str, state: &SaveLoadState, vars: &GameVariables, stats: &LoadedStats) {
    let save = SaveState {
        schema_version: 1,
        project_name: state.project_name.clone(),
        scene_path: state.current_scene_path.clone(),
        variables: vars.0.clone(),
        global_stats: stats.0.iter().map(|(k, v)| (k.clone(), SavedStat::from(v))).collect(),
    };
    let json = serde_json::to_string(&save).expect("SaveState serialisation failed");

    #[cfg(not(target_arch = "wasm32"))]
    {
        let path = format!("saves/{}.json", slot);
        std::fs::create_dir_all("saves").ok();
        std::fs::write(&path, &json).ok();
    }

    #[cfg(target_arch = "wasm32")]
    {
        if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
            let key = format!("ironhold_save_{}_{}", state.project_name, slot);
            storage.set_item(&key, &json).ok();
        }
    }
}
```

### `load_game_system` (triggered by `Action::LoadGame`)

1. Read JSON from disk / localStorage.
2. Deserialise into `SaveState`. On error, emit `save.load_failed:{slot}` and return.
3. Write `GameVariables.0 = save.variables`.
4. For each `(key, saved_stat)` in `save.global_stats`: find the matching `LiveStat` in `LoadedStats` by key, restore `current` value, re-apply modifiers via `ApplyModifier` logic (or direct `active_modifiers` write).
5. Emit `save.loaded:{slot}`.
6. Queue `Action::LoadScene(save.scene_path)` to return to the saved scene.

---

## Designer usage patterns

**Autosave on checkpoint:**
```ron
( on: "trigger.checkpoint_a", do_actions: [ SaveGame(slot: "autosave") ] ),
```

**Load autosave from main menu:**
```ron
( on: "ui.button_pressed:continue_button", do_actions: [ LoadGame(slot: "autosave") ] ),
```

**Check if save exists (v1 workaround — use a variable):**
```ron
// On each SaveGame, also set a variable as a "save exists" flag
( on: "trigger.checkpoint_a", do_actions: [
    SaveGame(slot: "autosave"),
    SetVariable("save_exists", "true"),
] ),
// On load, check the variable to show/hide the Continue button
```

**Slot-based manual save (RPG pause menu):**
```ron
( on: "ui.button_pressed:save_slot_1", do_actions: [ SaveGame(slot: "slot_1") ] ),
( on: "ui.button_pressed:load_slot_1", do_actions: [ LoadGame(slot: "slot_1") ] ),
```

---

## New Rust changes

- `schema/actions.rs` — add `SaveGame { slot: String }`, `LoadGame { slot: String }`, `DeleteSave { slot: String }`.
- `capabilities/save_load.rs` (new file) — `SaveState`, `SavedStat`, `SavedModifier`, `SaveLoadState`, `execute_save`, `execute_load`.
- `capabilities/mod.rs` — register module.
- `runtime/scene_manager/action_executor.rs` — handle `SaveGame`, `LoadGame`, `DeleteSave`.
- `runtime/scene_manager/scene_loader.rs` — update `SaveLoadState.current_scene_path` on each scene load.
- `Cargo.toml` (ironhold_core) — add `serde_json`.
- `Cargo.toml` (ironhold_web) — add `web_sys` with `"Storage"` feature if not already present.
- Derive `Serialize` on `GameVariables`, `LiveStat`, `ActiveModifier` in `schema/stats.rs` and `lib.rs`.

---

## Tasks

- [ ] Decisions from pre-implementation checklist resolved
- [ ] `Serialize` derived on `GameVariables`, `LiveStat`, `ActiveModifier`
- [ ] `SaveState`, `SavedStat`, `SavedModifier` structs in `capabilities/save_load.rs`
- [ ] `SaveLoadState` resource; updated on every scene load
- [ ] `Action::SaveGame`, `LoadGame`, `DeleteSave`
- [ ] `execute_save` — cfg-gated native (fs) and WASM (localStorage) backends
- [ ] `execute_load` — deserialise, restore variables + stats, emit event, queue LoadScene
- [ ] `serde_json` dependency in `ironhold_core/Cargo.toml`
- [ ] `web_sys` Storage feature in `ironhold_web/Cargo.toml`
- [ ] Pipeline events: `save.written`, `save.load_failed`, `save.loaded`
- [ ] Demo: wire `SaveGame` + `LoadGame` to buttons in `entity_logic_demo` or `3rd_person_game_demo`
- [ ] Integration tests: round-trip GameVariables, round-trip LoadedStats current values, `LoadGame` with missing slot emits `save.load_failed`, scene path restored
- [ ] Docs: actions + events in `docs/30_runtime_events_and_logic.md`; storage backends in `docs/20_data_formats.md`

---

## Open questions

- **Per-entity stat persistence (v2)**: saving `StatMap` per entity requires post-scene-load restoration by SpawnId. The mechanism: `PendingSaveRestore(HashMap<String, HashMap<String, f32>>)` resource populated on `LoadGame`, drained by a one-shot system after `SceneEvent::Ready`. Document as a v2 follow-up in the tasks list.
- **Save file location on native**: `saves/` relative to the working directory is the simplest. A `{AppDirs}` pattern (OS-appropriate save location) is more correct for shipped games — deferred.
- **Save conflicts on WASM**: `localStorage` is synchronous and single-threaded on the browser main thread — no conflict risk.
- **Slot enumeration**: a `ListSaves` action or query is not in scope for v1 — designers hardcode slot names. A `query saves` CLI command or a UI-side listing can come later.

---

## Acceptance criteria

- Given `SaveGame(slot: "test")`, `GameVariables` and global `LoadedStats` current values are written to disk (native) or localStorage (WASM).
- Given `LoadGame(slot: "test")`, `GameVariables` are restored to their saved values, global stats are restored, `save.loaded:test` is emitted, and `LoadScene` is queued with the saved scene path.
- Given `LoadGame(slot: "nonexistent")`, `save.load_failed:nonexistent` is emitted and no scene change occurs.
- Given `DeleteSave(slot: "test")`, the save data is removed; a subsequent `LoadGame` emits `save.load_failed`.
- Given a WASM build, `SaveGame` writes to localStorage and `LoadGame` reads from it.
- Given `GameVariables["score"] = "42"` at save time, after `LoadGame` the variable is `"42"`.
