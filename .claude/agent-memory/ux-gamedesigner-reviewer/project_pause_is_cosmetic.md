---
name: pause-is-cosmetic
description: "Pause in Ironhold is a UI overlay only — nothing in the sim stops; docs/10, docs/30, docs/00 and STATUS all imply a real Paused state that does not exist"
metadata:
  type: project
---

`LoadSceneOverlay("scenes/pause.scene.ron")` + an FSM `"paused"` state is the shipped
"pause menu" pattern, but **nothing in the engine is gated by it**. NPC AI, physics,
player movement, interact/loot, particles, timers all keep running behind the overlay.
There is no `GamePaused`/`is_paused`/`Time<Virtual>` pause anywhere in `ironhold_core`.

**Why:** pause was only ever built as a *scene-layer* feature (overlay rendering +
FSM state), never as a *simulation-layer* one. Frank hit it live in
`3rd_person_game_demo` (2026-09-15): monsters kept chasing, looting still worked, the
character still walked around under the menu.

**How to apply:**

- Two shipped projects author the broken pattern: `3rd_person_game_demo`
  (`paused`) and `primitive_world` (`paused` **and** `game_over` — a game-over screen
  the player can still play behind).
- Four doc surfaces currently assert or imply pause is real and must be corrected
  together with any fix:
  - `docs/10_architecture.md` ~110 — lists `Paused` as an engine lifecycle state.
  - `docs/00_overview.md` ~121 — "pause/menu flow" as a reason to use schema v3.
  - `docs/30_runtime_events_and_logic.md` ~229/~360 — the canonical copy-paste
    pause FSM example (no simulation stop in it).
  - `docs/STATUS.md` ~158 — the same example again.
- `docs/30` ~599-606 **and** the long RON comment at
  `3rd_person_game_demo/logic/state_machine.ron` lines 12-30 both rest on
  "`tick_delayed_events_system` ticks on raw `Time` with no pause-gate, so respawn
  timers fire while paused." Any real pause (which freezes `Res<Time>` via
  `Time<Virtual>`) makes that paragraph false. The `global_on` advice itself stays
  correct for its *other* reason (the arming entity is despawned by then) — correct
  the paused-specific rationale, don't delete the rule.
- `interactable_system`/`collectible_system`/`tab_targeting_system` early-return only
  on `LoadedInventoryUi.panels_open > 0`. A scene overlay never touches that counter.
  Any pause work should unify these three gates + pause behind one shared run
  condition, or the next system added will forget one.
- There is **no UI focus / keyboard-menu-navigation concept anywhere** — inventory,
  shop, container, dialogue and pause are all mouse-click-only. The only keyboard
  route to a UI action is `global_key_bindings`/`scene_key_bindings` mapping a key
  straight to a trigger name. Do not scope keyboard menu nav to pause alone.
- Clock-control collision risk: `planning/features/static_scene_mode.md` also plans to
  call `time.pause()` (for `?static=1` screenshot baselines). Whoever lands second must
  not fight the first — they need one owner. See [[docs-lag-actions]] for the action
  doc-surface checklist any new `PauseGame`/`ResumeGame`/`TogglePause` variant must hit.
