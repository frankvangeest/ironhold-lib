# Feature: Nameplate System

_Status: Draft_
_Planned at: `32df2ec` (2026-06-02)_

---

> ## Pre-implementation checklist
>
> - [x] **Decide: nameplate widget composition.** Two `Text2d` entities for the name line; pixel bars are `Mesh2d` + `ColorMaterial2d` quads (background + foreground) per stat bar. All children parented under a billboard anchor entity.
>
> - [x] **Decide: name source.** `display_name: Option<String>` on `PrefabDef`, falling back to the prefab key (not spawn ID — spawn IDs contain counters like `orc_01`).
>
> - [x] **Decide: health bar type.** Pixel bars using `Mesh2d` + `ColorMaterial2d` (two quads per bar: background full-width, foreground scaled by `current/max` fraction). **Not** ASCII Unicode blocks — those are for `world_stat_bar`. Bars render only if the entity has the named stat in its `StatMap`; missing stats skip the bar silently.
>
> - [x] **Decide: multiple stat bars.** `stat_bars: Vec<StatBarDef>` in `NameplateOptionsDef`. Each entry has `stat_key`, `fill_color`, `bg_color`. Bars stack vertically below the name with `bar_spacing` gap. For `3rd_person_game_demo`: health (green) + mana (blue) — mana bar only appears for entities that have the mana stat.
>
> - [x] **Decide: player included.** Player entity gets a nameplate when `show_nameplates: true`. No player exclusion guard. The faction filter (`HostileOnly`, `All`, etc.) controls visibility, but player can be explicitly shown via `nameplate: Some(true)` on the player prefab.
>
> - [x] **Decide: drop shadow.** Name `Text2d` gets a `TextShadow` (Bevy 0.18) — same pattern used in floating combat text. Configurable via `text_shadow: bool` in `NameplateOptionsDef` (default `true`).
>
> - [x] **Decide: distance reference point — camera.** Camera position — matches what the player sees visually.
>
> - [x] **Decide: faction filter v1 approximation.** `HostileOnly` = `has NpcAgent`. Document as v1 stub; replaced when Group system ships.
>
> - [x] **Confirm `world_stat_bar` and `nameplate` can coexist.** Nameplate offset is 0.2–0.4 units above `world_stat_bar.offset` to avoid overlap. If both are present, set `world_stat_bar: None` on the prefab and use the nameplate bars alone.

---

## What

A scene-managed system that renders a combined **name + pixel health/mana bar widget** above 3D entities, with automatic show/hide based on camera distance and faction stance.

Unlike `stat_label` and `world_stat_bar` (always-visible, per-prefab), the nameplate system manages visibility globally — a single system scans all tagged entities each frame and toggles widget visibility based on scene-wide rules.

**Scene-wide opt-in** via `show_nameplates: true` in scene RON. Per-prefab override with `nameplate: true/false` on `PrefabDef`. Designer controls distance cutoff, faction filter, colors, drop shadow, and which stat bars appear.

**Pixel bars** — colored mesh quads, not ASCII characters. Each stat bar entry is two quads: a background (full width) and a foreground (width × fraction). Missing stats render no bar.

---

## Why

In combat scenes with multiple enemies, always-visible `world_stat_bar` widgets become visual noise at a distance. A nameplate system solves this by:

1. Showing nameplates only for entities the player needs awareness of (hostile, within range).
2. Auto-hiding at distance so the screen isn't cluttered when many enemies exist.
3. Providing entity names as context, distinct from raw stat values.
4. Stacking health + mana bars so at-a-glance combat state is clear.

Unblocks: targeted-entity nameplate highlighting, dialogue name display, quest objective markers.

---

## Schema changes

### `GameSceneV2` — two new fields

```ron
// scenes/dungeon.scene.ron
(
    show_nameplates: true,           // NEW — enable the nameplate system for this scene
    nameplate_options: Some((        // NEW — optional config block
        faction_filter: All,
        max_distance: 25.0,
        text_shadow: true,
        stat_bars: [
            ( stat_key: "{self}.health", fill_color: (0.20, 0.85, 0.20, 1.0), bg_color: (0.15, 0.15, 0.15, 0.80) ),
            ( stat_key: "{self}.mana",   fill_color: (0.20, 0.45, 0.90, 1.0), bg_color: (0.15, 0.15, 0.15, 0.80) ),
        ],
    )),
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

### New `NameplateOptionsDef` and `StatBarDef` structs (`schema/scene_v2.rs`)

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

    /// Color of the name text as sRGB RGBA. Default: white.
    #[serde(default = "default_nameplate_name_color")]
    pub name_color: (f32, f32, f32, f32),

    /// Drop shadow on the name text. Default: true.
    #[serde(default = "default_true")]
    pub text_shadow: bool,

    /// Pixel stat bars rendered below the name, in declaration order.
    /// Each bar only appears if the entity has the named stat in its StatMap.
    #[serde(default)]
    pub stat_bars: Vec<StatBarDef>,

    /// Width of each stat bar in screen pixels. Default: 100.0.
    /// Same convention as `WorldStatBarStyle::Pixel { size: (w, h) }`.
    #[serde(default = "default_nameplate_bar_width")]
    pub bar_width: f32,

    /// Height of each stat bar in screen pixels. Default: 6.0.
    #[serde(default = "default_nameplate_bar_height")]
    pub bar_height: f32,

    /// Vertical gap between bars in screen pixels. Default: 9.0.
    #[serde(default = "default_nameplate_bar_spacing")]
    pub bar_spacing: f32,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct StatBarDef {
    /// Stat key — `{self}` is substituted with the entity's spawn ID at spawn time.
    pub stat_key: String,
    /// Fill color as sRGB RGBA.
    pub fill_color: (f32, f32, f32, f32),
    /// Background color as sRGB RGBA.
    pub bg_color: (f32, f32, f32, f32),
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

"player": (
    kind: "actor",
    model: "characters/player",
    display_name: Some("Player"),        // NEW
    nameplate: Some(true),               // NEW — show player nameplate when scene has show_nameplates
    // ...
)

"chest_prop": (
    kind: "prop",
    model: "props/chest",
    nameplate: Some(false),              // NEW — override: never show
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
/// Some(true) = always show (bypasses faction filter; respects max_distance).
/// Some(false) = never show even if show_nameplates: true.
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
#[derive(Component)]
pub struct NameplateAnchor(pub Entity);
```

Pixel bar fill entities use the **existing `WorldPixelBarFillMarker`** from `capabilities/stat_display.rs` — no new bar component or update system needed. The existing `world_pixel_bar_update_system` drives all bar fills (nameplate bars and `world_stat_bar` Pixel bars alike).

### Pixel bar layout

Each stat bar is two `Mesh2d` + `MeshMaterial2d<ColorMaterial>` entities, children of the nameplate anchor — the same structure `scene_loader.rs` already spawns for `WorldStatBarStyle::Pixel`:

```
WorldLabel anchor  (screen-space projected from world offset)
  ├── Text2d           (name line, with optional TextShadow)
  ├── bar_0_bg         (Mesh2d rectangle, full bar_width × bar_height px, bg_color)
  ├── bar_0_fg         (Mesh2d 1.0×bar_height, fill_color — WorldPixelBarFillMarker drives scale.x)
  ├── bar_1_bg         (optional second bar — mana)
  └── bar_1_fg         (WorldPixelBarFillMarker)
```

Bar sizes are **screen pixels** (same convention as `WorldStatBarStyle::Pixel { size: (w, h) }`). A `bar_width: 100.0, bar_height: 6.0` matches the visual scale of existing world stat bars.

### Systems (all in `capabilities/nameplate.rs`)

**`nameplate_setup_system`** — runs every `Update`, queries `Added<NameplateTag>` (entities with `NameplateTag` but no `NameplateAnchor` yet). This fires on any newly-tagged entity — scene-loaded entities, wave-spawned enemies, and dynamically `Action::Spawn`-ed actors alike:
1. Spawns a `WorldLabel` anchor entity at the configured offset.
2. Spawns a name `Text2d` child of the anchor; applies `TextShadow` if `text_shadow: true`.
3. For each `StatBarDef`, checks the entity's `StatMap` for the resolved stat key.
   - Present: spawns background quad + foreground quad with `WorldPixelBarFillMarker`.
   - Absent: skips that bar silently.
4. Attaches `NameplateAnchor(anchor_entity)` to the world entity.
5. Sets initial `Visibility::Hidden`.

**`nameplate_visibility_system`** — runs in `Update`. Reads `NameplateSceneConfig` resource:
- If `show_nameplates: false` and no per-prefab `Some(true)` override: skip all.
- For each entity with `NameplateTag` + `NameplateAnchor` + `GlobalTransform`:
  1. Apply `prefab_override`: `Some(false)` → hide; `Some(true)` → show (skip distance/faction checks).
  2. Faction filter: `HostileOnly` and entity has no `NpcAgent` → hide (player shown when `All` or `Some(true)`).
  3. Distance: camera distance > `max_distance` → hide.
  4. Otherwise → show.
- Change-detection guard: only writes `Visibility` when value changes.

Bar fill updates are handled entirely by the existing **`world_pixel_bar_update_system`** — no separate nameplate update system.

### `NameplateSceneConfig` resource

Populated from `GameSceneV2` on scene load, cleared and repopulated on each `LoadScene`.

```rust
#[derive(Resource, Default)]
pub struct NameplateSceneConfig {
    pub enabled: bool,
    pub options: Option<NameplateOptionsDef>,
}
```

---

## Spawn-time wiring

In `scene_loader.rs`, after spawning a prefab:

```rust
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

Anchor + quad children are spawned by `nameplate_setup_system` after `SceneEvent::Ready` — keeps the spawn path simple, avoids layout issues before the scene is fully loaded.

---

## Relationship to existing world-space widgets

| Widget | Authored on | Always visible | Distance culled | Faction filtered | Name line | Bar type |
|---|---|---|---|---|---|---|
| `stat_label` | `PrefabDef` | Yes | No | No | No | Text value only |
| `world_stat_bar` | `PrefabDef` | Yes | No | No | No | ASCII Unicode blocks |
| `nameplate` | Scene + PrefabDef | Scene-managed | Yes | Yes | Yes | Pixel quads |

Entities can have both `world_stat_bar` and a nameplate. If overlap is visually undesirable, set `world_stat_bar: None` on the prefab and rely on the nameplate bars.

---

## Demo configuration (`3rd_person_game_demo`)

```ron
// scenes/main.scene.ron
show_nameplates: true,
nameplate_options: Some((
    faction_filter: All,
    max_distance: 25.0,
    text_shadow: true,
    stat_bars: [
        ( stat_key: "{self}.health", fill_color: (0.20, 0.85, 0.20, 1.0), bg_color: (0.15, 0.15, 0.15, 0.80) ),
        ( stat_key: "{self}.mana",   fill_color: (0.20, 0.45, 0.90, 1.0), bg_color: (0.15, 0.15, 0.15, 0.80) ),
    ],
    bar_width: 100.0,
    bar_height: 6.0,
    bar_spacing: 9.0,
)),

// prefabs/prefabs.ron — add to enemies, NPCs, and player
"player":      ( ..., display_name: Some("Player"),       nameplate: Some(true) )
"enemy_snake": ( ..., display_name: Some("Snake"),        nameplate: Some(true) )
"enemy_spider":( ..., display_name: Some("Spider"),       nameplate: Some(true) )
"merchant":    ( ..., display_name: Some("Merchant"),     nameplate: Some(true) )
```

Snake and spider have no mana stat — their mana bar is silently skipped. Player and merchant have mana — both bars render.

---

## New Rust changes

- `schema/scene_v2.rs` — `show_nameplates: bool`, `nameplate_options: Option<NameplateOptionsDef>`, `NameplateOptionsDef`, `StatBarDef`, `NameplateFactionFilter`.
- `schema/catalog.rs` — `display_name: Option<String>`, `nameplate: Option<bool>` on `PrefabDef`.
- `capabilities/nameplate.rs` (new file) — `NameplateTag`, `NameplateAnchor`, `NameplateSceneConfig`, `nameplate_setup_system` (uses `Added<NameplateTag>`), `nameplate_visibility_system`. Reuses `WorldPixelBarFillMarker` + `world_pixel_bar_update_system` from `stat_display.rs` for bar fills — no new bar component or update system.
- `capabilities/mod.rs` — register `nameplate` module and systems.
- `runtime/scene_manager/scene_loader.rs` — populate `NameplateSceneConfig` on scene load; insert `NameplateTag` at spawn time.
- `lib.rs` — add `NameplatePlugin` (or inline registration alongside other capability systems).

---

## Tasks

- [x] Decisions from pre-implementation checklist resolved
- [ ] `show_nameplates: bool` + `nameplate_options: Option<NameplateOptionsDef>` on `GameSceneV2`
- [ ] `NameplateOptionsDef` + `StatBarDef` + `NameplateFactionFilter` in `schema/scene_v2.rs`
- [ ] `display_name: Option<String>` + `nameplate: Option<bool>` on `PrefabDef` in `schema/catalog.rs`
- [ ] `NameplateSceneConfig` resource populated on scene load and cleared on `LoadScene`
- [ ] Spawn-time: insert `NameplateTag` when conditions are met
- [ ] `nameplate_setup_system` — queries `Added<NameplateTag>` every frame; spawns `WorldLabel` anchor + `Text2d` name (with `TextShadow`) + pixel bar quads using `WorldPixelBarFillMarker`; skips bars for absent stats silently; works for scene-loaded and dynamically-spawned entities alike
- [ ] `nameplate_visibility_system` — distance + faction culling with change-detection guard; player shown when `All` or `nameplate: Some(true)`
- [ ] Bar fills driven by existing `world_pixel_bar_update_system` — no new update system needed
- [ ] Demo: `show_nameplates: true` + `nameplate_options` in `3rd_person_game_demo` main scene; `display_name` + `nameplate: Some(true)` on player, enemies, merchant
- [ ] Integration tests: nameplate visible within range; hidden beyond `max_distance`; `faction_filter: HostileOnly` hides non-NpcAgent entities; `nameplate: Some(false)` suppresses; mana bar skipped for entities without mana stat; `PlayerEquipment` not affected
- [ ] Docs: `show_nameplates`, `nameplate_options`, `stat_bars`, `display_name`, `nameplate` fields in `docs/20_data_formats.md`

---

## Open questions

- **Nameplate stacking at close range**: when multiple enemies cluster, nameplates overlap. v1 does no stacking/sorting — accept the overlap.
- **Faction filter granularity**: `HostileOnly` uses `has NpcAgent` as v1 approximation. Replace with faction stance query when Group system ships.
- **`display_name` localisation**: plain string for now; a future pass could make this a key into a string table.
- **Bar depth / render order**: pixel quads in world space may z-fight with geometry at extreme angles. Use `DepthBiasState` or set `render_layers` to keep bars always-on-top if needed.

---

## Acceptance criteria

- Given `show_nameplates: true` in a scene and an enemy NPC within `max_distance`, a floating name widget with drop shadow appears above the entity.
- Given the camera moves beyond `max_distance`, the nameplate widget becomes invisible.
- Given `faction_filter: HostileOnly`, a prefab with no `NpcAgent` does not show a nameplate even if within range.
- Given `nameplate: Some(false)` on a prefab in a scene with `show_nameplates: true`, no nameplate appears.
- Given `nameplate: Some(true)` on a prefab in a scene with `show_nameplates: false`, the nameplate still appears (respects `max_distance`, ignores faction filter).
- Given `display_name: Some("Orc Warrior")`, the name line reads "Orc Warrior". Given no `display_name`, the name line reads the prefab key.
- Given `stat_bars` containing `{self}.health` and `{self}.mana`, an entity with both stats shows two pixel bars; an entity with only health shows one bar (mana bar silently absent).
- Given a pixel health bar, the fill quad shrinks left-anchored as health decreases and returns to full width at max health.
- Given `text_shadow: true`, the name text renders with a visible drop shadow.
- Given a scene transition (`LoadScene`), all nameplate widgets are despawned and `NameplateSceneConfig` is reset.
- Given the player prefab has `nameplate: Some(true)` and `faction_filter: All`, the player's nameplate is visible when within range.
