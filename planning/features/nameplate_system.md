# Feature: Nameplate System

_Status: Draft_
_Planned at: `4c47cc6` (2026-06-02)_

---

> ## Pre-implementation checklist
>
> - [ ] **Decide: nameplate widget composition.** The nameplate combines two `Text2d` entities (name line + health bar line) parented under an anchor entity attached to the world entity. Alternatively, both lines can share one `Text2d` with newline separation. Recommendation: **two separate `Text2d` entities** — they have different colors, different font sizes, and different update frequencies. A shared `Text2d` with sections is also valid in Bevy 0.18 (`TextLayout`), but separate entities are simpler and match the pattern used by `world_stat_bar`.
>
> - [ ] **Decide: name source.** The nameplate needs a display string for the name line. Options: (a) a new `display_name: Option<String>` field on `PrefabDef`; (b) use the prefab key as the display name; (c) use the spawn ID. Recommendation: **`display_name: Option<String>` on `PrefabDef`**, falling back to the prefab key (not the spawn ID — spawn IDs contain counters like `orc_01`). This means adding one optional field to `PrefabDef`.
>
> - [ ] **Decide: health bar in nameplate — optional or always present.** The backlog says "name + health bar". But some entities (props, collectibles) have no `health` stat. Recommendation: bar is **conditional on `bar_stat_key` being set** in `NameplateOptionsDef`. When `bar_stat_key` is `None`, only the name line renders. Default for NPCs: `bar_stat_key: Some("{self}.health")`.
>
> - [ ] **Decide: distance reference point — camera or player.** Distance culling can measure from the active camera (what the player sees) or from the player entity (character position). Recommendation: **camera position** — it matches what the player would expect visually; a distant target could still be on-screen if the camera is orbiting.
>
> - [ ] **Decide: faction filter v1 approximation.** Without the Group system, faction cannot be determined precisely. Use the same approximation as the targeting system: `HostileOnly` = `has NpcAgent`. Document this as a v1 stub to be replaced when Group system ships.
>
> - [ ] **Confirm `world_stat_bar` and `nameplate` can coexist.** An entity could have both `world_stat_bar` (always visible, authored per-prefab) and `nameplate: true` (scene-managed, distance-culled). Both produce separate `Text2d` children. The nameplate offset should be 0.2–0.4 units above `world_stat_bar.offset` to avoid overlap. Verify at demo time with an entity that has both.

---

## What

A scene-managed system that renders a combined **name + health bar widget** above 3D entities, with automatic show/hide based on camera distance and faction stance.

Unlike `stat_label` and `world_stat_bar` (which are always-visible, per-prefab widgets), the nameplate system manages visibility globally — a single system scans all tagged entities each frame and toggles widget visibility based on scene-wide rules.

**Scene-wide opt-in** via `show_nameplates: true` in scene RON. Per-prefab override with `nameplate: true/false` on `PrefabDef`. Designer controls distance cutoff, faction filter, colors, and whether the bar appears.

---

## Why

In combat scenes with multiple enemies, always-visible `world_stat_bar` widgets become visual noise at a distance. A nameplate system solves this by:

1. Showing nameplates only for entities the player needs awareness of (hostile, within range).
2. Auto-hiding at distance so the screen isn't cluttered when many enemies exist.
3. Providing entity names as context, distinct from raw stat values.

Unblocks: targeted-entity nameplate highlighting (combine with targeting system — the selected entity's nameplate stays visible regardless of distance), dialogue name display, quest objective markers.

---

## Schema changes

### `GameSceneV2` — two new fields

```ron
// scenes/dungeon.scene.ron
(
    show_nameplates: true,           // NEW — enable the nameplate system for this scene
    nameplate_options: (             // NEW — optional config block
        faction_filter: HostileOnly,
        max_distance: 20.0,
        bar_stat_key: Some("{self}.health"),
    ),
    // ... existing fields
)
```

```rust
// schema/scene_v2.rs
#[serde(default)]
pub show_nameplates: bool,

#[serde(default)]
pub nameplate_options: Option<NameplateOptionsDef>,
```

### New `NameplateOptionsDef` struct (`schema/scene_v2.rs`)

```rust
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct NameplateOptionsDef {
    /// Which entities receive a nameplate. Default: HostileOnly.
    #[serde(default = "default_nameplate_faction")]
    pub faction_filter: NameplateFactionFilter,

    /// Maximum camera distance (world units) for visible nameplates. Default: 20.0.
    #[serde(default = "default_nameplate_max_distance")]
    pub max_distance: f32,

    /// World-space offset from entity origin to the nameplate anchor. Default: (0.0, 2.4, 0.0).
    #[serde(default = "default_nameplate_offset")]
    pub offset: (f32, f32, f32),

    /// Font size of the name line in screen pixels. Default: 14.
    #[serde(default = "default_nameplate_font_size")]
    pub name_font_size: f32,

    /// Color of the name text as linear RGBA. Default: white.
    #[serde(default = "default_nameplate_name_color")]
    pub name_color: (f32, f32, f32, f32),

    /// Stat key for the health bar below the name. `{self}` is substituted at spawn.
    /// When `None`, no bar is rendered — name line only. Default: None.
    #[serde(default)]
    pub bar_stat_key: Option<String>,

    /// Width of the health bar in characters (ASCII block mode). Default: 10.
    #[serde(default = "default_nameplate_bar_width")]
    pub bar_width: u8,

    /// Health bar fill color as linear RGBA. Default: green.
    #[serde(default = "default_nameplate_bar_fill")]
    pub bar_fill_color: (f32, f32, f32, f32),

    /// Health bar background color as linear RGBA. Default: dark grey.
    #[serde(default = "default_nameplate_bar_bg")]
    pub bar_bg_color: (f32, f32, f32, f32),
}

#[derive(Deserialize, Debug, Clone, Default)]
pub enum NameplateFactionFilter {
    #[default]
    HostileOnly,
    FriendlyOnly,
    All,
}
```

### `PrefabDef` — two new fields (`schema/catalog.rs`)

```ron
// prefabs/prefabs.ron
"orc_enemy": (
    kind: "actor",
    model: "creatures/orc",
    display_name: Some("Orc Warrior"),   // NEW — name line text
    nameplate: Some(true),               // NEW — override: always show regardless of scene filter
    stat_templates: [...],
    // ...
)

"chest_prop": (
    kind: "prop",
    model: "props/chest",
    nameplate: Some(false),              // NEW — override: never show even if show_nameplates: true
    // ...
)
```

```rust
// schema/catalog.rs — in PrefabDef
/// Display name shown in the nameplate widget.
/// Falls back to the prefab key (e.g. "orc_enemy") when None.
#[serde(default)]
pub display_name: Option<String>,

/// Nameplate visibility override.
/// None = inherit scene `show_nameplates` + `nameplate_options.faction_filter`.
/// Some(true) = always show a nameplate for this prefab (bypasses faction filter; respects max_distance).
/// Some(false) = never show a nameplate for this prefab even if show_nameplates: true.
#[serde(default)]
pub nameplate: Option<bool>,
```

---

## Runtime

### Components (`capabilities/nameplate.rs`)

```rust
/// Marker on any entity that may have a nameplate. Set at spawn time.
#[derive(Component)]
pub struct NameplateTag {
    pub display_name: String,            // resolved from PrefabDef.display_name or prefab key
    pub prefab_override: Option<bool>,   // from PrefabDef.nameplate
}

/// Entity ID of the nameplate anchor spawned as a child.
/// Stored on the world entity so `nameplate_update_system` can find the bar text quickly.
#[derive(Component)]
pub struct NameplateAnchor(pub Entity);

/// Marker on the health bar `Text2d` child of a nameplate anchor.
/// Carries the resolved stat key for update queries.
#[derive(Component)]
pub struct NameplateBar {
    pub stat_key: String,    // "{self}.health" resolved to e.g. "orc_01.health"
    pub bar_width: u8,
    pub fill_color: Color,
    pub bg_color: Color,
}
```

### Systems (all in `capabilities/nameplate.rs`)

**`nameplate_setup_system`** — runs once on `SceneEvent::Ready` (after entities are spawned). For each entity with `NameplateTag` that has no `NameplateAnchor` yet:
1. Spawns an anchor entity (child of the world entity) at the configured offset.
2. Spawns a name `Text2d` child of the anchor.
3. If `bar_stat_key` is set, spawns a bar `Text2d` child of the anchor (0.2 units below the name).
4. Attaches `NameplateAnchor(anchor_entity)` to the world entity.
5. Sets initial `Visibility::Hidden`.

**`nameplate_visibility_system`** — runs in `Update`. Reads `NameplateSceneConfig` resource (populated from scene RON on load):
- If `show_nameplates: false` and no per-prefab `Some(true)` override: skip all.
- For each entity with `NameplateTag` + `NameplateAnchor` + `GlobalTransform`:
  1. Apply `prefab_override`: if `Some(false)` → hide; if `Some(true)` → skip distance/faction checks, show.
  2. Faction filter: if `HostileOnly` and entity has no `NpcAgent` → hide.
  3. Distance: if camera distance > `max_distance` → hide.
  4. Otherwise → show.
- Uses change-detection guard: only writes `Visibility` when the value actually changes (avoids re-triggering render graph each frame).

**`nameplate_update_system`** — runs in `Update`, only when `NameplateBar` entities exist. Reads `StatMap` from the world entity (looked up via `NameplateBar.stat_key`), computes fill fraction, updates the bar `Text2d` string using the same Unicode block algorithm as `world_stat_bar`. Guards on actual value change.

### `NameplateSceneConfig` resource (`capabilities/nameplate.rs`)

Populated from `GameSceneV2` on scene load, stored as a `Resource`. Cleared and repopulated on each `LoadScene`.

```rust
#[derive(Resource, Default)]
pub struct NameplateSceneConfig {
    pub enabled: bool,
    pub options: Option<NameplateOptionsDef>,
}
```

---

## Spawn-time wiring

In `scene_loader.rs` (or `entity_spawner.rs`), after spawning a prefab:

```rust
// If show_nameplates AND (nameplate != Some(false)):
if scene.show_nameplates || prefab.nameplate == Some(true) {
    if prefab.nameplate != Some(false) {
        let display_name = prefab.display_name.clone()
            .unwrap_or_else(|| prefab_key.to_string());
        commands.entity(entity).insert(NameplateTag {
            display_name,
            prefab_override: prefab.nameplate,
        });
    }
}
```

The anchor + `Text2d` children are NOT spawned at entity spawn time — they're spawned by `nameplate_setup_system` after `SceneEvent::Ready`. This keeps the spawn path simple and avoids Text2d layout issues before the scene is fully loaded.

---

## Relationship to existing world-space widgets

| Widget | Authored on | Always visible | Distance culled | Faction filtered | Name line | Bar line |
|---|---|---|---|---|---|---|
| `stat_label` | `PrefabDef` | Yes | No | No | No | No (text value only) |
| `world_stat_bar` | `PrefabDef` | Yes | No | No | No | Yes |
| `nameplate` | Scene + PrefabDef | Scene-managed | Yes | Yes | Yes | Optional |

Entities can have both `world_stat_bar` and `nameplate: true` simultaneously — the nameplate bar appears above the `world_stat_bar`. If overlap is visually undesirable, set `world_stat_bar: None` on the prefab and rely on the nameplate bar alone.

---

## New Rust changes

- `schema/scene_v2.rs` — add `show_nameplates: bool`, `nameplate_options: Option<NameplateOptionsDef>`, `NameplateOptionsDef`, `NameplateFactionFilter`.
- `schema/catalog.rs` — add `display_name: Option<String>`, `nameplate: Option<bool>` to `PrefabDef`.
- `capabilities/nameplate.rs` (new file) — `NameplateTag`, `NameplateAnchor`, `NameplateBar`, `NameplateSceneConfig`, `nameplate_setup_system`, `nameplate_visibility_system`, `nameplate_update_system`.
- `capabilities/mod.rs` — register `nameplate` module and its systems.
- `runtime/scene_manager/scene_loader.rs` — populate `NameplateSceneConfig` on scene load; insert `NameplateTag` at spawn time.
- `lib.rs` — add `NameplatePlugin` (or inline system registration alongside other capability systems).

---

## Tasks

- [ ] Decisions from pre-implementation checklist resolved
- [ ] `show_nameplates: bool` + `nameplate_options: Option<NameplateOptionsDef>` on `GameSceneV2`
- [ ] `NameplateOptionsDef` + `NameplateFactionFilter` in `schema/scene_v2.rs`
- [ ] `display_name: Option<String>` + `nameplate: Option<bool>` on `PrefabDef` in `schema/catalog.rs`
- [ ] `NameplateSceneConfig` resource populated on scene load and cleared on `LoadScene`
- [ ] Spawn-time: insert `NameplateTag` when conditions are met
- [ ] `nameplate_setup_system` — spawn anchor + Text2d children after `SceneEvent::Ready`
- [ ] `nameplate_visibility_system` — distance + faction culling with change-detection guard
- [ ] `nameplate_update_system` — health bar text update reusing `world_stat_bar` fill algorithm
- [ ] Demo: add `show_nameplates: true` + `display_name` + `nameplate: true` to enemies in `primitive_world` or `3rd_person_game_demo`
- [ ] Integration tests: nameplate visible within range, hidden beyond `max_distance`, hidden when faction filter excludes the entity, per-prefab `Some(false)` suppresses even when scene enabled
- [ ] Docs: `show_nameplates`, `nameplate_options`, `display_name`, `nameplate` fields in `docs/20_data_formats.md`

---

## Open questions

- **Nameplate stacking at close range**: when multiple enemies cluster, nameplates overlap. v1 does no stacking/sorting — accept the overlap. A future pass could sort by depth and offset overlapping plates.
- **Faction filter granularity**: `HostileOnly` uses `has NpcAgent` as v1 approximation. When Group system ships, replace with faction stance query.
- **Nameplate for player**: by convention, `nameplate` is for non-player entities. If a scene sets `show_nameplates: All`, the player entity should still be excluded. Guard: skip entities with `PlayerController` in `nameplate_visibility_system`.
- **`display_name` localisation**: for now, `display_name` is a plain string. A future localisation pass could make this a key into a string table.

---

## Acceptance criteria

- Given `show_nameplates: true` in a scene and an enemy NPC with no `nameplate` override, a floating name widget appears above the entity when within `max_distance`.
- Given the camera moves beyond `max_distance`, the nameplate widget becomes invisible.
- Given `faction_filter: HostileOnly`, a prefab with no `NpcAgent` component does not show a nameplate even if within range.
- Given `nameplate: Some(false)` on a prefab in a scene with `show_nameplates: true`, no nameplate appears for that prefab.
- Given `nameplate: Some(true)` on a prefab in a scene with `show_nameplates: false`, the nameplate still appears (respects max_distance, ignores faction filter).
- Given `display_name: Some("Orc Warrior")`, the name line reads "Orc Warrior". Given no `display_name`, the name line reads the prefab key.
- Given `bar_stat_key: Some("{self}.health")`, the bar below the name reflects current/max health and updates when health changes.
- Given `bar_stat_key: None` in `nameplate_options`, only the name line appears.
- Given `show_nameplates: false` and no per-prefab overrides, no nameplate widgets are spawned or updated.
- Given a scene transition (`LoadScene`), all nameplate widgets are despawned and `NameplateSceneConfig` is reset.
