# Feature: Status Effect Icon Display

_Status: Draft_
_Planned at: `5f72600` (2026-05-31)_

## Phases

| Phase | Backlog item | Status | Completed |
|---|---|---|---|
| v1 | HUD `StatusEffectBar` UI node — player buffs/debuffs only | Queued | — |
| v2 | World-space icon strip above entities | Queued | — |

---

> ## Pre-implementation checklist
>
> - [ ] **Add `icon` and `label` fields to `ModifierDef` in `schema/stats.rs`.** Both optional — modifiers without an icon are invisible to the strip (useful for internal/invisible effects). This is a non-breaking schema change (both fields default to `None`).
>
> - [ ] **Decide: one strip mode or two separate features?** Two modes are described below — HUD panel (player-focused) and world-space above entity (all tagged entities). Both share the same icon-resolution logic. Recommendation: implement both in one feature since they share most of the system, but keep the rendering paths separate.
>
> - [ ] **Decide: duration display style.** Options: (a) sweep overlay (clock-wipe over the icon, like WoW); (b) countdown number below the icon; (c) icon fades as duration drains; (d) no duration display — just the icon. A sweep overlay is most informative; a countdown number is simpler to implement in Bevy UI. Decide before wiring the visual system.
>
> - [ ] **Decide: stack count badge.** If a modifier has `stack_rule: Add` and multiple instances are active, show a count badge (e.g. "x3") in the icon corner. Recommended yes — without it stacking modifiers are invisible to the player. Stack count is simply `active_modifiers.iter().filter(|m| m.key == key).count()`.

---

## What

An icon strip that shows the active buffs and debuffs on an entity. Two display positions:

1. **HUD panel** — a `StatusEffectBar` UI node in scene RON, bound to the player entity; sits in a fixed screen position (e.g. below the health bar).
2. **World-space above entity** — a `world_status_effects: true` field on `PrefabDef`; renders a small icon row above each entity that has active modifiers (similar to how nameplates sit above NPCs).

Each icon represents one modifier key. Stack count badge for stackable modifiers. Optional duration sweep overlay. Modifiers without an `icon` field are not shown.

---

## Why

The buffs/modifiers system shipped in Beta stats Phase 2, but there is no visual feedback when a modifier is active. A designer applying `ApplyModifier("burning")` to an enemy has no way to confirm it worked without logging. This feature closes the feedback loop and enables gameplay that depends on players reading buff/debuff state.

---

## Schema changes

### `ModifierDef` — add `icon` and `label` (`schema/stats.rs`)

```rust
pub struct ModifierDef {
    pub stat: String,
    pub kind: ModifierKind,
    #[serde(default)]
    pub duration_secs: Option<f32>,
    #[serde(default = "default_stack_rule")]
    pub stack_rule: StackRule,
    // New fields:
    #[serde(default)]
    pub icon: Option<String>,    // asset catalog texture key; None = invisible in strips
    #[serde(default)]
    pub label: Option<String>,   // tooltip/accessibility label; defaults to modifier key
}
```

RON example in `stats.ron`:

```ron
modifiers: {
    "burning": (
        stat: "health",
        kind: Additive(-5.0),
        duration_secs: 3.0,
        stack_rule: Add,
        icon: "icons/status/burning",
        label: "Burning",
    ),
    "speed_boost": (
        stat: "speed",
        kind: Multiplicative(1.5),
        duration_secs: 10.0,
        icon: "icons/status/speed_boost",
        label: "Speed Boost",
    ),
    "internal_regen": (   // no icon — never shown in strip
        stat: "health",
        kind: Additive(1.0),
    ),
}
```

### New scene UI node: `StatusEffectBar` (`schema/scene_v2.rs` or `schema/ui.rs`)

```ron
// In a scene's ui: [...] block
StatusEffectBar(
    position: (x: 10.0, y: 80.0),
    anchor: TopLeft,
    icon_size: 32.0,
    icon_gap: 4.0,
    max_icons: 8,
    show_duration: true,    // sweep overlay
    show_stack_count: true, // badge for stacking modifiers
    target: Player,         // Player | Entity("spawn_id")
)
```

### New `PrefabDef` field for world-space strips

```ron
// In prefabs.ron on any prefab
world_status_effects: (
    icon_size: 24.0,
    max_icons: 5,
    offset_y: 2.2,          // world units above entity origin
    show_duration: false,   // simpler display above NPCs
    show_stack_count: true,
),
```

---

## How the strip is built

The icon strip system queries:
1. The entity's `StatMap` — iterates all `LiveStat` entries
2. Each `LiveStat.active_modifiers` — collects `ActiveModifier` instances
3. Deduplicates by `key` — one icon slot per unique modifier key
4. Looks up `ModifierDef` in `LoadedModifiers` — skips entries with `icon: None`
5. For each visible key: icon texture, remaining duration, stack count

```
StatMap → [LiveStat(health), LiveStat(speed), ...]
           each has active_modifiers: [ActiveModifier { key, remaining_secs }]
           
dedup by key → [("burning", max_remaining, stack_count), ("speed_boost", ...)]
filter icon != None → load textures, build strip
```

Duration shown is the maximum `remaining_secs` across all instances of that key (most intuitive for stacked modifiers).

---

## Key Rust changes

1. **`schema/stats.rs`** — add `icon: Option<String>` and `label: Option<String>` to `ModifierDef`.

2. **`schema/ui.rs`** (or `scene_v2.rs`) — add `StatusEffectBar(StatusEffectBarDef)` to the UI node enum; add `StatusEffectBarDef` struct.

3. **`schema/catalog.rs`** (or `prefabs.rs`) — add `world_status_effects: Option<WorldStatusEffectsDef>` to `PrefabDef`.

4. **`capabilities/status_effect_icons.rs`** (new file)
   - `status_effect_hud_system`: queries `StatusEffectBar` UI nodes, resolves the target entity's `StatMap`, builds/updates icon strip UI children using change detection on `active_modifiers`.
   - `status_effect_world_system`: queries entities with `WorldStatusEffectsDef` component, builds/updates a world-space `Node` strip above each entity.
   - Shared helper `collect_visible_modifiers(stat_map, loaded_modifiers) -> Vec<VisibleModifier>` used by both systems.

5. **`runtime/scene_manager/scene_loader.rs`** — handle `StatusEffectBar` UI node spawn.

6. **`runtime/scene_manager/entity_spawner.rs`** — attach `WorldStatusEffectsDef` component when `world_status_effects` is set on a prefab.

---

## Tasks

- [ ] Decisions from pre-implementation checklist resolved
- [ ] Add `icon` and `label` to `ModifierDef`; update `stats.ron` in example projects
- [ ] `StatusEffectBarDef` struct and `StatusEffectBar` UI node variant
- [ ] `WorldStatusEffectsDef` struct and `world_status_effects` field on `PrefabDef`
- [ ] `collect_visible_modifiers` helper — dedup, filter, resolve icon handle
- [ ] `status_effect_hud_system` — HUD strip, change-detection update
- [ ] `status_effect_world_system` — world-space strip above entities
- [ ] Duration sweep overlay (if `show_duration: true`)
- [ ] Stack count badge (if `show_stack_count: true`)
- [ ] Tooltip on hover showing `label` (can be deferred if tooltip system doesn't exist yet)
- [ ] Wire `StatusEffectBar` in `scene_loader.rs`
- [ ] Wire `WorldStatusEffectsDef` in `entity_spawner.rs`
- [ ] Demo: add a `StatusEffectBar` to `primitive_world` showing burning/speed_boost on the attack dummies
- [ ] Integration tests: modifier applied → icon appears; modifier expired → icon removed; stack count correct
- [ ] Docs: add `StatusEffectBar` to `docs/20_data_formats.md` UI node reference; add `icon`/`label` to modifier template docs

---

## Open questions

- **Tooltip system**: showing `label` on hover requires a tooltip UI primitive that doesn't exist yet. Defer tooltip to a future UI feature; `label` field is still worth adding now for when tooltips land.
- **Permanent modifiers**: a modifier with `duration_secs: None` has no duration to display. When `show_duration: true`, permanent modifiers show the icon without a sweep overlay (full, no drain).
- **Icon ordering**: buffs before debuffs, or chronological (most recently applied first)? Chronological is simpler; buff/debuff separation requires a `kind: Buff | Debuff` tag on `ModifierDef`. Leave ordering as chronological for v1.
- **World-space strip and camera**: the world-space strip uses `Node` (Bevy UI) rather than a `Mesh` billboard so it always faces the camera. Confirm this is the correct approach — Bevy UI in world-space can have z-ordering issues. Alternative: a `Text2d` + sprite billboard approach like existing world-space stat bars.

---

## Acceptance criteria

- Given a `ModifierDef` with `icon: "icons/status/burning"` and an entity with that modifier applied, the icon appears in the `StatusEffectBar` HUD strip.
- Given the modifier expiring (`remaining_secs` reaches 0), the icon is removed from the strip within one frame.
- Given a modifier with `stack_rule: Add` applied three times, the strip shows one icon with a "x3" badge.
- Given a modifier with `icon: None`, it does not appear in any strip regardless of whether it is active.
- Given `show_duration: true` and a modifier with `duration_secs: 3.0`, the sweep overlay drains from full to empty over 3 seconds.
- Given `world_status_effects` on a prefab and an active modifier with an icon, the icon strip appears above the entity in world space.
- Given a modifier with `duration_secs: None` (permanent) and `show_duration: true`, the icon renders without a sweep overlay.
