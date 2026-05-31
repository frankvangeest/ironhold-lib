# Feature: AoE Ground Targeting

_Status: Draft_
_Planned at: `2f19309` (2026-05-31)_
_Hard dep: Skill action bar (`planning/features/skill_action_bar.md`)_

---

> ## Pre-implementation checklist
>
> - [ ] **Skill action bar must be implemented first.** This feature extends `ActionSlotDef` with an optional `targeting_mode` field. It cannot be built or tested without the action bar.
>
> - [ ] **Decide: how does `{aoe_position}` reach positional action fields?**
>   `SpawnEffect` and `Spawn` already accept `position: Option<(f32, f32, f32)>` — a typed tuple, not a string. Simple string substitution (`{aoe_position}` → `"12.5,0.0,8.3"`) works for event name fields (`EmitEvent("cast:{aoe_position}")`) but cannot inject into typed tuple fields.
>
>   Recommended approach: add `position: AoEPosition` as an enum on `SpawnEffect` and `Spawn`:
>   ```rust
>   pub enum AoEPosition {
>       Literal(f32, f32, f32),   // existing inline position
>       FromAoE,                  // sentinel: resolved from AoETargetingState at execution time
>   }
>   ```
>   In RON: `position: FromAoE`. The executor checks for `FromAoE` and reads `AoETargetingState.confirmed_position`. This is type-safe, requires no string parsing, and keeps the action schema clean.
>
>   Alternative (simpler, less ergonomic): store the confirmed position in `GameVariables["_aoe_x/y/z"]` and leave positional spawning out of v1 scope — designers use `EmitEvent("aoe.confirmed:{aoe_position}")` and handle it in rules.
>
> - [ ] **Decide: ground plane detection method.** Two options:
>   - **y=0 plane intersection** (v1 default): intersect camera ray with the XZ plane at a configurable `ground_y` offset; simple, no physics dependency, works for flat terrain.
>   - **Physics raycast** (accurate): cast a ray into the Rapier collider world; works for sloped terrain and hills. More complex, requires access to the physics world in the input system.
>   Recommendation: y=0 plane for v1 with a `ground_y: f32` field on `GroundAoE`; note that sloped terrain will show the indicator floating or clipping. Physics raycast can be a follow-up.
>
> - [ ] **Confirm the placement indicator approach.** Two options:
>   - **Reuse `ProjectDecal`**: spawn a decal entity on targeting mode entry, move it each frame, despawn on exit. Clean — reuses existing infrastructure. Requires `ProjectDecal` to support a live-moving entity.
>   - **Dedicated indicator entity**: a simple flat quad mesh with a ring texture, moved each frame. Self-contained and doesn't depend on the decal system internals.
>   Recommendation: dedicated indicator entity — simpler to move per-frame without coupling to decal lifecycle.

---

## What

An optional targeting mode for skill action bar slots. When a slot has `targeting_mode: GroundAoE(...)`, pressing the slot key enters a placement phase instead of immediately firing actions. A circle indicator follows the cursor on the ground plane. Left-click (or Enter) confirms the position and fires `do_actions` with the AoE position injected. Right-click (or Escape) cancels silently.

This allows designers to create abilities like: AoE heals, ground-placed traps, explosion impacts, and cast indicators — entirely from RON.

---

## Why

Without targeted positioning, skills can only affect `{self}` or `{target}` (an entity). Many RPG ability patterns require a chosen ground location: a fireball aimed at a spot, a healing circle placed under an ally, a trap dropped in a corridor. This feature unblocks all of them.

---

## Schema changes

### `ActionSlotDef` — add `targeting_mode` field (`schema/ui.rs`)

```rust
pub struct ActionSlotDef {
    pub key: String,
    pub icon: String,
    pub do_actions: Vec<Action>,
    pub cooldown_secs: Option<f32>,
    pub cost: Option<SlotCost>,
    pub label: Option<String>,
    // New:
    #[serde(default)]
    pub targeting_mode: Option<TargetingMode>,
}

pub enum TargetingMode {
    GroundAoE {
        radius: f32,
        indicator_texture: String,  // asset catalog texture key for the circle indicator
        ground_y: f32,              // world-space Y of the ground plane; default 0.0
    },
}
```

RON example:

```ron
slots: [
    (
        key: "3",
        icon: "icons/fireball",
        targeting_mode: GroundAoE(
            radius: 4.0,
            indicator_texture: "decals/aoe_ring",
            ground_y: 0.0,
        ),
        do_actions: [
            SpawnEffect(key: "effects/fireball_explosion", position: FromAoE),
            EmitEvent("ability.fireball_cast:{aoe_position}"),
        ],
        cooldown_secs: 3.0,
        cost: (stat: "mana", amount: 25.0),
    ),
]
```

### `AoEPosition` sentinel in `SpawnEffect` and `Spawn` (`schema/actions.rs`)

```rust
// New enum for position source
pub enum AoEPosition {
    Literal(f32, f32, f32),
    FromAoE,    // resolved from AoETargetingState.confirmed_position at execution time
}

// SpawnEffect gains an alternative position field:
SpawnEffect {
    key: String,
    position: Option<(f32, f32, f32)>,     // existing literal position
    aoe_position: bool,                     // if true, use AoETargetingState
    entity: Option<String>,
}
```

In RON: `SpawnEffect(key: "effects/explosion", aoe_position: true)`.

(Keeping the existing `position` field unchanged preserves full backwards compatibility.)

---

## New state resource

```rust
#[derive(Resource, Default)]
pub struct AoETargetingState {
    /// Which slot is currently in placement mode, if any.
    pub active_slot: Option<String>,
    /// The radius from the active slot's GroundAoE config.
    pub radius: f32,
    /// Current cursor world position projected onto the ground plane.
    pub cursor_world_pos: Vec3,
    /// Set to Some(pos) when the player confirms. Cleared after do_actions fire.
    pub confirmed_position: Option<Vec3>,
}
```

---

## New events into the pipeline

```ron
action_bar.targeting_started:{slot_key}             // entered placement mode
action_bar.targeting_confirmed:{slot_key}:{x},{y},{z}  // confirmed; fires before do_actions
action_bar.targeting_cancelled:{slot_key}           // cancelled via right-click or Escape
```

The `{aoe_position}` substitution in string-typed action fields (e.g. event names) expands to `"{x},{y},{z}"` using the confirmed position.

---

## Input flow

```
Normal state
  → slot key pressed (slot has targeting_mode: GroundAoE)
  → cooldown / cost check (same as normal slot)
  → enter Placing state: set AoETargetingState.active_slot, spawn indicator entity

Placing state (each frame)
  → mouse move: project cursor ray onto ground plane, update AoETargetingState.cursor_world_pos,
                move indicator entity transform

  → left-click or Enter:
      set AoETargetingState.confirmed_position = cursor_world_pos
      emit action_bar.targeting_confirmed:{slot}:{pos}
      fire do_actions (executor reads FromAoE from AoETargetingState.confirmed_position)
      start cooldown, deduct cost
      despawn indicator entity
      clear AoETargetingState → back to Normal

  → right-click or Escape:
      emit action_bar.targeting_cancelled:{slot}
      despawn indicator entity
      clear AoETargetingState → back to Normal (no cooldown, no cost)
```

---

## Placement indicator

A dedicated flat quad entity with the slot's `indicator_texture` scaled to `radius * 2`. Spawned on targeting mode entry, despawned on confirm/cancel. Moved to `AoETargetingState.cursor_world_pos` each frame with a fixed y-offset (+0.02) to avoid z-fighting.

The indicator scales uniformly: `scale = Vec3::splat(radius * 2.0)`. The texture should be a ring or filled circle; ring is more readable for large radii.

---

## Key Rust changes

1. **`schema/ui.rs`** — add `targeting_mode: Option<TargetingMode>` to `ActionSlotDef`; add `TargetingMode` enum.

2. **`schema/actions.rs`** — add `aoe_position: bool` field to `SpawnEffect` and `Spawn`; update executor to resolve it from `AoETargetingState`.

3. **`capabilities/action_bar.rs`** (extends existing)
   - `aoe_targeting_input_system`: handles mouse movement (ground projection) and confirm/cancel input during placement mode; updates `AoETargetingState`.
   - `aoe_indicator_system`: moves the indicator entity to `cursor_world_pos` each frame.
   - Update `action_bar_input_system`: when slot has `targeting_mode`, enter placement instead of immediately firing.

4. **`runtime/scene_manager/action_executor.rs`** — handle `aoe_position: true` in `SpawnEffect`/`Spawn`: read `AoETargetingState.confirmed_position`, substitute into position field.

5. **`runtime/scene_manager/message_interpreter.rs`** — add `{aoe_position}` substitution using `AoETargetingState.confirmed_position` (for event name fields).

---

## Tasks

- [ ] Decisions from pre-implementation checklist resolved
- [ ] `TargetingMode` enum and `targeting_mode` field on `ActionSlotDef`
- [ ] `AoETargetingState` resource
- [ ] `aoe_position: bool` on `SpawnEffect` and `Spawn`; executor reads from `AoETargetingState`
- [ ] Ground plane ray intersection (camera ray → XZ plane at `ground_y`)
- [ ] `aoe_targeting_input_system` — mouse move, confirm, cancel
- [ ] `aoe_indicator_system` — spawn/move/despawn flat quad indicator
- [ ] `{aoe_position}` string substitution in `message_interpreter.rs`
- [ ] `action_bar.targeting_*` events emitted into pipeline
- [ ] `action_bar_visual_system` — grey out slot during placement mode (already in targeting, can't re-activate)
- [ ] Demo: add a fireball AoE slot to `particles_demo` or `3rd_person_game_demo`
- [ ] Integration tests: confirm fires at correct position; cancel fires no actions; `FromAoE` resolves in SpawnEffect
- [ ] Docs: `targeting_mode` field in `docs/20_data_formats.md`; `FromAoE` position sentinel documented

---

## Open questions

- **Sloped terrain**: the y=0 plane projection will visually float the indicator above hills or clip it into slopes. Physics raycast fixes this but is deferred. Should the indicator mesh be projected onto terrain using a depth-offset trick or just float at `ground_y`?
- **Indicator texture ownership**: `indicator_texture` is an asset catalog key. Confirm the indicator entity is treated as a temporary scene entity (not a prefab) so it doesn't need a prefab catalog entry.
- **Cancelling on scene transition**: if `LoadScene` fires while in placement mode, `AoETargetingState` must be cleared and the indicator entity despawned. This should happen in the scene unload path — add a check there.
- **Multiple simultaneous bars**: if two `ActionBar` nodes exist (main + secondary), only one slot can be in placement mode at a time. `AoETargetingState.active_slot` stores the key but not which bar it belongs to. Add a `bar_id` if multiple bars ship before this feature.

---

## Acceptance criteria

- Given a slot with `targeting_mode: GroundAoE(radius: 4.0)`, pressing the slot key enters placement mode without firing `do_actions`.
- Given placement mode active, moving the mouse moves the circle indicator to the projected cursor position on the ground plane.
- Given left-clicking to confirm, `do_actions` fire with `SpawnEffect(aoe_position: true)` placing the effect at the confirmed world position.
- Given right-clicking or pressing Escape to cancel, no `do_actions` fire, no cooldown starts, no cost is deducted.
- Given `EmitEvent("cast:{aoe_position}")` in `do_actions`, the emitted event name contains the confirmed `x,y,z` position.
- Given a slot on cooldown, pressing the key emits `action_bar.on_cooldown:{slot}` — placement mode is not entered.
- Given a cost stat below the required amount, pressing the key emits `action_bar.insufficient_resource:{slot}` — placement mode is not entered.
