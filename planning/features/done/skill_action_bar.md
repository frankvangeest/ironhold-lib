# Feature: Skill Action Bar

_Status: Draft_
_Planned at: `5bfd752` (2026-05-31)_

---

> ## Pre-implementation checklist
>
> - [ ] **Decide: `ActionBar` as a scene RON UI node vs. a standalone config file.** A scene UI node fits the existing pattern (buttons, stat bars, etc. all live in scene RON). A standalone file allows sharing the same bar across scenes without copy-paste. Recommendation: scene UI node for v1 — a shared bar can be a scene `include:` pattern later.
>
> - [ ] **Decide: cooldown tracking via `CooldownMap` vs. stat regen mechanism.** The stats regen system already ticks over time, but it's designed for stat values not discrete cooldown timers. A separate `CooldownMap` resource (slot-id → remaining secs, ticked in a dedicated system) is cleaner and avoids leaking action-bar concepts into the stats system. Confirm before writing code.
>
> - [ ] **Decide: slot cost deduction — which stat and when.** Cost is expressed as `cost: (stat: "mana", amount: 10.0)`. Deduction happens at activation time via `ModifyStat`. If the stat is insufficient, the slot does not fire and emits `action_bar.insufficient_resource:{slot}`. Verify this integrates cleanly with the existing `ModifyStat` action before adding a special cost path.
>
> - [ ] **Confirm `{target}` substitution scope.** Slot `do_actions` support `{self}` and `{target}` substitution. `{target}` resolves from the `CurrentTarget` resource (shared with tab targeting and click-to-select). If `CurrentTarget` is `None` and a slot uses `{target}`, the slot should silently no-op or emit a `action_bar.no_target:{slot}` event — decide which before wiring substitution.

---

## What

A configurable 9-slot action bar declared in scene RON as a new `ActionBar` UI node. Each slot has a keybind (1–9), an icon, one or more `do_actions`, an optional cooldown, and an optional stat cost. Pressing a slot key fires its actions through the existing pipeline. Slots show a greyed-out state when on cooldown or when the cost stat is insufficient. Slot state events flow into the pipeline so designers can react (play a sound, show a flash, etc.).

---

## Why

Many game genres (RPG, MOBA, action) need quick-access abilities bound to number keys. Without this, designers must wire each ability to a separate keybind via `InputAction` entries — no shared UI, no cooldowns, no cost management. This feature unblocks:

- Skill-based combat in any project without Rust changes
- AoE ground targeting (hard dep on action bar slots)
- Meaningful use of the `{target}` substitution once tab/click targeting ships

---

## Schema

### New UI node type: `ActionBar`

```ron
// In a scene's ui: [...] block
ActionBar(
    position: (x: 10.0, y: 10.0),
    anchor: BottomCenter,
    slot_size: 64.0,
    slot_gap: 4.0,
    slots: [
        (
            key: "1",
            icon: "icons/fireball",          // asset catalog texture key
            do_actions: [
                SpawnEffect("effects/fireball_cast"),
                ModifyStat("mana", -10.0),
            ],
            cooldown_secs: 2.0,
            cost: (stat: "mana", amount: 10.0),
            label: "Fireball",               // optional tooltip label
        ),
        (
            key: "2",
            icon: "icons/heal",
            do_actions: [ ModifyStat("{target}.health", 25.0) ],
            cooldown_secs: 5.0,
            cost: (stat: "mana", amount: 20.0),
        ),
        // ... up to 9 slots
    ],
)
```

### New Rust schema types (`schema/ui.rs`)

```rust
pub struct ActionBarDef {
    pub position: Vec2,
    pub anchor: UiAnchor,       // reuse existing anchor enum
    pub slot_size: f32,         // px, default 64.0
    pub slot_gap: f32,          // px, default 4.0
    pub slots: Vec<ActionSlotDef>,
}

pub struct ActionSlotDef {
    pub key: String,            // "1"–"9"
    pub icon: String,           // asset catalog texture key
    pub do_actions: Vec<Action>,
    pub cooldown_secs: Option<f32>,
    pub cost: Option<SlotCost>,
    pub label: Option<String>,
}

pub struct SlotCost {
    pub stat: String,           // stat key (supports dot-routing, e.g. "{self}.mana")
    pub amount: f32,
}
```

---

## New actions / events

```ron
// No new actions needed — slot activation fires existing do_actions directly.

// New events into the pipeline:
action_bar.activated:{slot_key}          // slot fired successfully
action_bar.on_cooldown:{slot_key}        // pressed while on cooldown
action_bar.insufficient_resource:{slot_key}  // cost stat too low
action_bar.no_target:{slot_key}          // {target} used but CurrentTarget is None
```

---

## New runtime resource

```rust
// CooldownMap tracks remaining cooldown per slot (keyed by slot_key string)
#[derive(Resource, Default)]
pub struct CooldownMap(pub HashMap<String, f32>);
```

Ticked by `cooldown_tick_system`: drains all entries by the elapsed time each tick, removes when ≤ 0.

> **Tick schedule note:** For v1 (single-player), `cooldown_tick_system` runs on the variable-rate render schedule using `time.delta_secs()`. This is non-deterministic — frame timing varies slightly between machines — which is acceptable for single-player but incompatible with multiplayer replay (Beta 0.5+).
>
> When Beta 0.5 ships its fixed-tick schedule, `cooldown_tick_system` must migrate to that schedule, subtracting the fixed timestep per tick instead of `delta_secs`. The visual sweep overlay (`action_bar_visual_system`) is presentation-only and must stay on the render tick regardless.
>
> Design `cooldown_tick_system` as a standalone system with no render-only dependencies so the schedule migration is a one-line change.

---

## Key Rust changes

1. **`schema/ui.rs`** — add `ActionBarDef`, `ActionSlotDef`, `SlotCost`; add `ActionBar(ActionBarDef)` variant to the scene UI node enum.

2. **`capabilities/action_bar.rs`** (new file)
   - `action_bar_spawn_system`: reads `ActionBarDef` from the scene, spawns slot UI nodes (icon image + keybind label + cooldown overlay).
   - `action_bar_input_system`: listens for key presses (1–9), looks up the slot, checks cooldown and cost stat, fires `do_actions` via `ActionQueue`, emits pipeline events.
   - `cooldown_tick_system`: ticks `CooldownMap` each frame.
   - `action_bar_visual_system`: updates slot greyed-out state and cooldown sweep overlay based on `CooldownMap` and stat values.

3. **`runtime/scene_manager/scene_loader.rs`** — handle `ActionBar` UI node: spawn the `ActionBarDef` component on the scene entity.

4. **`schema/actions.rs`** — no new variants; slot `do_actions` reuse existing `Action` enum.

5. **`runtime/scene_manager/message_interpreter.rs`** — `{target}` substitution must resolve from `CurrentTarget` resource (same path as tab targeting will use). Wire it here so both features share the resolution logic.

---

## Implementation notes

- Schema structs (`ActionBarDef`, `ActionSlotDef`, `SlotCost`) added to `schema/scene_v2.rs`.
- `ActionBar(ActionBarDef)` variant added to `UiNodeDef`; all `id/size/position/absolute/align` match arms updated.
- `capabilities/action_bar.rs` — `ActionBarPlugin`, `CooldownMap`, `CurrentTarget`, `ActionSlotUi`, `CooldownOverlay`, three systems.
- Slot UI spawned directly in `spawn_ui_element_node` (scene_loader.rs); no separate spawn system needed.
- `{target}` substitution resolved against `CurrentTarget` resource (always `None` until the targeting system ships).
- Demo: 3-slot bar in `primitive_world` — Heal (1), Speed Boost (2), Fire Burst (3); mana bar added alongside.
- `action_bar.*` pipeline events wired in `primitive_world/logic/state_machine.ron` to show status messages.
- Cooldown visual: top-anchored dark overlay that shrinks to 0 as cooldown depletes (no clock-wipe shader needed).
- Icon field exists in schema but not rendered in v1 (no icon assets yet in `assets/shared/textures/ui/`).

## Tasks

- [x] Decisions from pre-implementation checklist resolved and noted above
- [x] `ActionBarDef`, `ActionSlotDef`, `SlotCost` in `schema/scene_v2.rs`
- [x] `ActionBar(ActionBarDef)` variant added to scene UI node enum
- [x] `CooldownMap` resource in `capabilities/action_bar.rs`
- [x] Slot UI spawned in `scene_loader.rs` `spawn_ui_element_node`
- [x] `action_bar_input_system` — key press → cooldown check → cost check → fire actions → emit events
- [x] `cooldown_tick_system` — drain `CooldownMap` each frame
- [x] `action_bar_visual_system` — dim overlay + cooldown fill height
- [x] `CurrentTarget` resource defined (placeholder for targeting system)
- [x] `scene_loader.rs` handles `ActionBar` UI node
- [x] Demo: 3-slot action bar in `primitive_world`
- [x] Docs: `ActionBar` documented in `docs/20_data_formats.md`

---

## Open questions

- **Slot count**: hardcoded 1–9, or configurable? 1–9 covers the standard action bar pattern. A `max_slots: 9` field allows shorter bars (e.g. 4 slots for a mobile-style layout) without schema changes.
- **Multiple action bars**: one bar per scene, or multiple? A single bar per scene is fine for v1. Multiple bars (e.g. main skills + consumables) can be handled by having two `ActionBar` nodes with non-overlapping key ranges.
- **Cooldown visual**: sweep (clock wipe) or simple opacity fade? Sweep is more informative; opacity is simpler to implement in Bevy UI. Decide during implementation based on what Bevy UI supports cleanly.
- **Gamepad support**: number keys only for now. Gamepad face button mapping is deferred to the gamepad input icebox item.

---

## Acceptance criteria

- Given a scene with an `ActionBar` node containing a slot `key: "1"`, pressing `1` fires the slot's `do_actions` through the pipeline.
- Given a slot with `cooldown_secs: 2.0`, pressing the key again within 2 seconds emits `action_bar.on_cooldown:1` and does not fire `do_actions`.
- Given a slot with `cost: (stat: "mana", amount: 10.0)` and the entity's mana below 10, pressing the key emits `action_bar.insufficient_resource:1` and does not fire or start the cooldown.
- Given a slot using `{target}` in `do_actions` and `CurrentTarget` is `None`, pressing the key emits `action_bar.no_target:1` and does not fire.
- Given a slot on cooldown, its icon renders greyed-out with a cooldown sweep overlay that clears when the cooldown expires.
- Given a `label: "Fireball"` on a slot, hovering the slot shows the label as a tooltip.
