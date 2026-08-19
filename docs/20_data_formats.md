# Data Formats

> **Doc type:** Reference
>
> **Status legend:**
> - ✅ **Implemented** — exists in code today
> - 🧪 **Prototype / Partial** — exists but incomplete or unstable
> - 🧭 **Planned** — intended design; not implemented yet

## Status
✅ Schema v2 implemented and in use across all example projects.

---

## Rendering philosophy

The runtime prioritises **consistent, performant rendering across all supported platforms** over per-platform visual quality upgrades.

- **Web builds are the performance baseline.** Every rendering feature must work acceptably in a WebGPU WASM build. If a feature cannot meet that bar it is not enabled on any platform.
- **All platforms look the same.** There are no native-only graphics options. A scene authored once should render identically on desktop and in the browser.
- **Performance over visual fidelity.** When a trade-off must be made the engine chooses the faster option. Developers can push visual quality within the allowed feature set, but cannot opt in to slower paths.

**Excluded features and why:**

| Feature | Why excluded |
|---------|--------------|
| `TonyMcMapface` tonemapper | Requires a LUT texture lookup — additional bandwidth, slower, and web-unfriendly |
| `BlenderFilmic` tonemapper | Same as above |
| HDR | Adds memory and bandwidth overhead; consistent non-HDR output avoids per-platform clip differences |
| Bloom | Post-processing pass; not worth the GPU cost at the web performance baseline |

---

## Color conventions ✅

All color tuples in RON files are **sRGB** — author them the same way you would in an image editor or CSS. The engine calls `Color::srgba()` / `Color::srgb()` on every color field it reads; Bevy linearises internally before the GPU. Do **not** pre-linearise values in RON — they will appear washed out.

- **3-component** `(r, g, b)` — lights and primitive mesh `color` fields.
- **4-component** `(r, g, b, a)` — UI, icon tints, stat bars, particles, and everything else.

---

## Versioning ✅

All top-level data files include `schema_version: <u32>`. The runtime validates this on load and rejects unsupported versions. Both v1 and v2 are accepted where noted.

---

## Project folder layout ✅

Each game project lives under `assets/projects/{name}/`:

```
assets/projects/{name}/
  {name}.project.ron          ← ProjectConfig (entry point)
  assets.ron                  ← AssetCatalog  (model/texture/audio/material keys)
  prefabs/prefabs.ron         ← PrefabCatalog (named entity templates)
  prefabs/animation/*.ron     ← AnimationPolicy per character type
  behaviors/*.behavior.ron    ← per-entity FSM behavior files (optional)
  scenes/*.scene.ron          ← GameSceneV2   (one file per scene)
  logic/rules.ron             ← LogicRulesAsset (event → action rules)
  overrides/model_fixes.ron   ← ModelFixesAsset (per-asset transform corrections)
  stats/stats.ron             ← StatCatalog     (global named stat definitions; optional)
```

The native runner selects a project by name: `cargo run -p ironhold_native -- --project quick_scene`.
The web runner uses a URL param: `play.html?project=quick_scene`.
Both default to `quick_scene` if nothing is specified.

### File naming conventions

The engine identifies files by their path (as set in the project config) and by extension for a few special types. Here is the full convention:

| File type | Extension | Typical path |
|---|---|---|
| Project config | `.project.ron` | `{name}.project.ron` |
| Scene | `.scene.ron` | `scenes/{name}.scene.ron` |
| Entity behavior | `.behavior.ron` | `behaviors/{id}.behavior.ron` |
| Prefab catalog | `.ron` | `prefabs/prefabs.ron` |
| Animation policy | `.ron` | `prefabs/animation/{name}_policy.ron` |
| Asset catalog | `.ron` | `assets.ron` |
| Logic rules | `.ron` | `logic/rules.ron` |
| State machine | `.ron` | `logic/state_machine.ron` |
| Model overrides | `.ron` | `overrides/model_fixes.ron` |
| Stat catalog | `.ron` | `stats/stats.ron` |
| Dialogue | `.dialogue.ron` | `dialogues/{name}.dialogue.ron` |

`.scene.ron`, `.project.ron`, `.behavior.ron`, and `.dialogue.ron` use a double extension so the engine can discover them by suffix. All other `.ron` files are found by their exact path, which is set in the project config — the filename itself is not significant to the runtime.

---

## `{name}.project.ron` — ProjectConfig ✅

Entry point for a project. References all other files.

**Fields:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `schema_version` | `u32` | ✅ | Must be `1`, `2`, or `3` |
| `initial_scene` | `String` | ✅ | Path to starting scene, relative to project root |
| `project_id` | `Option<String>` | v2+ | Machine-readable identifier |
| `display_name` | `Option<String>` | v2+ | Human-readable name |
| `asset_catalog` | `Option<String>` | v2+ | Path to `assets.ron` |
| `prefab_catalog` | `Option<String>` | v2+ | Path to `prefabs/prefabs.ron` |
| `rules_path` | `Option<String>` | v2 | Path to `logic/rules.ron` (rules workflow) |
| `state_machine_path` | `Option<String>` | v3 | Path to `logic/state_machine.ron` (FSM workflow; use instead of `rules_path`) |
| `model_fixes_path` | `Option<String>` | v1+ | Path to `overrides/model_fixes.ron` |
| `global_environment` | `Option<EnvironmentMapConfig>` | — | Project-wide fallback IBL lighting |
| `global_key_bindings` | `Map<String, String>` | — | Key name → trigger name (e.g. `"Escape": "toggle_pause"`) |
| `global_unclaimed_gamepad_bindings` | `Map<String, String>` | — | Gamepad button name → trigger name, project-wide. **Not a general gamepad analogue of `global_key_bindings`** — only ever fires on a gamepad not currently assigned to any live player (a player whose `gamepad_index` hasn't resolved to a real controller yet does not reserve one), intended for join-style triggers. See [Gamepad-triggered hot join](#gamepad-triggered-hot-join) below. |
| `primitive_default_color` | `Option<(f32,f32,f32)>` | — | Default sRGB color for all `kind: "primitive"` prefabs that omit their own `color`. Falls back to grey `(0.7, 0.7, 0.7)` when absent. |
| `stats_path` | `Option<String>` | — | Path to a `stats.ron` file. When absent, the stat system is inactive for this project. |
| `items_path` | `Option<String>` | — | Path to an `items/items.ron` file. When absent, the inventory system is inactive for this project. |
| `damage_popup_style` | `Option<DamagePopupStyle>` | — | Visual style for `Action::ShowDamagePopup` popups. Omit for built-in defaults. See [DamagePopupStyle](#damagepopupstyle) below. |
| `audio` | `AudioConfig` | — | Project-level audio settings. Omit for defaults (`max_volume: 1.0, mute_on_start: false`). See [AudioConfig](#audioconfig) below. |
| `rules` | `Vec<LogicRule>` | v1 only | Inline rules (v1 only; use `rules_path` in v2) |
| `model_fixes` | `Map<String, TransformFix>` | v1 only | Inline fixes (v1 only; use `model_fixes_path` in v2+) |

**Example (v2 — rules workflow):**
```ron
(
    schema_version: 2,
    project_id: "quick_scene",
    display_name: "Quick Scene",

    initial_scene: "scenes/main.scene.ron",

    asset_catalog: "assets.ron",
    prefab_catalog: "prefabs/prefabs.ron",
    rules_path: "logic/rules.ron",
    model_fixes_path: "overrides/model_fixes.ron",
)
```

**Example (v3 — FSM workflow):**
```ron
(
    schema_version: 3,
    project_id: "my_game",
    display_name: "My Game",

    initial_scene: "scenes/start_menu.scene.ron",

    asset_catalog: "assets.ron",
    prefab_catalog: "prefabs/prefabs.ron",
    state_machine_path: "logic/state_machine.ron",
    model_fixes_path: "overrides/model_fixes.ron",

    global_key_bindings: {
        "Escape": "toggle_pause",
    },
    // global_unclaimed_gamepad_bindings: { "South": "join" },  // see Gamepad-triggered hot join
)
```

---

## `scenes/*.scene.ron` — GameSceneV2 ✅

Declaratively defines a scene: entities, UI, lighting, terrain, spawn points.
File extension must be `.scene.ron`.

**Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `schema_version` | `u32` | Must be `2` |
| `name` | `String` | Scene identifier (used in `SceneEvent::Ready`) |
| `tonemapping` | `TonemappingOption` | Tonemapping for all cameras in this scene. Defaults to `AcesFitted`. See below. |
| `lighting` | `Option<SceneLightingV2>` | Ambient, directional, and point light config. See below. |
| `terrain` | `Option<TerrainConfigV2>` | Heightmap-based terrain |
| `spawn_points` | `Map<String, (f32,f32,f32)>` | Named world-space positions |
| `camera_modes` | `Map<String, CameraModeDef>` | Named, switchable camera presets `Action::SetCameraMode` can jump to at runtime (v2). Does not affect what any camera starts as. See [`camera_modes:` registry and `SetCameraMode`](#camera_modes-registry-and-setcameramode-v2-) below. |
| `entities` | `Vec<SceneEntityDef>` | Prefab instances to spawn |
| `ui` | `Vec<UiNodeDef>` | UI elements (buttons, labels, rects) to show in this scene |
| `ui_panel` | `Option<UiPanelDef>` | When set, UI elements are laid out in a centered panel box instead of absolute positioning |
| `scene_key_bindings` | `Map<String, String>` | Per-scene key overrides; same format as `global_key_bindings`. Cleared on each scene load. |
| `scene_unclaimed_gamepad_bindings` | `Map<String, String>` | Per-scene gamepad button overrides; same format and same unclaimed-pad-only behavior as `global_unclaimed_gamepad_bindings`. Overlays per-key on top of the project base — a key present here replaces just that entry, every other project-level binding still applies (same merge rule as `scene_key_bindings`). Cleared and rebuilt on each scene load. |
| `world_labels` | `Vec<WorldLabelDef>` | 3D world-space text labels that project to screen space and face the camera |
| `label_depth_scale` | `Option<LabelDepthScaleDef>` | When set, all labels shrink as camera distance increases. World labels (`world_labels:`, entity `label:`) can override per-label with `depth_scale: false` or `depth_scale: true`. Stat labels/bars (`stat_label`/`world_stat_bar` on a prefab — all four styles, `Ascii`/`Pixel`/`Icon`/`Textured`), and nameplates (`show_nameplates`/`show_player_nameplate`), have no per-widget override — they always simply inherit this scene setting, whether the entity is scene-placed or spawned at runtime via `Action::Spawn` (e.g. a wave-spawned enemy). Tune `reference_distance` to the scene's actual camera-to-widget distance range, not just `Orbit`/`Party`'s `min_radius`/`max_radius` — entities can sit elsewhere in the scene than the camera's own orbit target, so their real distance from the camera can exceed the radius range. A `reference_distance` well outside that real range (e.g. the `50.0` default against a project whose camera never gets that far from anything) means the `(reference_distance / distance)` ratio never drops below 1.0, so scaling silently never engages. See [Label depth scaling](#label-depth-scaling-labeldepthscaledef) below for the full field reference and tuning guidance. |
| `particle_budget` | `Option<u32>` | Maximum live particle count for this scene. Default: `2000`. `Ambient` effects are dropped when full; `Npc` effects are halved; `Player` effects always fire. |
| `target_indicator` | `Option<TargetIndicatorDef>` | Ground-ring decal shown under the selected target entity. Omit to disable. See below. |
| `target_hud` | `Option<TargetHudDef>` | Per-viewport target-name HUD readout for split-screen scenes (2+ players). Omit to disable. See [Per-player split-screen targeting](#per-player-split-screen-targeting) below. |
| `show_nameplates` | `bool` | Enable the nameplate system for NPCs/props in this scene. Default: `false`. When `true`, entities tagged at spawn time display a floating name + pixel stat-bar widget above them. Individual prefabs can override this per-entity via `PrefabDef.nameplate`. Does **not** govern the player's own nameplate — see `show_player_nameplate` below. |
| `nameplate_options` | `Option<NameplateOptionsDef>` | Scene-wide nameplate display configuration. Cosmetic fields (offset, font, colors, bars) apply regardless of `show_nameplates`/`show_player_nameplate`; `faction_filter` only matters when `show_nameplates: true`. Omit to use all defaults. See [Nameplate system](#nameplate-system-nameplateoptionsdef-) below. |
| `max_view_box` | `Option<(f32,f32,f32,f32)>` | Hard XZ movement boundary `(min_x, min_z, max_x, max_z)` in world units. Every player's position is clamped inside this box each physics tick (vertical movement/jumping is unaffected); velocity on the clamped axis is zeroed so the player doesn't jitter against the edge. Intended for local co-op scenes with a shared camera, so players can't wander out of frame — but works for any scene. Omit to disable (most scenes have no boundary). |
| `join_prefab_keys` | `Vec<Option<String>>` | Per-slot player prefab keys for `Action::JoinPlayer` hot-join. See [Local co-op hot join](#local-co-op-hot-join) below. |

**Example:**
```ron
(
  schema_version: 2,
  name: "main",

  lighting: (
    ambient: (0.35, 0.35, 0.4),
    directional: (
      color: (1.0, 0.98, 0.92),
      intensity: 12000.0,
      rotation_euler_deg: (-45.0, 35.0, 0.0),
    ),
  ),

  spawn_points: {
    "player_start": (0.0, 4.0, 0.0),
  },

  entities: [
    (
      id: "player_01",
      prefab: "player_warrior",
      transform: (
        translation: (0.0, 4.0, 0.0),
        rotation_euler_deg: (0.0, 0.0, 0.0),
        scale: (1.0, 1.0, 1.0),
      ),
    ),
    (
      id: "chest_01",
      prefab: "chest_01",
      transform: (
        translation: (5.0, 0.0, 3.0),
        rotation_euler_deg: (0.0, 45.0, 0.0),
        scale: (1.0, 1.0, 1.0),
      ),
    ),
  ],

  ui: [
    Button((
      id: "dance_button",
      text: "Dance",
      action: "ui.dance",
      position: (20.0, 60.0),
      size: (150.0, 40.0),
    )),
    Button((
      id: "quit_button",
      text: "Quit",
      action: "ui.quit",
      position: (20.0, 100.0),
      size: (150.0, 40.0),
    )),
  ],
)
```

### Scene entities (`SceneEntityDef`) ✅

Each entry in the `entities` list places one prefab instance into the scene.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `id` | `String` | required | Unique spawn ID for this instance (used in rules, behaviors, and stat addressing) |
| `prefab` | `String` | required | Key into `PrefabCatalog.prefabs` |
| `transform` | `SceneTransformV2` | `()` | Position, rotation, and scale. All sub-fields optional; defaults to origin, no rotation, scale `(1,1,1)`. |
| `label` | `Option<EntityLabelDef>` | `None` | Floating text annotation above the entity. See [Label depth scaling](#label-depth-scaling-labeldepthscaledef) for per-label `depth_scale` options. |
| `stat_overrides` | `Map<String, f32>` | `{}` | Override the initial value for named stats from the prefab's `stat_templates`. Keys are stat names; unknown keys log a warning. `min`/`max`/`regen`/`thresholds` are unchanged — only the starting value differs. |

**`stat_overrides` example** — place a wounded enemy and an inactive shrine from the same prefabs, without forking them:

```ron
entities: [
    // Full-health enemy
    ( id: "orc_01", prefab: "enemy_orc_melee", transform: ( translation: (10.0, 0.0, 5.0) ) ),

    // Already-wounded enemy — same prefab, starts at 30 HP instead of 100
    ( id: "orc_wounded", prefab: "enemy_orc_melee", transform: ( translation: (15.0, 0.0, 5.0) ),
      stat_overrides: { "health": 30 } ),

    // Inactive shrine — health starts at 0 so its "depleted" threshold fires immediately
    ( id: "shrine_inactive", prefab: "shrine", transform: ( translation: (0.0, 0.0, 10.0) ),
      stat_overrides: { "health": 0 } ),
],
```

> **Note:** `stat_overrides` only applies to scene-placed entities. Dynamically spawned entities via `Action::Spawn` always start at the template `base` value.

### Tonemapping (`TonemappingOption`)

Applied to **all cameras** spawned for the scene (flycam, orbit camera, and fallback camera). Omit the field to get the default.

| Value | Style | Notes |
|-------|-------|-------|
| `AcesFitted` *(default)* | Cinematic, high-contrast | No LUT. Good for most 3D scenes. |
| `None` | Raw linear | Colours clip at 1.0. For flat / stylised scenes. |
| `Reinhard` | Smooth, muted | Can look washed out at high exposures. |
| `ReinhardLuminance` | Detail-focused | Preserves hue better than plain Reinhard. |
| `SomewhatBoringDisplayTransform` | Neutral, predictable | Minimal artistic flavour. |

`TonyMcMapface` and `BlenderFilmic` are not available — they require a LUT texture which reduces performance and is not compatible with the web baseline. See [Rendering philosophy](#rendering-philosophy).

### Lighting (`SceneLightingV2`)

**`SceneLightingV2` fields:**

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `ambient` | `Option<(f32,f32,f32)>` | engine default | Ambient light colour as sRGB |
| `ambient_brightness` | `Option<f32>` | `150.0` | Ambient brightness in lux. Without HDR colours clip at 1.0, so keep this low (50–300 is typical). |
| `directional` | `Option<DirectionalLightDefV2>` | none | A single directional (sun) light |
| `point_lights` | `Vec<PointLightDefV2>` | `[]` | Point (omnidirectional) lights |
| `shadow_map_size` | `Option<u32>` | `2048` | Texel resolution of the directional-light shadow atlas. Must be a power of two. Lower values (`512`, `1024`) improve GPU performance; higher values (`4096`) give sharper shadows on large scenes. |
| `point_shadow_map_size` | `Option<u32>` | `1024` | Texel resolution of each point-light shadow cube face. Same power-of-two rule applies. Only relevant when a point light has `shadows_enabled: true`. |

**`DirectionalLightDefV2` fields:**

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `color` | `(f32,f32,f32)` | required | sRGB colour |
| `intensity` | `f32` | required | Illuminance in lux |
| `rotation_euler_deg` | `(f32,f32,f32)` | required | Euler angles in degrees (XYZ order) |
| `shadows_enabled` | `bool` | `true` | Whether this light casts shadows |
| `shadow_distance` | `Option<f32>` | engine default | Maximum world-unit distance at which shadow cascades are rendered. Tune downward for sharper shadows on a small scene; set to the full scene depth on large showcases. |
| `cascade_overlap` | `Option<f32>` | `0.2` | Fraction of each cascade's range that overlaps the next cascade (0.0–1.0). A wider overlap blends the transition zone so the seam between cascades is invisible. `0.5` eliminates most visible seam bands on large flat surfaces. |
| `num_cascades` | `Option<u32>` | `4` | Number of shadow cascade splits. Fewer cascades (`1`–`2`) reduce the GPU shadow pass cost significantly; more cascades give better shadow resolution over large distances. |

**`PointLightDefV2` fields:**

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `position` | `(f32,f32,f32)` | required | World-space position |
| `color` | `(f32,f32,f32)` | `(1,1,1)` | sRGB colour |
| `intensity` | `f32` | `800.0` | Luminous power in lumens (≈ a bright 60 W bulb) |
| `radius` | `f32` | `0.0` | Sphere radius for specular highlights |
| `range` | `f32` | `20.0` | Maximum reach in world units |
| `shadows_enabled` | `bool` | `false` | Whether this light casts shadows (expensive — use sparingly) |

**Example (full lighting block):**
```ron
lighting: (
  ambient: (0.25, 0.30, 0.45),
  ambient_brightness: 15.0,

  directional: (
    color: (1.0, 0.95, 0.85),
    intensity: 30000.0,
    rotation_euler_deg: (-45.0, 25.0, 0.0),
    shadows_enabled: true,
    shadow_distance: 450.0,
    cascade_overlap: 0.5,
  ),

  point_lights: [
    (
      position: (0.0, 15.0, -40.0),
      color: (0.5, 0.7, 1.0),
      intensity: 80000.0,
      range: 60.0,
    ),
  ],
),
```

### Label depth scaling (`LabelDepthScaleDef`)

Controls how labels scale with camera distance. Set at scene level; individual labels can opt out.

Together, `reference_distance` and `min_scale` define a **working band**: labels are full size from
`0` to `reference_distance`, shrink proportionally between `reference_distance` and
`reference_distance / min_scale`, and are flat (at `min_scale`) beyond that. For example,
`(reference_distance: 8.0, min_scale: 0.5)` means full size out to 8m, shrinking to half size by
16m, then holding at half size beyond that. **Pick `reference_distance` near where your camera
normally sits** — typically somewhere in the middle of your `Orbit`/`Party` camera's
`min_radius..max_radius` range (or, if the camera's *target* isn't at the camera's own position —
e.g. NPCs standing elsewhere in the scene — near the camera's actual distance to the entities you
care about, which can be larger than the orbit radius alone).

**This system only ever shrinks, never grows.** A label closer than `reference_distance` stays
pinned at its authored size (scale never exceeds 1.0) — so if labels look oversized when the
camera is zoomed all the way in, lowering `reference_distance` won't fix it; that's a
font-size/offset tuning problem instead, unrelated to this system.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `reference_distance` | `f32` | `50.0` | Camera distance where labels are at their authored size. Zoom closer than this and nothing changes. Zoom further and they shrink in proportion to distance — at twice this distance they are half size. Leaving this at the `50.0` default (or omitting the block entirely) on a normal 3rd-person camera whose max zoom distance never reaches 50 means scaling never engages at any zoom level — this is a common way to accidentally ship the "labels never shrink" bug. |
| `min_scale` | `Option<f32>` | `None` | Minimum scale floor as a fraction of authored size (0.0–1.0) — together with `reference_distance` this sets where the working band ends (see above). `0.25` means labels never shrink below 25% of their authored size. Omitting `min_scale` means no floor — labels scale toward zero at extreme distances (usually not what you want; a very small but nonzero floor like `0.2`–`0.5` keeps distant labels legible instead of vanishing). |

**Per-label override** — both `WorldLabelDef` and `EntityLabelDef` accept a `depth_scale: Option<bool>` field:
- `depth_scale: false` — pin this label at its authored size regardless of scene setting
- `depth_scale: true` — force depth scaling on even if the scene has no `label_depth_scale` block (uses `reference_distance: 50.0`, no floor)
- `depth_scale` omitted — inherits the scene setting (default)

`stat_label`/`world_stat_bar` (all four styles) and nameplates have no per-widget override at all
— they always simply inherit the scene setting, so a `label_depth_scale` block affects every
floating widget in the scene at once, not just nameplates. Check your smallest `font_size`/bar
`size` against `min_scale` — e.g. a 14px stat label with `min_scale: 0.5` reads as low as 7px at
range.

> **Stacking multiple widgets on one entity — use `screen_offset`, not slightly different
> `offset`s.** A `stat_label` and `world_stat_bar` tracking the same entity are two independent
> `WorldLabel`s, each perspective-projected separately. If they're given close-but-not-identical
> world-space `offset`s (e.g. `2.8` vs `3.1`) to stack a number above its bar, the *world* gap
> between them projects to a *pixel* gap that grows the closer the camera gets and shrinks the
> farther out it gets — the same widget stack that looks fine at one zoom level visibly spreads
> apart or collapses at another (a real bug, caught via a playtest screenshot). Fix: give
> co-located widgets the SAME `offset`, and use each one's `screen_offset` (`StatLabelDef`/
> `WorldStatBarDef`) — a pixel-space offset applied after projection — for the stacking instead.
> `screen_offset` doesn't scale with camera distance on its own, but IS scaled by the same
> depth-scale factor as everything else here, so the whole stack still shrinks together as one
> cohesive unit rather than the gap staying a fixed size while the widgets around it shrink.
> `screen_offset` is available on `stat_label`/`world_stat_bar` only — scene `world_labels:` and
> entity `label:` don't have it yet.
>
> **The three widget types default to different `offset`s** (`nameplate_options.offset` `2.4`,
> `StatLabelDef.offset` `2.5`, `WorldStatBarDef.offset` `2.8`) — so simply omitting `offset` on a
> new prefab's `stat_label`/`world_stat_bar` re-creates this exact drift bug. You must set `offset`
> explicitly, matching whatever value your nameplate (or other reference widget) uses.
> `nameplate_options.offset` is scene-wide with no per-prefab override, so it's usually the value
> every prefab in the scene copies — if you change it, every prefab sharing it needs re-tuning.
>
> **Picking `screen_offset` values for a new creature:** you do not need to convert metres to
> pixels. `screen_offset` is in the same pixel-space unit as `font_size` and the bar's own `size`
> — just clear the number by roughly its own `font_size`, and the bar by roughly its own height,
> tuning by eye. (`3rd_person_game_demo`'s `enemy_zombie` stacks its health number ~21px above its
> bar — its `font_size` is 14, plus a few pixels of padding — a good rule of thumb to start from.)
> If a creature is much shorter/taller than the scene's nameplate offset (see `enemy_snake` in the
> same project), share a separate offset among the widgets you control instead of the nameplate's
> — there's no per-prefab nameplate offset override today, so the nameplate itself will sit apart;
> that's a distinct, known limitation, not the bug this section is about.

**Example** (a scene whose camera zooms between roughly 5m and 20m):
```ron
label_depth_scale: (
  reference_distance: 10.0,  // ~midpoint of the camera's zoom range
  min_scale: 0.4,
),

// In entities — a nearby header pinned at full size:
label: (text: "Header", depth_scale: false),
```

### Target indicator (`TargetIndicatorDef`)

A flat, double-sided, unlit ground-ring decal that tracks the selected entity's XZ position each frame.
Add a `target_indicator:` block to a scene RON to enable it; omit the field to disable silently.
The texture key must exist in the scene's `assets.ron` `decals:` map.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `texture` | `String` | *(required)* | Decal catalog key from `assets.ron` `decals:` section — **not** the `textures:` section. |
| `radius` | `f32` | `1.0` | Ring radius in metres. The quad is scaled to `radius × 2` in X and Z. |
| `color` | `(f32, f32, f32, f32)` | `(0.3, 0.8, 1.0, 0.75)` | RGBA tint — scene-level fallback used when a prefab has no `indicator_color` or `indicator_category`. |
| `offset_y` | `f32` | `0.05` | Y lift above ground to avoid z-fighting. |
| `named_colors` | `HashMap<String, (f32,f32,f32,f32)>` | `{}` | Named colour palette for per-prefab `indicator_category` lookups. Keys are arbitrary strings (e.g. `"enemy"`, `"ally"`); values are RGBA. A category key absent from this map falls through to `color`. |

**Ring colour resolution** (highest precedence first):
1. Prefab `indicator_color` — direct RGBA override on the selected entity's `PrefabDef`
2. Prefab `indicator_category` — string key looked up in `named_colors`
3. Scene-level `color` — fallback for any prefab that declares neither

> **Silent fallthrough:** if a prefab's `indicator_category` key is not present in `named_colors`
> (including a typo or case mismatch), the ring silently falls back to the scene-level `color`.
> There is no error at load time. Double-check key spelling if the wrong colour appears.

The indicator only appears when an entity is selected (via click or Tab). It disappears on `ClearTarget`,
when the target entity is hidden (e.g. dead), or when a new scene loads.
Entities must have `click_selectable: true` or `targetable: true` in their `PrefabDef` to be selectable.

**`assets.ron` entry** (`texture:` must be in the `decals:` section, not `textures:`):
```ron
decals: {
  "target_ring": "shared/textures/decals/ring_thick.png",
},
```

**`scene.ron` usage:**
```ron
target_indicator: (
  texture: "target_ring",  // key from assets.ron decals:, NOT textures:
  radius: 1.2,
  color: (0.3, 0.8, 1.0, 0.75),  // cyan fallback for prefabs with no category
  offset_y: 0.05,
  named_colors: {
    "enemy":    (1.0, 0.15, 0.15, 0.85),  // red
    "creature": (1.0, 0.75, 0.15, 0.85),  // gold
  },
),
```

**`prefabs.ron` usage:**
```ron
// Category-driven colour (key must match a named_colors entry in the scene):
"enemy_orc_melee": (
  targetable: true,
  click_selectable: true,
  indicator_category: "enemy",
  ...
),
// Direct per-prefab RGBA override (bypasses named_colors entirely):
"special_boss": (
  targetable: true,
  indicator_color: (0.8, 0.0, 0.9, 0.95),  // unique purple ring
  ...
),
// No fields set → uses scene-level color fallback:
"neutral_npc": (
  targetable: true,
  ...
),
```

### Per-player split-screen targeting

_Used in: local co-op / split-screen scenes with 2+ `tags: ["player"]` entities._

Each player has their own independent target — clicking or Tab-cycling as player 2 no longer
overwrites player 1's selection. This changes three things about the pre-existing targeting
feature (click-to-select, Tab-cycle, the target indicator ring, above) once **2+ players are
present**, and adds one new opt-in RON block:

**Target indicator rings are tinted per-player.** Whenever 2+ players are present (checked by
counting player entities directly — **including party/shared-camera mode**, not just real
split-screen), every ring uses the same fixed palette as the "P{n}" corner HUD label
(`PLAYER_LABEL_COLORS` — P1 blue, P2 pink, P3 green, P4 red, matching `local_coop_demo`'s room6
tints; see "Split-screen player HUD labels" below) instead of the usual
`indicator_color`/`indicator_category`/scene `color` precedence — so it's visually obvious whose
ring belongs to whom when two players are looking at different (or the same) entity at once. If
both players select the same entity, both rings render, coincident, each in its own player's
colour — there is no deduplication. **Single-player scenes are completely unaffected** — the ring
keeps the usual prefab/category/scene colour precedence exactly as documented above.

> **Restricting a ring to its owner's viewport only.** By default the tinted ring above is still
> visible in *every* split viewport, not just its owner's — a P2 ring shows up in P1's half of the
> screen too. To instead make each player's ring visible only in their own viewport, set
> `SplitScreenDef.own_viewport_only: true` on the first player's `camera.split` block — see
> [Per-viewport target ring visibility](#per-viewport-target-ring-visibility-own_viewport_only)
> under [Split-screen camera](#split-screen-camera-splitscreendef-) below. This only changes
> *where* a ring renders — it does not change or replace the tinting described above.

**The legacy `target_display`/`target_name`/`target_id` `GameVariables` (see "GameVariables
auto-written by capabilities" below) go blank whenever 2+ players are present** — the same
player-count check as the ring tinting above, **including party mode** — rather than showing only
one player's value with no indication that a second player's selection isn't reflected. If your
project authored a `Label` bound to `target_display` for a single-player HUD and later adds a
second player, that label will read blank once a second player is present. Use the new
`target_hud:` block below instead — **but only for split-screen scenes**: a party-mode 2-player
scene has no `SplitViewportSlot` camera for `target_hud:` to attach to, so it gets no readout at
all today (blank legacy vars, no replacement) — a known Phase 1 gap, not yet built. **Single-player
scenes are unaffected** — the vars keep populating exactly as before.

**Only the primary player's selection drives `rules.ron`/`state_machine.ron`/behavior `do_actions`
through the shared pipeline.** The first player-tagged scene entity (matching `PlayerIndex(0)`, or
no `PlayerIndex` at all — a case only reachable via the terrain-deferred or character-select spawn
paths, since neither currently spawns primitive-shaped players; every player spawned via the
ordinary scene-load path, GLB or primitive, gets a `PlayerIndex`) is "primary". `{target}`
substitution in `rules.ron`/`state_machine.ron`/behaviors still resolves against the
primary player's target only — a second (or third, or fourth) player's selection drives their own
visual feedback (ring, HUD readout) but has no effect on those global `do_actions`.

**Action bars are the one exception, as of per-player action bars (below).** A bar tagged with
`owner_player` resolves its own slots' `{target}` (the rewrite and the no-target gate) against
*that bar's own player's* selection, not the primary player's — see "Per-player action bars
(split-screen)" under `ActionBar` for the field and its scope boundaries (a rule that overrides a
slot's intent is the one path that still falls back to the primary player's target).

> **`target.clicked:{id}` fires for every player's click, unlike `target.changed`/
> `target.cleared` (primary-player only).** A rule matched on `target.clicked:{id}` already has
> the clicked entity's exact id in the event name itself — write the rule's `do_actions` against
> that id directly (or `{self}` inside a matching behavior file), not `{target}`. Using `{target}`
> in a `target.clicked:{id}`-triggered rule resolves against the *primary* player's target, which
> may be a different entity than the one just clicked if a non-primary player did the clicking.

**One physical mouse can only ever click-select for one player per click** — an unavoidable,
accepted limitation, not a bug. Tab-cycling (each player bound to their own `target_next` key —
or a gamepad button, since co-op commonly pairs one keyboard player with one gamepad player) is the
only mechanism that lets both players change their target in the same moment.

#### Per-viewport target HUD readout (`TargetHudDef`)

_Used in: `GameSceneV2.target_hud`_

Opt-in — omit this field entirely to skip the per-viewport readout (the legacy `target_display`
`GameVariables` above still exist for single-player scenes regardless). When set, each
split-screen viewport (any camera tagged `SplitViewportSlot` — `Vertical`/`Horizontal`/`Grid`)
automatically gets its own bottom-left-anchored text readout showing that viewport's own player's
currently selected target — independent of every other player's. **Fully engine-automatic
placement**, same "no opt-out, no repositioning" precedent as the "P{n}" corner label (below) —
only the text format/font/colour are configurable.

| Field | Type | Default | Description |
|---|---|---|---|
| `show` | `TargetHudDisplay` | `Full` | Which of prefab/id/name to display. `Full` = `"prefab_key id"` (matches the legacy `target_display` format, e.g. `"enemy_orc_melee orc_01"`); `NameOnly` = prefab catalog key only; `IdOnly` = per-instance spawn id only. |
| `font_size` | `f32` | `16.0` | Screen-space font size in pixels. |
| `color` | `(f32, f32, f32, f32)` | `(0.9, 0.9, 0.9, 1.0)` | Text colour (sRGB RGBA). |

```ron
// scene.ron
target_hud: (
  show: Full,
  font_size: 16.0,
  color: (0.9, 0.9, 0.9, 1.0),
),
```

Party-mode and single-player scenes never get a readout — like the corner label, there is no
`SplitViewportSlot` camera to attach one to. Has no effect if authored on a scene with 0-1 players.

### Nameplate system (`NameplateOptionsDef`) ✅

_Used in: `GameSceneV2.nameplate_options`_

Enable the nameplate system on a scene by setting `show_nameplates: true`. Each spawned NPC/prop entity that passes the faction filter and has no `nameplate: false` override receives a floating widget above it: a name line (from `PrefabDef.display_name` or the prefab key) plus any number of pixel stat bars. The widget hides automatically when the camera moves beyond `max_distance`.

The whole widget (name text + all bars) inherits the scene's [`label_depth_scale`](#label-depth-scaling-labeldepthscaledef) setting, same as `stat_label`/`world_stat_bar` — no per-widget override. Without a `label_depth_scale` block, nameplates render at a fixed screen size regardless of camera zoom, which can make two nearby entities' nameplates crowd together when zoomed far out. If your project uses an `Orbit`/`Party` camera with a wide zoom range, set `reference_distance` to somewhere inside that range (not the `50.0` default) so scaling actually engages — see the tuning note on `label_depth_scale` above.

The player's own nameplate is controlled independently by `show_player_nameplate` (below) — it is never subject to `show_nameplates` or `faction_filter`, since faction hostility categorization doesn't apply to "should I see my own name." A per-prefab `nameplate: true`/`false` override still wins over either scene toggle, exactly the same way for the player as for any other entity.

> **Two independent toggles:** `show_nameplates` covers NPCs/props; `show_player_nameplate` covers only your player. Setting `show_nameplates: true` does **not** show the player's own nameplate — you must also set `show_player_nameplate: true` (or a per-prefab `nameplate: true` on the player prefab) if you want it.

> **Runtime player toggle:** `Action::ToggleOwnNameplate` lets a player flip their own nameplate on/off at runtime (e.g. bound to a settings-menu button), independent of `show_player_nameplate`. It emits `nameplate.own_shown`/`nameplate.own_hidden` — bind these to an `IconButton`'s `bind` `GameVariable` the same way `audio.muted`/`audio.unmuted` drive the mute-button toggle (see [`IconButton((...))`](#iconbutton-) above). Has no effect on NPC/prop nameplates. If the player prefab has an explicit `nameplate: Some(true)`/`Some(false)` override, that always wins — the toggle still flips internally (and the button's bound label will still change), but the nameplate's actual visibility won't change, since the override bypasses the runtime preference entirely.
>
> ⚠️ **This preference does not persist across a scene transition.** It resets to the current scene's `show_player_nameplate` default on every scene load — including `LoadScene` to the same scene. If your project needs the choice to persist (e.g. across a portal or scene reload), that isn't built in yet; treat it as a per-scene runtime toggle, not a saved setting.

| Field | Type | Default | Description |
|---|---|---|---|
| `faction_filter` | `NameplateFactionFilter` | `HostileOnly` | Which NPC/prop entities receive a nameplate. `HostileOnly` shows nameplates on entities with NPC AI (hostile actors). `FriendlyOnly` shows them on non-NPC entities. `All` shows nameplates on every tagged entity. Individual prefabs can override this with `PrefabDef.nameplate`. Never applies to the player — see `show_player_nameplate`. |
| `show_player_nameplate` | `bool` | `false` | Whether the player's own nameplate is shown, independent of `show_nameplates`/`faction_filter`. Defaults to `false`, matching genre convention (most 3rd-person RPGs hide your own nameplate since it only occludes your own character). A per-prefab `nameplate: true`/`false` override on the player prefab still wins over this default. |
| `max_distance` | `f32` | `20.0` | Maximum camera distance in world units (metres) at which nameplates remain visible. Nameplates beyond this distance are hidden each frame. |
| `offset` | `(f32, f32, f32)` | `(0.0, 2.4, 0.0)` | World-space offset from the entity's origin to the nameplate anchor point. Adjust the Y component to place the widget above the entity's head (e.g. `2.4` for a human-scale character). |
| `name_font_size` | `f32` | `14.0` | Font size of the name text line in screen pixels. |
| `name_color` | `(f32, f32, f32, f32)` | `(0.95, 0.95, 0.95, 1.0)` | RGBA colour of the name text (sRGB). |
| `text_shadow` | `bool` | `true` | When `true`, a drop shadow is rendered behind the name text to improve legibility against bright backgrounds. |
| `stat_bars` | `Vec<NameplateBarDef>` | `[]` | Ordered list of pixel stat bars shown below the name line. Bars for stats the entity does not have are silently skipped. See `NameplateBarDef` below. |
| `bar_width` | `f32` | `100.0` | Width of each stat bar in screen pixels. |
| `bar_height` | `f32` | `6.0` | Height of each stat bar in screen pixels. |
| `bar_spacing` | `f32` | `9.0` | Vertical gap between consecutive stat bars in screen pixels. |

**Pipeline events** emitted by `Action::ToggleOwnNameplate`:

| Event | Trigger |
|-------|---------|
| `nameplate.own_shown` | `ToggleOwnNameplate` transitions the player's own nameplate to shown |
| `nameplate.own_hidden` | `ToggleOwnNameplate` transitions the player's own nameplate to hidden |

### `NameplateBarDef`

_Used in: `NameplateOptionsDef.stat_bars`_

Each entry in `stat_bars` defines one pixel bar row in the nameplate widget.

| Field | Type | Default | Description |
|---|---|---|---|
| `stat_key` | `String` | required | The stat to track. Supports `{self}` substitution — e.g. `"{self}.health"` becomes `"orc_01.health"` at spawn time for entity `orc_01`. If the entity does not have this stat in its `StatMap`, the bar is silently omitted. |
| `fill_color` | `(f32, f32, f32, f32)` | required | RGBA colour of the filled portion of the bar (sRGB). |
| `bg_color` | `(f32, f32, f32, f32)` | required | RGBA colour of the unfilled background track of the bar (sRGB). |

**`NameplateFactionFilter` variants:**

| Variant | Which entities show a nameplate |
|---|---|
| `HostileOnly` *(default)* | Entities with NPC AI (`NpcAgent` component). Note: this matches any entity with an `npc:` block, including friendly NPCs — the filter name refers to engine-side AI presence, not in-world faction. |
| `FriendlyOnly` | Entities without NPC AI — friendly characters, the player, and non-actor props. |
| `All` | Every entity that the scene marks for nameplates (subject to per-prefab overrides). |

> **Per-prefab override:** `PrefabDef.nameplate: true` always shows the nameplate regardless of faction filter (still hides beyond `max_distance`). `PrefabDef.nameplate: false` suppresses it regardless of scene settings. Both take precedence over the faction filter.

**Example:**
```ron
// In scenes/*.scene.ron
// Nameplates (and every other world label in this scene) shrink with camera distance.
// Set reference_distance somewhere inside your camera's actual zoom range — leaving this block
// out entirely (or using the 50.0 default) means scaling never engages on a normal orbit camera,
// and nearby entities' nameplates will crowd together when the camera zooms out.
label_depth_scale: (
    reference_distance: 8.0,   // full size at 8m camera distance and closer
    min_scale: 0.5,            // never smaller than half size (reached at 16m)
),
show_nameplates: true,
nameplate_options: (
    faction_filter: All,
    show_player_nameplate: false,  // player's own nameplate stays off (the genre-conventional default)
    max_distance: 25.0,
    offset: (0.0, 2.4, 0.0),
    name_font_size: 14.0,
    name_color: (0.95, 0.95, 0.95, 1.0),
    text_shadow: true,
    stat_bars: [
        ( stat_key: "{self}.health", fill_color: (0.20, 0.85, 0.20, 1.0), bg_color: (0.15, 0.15, 0.15, 0.80) ),
        ( stat_key: "{self}.mana",   fill_color: (0.20, 0.45, 0.90, 1.0), bg_color: (0.15, 0.15, 0.15, 0.80) ),
    ],
    bar_width: 100.0,
    bar_height: 6.0,
    bar_spacing: 9.0,
),

// In prefabs/prefabs.ron — enemy with a custom display name
"enemy_zombie": (
    kind: Actor,
    model: "zombie",
    display_name: "Zombie",   // shown in the nameplate name line
    // nameplate omitted — inherits from scene faction_filter
    ...
),

// Force this specific player prefab to always show its own nameplate, regardless of
// the scene's show_player_nameplate default (per-prefab override wins, same as any entity).
// Note: this will show a NAME ONLY, no bars — the scene's stat_bars above use "{self}.health"/
// "{self}.mana", which only resolve for entities with a matching per-entity stat_templates
// entry. This specific player prefab has none, so it falls back to global stats (e.g.
// player_health from stats/stats.ron), which are not entity-scoped and silently fail to match
// {self}.* — see the {self}.stat note below. Player prefabs CAN declare their own stat_templates
// (same field as any NPC/prop prefab) to make {self}.* resolve for them too — this also gives the
// player their own independent action-bar SlotCost pool, see "Per-player action bars" below.
"player_warrior": (
    kind: Actor,
    model: "hero",
    display_name: "Warrior",
    nameplate: true,           // always show even if show_player_nameplate: false
    ...
),

// Chest never shows a nameplate
"chest_01": (
    kind: Prop,
    model: "chest_01",
    nameplate: false,          // never show even if show_nameplates: true
    ...
),
```

> **Stat bar visibility:** bars for stats the entity does not have are silently skipped — no error is logged. For example, if `stat_bars` contains `"{self}.mana"` but a skeleton enemy has no mana stat, the skeleton only shows the health bar while mana-capable entities show both bars.
>
> **`{self}.stat` requires a per-entity `stat_templates` entry.** `{self}.health` resolves to `"spawn_id.health"` and is looked up in the entity's `StatMap`. Only entities that declare a `stat_templates` block with key `"health"` have this stat in their `StatMap`. Global stats defined in `stats/stats.ron` (such as `player_health` or `score`) are not entity-scoped and will never satisfy a `{self}.` stat key — they belong to the shared game-variable pool, not any individual entity's `StatMap`. If you want a nameplate bar on the player, add a `stat_templates` entry to the player's prefab (see [Instance stats](#instance-stats-stat_templates-) for the format) and update your logic to use `ModifyStat(key: "{self}.health", ...)` targeting the player's spawn ID — player prefabs now support `stat_templates` the same as any NPC/prop prefab (this also gives the player their own independent action-bar `SlotCost` pool, see "Per-player action bars" below).

> **Coexistence with `world_stat_bar`:** an entity can have both a nameplate (scene-managed, distance-culled) and a `world_stat_bar` (always visible, per-prefab). If the overlap is visually undesirable, remove `world_stat_bar` from the prefab and use the nameplate's `stat_bars` alone. To stack them cleanly instead, give the bar the same `offset` as `nameplate_options.offset` and separate them with `screen_offset` — see [Label depth scaling](#label-depth-scaling-labeldepthscaledef)'s stacking note.

### Terrain (`TerrainConfigV2`)

| Field | Type | Description |
|-------|------|-------------|
| `heightmap` | `String` | Path to greyscale PNG heightmap |
| `splatmap` | `String` | Path to RGBA splatmap (one channel per layer) |
| `scale` | `(f32, f32, f32)` | `(horizontal, max_height, horizontal_z)` — world units per heightmap pixel (X/Z) and max terrain elevation in world units (Y). E.g. `(5.0, 30.0, 5.0)` with a 128×128 heightmap gives a ~635×635 unit terrain with 30 units of elevation. |
| `position` | `Option<(f32,f32,f32)>` | World-space offset for the entire terrain mesh. Defaults to `(0, 0, 0)`. Set a negative Y to sink the terrain so player spawn points sit above the surface. |
| `material_paths` | `Vec<String>` | Texture paths for up to 4 terrain layers |
| `chunk_size` | `u32` | Mesh chunk size in vertices (default `64`) |
| `uv_scale` | `f32` | UV tiling scale for layer textures. Higher values tile textures more finely. Defaults to `10.0`. |

Terrain generation runs on `AsyncComputeTaskPool` — do not block the main thread.

### Heightmap files

Heightmaps live at `projects/{name}/terrain/` and consist of two files:

| File | Purpose |
|------|---------|
| `heightmap.png` | Greyscale PNG — white = maximum elevation, black = sea level. The `scale.y` value in `TerrainConfigV2` maps the full white-to-black range to world units. Any image editor can produce one. |
| `heightmap.json` | Generation manifest written by `tools/texture_gen/generate.py`. The engine does **not** read this file — it is only used by the tool to regenerate the heightmap with tweaked parameters. Do not edit it by hand. |

**Generating a heightmap with the texture tool:**
```bash
python tools/texture_gen/generate.py --project my_game --type fbm --size 128 --seed 42
```
Run `python tools/texture_gen/generate.py --help` for all options. `tools/texture_gen/CLAUDE.md` describes every noise type and parameter.

**Using a hand-painted heightmap:** copy your greyscale PNG to `terrain/heightmap.png` and create a minimal manifest next to it so the tool knows where the file lives:
```json
{ "type": "custom", "output": "assets/projects/my_game/terrain/heightmap.png" }
```
The engine only reads the PNG — the JSON is for tooling only.

### UI Elements (`UiNodeDef`) ✅

UI elements are rendered by Bevy UI inside the WebGPU canvas. They are **not** DOM elements — clicks in browser automation must use canvas pixel coordinates.

Each element is a typed RON enum variant. Typos in field names fail at parse time with a clear error message.

#### `Button((...))`

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `id` | `String` | required | Unique identifier within the scene |
| `text` | `String` | required | Button label text |
| `action` | `String` | `""` | Trigger string; `"ui."` prefix is stripped (e.g. `"ui.dance"` → `"dance"`) |
| `position` | `(f32, f32)` | `(0,0)` | Top-left corner in pixels. Ignored in panel mode unless `absolute: true`. |
| `size` | `(f32, f32)` | `(120.0, 32.0)` | Width and height in pixels |
| `color` | `(f32,f32,f32,f32)` | `(0.15,0.15,0.15,1)` | Background colour as sRGB RGBA |
| `align` | `UiTextAlign` | `Center` | Text alignment: `Left`, `Center`, `Right` |
| `absolute` | `bool` | `false` | In panel mode: position absolutely relative to panel top-left |

> **Text always renders at a fixed 26px font size — there is no `font_size` field.** Long `text` that doesn't fit inside `size` overflows the button box rather than wrapping or shrinking to fit (a real bug caught via playtest — see `3rd_person_game_demo`'s `toggle_nameplate_button` for an example of working around it by shortening the text). Roughly 11-12 average-width characters fit comfortably in a 200px-wide button at this font size; widen `size` or shorten `text` for anything longer.

#### `IconButton((...))`

An icon-only button that swaps between two catalog textures depending on a bound `GameVariables` key, and fires the same click pipeline as `Button` (`UiEvent::ButtonPressed` → `action` trigger). The button itself has no background/border — only the icon (and optional drop-shadow copy) is visible. Internally it spawns a clickable root entity (hit-test surface, no image) with one or two `ImageNode` children: an optional shadow (behind) and the foreground icon (on top).

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `id` | `String` | required | Unique identifier within the scene |
| `action` | `String` | `""` | Trigger string; `"ui."` prefix is stripped (e.g. `"ui.toggle_mute"` → `"toggle_mute"`) |
| `icon_on` | `String` | required | Asset catalog texture key shown when `bind` resolves to `"true"` |
| `icon_off` | `String` | required | Asset catalog texture key shown when `bind` resolves to anything else, including when the key is missing from `GameVariables` |
| `bind` | `String` | required | `GameVariables` key holding `"true"`/`"false"`. Re-checked every frame. |
| `position` | `(f32, f32)` | `(0,0)` | Top-left corner in pixels. Ignored in panel mode unless `absolute: true`. |
| `size` | `(f32, f32)` | `(36.0, 36.0)` | Width and height in pixels |
| `absolute` | `bool` | `false` | In panel mode: position absolutely relative to panel top-left |
| `icon_color` | `Option<(f32,f32,f32,f32)>` | `None` | RGBA replacement color for the icon's resting state while `bind` is `"false"` (same convention as `ActionSlotDef.icon_color` — a multiply tint, so it works cleanly on white/greyscale source art). Omit to render the icon as-is (unmodified white). |
| `active_color` | `Option<(f32,f32,f32,f32)>` | `None` | RGBA replacement color for the icon's resting state while `bind` is `"true"` (i.e. `icon_on` is showing). Falls back to `icon_color` (or as-is) when unset — active/inactive look identical by default. |
| `hover_color` | `Option<(f32,f32,f32,f32)>` | `None` | RGBA replacement color while the cursor hovers the button (not pressed). Falls back to `icon_color` (or as-is) when unset — no hover feedback by default. |
| `click_color` | `Option<(f32,f32,f32,f32)>` | `None` | RGBA replacement color while the button is pressed. Falls back to `icon_color` (or as-is) when unset — no click feedback by default. |
| `shadow_offset` | `(f32, f32)` | `(-2.0, 2.0)` | Pixel offset `(dx, dy)` of the drop-shadow copy relative to the icon. Positive `dx` shifts right, positive `dy` shifts down. Only used when `shadow_color` is set. |
| `shadow_color` | `Option<(f32,f32,f32,f32)>` | `None` | RGBA replacement color for a drop-shadow copy of the icon, rendered behind the main icon. Omit to disable the shadow entirely (no shadow entity is spawned). The shadow always shows whichever of `icon_on`/`icon_off` the foreground is currently showing, but never reacts to hover/click. |

`bind` is not audio-specific — any bool-shaped `GameVariable` (`"true"`/`"false"`) works, so the same node can drive a settings-gear icon, a notification-bell badge, or any other two-state toggle.

```ron
// Mute/unmute toggle — see assets/projects/3rd_person_game_demo/scenes/main.scene.ron
IconButton((
  id: "hud_audio_toggle",
  action: "ui.toggle_mute",
  icon_on: "ui/audio_on",
  icon_off: "ui/audio_off",
  bind: "audio_muted",
  position: (976.0, 26.0),
  size: (36.0, 36.0),
  icon_color: (0.90, 0.75, 0.40, 1.0),   // warm brass, matches the desert HUD palette
  active_color: (0.75, 0.40, 0.30, 1.0), // dusty terracotta while muted, draws attention
  hover_color: (1.0, 0.90, 0.65, 1.0),   // lighter brass, subtle hover feedback
  click_color: (1.0, 0.60, 0.15, 1.0),   // brighter amber flash while held
  shadow_offset: (-2.0, 2.0),            // left + down
  shadow_color: (0.75, 0.75, 0.75, 0.55), // light grey, semi-transparent
)),
```

The `bind` variable is kept in sync by rules in `logic/state_machine.ron` that listen for `audio.muted` / `audio.unmuted` events and call `SetVariable("audio_muted", "true"|"false")`.

#### `Label((...))`

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `id` | `String` | required | Unique identifier within the scene |
| `text` | `String` | `""` | Static display text (overridden at runtime when `bind` is set) |
| `position` | `(f32, f32)` | `(0,0)` | Top-left corner in pixels. Ignored in panel mode unless `absolute: true`. |
| `size` | `(f32, f32)` | `(120.0, 32.0)` | Width and height in pixels |
| `align` | `UiTextAlign` | `Center` | Text alignment: `Left`, `Center`, `Right` |
| `bind` | `Option<String>` | `None` | `GameVariables` key — when set, label text is replaced each frame with the variable's value |
| `format` | `Option<String>` | `None` | Template for `bind`; `"{}"` is replaced by the value (e.g. `"Score: {}"`). Raw value used when omitted. |
| `absolute` | `bool` | `false` | In panel mode: position absolutely relative to panel top-left |

**GameVariables auto-written by capabilities** (bind a `Label` to these — no rule wiring needed):

| Key | Written by | Value |
|-----|-----------|-------|
| `target_display` | targeting | `"<prefab> <id>"` of the current target (e.g. `"enemy_orc_melee orc_01"`); empty string when no target |
| `target_name` | targeting | prefab catalog key of the current target (e.g. `"enemy_orc_melee"`) |
| `target_id` | targeting | spawn id of the current target (e.g. `"orc_01"`) |
| `score` | action executor | running score, derived from `IncrementVariable("score", …)` |

The targeting variables update on every selection change (click, Tab, or `SetTarget`) and blank on clear/`LoadScene`. Example: `Label((id: "target_label", bind: "target_display", format: "Target: {}"))` — see `assets/projects/3rd_person_game_demo`.

> **2+ players present (including party mode):** these three variables go blank whenever 2+
> players are present, rather than reflecting only one player's target with no indication a
> second player's selection isn't shown. Use the per-viewport `target_hud:` block instead for a
> **split-screen** scene's target readout — party mode has no readout replacement today (no
> `SplitViewportSlot` camera for `target_hud:` to attach to). See
> [Per-player split-screen targeting](#per-player-split-screen-targeting) above.

#### `Rect((...))`

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `id` | `String` | required | Unique identifier within the scene |
| `position` | `(f32, f32)` | `(0,0)` | Top-left corner in pixels. Ignored in panel mode unless `absolute: true`. |
| `size` | `(f32, f32)` | `(120.0, 32.0)` | Width and height in pixels |
| `color` | `(f32,f32,f32,f32)` | `(0.15,0.15,0.15,1)` | Fill colour as sRGB RGBA |
| `absolute` | `bool` | `false` | In panel mode: position absolutely relative to panel top-left |

Click coordinates for browser tests: **center = `(position.x + size.w/2, position.y + size.h/2)`**.

#### `StatBar((...))` ✅

A bar that fills proportionally to `current / max` of a named stat from `LoadedStats`. Updates automatically every frame — no event wiring or `GameVariables` binding needed.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `id` | `String` | required | Unique identifier within the scene |
| `stat_key` | `String` | required | Key of the stat to display (must match a key in `stats.ron`) |
| `orientation` | `BarOrientation` | `Horizontal` | `Horizontal` (left→right) or `Vertical` (bottom→top) |
| `position` | `(f32, f32)` | `(0,0)` | Top-left corner in pixels. Ignored in panel mode unless `absolute: true`. |
| `size` | `(f32, f32)` | `(200.0, 20.0)` | Width and height in pixels |
| `fill_color` | `(f32,f32,f32,f32)` | red | Colour of the filled portion as sRGB RGBA |
| `background_color` | `(f32,f32,f32,f32)` | dark red | Colour of the unfilled portion |
| `show_value` | `bool` | `false` | Overlay `"current / max"` text centred on the bar |
| `color_bands` | `Vec<ColorBand>` | `[]` | Threshold-based colour overrides. Each band: `( above_percent: f32, color: (r,g,b,a) )`. The highest `above_percent` ≤ current fill ratio is selected. |
| `absolute` | `bool` | `false` | In panel mode: position absolutely relative to panel top-left |

```ron
StatBar((
  id: "health_bar",
  stat_key: "player_health",
  position: (16.0, 60.0),
  size: (200.0, 18.0),
  fill_color:       (0.85, 0.15, 0.15, 1.0),
  background_color: (0.20, 0.06, 0.06, 1.0),
  show_value: true,
  color_bands: [
    ( above_percent: 0.5,  color: (0.85, 0.15, 0.15, 1.0) ),  // red   (normal)
    ( above_percent: 0.25, color: (1.0,  0.55, 0.0,  1.0) ),  // orange (low)
    ( above_percent: 0.0,  color: (0.6,  0.0,  0.0,  1.0) ),  // dark red (critical)
  ],
  absolute: true,
)),
```

**Stat not found:** If `stat_key` is not present in `LoadedStats`, the bar renders as empty (0 % fill). A warning is logged in debug builds. No panic occurs.

#### `StatSpread((...))` ✅

A panel that lists multiple stats as labelled minibar rows. Each row shows the stat name, a minibar fill, and optionally the numeric value.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `id` | `String` | required | Unique identifier within the scene |
| `stats` | `Vec<String>` | required | Ordered list of stat keys to display |
| `layout` | `StatSpreadLayout` | `Rows` | `Rows` (one row per stat) |
| `position` | `(f32, f32)` | `(0,0)` | Top-left corner in pixels. Ignored in panel mode unless `absolute: true`. |
| `label_width` | `f32` | `80.0` | Width of the stat-name label column in pixels |
| `bar_width` | `f32` | `120.0` | Width of the minibar column in pixels |
| `row_height` | `f32` | `22.0` | Height of each row in pixels |
| `row_gap` | `f32` | `4.0` | Vertical gap between rows in pixels |
| `label_color` | `(f32,f32,f32,f32)` | near-white | Colour of the stat-name and value text |
| `bar_fill_color` | `(f32,f32,f32,f32)` | blue | Minibar fill colour |
| `bar_background_color` | `(f32,f32,f32,f32)` | dark blue | Minibar background colour |
| `show_values` | `bool` | `true` | Show `"current / max"` text after each minibar |
| `absolute` | `bool` | `false` | In panel mode: position absolutely relative to panel top-left |

```ron
StatSpread((
  id: "stat_panel",
  stats: ["player_health", "player_mana", "player_stamina"],
  position: (16.0, 88.0),
  label_width: 110.0,
  bar_width: 160.0,
  row_height: 24.0,
  row_gap: 5.0,
  bar_fill_color:       (0.35, 0.75, 0.35, 1.0),
  bar_background_color: (0.08, 0.18, 0.08, 1.0),
  show_values: true,
  absolute: true,
)),
```

#### `ActionBar((...))` ✅

A row of skill slots, each bound to a keyboard key and, optionally, a gamepad button (see `gamepad_key` below). Pressing a slot's bound key or button fires its `do_actions` through the existing `Action` pipeline. Slots show a cooldown fill overlay while on cooldown and dim when the cost stat is insufficient. Always positioned absolutely. **No mouse-click binding** — a designer-clicked slot button does nothing; only the bound key/button fires it. `owner_player` (below) controls the *acting* player (target/cost resolution) for a slot, not *which device* may press it: `key` is genuinely shared keyboard hardware, so any player's keyboard can press it regardless of `owner_player` — this has always been true and isn't new. `gamepad_key` is different: it only fires from the *owning* player's own controller (`owner_player` → that player's own controller), since a gamepad is not shared hardware the same way. This makes the realistic "one keyboard player + one gamepad player" split-screen pairing fully usable — see the worked example under `gamepad_key` below.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `id` | `String` | required | Unique identifier |
| `owner_player` | `Option<u32>` | `None` | Which player this bar belongs to — set to the same value as that player's `player_index` field (`PrefabDef.player_index`, see `PrefabDef`'s field table). `None` (default): resolves against the primary player (`player_index: 0` or the field omitted entirely) — unchanged single-bar behavior. `Some(n)`: this bar's slots act on whichever player prefab has `player_index: n` — a split-screen scene authors one `ActionBar` per player, each positioned in that player's own half, each `owner_player` matching that player's `player_index`. A slot whose `owner_player` matches no player present in the scene never fires. Edge cases and the full RON example are under "Per-player action bars" below |
| `position` | `(f32, f32)` | `(0.0, 0.0)` | Top-left corner in pixels (always absolute) |
| `slot_size` | `f32` | `64.0` | Width and height of each slot square in pixels |
| `slot_gap` | `f32` | `4.0` | Pixel gap between slots |
| `background_color` | `(f32,f32,f32,f32)` | near-black 70 % | Bar container background as sRGB RGBA |
| `icon_sheet` | `Option<String>` | `None` | Catalog texture key for a shared icon atlas (4×4 grid by default) |
| `icon_cols` | `u32` | `4` | Columns in the icon atlas grid |
| `icon_rows` | `u32` | `4` | Rows in the icon atlas grid |
| `icon_cell_size` | `u32` | `64` | Pixel size of each square atlas cell |
| `slots` | `Vec<ActionSlotDef>` | required | Ordered list of slot definitions |

**`ActionSlotDef` fields:**

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `key` | `String` | required | Key that activates the slot — see "Accepted key names" below. Also the slot's identity: cooldown tracking and every emitted `action_bar.*:{key}` event use this string verbatim, so rebinding a slot (changing `key`) also renames its event contract — update any `rules.ron`/`state_machine.ron` wired to the old key string. Stays keyboard-only and required even for a gamepad-routed slot — see `gamepad_key` below |
| `gamepad_key` | `Option<String>` | `None` | Gamepad button that **also** activates this slot, in addition to `key` — see "Valid gamepad button names" in the `InputMap` section below. Resolved against the slot's **owning player's own** controller (`owner_player` → that player's own controller), never any connected pad — a gamepad is not shared hardware the way a keyboard is. An unrecognised name warns at scene load (bar + slot named) and an `ironhold_cli validate` error; the slot's `key` binding, if any, still works. Omit for a keyboard-only slot (default) |
| `icon` | `String` | `""` | Per-slot texture catalog key override (overrides `icon_sheet` for this slot) |
| `icon_index` | `u32` | `0` | Zero-based atlas cell (row-major). `icon_sheet` on the bar must be set |
| `icon_color` | `Option<(f32,f32,f32,f32)>` | `None` | sRGB RGBA multiplicative tint for the icon. White pixels show the exact specified color; dark pixels stay dark. Omit to render the icon untinted (see note below) |
| `do_actions` | `Vec<Action>` | required | Actions fired through the pipeline on activation |
| `cooldown_secs` | `Option<f32>` | `None` | Seconds before the slot can activate again |
| `cost` | `Option<SlotCost>` | `None` | Stat cost checked and deducted at activation time |
| `label` | `Option<String>` | `None` | Ability/tooltip name (e.g. `"Heavy Strike"`) — **reserved for a future hover tooltip, not yet rendered anywhere.** Does **not** affect the on-screen corner glyph — see `key_hint` |
| `key_hint` | `Option<String>` | `None` | Overrides the on-screen corner key glyph. Omit to pretty-print `key` (strips the `"Key"` prefix, so `"KeyQ"` → `"Q"`; digits and `"F2"`-style names render as-is — but modifier/arrow keys render their full raw name, e.g. `"ShiftLeft"`/`"ArrowUp"`, since only the `"Key"` prefix is stripped; set `key_hint` to a short glyph for those). Distinct from `label` — set both when you want a named ability with a custom glyph |

**Accepted key names** (`key` / any `parse_key`-recognised string): digits `"0"`-`"9"`; numpad digits `"Numpad0"`-`"Numpad9"`; bare letters (`"q"`, `"Q"`, case-insensitive) or `"KeyQ"`-style names; function keys `"F1"`-`"F12"`; `"Space"`, `"Escape"`, `"Tab"`, `"Enter"`, `"Backspace"`, `"Delete"`; arrow keys `"ArrowUp"`/`"ArrowDown"`/`"ArrowLeft"`/`"ArrowRight"`; modifier keys `"ShiftLeft"`/`"ShiftRight"`/`"ControlLeft"`/`"ControlRight"`/`"AltLeft"`/`"AltRight"`. **Not supported** (the slot renders but never fires from keyboard — a `warn!` at scene load and an `ironhold_cli validate` error both flag this): mouse buttons, modifier chords (e.g. `"Shift+1"`). Gamepad buttons are a separate field — see `gamepad_key` above, not `key`. Two slots in the same bar resolving to the same key is also flagged (both the runtime `warn!` and `validate`) — the first-listed slot fires, the other never does. This check is **scene-wide, not just within one bar** — two *different* action bars sharing a key are also flagged, since the intent/cooldown pipeline is keyed by `key` alone across the whole scene (see `error_type: "cross_bar_duplicate_key"`). `gamepad_key` collisions are checked separately, scoped per-player (see the worked example below) — a shared `gamepad_key` across two *different* players' bars is not a collision, since each player has their own physical pad.

> **Icon colors are sRGB** — author values the same way you would in an image editor or CSS.
> `(0.85, 0.15, 0.15, 1.0)` renders as the red you expect; no gamma conversion needed.
>
> The tint is **multiplicative**: the icon's pixel RGB is multiplied by `icon_color`.
> For white-on-transparent icons (the most common atlas style), white × color = exact color —
> so the specified value is exactly what appears. Dark outlines stay dark; the icon's shading is preserved.
> For icons with non-white art, those pixels are tinted proportionally.

**`SlotCost` fields:**

| Field | Type | Description |
|-------|------|-------------|
| `stat` | `String` | Key of the stat to check and deduct from — either a key in `stats.ron` (shared, global) **or** a key in the owning player's own `stat_templates` (per-player, see below) |
| `amount` | `f32` | Amount to deduct. Slot blocks if `current < amount` |

> **Cost/resource gating is per-player when the owning player opts in, global otherwise.** `cost`
> resolves against the acting player's own `stat_templates`-backed pool first — the exact same
> `stat_templates` field NPCs already use to declare stats like `health` — and only falls back to
> the single shared `LoadedStats` resource when that player's prefab declares no matching
> `stat_templates` entry for this stat. **Give the player prefab its own pool by adding
> `stat_templates` to it** (same field, same syntax as any NPC prefab):
> ```ron
> // prefabs.ron
> "player_p1": (
>   kind: Actor,
>   model: "character_male",
>   player_index: 0,
>   stat_templates: [
>     ( key: "mana", base: 100.0, min: 0.0, max: 100.0, regen_rate: 5.0, regen_delay: 1.0 ),
>   ],
>   components: ( tags: ["player"], /* ... */ ),
> ),
> ```
> ```ron
> // scene.ron — this bar's cost now resolves against player_p1's own "mana" pool above
> ActionBar((
>   id: "action_bar_p1",
>   owner_player: 0,
>   slots: [ ( key: "KeyG", cost: (stat: "mana", amount: 20.0), do_actions: [ /* ... */ ] ) ],
> )),
> ```
> **Omit the `stat_templates` block on this player's prefab and `cost` silently falls back to the
> shared global `LoadedStats` pool instead** — spending on one player's bar would then also dim/
> block another player's bar referencing the same stat key, since both would read the same global
> stat. `ironhold_cli validate` and a scene-load `warn!` both catch the case where a player
> declares *some* `stat_templates` but not the specific key a `cost:` slot references (a likely
> authoring mistake, not an intentional shared-pool choice) — but declaring **no** `stat_templates`
> at all is the ordinary, silent fallback and is never flagged, since that's simply "this player
> doesn't have their own economy," not a mistake.

> **Gamepad-routed action bar slots.** A slot fires on **either** device, but the two resolve
> differently. Keyboard is genuinely shared hardware: `key` fires from the one global keyboard
> input regardless of which player is "supposed" to press it — unchanged, pre-existing behavior.
> A gamepad is not shared the same way, so `gamepad_key` only fires from the slot's **owning
> player's own** controller (`owner_player` → that player's own controller), never any other
> connected pad. Bind both on the same slot to let a mixed keyboard+gamepad local-coop pairing use
> either device. `local_coop_demo`'s `scenes/room3.scene.ron` has both halves live and working —
> copy the pairing from `action_bar_p1`/`action_bar_p2` there and `player_p1_split`/
> `player_p2_split` in `prefabs/prefabs.ron`:
> ```ron
> // prefabs.ron — the P2 player (player_index: 1) reads from gamepad slot 1 (pad numbering is
> // independent of player_index — with a single controller attached, use gamepad_index: 0 instead)
> "player_p2": (
>   kind: Actor,
>   model: "character_female",
>   player_index: 1,
>   components: ( tags: ["player"], inputs: ( /* ...movement keys... */ gamepad_index: 1 ), /* ... */ ),
> ),
> ```
> ```ron
> // scene.ron
> ActionBar((
>   id: "action_bar_p2",
>   owner_player: 1,
>   slots: [
>     ( key: "KeyL", gamepad_key: "RightTrigger", do_actions: [ /* ... */ ] ),
>   ],
> )),
> ```
> Pressing `L` on the keyboard **or** the right trigger (Xbox RB / PlayStation R1) on player 1's
> own gamepad fires this slot; player 0 pressing the same button on a *different* pad never does.
> Pick a button the owning player's own `InputMap` isn't already using — `gamepad_jump`/
> `gamepad_run`/`gamepad_interact`/`gamepad_target_next` default to South/East/West/North, and a
> slot sharing one of those fires the ability **and** the movement action on a single press; this
> overlap is not currently flagged by either the scene-load warning or `ironhold validate`.
>
> **Both halves of the pairing are required.** A `gamepad_key` on a player whose prefab sets no
> `gamepad_index` at all is silently inert — no crash, no console message, the slot simply never
> fires from gamepad (its `key` binding, if any, still works). This one **is** flagged: a scene-load
> warning in the browser console, and an `ironhold_cli validate` error
> (`gamepad_key_without_gamepad_index`).
>
> `gamepad_key` has no auto-derived on-screen glyph — `key_hint` has no gamepad-button mode, so a
> gamepad-routed slot's corner glyph is still whatever `key`/`key_hint` resolves to (or author
> `key_hint` manually with a short label like `"RT"` if you want to hint the gamepad binding
> instead — avoid circled-letter glyphs like `"Ⓐ"`; the engine's default UI font likely doesn't
> cover that Unicode range and the glyph would render blank). This is a known limitation, not an
> oversight.
>
> Gamepad activations emit the identical events as keyboard ones — every `action_bar.*`/
> `intent.slot.*` event name always uses the slot's keyboard `key`, never `gamepad_key`, regardless
> of which device fired it (see the events table below).
>
> **Collision checking example** — two different players' bars sharing a `gamepad_key` is fine
> (each has their own pad); the *same* player binding it twice is a real double-fire risk. Both are
> flagged in the browser console at scene load and by `ironhold_cli validate`:
> ```ron
> // Fine — different players, different physical pads. No warning, no validate error.
> ActionBar((id: "bar_p1", owner_player: 0, slots: [(key: "KeyG", gamepad_key: "RightTrigger", do_actions: [/*...*/])])),
> ActionBar((id: "bar_p2", owner_player: 1, slots: [(key: "KeyL", gamepad_key: "RightTrigger", do_actions: [/*...*/])])),
> ```
> ```ron
> // Error — same player (owner_player: 0 twice), same button: one press fires both slots.
> ActionBar((id: "bar_a", owner_player: 0, slots: [(key: "KeyG", gamepad_key: "RightTrigger", do_actions: [/*...*/])])),
> ActionBar((id: "bar_b", owner_player: 0, slots: [(key: "KeyH", gamepad_key: "RightTrigger", do_actions: [/*...*/])])),
> ```

**Pipeline events emitted by the action bar:**

| Event | When fired |
|-------|-----------|
| `intent.slot.{key}:{entity}` | Before the slot's `do_actions` are committed — allows rules to intercept, redirect, or suppress the ability |
| `action_bar.pressed:{key}` | Key pressed and passed all gate checks — fires even when a rule later cancels the intent; use for unconditional UI/telemetry |
| `action_bar.activated:{key}` | Slot `do_actions` committed (not suppressed); cooldown starts at the same time — use to react to confirmed ability execution. **Note:** rules on this event fire one frame after the slot's own `do_actions` (the event is emitted after the interpreter chain runs). |
| `action_bar.on_cooldown:{key}` | Key pressed while slot is on cooldown |
| `action_bar.insufficient_resource:{key}` | Key pressed but cost stat too low |
| `action_bar.no_target:{key}` | `{target}` used in `do_actions` but no target is selected |

**Intent event layer:** When a slot key is pressed and passes all checks (cooldown, cost, target), the action bar emits `intent.slot.{key}:{entity}` (e.g. `intent.slot.1:player_01`) before committing the slot's `do_actions`. If any rule in `rules.ron`, `state_machine.ron`, or a `.behavior.ron` file matches this event, its `do_actions` run **and the slot's built-in `do_actions` are suppressed — including the cooldown and `action_bar.activated` event**. If no rule matches, the slot's `do_actions` fire unchanged, the cooldown starts, and `activated` fires — so existing projects with no intent rules behave identically to before.

```ron
// Suppress slot 1 and show a "Silenced!" popup when the player is in the "silenced" state
( on: "intent.slot.1:player_01", when: "silenced", do_actions: [
    ShowFloatingText(entity: "player_01", text: "Silenced!"),
    // no damage action — intent is consumed with no effect
] )

// Redirect slot 1 to a rage-strike when the player is in "berserk" state
( on: "intent.slot.1:player_01", when: "berserk", do_actions: [
    PlayAnimation("rage_strike"),
    ModifyStat(key: "{target}.health", delta: -25.0),
    EmitEvent("combat.hit:player_01"),
] )

// No rule on intent.slot.1 → slot's own do_actions run as normal
```

> **In a split-screen scene with per-player `owner_player` bars, a rule that intercepts a
> non-primary player's slot intent (like the "rage-strike" example above) still resolves `{target}`
> against the **primary player's** target, not the firing player's own selection.** The slot's own
> built-in `do_actions` (bypassed when a rule handles the intent) do resolve per-owning-player — only
> a *rule's replacement* `do_actions` fall back to the global, primary-player-only `{target}`
> substitution the interpreter has always used. This is a documented scope boundary (see
> `planning/features/per_player_split_screen_targeting.md`), not an inconsistency to fix — designers
> writing an intent-override rule for a non-primary player's bar should avoid `{target}` in that
> rule's `do_actions`, or expect it to affect the primary player's target instead of that bar's own
> player.

**`{target}` substitution:** Any occurrence of `{target}` in a slot's `do_actions` (and in all rule / FSM `do_actions`) is replaced with the spawn ID of the entity in `CurrentTarget`. For action bar slots, if `CurrentTarget` is `None` the slot emits `action_bar.no_target:{key}` and does not fire. `CurrentTarget` is populated by the targeting system — set `click_selectable: true` or `targetable: true` on a `PrefabDef` to enable.

```ron
ActionBar((
  id: "skill_bar",
  position: (16.0, 580.0),
  slot_size: 64.0,
  background_color: (0.05, 0.05, 0.08, 0.85),
  icon_sheet: "icons_basic_skills",  // optional; 4x4 atlas, 64 px cells
  icon_cols: 4,
  icon_rows: 4,
  icon_cell_size: 64,
  slots: [
    (
      key: "1",
      icon_index: 0,
      icon_color: (0.3, 0.5, 1.0, 1.0),  // blue; omit to render icon as-is
      do_actions: [
        PlayAnimationOn(target: "player_01", clip: "heal"),
        SpawnEffect(key: "heal_burst", entity: "player_01"),
        ModifyStat(key: "player_health", delta: 30.0),
      ],
      cooldown_secs: 5.0,
      cost: (stat: "player_mana", amount: 15.0),
      label: "Heal",
    ),
    (
      key: "2",
      icon_index: 1,
      do_actions: [ ApplyModifier(modifier_key: "speed_boost") ],
      cooldown_secs: 12.0,
      cost: (stat: "player_mana", amount: 20.0),
    ),
    // Non-digit key — `label` (tooltip name, not yet rendered anywhere) and `key_hint`
    // (the corner glyph the player actually sees) are independent fields, shown together
    // here so the distinction is unambiguous.
    (
      key: "KeyE",
      icon_index: 2,
      label: "Dodge Roll",
      key_hint: "Dash",  // <- this is what renders in the slot's corner, not `label`
      do_actions: [ PlayAnimationOn(target: "player_01", clip: "roll") ],
      cooldown_secs: 2.0,
    ),
  ],
))
```

Wire feedback events in `rules.ron` or `state_machine.ron` to surface cooldown or low-mana messages:

```ron
( event: "action_bar.on_cooldown:1",           do_actions: [ SetVariable("status", "Skill on cooldown") ] ),
( event: "action_bar.insufficient_resource:1", do_actions: [ SetVariable("status", "Not enough mana") ] ),
```

**Per-player action bars (split-screen):** author one `ActionBar` block per player, each tagged
with `owner_player` and positioned in that player's own half of the screen. No new engine
duplication mechanism is involved — `position` is always absolute, so this is manual authoring
the same way a split-screen scene already authors two corner-label-adjacent UI elements per
player. Give each bar's slots disjoint `key`s — two bars sharing a key are flagged by both a
runtime `warn!` at scene load and an `ironhold_cli validate` error, since the intent/cooldown
pipeline (`CooldownMap`/`PendingIntentActions`/`HandledIntentSlots`) is keyed by the slot key
string alone, scene-wide — a collision silently suppresses the *other* bar's pending slot, not
just picks the wrong target. To let a gamepad player fire their own bar (the realistic "one
keyboard player + one gamepad player" pairing), add `gamepad_key` to their slots — see "Gamepad-
routed action bar slots" above.

```ron
ActionBar((
  id: "action_bar_p1",
  owner_player: 0,           // matches the player prefab with player_index: 0 (or omitted)
  position: (200.0, 560.0),  // left half of a vertical split
  slot_size: 56.0,
  slots: [
    (
      key: "KeyG",           // disjoint from action_bar_p2's key below
      key_hint: "P1",
      do_actions: [
        ModifyStat(key: "{target}.health", delta: -10.0),
        ShowDamagePopup(entity: "{target}", amount: -10.0),
      ],
    ),
  ],
)),
ActionBar((
  id: "action_bar_p2",
  owner_player: 1,           // matches the player prefab with player_index: 1
  position: (900.0, 560.0),  // right half of a vertical split
  slot_size: 56.0,
  slots: [
    (
      key: "KeyL",
      key_hint: "P2",
      do_actions: [
        ModifyStat(key: "{target}.health", delta: -10.0),
        ShowDamagePopup(entity: "{target}", amount: -10.0),
      ],
    ),
  ],
)),
```

Each bar's `{target}` resolves against **that bar's own player's** `PlayerTarget` (their own
Tab-cycle/click selection, independent of the other player's) — not the global `CurrentTarget` —
so player 1 pressing `G` only ever affects whichever entity player 1 has selected, regardless of
what player 2 currently has targeted. A `cost:`-gated slot resolves per-player too, as long as the
owning player's own prefab declares a matching `stat_templates` entry (see `SlotCost` above) —
otherwise it falls back to the shared global pool. See the rule-override caveat above for the one
thing that stays *not* per-player even with `owner_player` set.

#### `DialoguePanel((...))` ✅

A full-width conversation panel that displays NPC speaker name, body text, and dynamically spawned choice buttons. Always positioned absolutely. Hidden by default; shown when `StartDialogue` executes, hidden when `EndDialogue` fires or `LoadScene` occurs.

Each scene that needs NPC dialogue must include exactly one `DialoguePanel`. If multiple NPCs share a scene, they all use the same panel.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `id` | `String` | required | Unique identifier within the scene |
| `position` | `(f32, f32)` | `(0, 0)` | Top-left corner in pixels (always absolute) |
| `size` | `(f32, f32)` | `(1200.0, 200.0)` | Width and height in pixels |
| `background_color` | `(f32,f32,f32,f32)` | dark blue-black | Panel background as sRGB RGBA |
| `speaker_font_size` | `f32` | `18.0` | Font size for the speaker name label |
| `body_font_size` | `f32` | `15.0` | Font size for the body text |
| `choice_font_size` | `f32` | `13.0` | Font size for each choice button label |
| `initially_hidden` | `bool` | `true` | Whether the panel starts hidden. Should always be `true` in practice. |

```ron
DialoguePanel((
    id: "npc_dialogue_panel",
    position: (16.0, 440.0),
    size: (1220.0, 200.0),
    background_color: (0.04, 0.04, 0.07, 0.93),
    speaker_font_size: 18.0,
    body_font_size: 15.0,
    choice_font_size: 13.0,
)),
```

#### `InventoryPanel((...))` ✅

A grid of item slots that displays the player's `PlayerInventory`. Always positioned absolutely. Hidden by default; toggled by `ToggleInventory` or shown/hidden explicitly with `OpenInventory`/`CloseInventory`. Slot icons and count labels update automatically via change detection whenever `PlayerInventory` changes. Requires `items_path` to be set in `project.ron`.

When `icon_sheet` is set, each non-empty slot shows the icon at the item's `icon_index` (from `items.ron`); a small count label (`x3`) appears in the corner for stacks greater than 1.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `id` | `String` | required | Unique identifier within the scene |
| `position` | `(f32, f32)` | required | Top-left corner in pixels (always absolute) |
| `columns` | `u32` | `5` | Number of slot columns |
| `rows` | `u32` | `4` | Number of slot rows |
| `slot_size` | `f32` | `48.0` | Width and height of each slot in pixels |
| `slot_gap` | `f32` | `4.0` | Gap between slots in pixels |
| `background_color` | `(f32,f32,f32,f32)` | dark semi-transparent | Panel background as sRGB RGBA |
| `font_size` | `f32` | `11.0` | Font size for slot count labels |
| `icon_sheet` | `Option<String>` | `None` | Catalog texture key for the item icon atlas (default sheet; items can override per-item with `ItemDef.icon_sheet`) |
| `icon_cols` | `u32` | `8` | Columns in the icon atlas grid |
| `icon_rows` | `u32` | `8` | Rows in the icon atlas grid |
| `icon_cell_size` | `u32` | `64` | Pixel size of each square icon cell |
| `initially_hidden` | `bool` | `true` | Whether the panel starts hidden |

```ron
InventoryPanel((
    id: "player_inventory",
    position: (20.0, 100.0),
    columns: 5,
    rows: 4,
    slot_size: 52.0,
    slot_gap: 4.0,
    icon_sheet: "icons_items",   // 8×8 grid, 64 px cells — set in assets.ron
    icon_cols: 8,
    icon_rows: 8,
    icon_cell_size: 64,
)),
```

#### `ShopPanel((...))` ✅

A scrollable list of merchant stock entries. Always positioned absolutely. Hidden by default; shown by `OpenShop(merchant_id)` and hidden by `CloseShop`. Stock is repopulated from the merchant's `MerchantDef` every time `OpenShop` fires. Requires `items_path` to be set in `project.ron`.

> **v1 scope note:** The shop panel is display-only. It shows item names, prices, and stock counts but does not yet process buy/sell transactions.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `id` | `String` | required | Unique identifier within the scene |
| `position` | `(f32, f32)` | required | Top-left corner in pixels (always absolute) |
| `size` | `(f32, f32)` | `(320.0, 400.0)` | Width and height of the panel in pixels |
| `background_color` | `(f32,f32,f32,f32)` | dark semi-transparent | Panel background as sRGB RGBA |
| `font_size` | `f32` | `13.0` | Font size for stock entry labels |
| `initially_hidden` | `bool` | `true` | Whether the panel starts hidden |

```ron
ShopPanel((
    id: "shop_panel",
    position: (400.0, 100.0),
    size: (320.0, 400.0),
)),
```

> **Close button**: the ShopPanel now spawns its own close button as an embedded child (header row). No standalone `Button` is needed alongside the panel. The button fires `ui.button_pressed:close_shop` → `CloseShop`.

#### `ContainerPanel((...))` ✅

A slot grid that displays a container entity's `Inventory` (chest, crate, etc.). Always positioned absolutely. Hidden by default; shown by `OpenContainer(entity_id)` and hidden by `CloseContainer`. Includes an embedded close button and a "Take All" button. Requires `items_path` to be set in `project.ron`.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `id` | `String` | required | Unique identifier within the scene |
| `position` | `(f32, f32)` | required | Top-left corner in pixels (always absolute) |
| `columns` | `u32` | `3` | Slot grid columns |
| `rows` | `u32` | `3` | Slot grid rows |
| `slot_size` | `f32` | `52.0` | Size of each slot square in pixels |
| `slot_gap` | `f32` | `4.0` | Gap between slots in pixels |
| `background_color` | `(f32,f32,f32,f32)` | dark semi-transparent | Panel background as sRGB RGBA |
| `font_size` | `f32` | `11.0` | Font size for count labels inside slots |
| `icon_sheet` | `Option<String>` | `None` | Catalog key for the item icon atlas |
| `icon_cols` | `u32` | `8` | Columns in the icon atlas grid |
| `icon_rows` | `u32` | `8` | Rows in the icon atlas grid |
| `icon_cell_size` | `u32` | `64` | Pixel size of each icon cell |

```ron
ContainerPanel((
    id: "chest_panel",
    position: (230.0, 80.0),
    columns: 3,
    rows: 3,
    slot_size: 52.0,
    icon_sheet: "icons_items",
    icon_cols: 8,
    icon_rows: 8,
    icon_cell_size: 64,
)),
```

### UI Panel (`UiPanelDef`) ✅

When a scene includes a `ui_panel` block, all `ui` elements are arranged in a vertically-flowing centered panel instead of using absolute positioning. Elements with `absolute: true` are still positioned relative to the panel's top-left corner.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `background_color` | `(f32,f32,f32,f32)` | `(0.1,0.1,0.1,0.95)` | Background colour as sRGB RGBA (0.0–1.0) |
| `padding` | `f32` | `20.0` | Inner padding around panel contents in pixels |
| `gap` | `f32` | `12.0` | Vertical gap between child elements in pixels |
| `width` | `Option<f32>` | `None` | Fixed panel width in pixels; auto-sized when omitted |
| `height` | `Option<f32>` | `None` | Fixed panel height in pixels; auto-sized when omitted (required for panels with absolutely-positioned children such as maps) |

```ron
ui_panel: (
  background_color: (0.08, 0.08, 0.08, 0.95),
  padding: 24.0,
  gap: 14.0,
  width: 380.0,
),
```

---

## `assets.ron` — AssetCatalog ✅

Named registry of all assets available to prefabs and scenes.

```ron
(
  models: {
    "hero": ( path: "shared/models/character-01.glb#Scene0" ),
    "orc":  ( path: "shared/models/creatures/orc.glb#Scene0" ),
  },
  textures: {
    "grass": "shared/terrain/grass.png",
  },
  audio: {
    "click": (path: "shared/audio/menu-button-click.wav"),
    "bg_music": (path: "shared/audio/theme.ogg", volume: 0.6),
  },
  effects: {
    "hit_spark": (
      particle_count: 12,
      lifetime_secs: 0.45,
      speed: 3.5,
      speed_jitter: 0.8,
      spread_deg: 180.0,
      offset: (0.0, 1.0, 0.0),
      size: 0.055,
      size_end: Some(0.0),
      color_start: (1.0, 0.8, 0.2, 1.0),
      color_end: (1.0, 0.1, 0.0, 0.0),
      gravity: -5.0,
    ),
  },
  materials: {
    "wood_crate": (
      kind: Standard((
        base_color_texture: "shared/textures/wood_crate_albedo.png",
        metallic: 0.0,
        perceptual_roughness: 0.85,
      )),
      alpha_mode: Opaque,
      double_sided: false,
    ),
  },
)
```

**EffectDef fields** (used inside `effects: { "key": ( … ) }`):

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `particle_count` | `u32` | `12` | Number of particles per burst. Must be ≤ 256 (validated at catalog load). Unused when `layers` is non-empty. |
| `lifetime_secs` | `f32` | `1.0` | How long each particle lives. Provide a meaningful value for single-layer effects; unused when `layers` is non-empty. |
| `speed` | `f32` | `0.0` | Initial outward speed in metres/second along each particle's direction. |
| `speed_jitter` | `f32` | `0.0` | Per-particle speed variation in `[−jitter, +jitter]` — deterministic, index-based. |
| `spread_deg` | `f32` | `180.0` | Cone half-angle in degrees (0 = straight up column, 90 = hemisphere, 180 = full sphere). |
| `offset` | `(f32, f32, f32)` | `(0.0, 1.0, 0.0)` | World-space offset from the entity origin or explicit position (e.g. chest height). |
| `size` | `f32` | `0.06` | Particle radius in metres at birth. |
| `size_end` | `Option<f32>` | `None` | If set, particle radius lerps from `size` to `size_end` over the lifetime. |
| `size_jitter` | `f32` | `0.0` | Per-particle size variation in `[−jitter, +jitter]` at birth. |
| `color_start` | `(f32, f32, f32, f32)` | `(1,1,1,1)` | RGBA colour at birth (linear, 0.0–1.0). Provide a value for single-layer effects; unused when `layers` is non-empty. |
| `color_mid` | `Option<(f32,f32,f32,f32)>` | `None` | Optional midpoint colour for a three-stop gradient (start → mid → end). |
| `color_end` | `(f32, f32, f32, f32)` | `(1,1,1,0)` | RGBA colour at death. Provide a value for single-layer effects; unused when `layers` is non-empty. |
| `gravity` | `f32` | `0.0` | Vertical acceleration in m/s² (negative = falls, positive = rises). |
| `turbulence` | `f32` | `0.0` | Per-frame lateral noise strength; creates billowing and swirling motion. |
| `sprite` | `Option<String>` | `None` | Asset key for a sprite texture (from `AssetCatalog.textures`). When set, particles are camera-facing quads instead of sphere meshes. |
| `sprites` | `Vec<String>` | `[]` | Multiple sprite keys; each particle picks one by deterministic hash. Takes precedence over `sprite`. |
| `additive` | `bool` | `false` | `true` → `AlphaMode::Add` (fire, glow); `false` → `AlphaMode::Blend` (smoke). Has no effect without a sprite. |
| `uv_distort` | `f32` | `0.0` | UV distortion for the flame shader. Non-zero switches the particle to `PoolFlameMaterial` (animated WGSL). Range 0.0–1.0; typical: 0.4–0.6. |
| `uv_scroll_speed` | `f32` | `0.0` | Upward UV scroll in texture-heights per second. Combine with `uv_distort` for flowing flame. |
| `emit_radius` | `f32` | `0.0` | Disc scatter radius. Deprecated in favour of `emitter: Disc(radius: …)` — kept for backward compatibility. |
| `rotation_start_deg` | `f32` | `0.0` | Billboard quad rotation at spawn, in degrees. |
| `rotation_end_deg` | `f32` | `0.0` | Rotation at end of lifetime (lerped from `rotation_start_deg`). Ignored when `rotation_speed_deg` is non-zero. |
| `rotation_speed_deg` | `f32` | `0.0` | Constant angular velocity in degrees/second. When non-zero, overrides `rotation_start_deg` / `rotation_end_deg`. |
| `size_x` | `Option<f32>` | `None` | Independent billboard width in metres. Overrides `size` for the X axis. Use `size_x < size_y` for tall narrow shapes. |
| `size_y` | `Option<f32>` | `None` | Independent billboard height in metres. Overrides `size` for the Y axis. |
| `size_x_end` | `Option<f32>` | `None` | End-of-life billboard width. Falls back to `size_end` when not set. |
| `size_y_end` | `Option<f32>` | `None` | End-of-life billboard height. Falls back to `size_end` when not set. |
| `emitter` | `EmitterShape` | `Point` | Spawn-position distribution — see "Emitter shapes" section below. |
| `velocity_curve` | `VelocityCurve` | `Linear` | Speed envelope over lifetime — see "Velocity curves" section below. |
| `layers` | `Vec<LayerDef>` | `[]` | Multi-layer emitter list — see section below. When non-empty, all flat fields above are unused. |
| `light` | `Option<EffectLightDef>` | `None` | Dynamic point light spawned at the effect origin — see section below. |
| `priority` | `EffectPriority` | `Npc` | Budget shedding bucket. `Ambient` = dropped when full; `Npc` = halved (min 1); `Player` = always fires. See "Quality & Budget" below. |
| `quality` | `Option<QualityOverride>` | `None` | Explicit per-tier counts. Bypasses the global multiplier when set. `high` is optional — omit to use `particle_count` at High. Example: `quality: (minimal: 1, low: 3, medium: 6)`. |
| `flipbook` | `Option<FlipbookDef>` | `None` | Sprite-sheet animation — see "Flipbook animation" section below. Cannot be combined with `uv_distort > 0`. |

**Emitter shapes (`emitter`)**

Controls where particles are spawned relative to the effect origin. All shapes are deterministic (no RNG).

| Variant | Example RON | Description |
|---------|-------------|-------------|
| `Point` | `emitter: Point` | All particles at origin (with optional `emit_radius` disc). Default. |
| `Disc` | `emitter: Disc(radius: 0.5)` | Uniform disc — particles scattered across a horizontal circle. |
| `Ring` | `emitter: Ring(radius: 1.5)` | All particles evenly spaced around the ring circumference. |
| `Sphere` | `emitter: Sphere(radius: 0.3)` | Uniform sphere surface via Fibonacci point distribution. |
| `Line` | `emitter: Line(length: 2.0, axis: Y)` | Particles spaced along a segment. `axis` is `X`, `Y` (default), or `Z`. |
| `Arc` | `emitter: Arc(radius: 1.0, angle_deg: 120.0)` | Partial ring subtending `angle_deg` degrees, centred on the origin. |

```ron
// orbiting rune particles — Ring emitter + rotation_speed_deg, flat single-layer effect
// (emitter: and rotation_* work at the top level, not only inside layers:[])
"magic_orbit": (
    particle_count: 14,
    lifetime_secs: 1.8,
    speed: 0.4,
    speed_jitter: 0.10,
    spread_deg: 12.0,
    emitter: Ring(radius: 0.8),
    offset: (0.0, 0.4, 0.0),
    size: 0.16,
    size_end: 0.0,
    rotation_speed_deg: 180.0,
    color_start: (0.80, 0.55, 1.0, 1.0),
    color_end:   (0.25, 0.08, 0.70, 0.0),
    gravity: 0.08,
    turbulence: 0.3,
    sprite: "particle/magic_03",
    additive: true,
),
```

**Velocity curves (`velocity_curve`)**

Scales the per-frame position delta over the particle's lifetime. The stored velocity vector is unmodified; only the step size changes.

| Variant | Scale at t=0 | Scale at t=1 | Use for |
|---------|-------------|-------------|---------|
| `Linear` | 1.0 | 1.0 | Constant speed (default). |
| `EaseOut` | 1.0 | 0.0 | Fast burst that decelerates to a stop — impact shards, explosions. |
| `EaseIn` | 0.0 | 1.0 | Slow start that accelerates — rising energy, charge-up. |
| `Pulse` | 1.0 | 1.0 | Fast → slow → fast (trough at mid-life) — orbit-like bob. |

```ron
// explosion shards: fast burst that coasts to a stop
"explosion_burst": (
    particle_count: 60, lifetime_secs: 1.1, speed: 7.0, spread_deg: 180.0,
    velocity_curve: EaseOut,
    color_start: (1.0, 0.98, 0.55, 1.0), color_end: (0.55, 0.04, 0.0, 0.0),
),

// tall narrow ice shards that tumble as they fly
"frost_shard": (
    particle_count: 20, lifetime_secs: 1.2, speed: 4.0, spread_deg: 75.0,
    size_x: 0.07, size_y: 0.30, size_y_end: 0.0,
    velocity_curve: EaseOut,
    rotation_speed_deg: 270.0,
    color_start: (0.85, 0.95, 1.0, 1.0), color_end: (0.25, 0.55, 1.0, 0.0),
    sprite: "particle/trace_01", additive: true,
),
```

**Field interactions**

These rules apply to both flat `EffectDef` fields and fields inside a `LayerDef` entry.

| Rule | Detail |
|------|--------|
| `rotation_speed_deg` overrides start/end | When non-zero, `rotation_start_deg` and `rotation_end_deg` are silently ignored. Only `rotation_speed_deg` takes effect. |
| `size_x` / `size_y` override `size` per axis | `size_x` replaces `size` for the X axis only; `size_y` replaces `size` for Y only. Unset axes still use the uniform `size`. |
| `size_x_end` / `size_y_end` fall back to `size_end` | If `size_x_end` is omitted it falls back to `size_end`; if that is also omitted the axis holds constant at its birth size. Same rule applies to `size_y_end`. |
| Non-`Point` emitter overrides `emit_radius` | When `emitter` is `Disc`, `Ring`, `Sphere`, `Line`, or `Arc`, `emit_radius` is ignored. Only `emitter: Point` (the default) honours `emit_radius` as a legacy disc-scatter fallback. |
| `layers:` makes all flat fields unused | When `layers` is non-empty, every field at the `EffectDef` level is ignored **except `light`**. Each `LayerDef` entry carries its own complete set of fields. |

**Dynamic effect lights (`light`)**

When `light` is set, a temporary `PointLight` is spawned at the effect origin the moment the effect fires. It fades in over `fade_in_secs`, holds at `intensity`, then fades out over `fade_out_secs` and despawns automatically. `LevelEntity` ensures cleanup on scene transitions.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `color` | `(f32, f32, f32)` | — | RGB light colour (linear, 0.0–1.0). |
| `intensity` | `f32` | — | Peak luminous power in lumens. 4000 ≈ torch, 8000 ≈ campfire, 30000 ≈ explosion. |
| `range` | `f32` | — | Radius of influence in metres. |
| `fade_in_secs` | `f32` | — | Seconds to reach full intensity. Use `0.0` for an instant flash. |
| `fade_out_secs` | `f32` | — | Seconds to fade out before despawn. |
| `duration_secs` | `Option<f32>` | `None` | Total lifetime. When omitted, defaults to the longest layer lifetime (or `lifetime_secs` for single-layer effects). |

```ron
"explosion_burst": (
    // ... particle fields ...
    light: (
        color: (1.0, 0.85, 0.40),
        intensity: 30000.0,
        range: 12.0,
        fade_in_secs: 0.0,
        fade_out_secs: 0.60,
    ),
),
```

The engine caps simultaneous fading lights at 16. When the cap is full, new light spawns are silently skipped — the particles still fire. This keeps within WebGPU mobile cluster limits (~32 total lights including authored scene fixtures).

**Flipbook animation (`flipbook`)**

Animates a sprite sheet by selecting UV sub-rectangles from a single texture over the particle's lifetime. Each particle advances through frames independently based on its own `elapsed` time.

```ron
// In EffectDef or LayerDef:
"sheet_explosion": (
    particle_count: 8,
    lifetime_secs: 1.4,
    sprite: "particle/explosion_4x4",   // 512×512 sprite sheet, 4 cols × 4 rows
    additive: true,
    flipbook: (
        cols: 4,    // number of columns in the sheet
        rows: 4,    // number of rows
        fps: 12.0,  // frame rate; at 12 fps a 16-frame sheet plays in 1.33 s
        loop: false,  // false: hold last frame until lifetime_secs expires (default)
                      // true: loop continuously for the full lifetime
    ),
),
```

**Sheet authoring conventions:**
- Row order: top-to-bottom, left-to-right (standard Aseprite / Photoshop export).
- Power-of-two PNGs recommended (256×256, 512×512, 1024×1024).
- White-on-transparent; tint via `color_start`/`color_end` gradient as usual.
- Place shared sheets in `assets/shared/textures/particles/sheets/`; register in the project's `assets.ron` textures section.

**Constraint:** `flipbook` and `uv_distort > 0` cannot be combined on the same layer. `uv_distort` animates UVs in the flame shader; flipbook bakes UV sub-rects into vertex data — they are mutually exclusive. Validation raises an error at catalog load time.

**No new pipeline variant:** flipbook particles use the existing `Additive` or `Blend` group key (same `StandardMaterial` pipeline). The sprite sheet is just a texture, so no additional WebGPU pipeline warmup is needed beyond the standard pattern for any new texture.

**Quality tiers & particle budget**

The engine supports a global quality level and a per-scene live-particle cap to keep frame times stable across hardware.

*Quality level* — set via `Action::SetParticleQuality`:

| Level | Multiplier | RON example |
|-------|-----------|-------------|
| `High` | 1.0× (default) | `SetParticleQuality(High)` |
| `Medium` | 0.75× | `SetParticleQuality(Medium)` |
| `Low` | 0.50× | `SetParticleQuality(Low)` |
| `Minimal` | 0.25× (min 1) | `SetParticleQuality(Minimal)` |

The multiplier is applied to each `particle_count` at spawn time. When `quality: (minimal: N, low: N, medium: N)` is set on an `EffectDef` or `LayerDef`, those explicit counts are used instead of the multiplier. The optional `high: N` field overrides the count at High quality too; when omitted, High uses `particle_count` directly.

```ron
// rules.ron — downgrade quality on scene load for mobile-class builds
( on: "scene.ready:main", do_actions: [ SetParticleQuality(Low) ] ),
```

`SetParticleQuality` persists across scene transitions (the `ParticleQuality` resource is never reset on `LoadScene`). Explicitly call `SetParticleQuality(High)` to restore full counts.

*Particle budget* — configured in the scene file:

```ron
// scene.ron — raise the cap for a particle-heavy boss arena
(
    schema_version: 2,
    particle_budget: 5000,
    entities: [ … ],
)
```

Default is 2000 when `particle_budget` is omitted. The cap is re-applied from the scene file on every scene load.

When the live count approaches `max_count`, effects are shed by `priority`:
- `Ambient` — silently skipped. Use for background fog, ambient embers, non-critical atmosphere.
- `Npc` (default) — particle count halved (minimum 1). The effect still fires with reduced density.
- `Player` — always fires at full count; may briefly exceed the budget.

> **Migration note**: effects defined without a `priority` field inherit `Npc` (the default). In a scene with a tight `particle_budget`, existing effects may now be halved when the budget is under pressure. If you observe effects being cut unexpectedly after adding `particle_budget` to a scene, add `priority: Player` to player-driven burst effects or `priority: Ambient` to background emitters.

```ron
// assets.ron
effects: {
    "campfire_smoke": (
        priority: Ambient,       // shed first when budget is full
        particle_count: 6, …
    ),
    "hit_spark": (
        priority: Player,        // always fires
        particle_count: 12, …
    ),
    // per-tier explicit counts override the global multiplier
    "ability_burst": (
        priority: Player,
        particle_count: 20,
        quality: ( minimal: 3, low: 8, medium: 14 ),
        …
    ),
}
```

**Multi-layer effects (`layers`)**

When `layers` is non-empty, each entry is spawned independently at the same origin. All flat fields on the parent `EffectDef` are ignored. Use this to compose complex effects — fire body + hot core, smoke + rising sparks — in a single catalog key and a single `SpawnEffect` call.

Each `LayerDef` accepts every field in the table above except `layers` itself. Fields work identically inside a layer. Example:

```ron
"campfire_fire": (
    layers: [
        // body — 4 large orange flame quads
        ( particle_count: 4, lifetime_secs: 1.00, spread_deg: 0.0, emit_radius: 0.16,
          offset: (0.0, 0.22, 0.0), size: 0.65, size_jitter: 0.08,
          color_start: (1.0, 0.52, 0.08, 0.0), color_mid: (1.0, 0.42, 0.05, 1.0),
          color_end: (0.55, 0.06, 0.0, 0.0),
          sprites: ["particle/flame_01", "particle/flame_02"], additive: true,
          uv_distort: 0.50, uv_scroll_speed: 0.55 ),
        // core — 2 bright white-hot quads
        ( particle_count: 2, lifetime_secs: 0.80, spread_deg: 0.0, emit_radius: 0.06,
          offset: (0.0, 0.26, 0.0), size: 0.28,
          color_start: (1.0, 1.0, 0.88, 0.0), color_mid: (1.0, 0.80, 0.18, 1.0),
          color_end: (1.0, 0.28, 0.0, 0.0),
          sprites: ["particle/flame_05", "particle/flame_06"], additive: true,
          uv_distort: 0.35, uv_scroll_speed: 1.00 ),
    ],
),
```

Full canonical example: `assets/projects/particles_demo/assets.ron` → `"campfire_fire"`.

Particles use `AlphaMode::Add` (additive blending) by default when no sprite is set — overlapping spheres glow brighter, no depth-sorting artefacts in WASM. Directions are sampled deterministically via a spherical-cap golden-angle spiral so the same effect always produces the same pattern.

**Particle material paths** — the engine selects one of three implementations at spawn time:

| Condition | Material | Notes |
|-----------|----------|-------|
| `sprite` is `None` | `StandardMaterial` + `AlphaMode::Add` | Sphere mesh, colour gradient only |
| `sprite` set, `uv_distort == 0` and `uv_scroll_speed == 0` | `StandardMaterial` + configurable alpha | Quad billboard, static sprite |
| `sprite` set, `uv_distort > 0` or `uv_scroll_speed > 0` | `FlameParticleMaterial` | Quad billboard, animated WGSL shader (`custom_flame_particle.wgsl`) |

`FlameParticleMaterial` is an engine-internal material — it is not available as a `Custom(…)` shader key. Its uniforms (`color`, `elapsed_time`) are updated every frame by the particle system.

---

**Ground decals (`decals`)**

The `decals` map in `assets.ron` registers texture paths for flat ground-projected quads. Decals are spawned by `Action::ProjectDecal` from `rules.ron` or behavior files. All decal textures are white-on-transparent PNGs — colour comes from the `color` field in the action.

```ron
// assets.ron
decals: {
  "aoe_fire_circle": "shared/textures/decals/ring_thick.png",
  "cast_indicator":  "shared/textures/decals/circle_filled.png",
},
```

**`Action::ProjectDecal` fields:**

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `key` | `String` | required | Decal key from the `decals` map in `assets.ron`. |
| `entity` | `Option<String>` | `None` | If set, the decal XZ position tracks this entity each frame. Wins over `position`. Use `"{self}"` in behavior files. |
| `position` | `Option<(f32, f32, f32)>` | `None` | World-space origin. Y is ignored; decals always float at y=0.02. |
| `radius` | `f32` | required | Decal radius in metres. |
| `duration_secs` | `f32` | required | Lifetime in seconds. The decal fades out over the last 20 % and then despawns. |
| `color` | `(f32, f32, f32, f32)` | `(1,1,1,1)` | RGBA tint in linear 0–1 range. |
| `pulse_speed` | `f32` | `0.0` | Opacity heartbeat cycles per second (`0.0` = no pulse). |

**Shared decal textures** (`assets/shared/textures/decals/`):

| Key prefix | File | Shape |
|------------|------|-------|
| `circle_filled` | `circle_filled.png` | Solid disc with hard edge |
| `ring_thin` | `ring_thin.png` | Thin 10 px ring |
| `ring_thick` | `ring_thick.png` | Thick 28 px ring |
| `splat_01` | `splat_01.png` | Soft-edged disc (feathered) |
| `shockwave` | `shockwave.png` | Two concentric thin rings |

Example rule:

```ron
( on: "entity.entered:explosion_pad_01", do_actions: [
    SpawnEffect(key: "explosion_burst", entity: "explosion_pad_01"),
    ProjectDecal(key: "aoe_fire_circle", entity: "explosion_pad_01",
                 radius: 3.5, duration_secs: 3.0,
                 color: (1.0, 0.40, 0.10, 0.75), pulse_speed: 0.6),
]),
```

Decals use `LevelEntity` — they are automatically cleaned up on scene transitions.

---

**Audio format recommendations:**

| Format | Use for | Notes |
|--------|---------|-------|
| `.wav` | Short SFX (jumps, clicks, pickups) | Uncompressed PCM — zero decode overhead, instant playback |
| `.ogg` | Music and long ambient loops | Compressed; smaller files, minor decode cost acceptable for long audio |
| `.mp3` | Music only (avoid for new work) | Worse quality/size ratio than OGG; use OGG instead |

Do not use `.aiff` or `.flac` — these formats are not supported and will produce a warning at load time with no audio playing.

Trim any leading silence from SFX files before exporting — silence baked into the file adds perceived latency on every play.

**MaterialDef top-level fields** (apply to all kinds):

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `kind` | `MaterialKind` | — | Required. `Standard(…)`, `Terrain(…)`, or `Custom(…)` |
| `alpha_mode` | `AlphaModeDef` | `Opaque` | `Opaque`, `Mask(f32)`, `Blend`, `Premultiplied`, `Add`, `Multiply` |
| `double_sided` | `bool` | `false` | Disables back-face culling. Use for geometry that must be visible from the inside (sky spheres, double-sided leaves, portals). Creates a separate GPU pipeline from the single-sided variant. |
| `unlit` | `bool` | `false` | Bypasses the lighting pipeline entirely — output colour is the raw shader result. Automatically adds `NotShadowCaster` to the entity so it cannot cast shadows. Required for additive emissive effects. |
| `uv_transform` | `Option<UvTransformDef>` | `None` | Offset, scale, and rotation applied to UVs before sampling |
| `tags` | `Vec<String>` | `[]` | Arbitrary string tags for runtime filtering |

**MaterialDef kinds:**
- `Standard(StandardMaterialDef)` — PBR material (base colour, textures, metallic, roughness, etc.)
- `Terrain(TerrainMaterialDef)` — splatmap + layer textures (WebGPU 16-byte alignment required)
- `Custom(CustomMaterialDef)` — shader path + arbitrary texture/float/colour uniforms

**`TerrainMaterialDef` fields** (used inside `kind: Terrain(…)`):

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `splatmap` | `String` | required | Path to RGBA splatmap (one channel per layer) |
| `layers` | `Vec<String>` | `[]` | Texture paths for terrain layers (R, G, B channels). Fewer than 3 logs a warning and missing slots render as magenta. A 4th path (A channel) is accepted but unused by the current shader. |
| `uv_scale` | `f32` | `10.0` | UV tiling scale for layer textures. Higher values tile textures more finely. |

---

## `prefabs/prefabs.ron` — PrefabCatalog ✅

Named entity templates. Scenes reference prefabs by key; the runtime resolves the model via the AssetCatalog.

```ron
(
  prefabs: {
    "player_warrior": (
      kind: Actor,
      model: "hero",               // key in AssetCatalog.models
      animation_policy: "prefabs/animation/player_policy.ron",
      components: (
        tags: ["player"],
        movement: (
          walk_speed: 4.0,
          run_speed: 8.0,
          double_jump: true,
          collider_radius: 0.35,
          collider_height: 1.75,
        ),
      ),
    ),
    "prop_anvil": (
      kind: Prop,
      model: "anvil",
      material: "wood_crate",      // overrides embedded material
      components: (
        tags: ["prop"],
      ),
    ),
  }
)
```

**PrefabDef fields:**

| Field | Type | Description |
|-------|------|-------------|
| `kind` | `PrefabKind` | `Actor`, `Prop`, `Primitive`, or `Foliage` (bare enum variant, no quotes) |
| `model` | `String` | Key into `AssetCatalog.models` for `Actor`/`Prop`. Must be `""` (empty) for `Primitive` and `Foliage` — use `shape` or `foliage.trunk` instead. |
| `shape` | `Option<PrimitiveShapeKind>` | Required for `Primitive` prefabs. Write the bare variant: `Cuboid`, `Sphere`, etc. (`implicit_some` is active; no `Some()` wrapper needed). Omit for `Actor`/`Prop`/`Foliage`. See [Primitive shapes](#primitive-shapes-) below. |
| `foliage` | `Option<FoliageDef>` | Required for `Foliage` prefabs. Defines the trunk model, cluster distribution, and leaf card material. See [kind: Foliage](#kind-foliage-) below. |
| `animation_policy` | `Option<String>` | Path to `.ron` animation policy, relative to project root |
| `material` | `Option<String>` | Key into `AssetCatalog.materials` to override the model's material |
| `components.tags` | `Vec<String>` | Runtime-meaningful tags: `"player"` and `"flycam"` affect spawning; others are design-time only |
| `components.movement` | `MovementConfig` | Movement tuning for player prefabs. See [Special tag: `"player"`](#special-tag-player-) below. |
| `components.inputs` | `Option<InputMap>` | Key bindings for the player character. Only read for `"player"` prefabs. Omit to use WASD defaults. See [Special tag: `"player"`](#special-tag-player-) below. |
| `components.flycam` | `Option<FlyCamDef>` | Speed and sensitivity tuning for the free-fly camera. Only read for `"flycam"` prefabs. Omit to use defaults. See [Special tag: `"flycam"`](#special-tag-flycam-) below. |
| `components.camera` | `Option<CameraConfig>` | Orbit camera settings (offset, zoom, orbit speed, radius limits). Only read for `"player"` prefabs. Omit to use engine defaults. See [Special tag: `"player"`](#special-tag-player-) below. |
| `components.npc` | `Option<NpcDef>` | NPC AI configuration. When set, the entity gets a dynamic physics body and an NPC behaviour driver. See [NPC behaviour](#npc-behaviour-componentsnpc-) below. |
| `components.sounds` | `HashMap<String, String>` | Informational map from event name to `AssetCatalog` audio key. Not auto-wired — reference these keys in `state_machine.ron` to bind sounds to events (e.g. `player.jumped → PlaySound(key: "sfx_jump")`). |
| `primitive` | `Option<PrimitiveParams>` | Shape dimensions and appearance; only used when `kind: Primitive` |
| `children` | `Vec<ChildPrimitiveDef>` | Sub-meshes composing a composite primitive (e.g. lamp post + orb). Only used when `kind: Primitive`. See below. |
| `colliders` | `Vec<ColliderDef>` | One or more static physics colliders for `kind: Actor` / `kind: Prop`. All shapes are combined into a single Rapier compound body — use multiple entries to approximate curved geometry or multi-part shapes. Empty list = no physics. See below. |
| `behavior` | `Option<String>` | Path to a `.behavior.ron` file relative to the project root. Loads an independent per-entity FSM; `{self}` in event patterns and action keys is replaced with the entity's spawn ID. Works for all `kind` values, including composite `Primitive` prefabs with `children`. See `docs/30_runtime_events_and_logic.md`. |
| `trigger_zone` | `Option<TriggerZoneDef>` | Spawns a Rapier sphere sensor. Emits `entity.entered:{id}` / `entity.exited:{id}` when the player overlaps. Field: `radius: f32`. Works on all prefab kinds, including composite primitives (`model: ""` + non-empty `children`). |
| `interactable` | `Option<InteractableDef>` | Emits `entity.interacted:{id}` when the player is within `radius` metres and presses the interact key (default `"KeyF"`). Field: `radius: f32`. |
| `click_selectable` | `bool` | `false` | When `true`, left-clicking near this entity on screen sets it as `CurrentTarget` and emits `target.clicked:{id}`, `target.changed:{id}`, and `target.changed`. Selection is screen-space proximity (the entity nearest the cursor within ~70px), so it works for animated/skinned GLB characters as well as primitives. Clicking empty space clears the target. |
| `targetable` | `bool` | `false` | When `true`, this entity participates in Tab-cycle targeting. Pressing Tab selects the nearest `targetable` entity within `target_range` units and emits `target.changed:{id}` and `target.changed`. |
| `indicator_color` | `Option<(f32,f32,f32,f32)>` | `None` | Direct RGBA override for the target-indicator ring colour when this entity is selected. Takes precedence over `indicator_category` and the scene-level `target_indicator.color`. Only meaningful when the prefab is selectable. |
| `indicator_category` | `Option<String>` | `None` | Category key (e.g. `"enemy"`, `"ally"`) looked up in the scene's `target_indicator.named_colors` map. Used only when `indicator_color` is unset; falls through to scene-level `color` if the key is absent. |
| `select_aim_height` | `f32` | `1.0` | Vertical offset (metres) from the entity world origin used when projecting to screen space for click-selection. Default `1.0` is correct for human-scale characters (~1.8 m capsule). Lower this for ground-hugging creatures: `0.4` for a snake (`collider_height: 0.8`), `0.6` for a spider (`collider_height: 1.2`). Only meaningful when `click_selectable: true`. |
| `stat_templates` | `Vec<StatTemplateDef>` | Per-entity stat shapes. Every spawned instance gets an independent `StatMap` component; stats are addressed as `"spawn_id.stat_name"` in `ModifyStat`/`SetStat`. Works on player prefabs too — a player prefab that declares this gets an independent action-bar `SlotCost` pool instead of sharing the global one. See [Instance stats](#instance-stats-stat_templates-) below. |
| `stat_label` | `Option<StatLabelDef>` | Floating world-space numeric stat label above the entity. Tracks a live stat and updates every frame. See [World-space stat widgets](#world-space-stat-widgets-stat_label-and-world_stat_bar-) below. |
| `world_stat_bar` | `Option<WorldStatBarDef>` | Floating world-space stat bar above the entity. Style is configurable: `Ascii` (two overlapping `Text2d` entities), `Pixel` (a `Mesh2d` quad hierarchy), `Icon` (a row of per-cell `Sprite` icons, e.g. hearts), or `Textured` (a 9-sliced continuous fill bar from designer art) — all rendered by the 2D camera and updated every frame. See [World-space stat widgets](#world-space-stat-widgets-stat_label-and-world_stat_bar-) below. |
| `dialogue` | `Option<String>` | Project-relative path to a `.dialogue.ron` conversation file. When combined with `interactable`, pressing the interact key auto-fires `StartDialogue`. See [`dialogues/*.dialogue.ron`](#dialoguesnamedialogueron--dialoguedef-). |
| `display_name` | `Option<String>` | `None` | Human-readable name shown in the nameplate widget above this entity. Falls back to the prefab catalog key (e.g. `"enemy_orc_melee"`) when absent. Only meaningful when the nameplate system is active. |
| `nameplate` | `Option<bool>` | `None` | Per-prefab nameplate visibility override. `true` = always show (bypasses scene faction filter; still respects `max_distance`). `false` = never show, even when the scene has `show_nameplates`/`show_player_nameplate: true`. Absent = inherit from the scene default — `show_nameplates` + `faction_filter` for NPCs/props, or `show_player_nameplate` for the player prefab (whichever the entity is). |
| `player_index` | `u32` | `0` | **Local co-op only.** Which player slot this prefab controls (P1 = `0`, P2 = `1`, ...) when a scene has 2+ entities tagged `"player"`. Meaningless for single-player scenes. Forwarded onto the spawned entity as a queryable `PlayerIndex` component, read by: the split-screen "P{n}" corner HUD label; per-player targeting (which player is "primary" — see the Targeting section); `ActionBar((owner_player: n))`, which matches a bar's slots to whichever player entity carries this value (see `ActionBar`'s "Per-player action bars" subsection); and `SplitScreenDef.own_viewport_only`'s ring/camera layer assignment (keys on `player_index % 4` — see [Per-viewport target ring visibility](#per-viewport-target-ring-visibility-own_viewport_only)). Always assign a unique index in `0..4` per player — a duplicate `player_index: 0` (or omitting it on 2+ players) makes them fight over "primary" status, and a runtime `warn!` fires if this happens; a duplicate or out-of-range value under `own_viewport_only` is separately warned about too. |

### Special tag: `"flycam"` ✅

A prefab with `components.tags: ["flycam"]` and any `kind` spawns a free-flying camera instead of a model. Any body-defining field — `model`, `shape`/`primitive`, or `children` (the latter two for a `Primitive` prefab) — is ignored, and warns if you set one, see below. The engine creates a `Camera3d` + `ActiveCameraMode::Flycam` component (plus a `FlycamCameraMode` marker) at the entity's transform.

**Controls:**
- **W/S** — forward / back
- **A/D** — strafe left / right
- **E / Space** — ascend
- **Q / LCtrl** — descend
- **LShift / RShift** — fast mode
- **Hold LMB or RMB + move mouse** — rotate view (mouse is free for UI when no button is held)

**`FlyCamDef` fields** (`components.flycam`, all optional — defaults apply when omitted):

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `speed` | `f32` | `100.0` | Normal movement speed in units/second |
| `fast_speed` | `f32` | `200.0` | Movement speed while Shift is held, in units/second |
| `sensitivity` | `f32` | `0.002` | Mouse look sensitivity in radians per pixel |
| `forward` | `String` | `"KeyW"` | Key for moving forward |
| `backward` | `String` | `"KeyS"` | Key for moving backward |
| `left` | `String` | `"KeyA"` | Key for strafing left |
| `right` | `String` | `"KeyD"` | Key for strafing right |
| `up` | `String` | `"Space"` | Key for ascending |
| `down` | `String` | `"KeyQ"` | Key for descending |
| `look_button` | `String` | `"Either"` | Mouse button that activates look mode: `"Left"`, `"Right"`, or `"Either"` |

To display the camera's world position in the UI, add a label element with `id: "flycam_position"` to the scene's `ui` array. The engine will update it every frame.

```ron
// In prefabs/prefabs.ron — minimal (all defaults)
"flycam": (
  kind: Prop,
  model: "",
  components: ( tags: ["flycam"] ),
),

// In prefabs/prefabs.ron — with custom speed tuning
"flycam_slow": (
  kind: Prop,
  model: "",
  components: (
    tags: ["flycam"],
    flycam: (
      speed:       20.0,
      fast_speed:  80.0,
      sensitivity: 0.001,
    ),
  ),
),

// In scenes/main.scene.ron — entity
(
  id: "camera_01",
  prefab: "flycam",
  transform: (
    translation: (0.0, 12.0, 0.0),
    rotation_euler_deg: (-25.0, 0.0, 0.0),
    scale: (1.0, 1.0, 1.0),
  ),
),

// In scenes/main.scene.ron — ui label
(
  kind: "label",
  id: "flycam_position",
  text: "",
  position: (16.0, 16.0),
  size: (300.0, 24.0),
),
```

**Duplicate flycam entities.** A scene should have at most one `tags: ["flycam"]` entity. If two or
more are authored, only the last one in `entities:` order is actually spawned — the rest are
silently discarded except for a console `warn!` naming both. Delete all but one to silence it.

**A flycam-tagged prefab's body-defining fields never render.** A flycam is camera-only — it has no
body to attach a visible mesh to, so `model:`, `shape:`/`primitive:`, or (for a `Primitive` prefab)
non-empty `children:` are all silently dropped at spawn time. Setting any of these now fires a
`warn!` **in the browser console** (the only diagnostic channel available if you're working from a
prebuilt WASM build with no `ironhold_cli` access) and fails `ironhold_cli validate` as a hard
error, naming the field(s) and prescribing the fix. This only covers entities placed directly in a
scene's `entities:` list — a flycam prefab spawned dynamically via `Action::Spawn` is not checked.

```ron
// ❌ never renders — a flycam has no body
"flycam_with_body": ( kind: Prop, model: "anvil", components: ( tags: ["flycam"] ) ),
// ✅ camera-only, as intended
"flycam":           ( kind: Prop, model: "",      components: ( tags: ["flycam"] ) ),
```

If you actually want a flying character with a visible body, this isn't the way — see "Two
authoring paths" below.

**Two authoring paths for a flycam, not one.** `tags: ["flycam"]` on its own standalone entity (this
section) is one way to get a free-flying camera. The other is `camera_mode: Flycam(...)` authored
directly on a **player** prefab (see [Unified camera modes (`camera_mode:`)](#unified-camera-modes-camera_mode-)
below) — that gives a `tags: ["player"]` entity itself free-fly controls instead of an orbit camera,
with no separate camera entity at all. Don't confuse the two: a standalone flycam entity is a
camera with no character behind it (no stats, no movement systems, no CharacterController); a
player prefab with `camera_mode: Flycam` is a full player character whose camera happens to fly
freely.

**Don't put both tags on the same prefab.** `components.tags: ["player", "flycam"]` on one prefab
doesn't combine the two — the flycam tag takes priority at scene load, so the entity spawns as a
camera-only flycam and its player components (movement, stats, `CharacterController`, everything)
never spawn at all. This is also a scene-load `warn!` and an `ironhold_cli validate` hard error. If
you want a flying player character, use `camera_mode: Flycam(...)` on a `"player"`-only prefab
(above) instead; if you wanted a plain camera-only flycam, remove the `"player"` tag.

#### Spectator mode: `"player"` + `"flycam"` in one scene

A scene can combine a normal `tags: ["player"]` entity with a standalone `tags: ["flycam"]` entity.
When both are present, **the flycam takes priority**: it becomes the scene's sole active camera,
and the player entity spawns and plays normally alongside it — movement, stats, targeting, AI
reactions, and nameplates all keep working exactly as if the player had its own camera — but the
player gets **no camera of its own**. A console `warn!` confirms this is intentional, not a bug,
and tells you how to get the player's camera back (remove the flycam entity).

This is useful for a spectator/debug view over live gameplay: watching NPC AI without controlling
the camera yourself, free-flying around a scene while the player (and anything scripted to react to
it) keeps running, or framing a screenshot/cinematic shot independently of the player's own camera
rig.

```ron
// In scenes/main.scene.ron — both entities in the same `entities:` list
(
  id: "player_01",
  prefab: "player_orbit_demo",   // any normal `tags: ["player"]` prefab
  transform: ( translation: (0.0, 0.5, 0.0), rotation_euler_deg: (0.0, 0.0, 0.0), scale: (1.0, 1.0, 1.0) ),
),
(
  id: "camera_01",
  prefab: "flycam_demo",         // any normal `tags: ["flycam"]` prefab
  transform: ( translation: (0.0, 6.0, 12.0), rotation_euler_deg: (-15.0, 0.0, 0.0), scale: (1.0, 1.0, 1.0) ),
),
```

What still works / what doesn't, compared to a normal player scene:
- ✅ Player movement, stats, targeting, AI reactions, nameplates, HUD — all unaffected.
- ✅ Free-fly camera controls (WASD + mouse look) on the flycam, independent of the player.
- ❌ The player gets no camera of its own — nothing follows it. If the scene has 2+ players and the
  first also authors `camera.split`/`camera.party`, that's ignored too (a second `warn!` names this
  specifically); split-screen is fully suppressed, not downgraded to a single camera. (`split`/
  `party` are already "local co-op only" and inert with a single player, flycam or not, so this
  doesn't change anything for the 1-player case — see the `split`/`party` field rows further below.)
- ❌ The flycam entity's own body-defining fields still never render (see **A flycam-tagged
  prefab's body-defining fields never render** above) — a flycam is camera-only, with or without a
  player also present — and now warns at scene load / fails `ironhold_cli validate` either way.
- ❌ `Action::CameraShake` still no-ops on the flycam (it has neither an orbit nor a party camera to
  shake) — same as a flycam-only scene.
- ❌ Both entities read input independently, so the player's default WASD/Space/Q keys and the
  flycam's default WASD/Space/Q fly controls fire from the same keypresses at once — neither system
  knows the other exists. Re-bind one side if you want them separate: give the flycam prefab its
  own `flycam: (forward: "ArrowUp", backward: "ArrowDown", left: "ArrowLeft", right: "ArrowRight",
  ...)`, or change the player prefab's `inputs:`.

See `assets/projects/camera_modes/scenes/flycam_spectator_test.scene.ron` for a working example
(reachable from that project's hub scene via the "Spectator" pad).

**Known limitations** (not yet handled by this priority rule):
- Dynamically spawning a `tags: ["player"]` prefab at runtime (`Action::Spawn`, e.g. character
  select or hot-join) always gives that player its own full-window camera, even in a scene that
  started in spectator mode — this priority rule only applies to entities present at scene load.
- `Action::SetCameraMode` with `owner_player` omitted targets every camera in the scene, including
  the flycam itself — it's possible to accidentally convert the flycam into a targetless `Orbit`
  camera this way. Use `"default"` to recover, or pass an explicit `owner_player` targeting only
  the player's own camera when one exists.
- Once suppressed, the player's camera has no in-scene way back — there's no action that re-spawns
  a player's own camera mid-scene. The only way to restore it is `LoadScene` (removing the flycam
  entity from the scene first).

### Special tag: `"player"` ✅

A prefab with `components.tags: ["player"]` spawns a third-person character controller with an orbit camera. Works on both `kind: "actor"` (GLB model) and `kind: "primitive"` (capsule shape). Movement is tuned via `components.movement`; key bindings are tuned via `components.inputs`.

**Which top-level `PrefabDef` fields take effect on a `tags: ["player"]` prefab** (same for `kind: "actor"` and `kind: "primitive"`, except where noted):

- **Work:** `player_index` (split-screen "P{n}" HUD label, per-player targeting's primary-player check, `ActionBar.owner_player` matching), `material` (visual override, applies after the body is built regardless of model source — **note:** it replaces the material on *every* mesh under the player, including any `children:` parts, so combining `material:` with `children:` flattens all of them to one uniform tone; give the composed parts their own distinct look via each child's own `primitive.color`/`roughness`/`metallic` instead, and put the body's own tone on the top-level `primitive.color` rather than a `material:` key), `stat_templates` (per-player `StatMap`, see "Per-player stat pools" below), `stat_label`/`world_stat_bar` (floating widgets bound to `{self}.<stat>`), `children` (cosmetic composed-body parts on a `kind: "primitive"` player — the physics capsule still comes entirely from `primitive.radius`/`primitive.height`, unaffected by `children`; a child with `physics: true` or `sensor: true` is rejected with a load-time error and skipped rather than spawned, since it would conflict with the player's own `RigidBody`).
- **Still silently no-op regardless of `kind`:** `behavior`, `interactable`, `dialogue`, `inventory`, `trigger_zone` — none of these are wired onto either player spawn path; this is a deliberate scope boundary (a player is input-driven, not state-machine-driven like an NPC), not a bug, and not blocked on anything above.
- **`kind: "primitive"` only, not yet supported at all:** a primitive-shaped player prefab combined with `scene.terrain: Some(...)` (validate error + runtime warning, not silent), referenced from a character-select `Action::Spawn` call (runtime warning, not silent), or listed in a scene's `join_prefab_keys` for gamepad/keyboard hot-join (runtime warning, not silent — hot-join only supports GLB/`Actor`-kind players as of this writing) — only a scene placed directly in a scene's `entities:` list spawns a primitive player today.

Both player model sources now get the same collider `Friction` (`combine_rule: Min` to still avoid catching hard on cube/step edges) — previously primitive-only, a GLB player's capsule sat at Rapier's default 0.5/`Average` friction. The coefficient is `0.15`, not `0.0` — a real hillside playtest (`quick_scene`) with `0.0` showed a visible, permanent downhill creep for an idle player on sloped terrain; `0.15` holds a slope while still avoiding noticeable edge-catching. No separate friction field exists on `MovementConfig` — this is a fixed engine value, not per-prefab-authorable (see the backlog for a possible future physics-material field).

See `local_coop_demo`'s `player_p1_primitive`/`player_p2_primitive` prefabs (`prefabs/prefabs.ron`) and `scenes/room7.scene.ron` for a minimal working example — two bare-capsule primitive-shaped players in one split-screen scene, each with its own tint, mana pool, and overhead bar (mana there is decorative; nothing spends it, so both bars stay full). For a **mixed** pairing — one GLB player and one composed-multi-part-body primitive player side by side, sharing per-player targeting, a live spendable action-bar mana pool, and own-viewport-only target rings — see `scenes/room10.scene.ron` and its `player_p2_primitive_split_ring` prefab.

**`InputMap` fields** (`components.inputs` — omit the entire block to use WASD defaults):

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `forward` | `String` | `"KeyW"` | Move forward |
| `backward` | `String` | `"KeyS"` | Move backward |
| `left` | `String` | `"KeyA"` | Rotate/strafe left |
| `right` | `String` | `"KeyD"` | Rotate/strafe right |
| `strafe_left` | `String` | `"KeyQ"` | Strafe left (camera-relative) |
| `strafe_right` | `String` | `"KeyE"` | Strafe right (camera-relative) |
| `jump` | `String` | `"Space"` | Jump |
| `run` | `String` | `"ShiftLeft"` | Hold to run |
| `interact` | `String` | `"KeyF"` | Interact with nearby `interactable` entities |
| `strafe_mouse_button` | `Option<String>` | `Some("Left")` | Mouse button that enables strafe-mode (A/D strafe instead of rotate): `"Left"`, `"Right"`, or `None` to disable entirely |
| `target_next` | `String` | `"Tab"` | Key to cycle to the next nearest `targetable: true` entity. Hold Shift while pressing to cycle in reverse. **Note:** `"Tab"` is intercepted by browsers for focus navigation in WASM builds — prefer another key such as `"KeyT"` (as `3rd_person_game_demo` does). |
| `target_range` | `f32` | `30.0` | Maximum world-space distance (units) for Tab targeting. Entities beyond this range are excluded. |
| `gamepad_index` | `Option<usize>` | `None` | **Local co-op only.** When set, this player **additionally** reads movement/camera input from a connected gamepad: left stick = move/strafe, right stick X = turn, right stick Y = camera pitch, jump/run/interact/target-cycle use the buttons below (all overridable, not fixed), and any `ActionSlotDef.gamepad_key` on a bar owned by this player (`owner_player` matching this player's `player_index`) resolves against this same gamepad. **This is additive, not a replacement** — the player's keyboard bindings stay fully active; gamepad input is layered on top, so one player can freely mix both at once (e.g. move with the stick, jump with the keyboard). `None` (default) means no gamepad is bound; keyboard behaves exactly as before. **Note:** this is a one-time *seed*, not a live slot — see "How a controller gets assigned to a player" below for exactly what it means and what it doesn't. |
| `gamepad_jump` | `String` | `"South"` | Gamepad button for jump — Xbox **A** / PlayStation **Cross**. |
| `gamepad_run` | `String` | `"East"` | Gamepad button to hold-run — Xbox **B** / PlayStation **Circle**. |
| `gamepad_interact` | `String` | `"West"` | Gamepad button to interact with nearby `interactable` entities — Xbox **X** / PlayStation **Square**, the genre-conventional "use/interact/action" face button. Works in local co-op (each player's own gamepad button is checked independently). |
| `gamepad_target_next` | `String` | `"North"` | Gamepad button to cycle Tab-targeting — Xbox **Y** / PlayStation **Triangle**, the genre-conventional "cycle/secondary" face button. To rebind to a shoulder button/stick-click instead (some games' convention), use `"RightThumb"` or `"RightTrigger"` — see the valid button names below. |
| `gamepad_deadzone` | `f32` | `0.15` | Analog stick input below this magnitude (movement, turn, and camera pitch) is ignored, to avoid stick drift producing tiny unintended input. |
| `look_left` | `Option<String>` | `None` | Keyboard-held camera yaw turn (left). Unbound by default. See the "Keyboard camera look" note below `CameraConfig`'s `look_speed` field. |
| `look_right` | `Option<String>` | `None` | Keyboard-held camera yaw turn (right). |
| `look_up` | `Option<String>` | `None` | Keyboard-held camera pitch (toward overhead). |
| `look_down` | `Option<String>` | `None` | Keyboard-held camera pitch (toward horizontal). |

#### How a controller gets assigned to a player ✅

> `gamepad_index` is read only **once**, at the moment a matching gamepad has been continuously
> connected for about half a second — not re-checked every frame, and not the very instant a
> controller appears. That brief delay is deliberate: some controllers (a confirmed real-hardware
> case) can briefly register as two separate entries before the browser/OS settles on one, and
> without the delay a player could permanently lock onto the wrong, short-lived one. Until a
> controller has been stably connected long enough, the player simply plays on keyboard — this is
> the ordinary, ongoing "no gamepad plugged in for this seed yet" case and is completely silent, no
> matter how long it lasts. Once a controller is assigned, it stays assigned to that same player
> for the rest of the session, no matter what happens to any *other* controller — unplugging and
> replugging a different pad, or even a whole session's worth of other players joining and leaving,
> never reassigns an already-assigned player's controller out from under them. If a player's own
> controller is unplugged, their gamepad input simply pauses (their keyboard bindings, if any,
> keep working) — plugging the **same** controller back into the **same** USB port/slot resumes
> their gamepad input automatically, no reload or rejoin needed. Plugging it into a *different*
> port is not guaranteed to be recognized as the same device (platform-dependent), so prefer
> reconnecting to the same port. This is a one-way assignment: there is no way to move an
> already-assigned controller to a different player without restarting the scene — in
> `local_coop_demo`, walking through a portal and back is enough; a full page reload also works.
>
> **Connect every controller before loading the scene, or press player 1's controller first.**
> `gamepad_index` counts controllers in the order the game sees them connect — and in the browser,
> a controller isn't visible at all until its first button press (see below), so on the web
> "connection order" really means **press order**. If a second player plugs in and presses a
> button *after* the first player has already been assigned, the two can end up swapped for the
> session, or — if two players share `gamepad_index` values 0 and 1 and only one pad is present so
> far — the second player can end up simply unable to claim a real controller once one is already
> taken by someone else's seed. In that case they are **not** automatically moved onto whatever pad
> is left free: they stay on keyboard for the rest of the session, and after a few seconds the
> browser console logs a one-time reminder naming them (e.g. `Player P2: gamepad_index 1 resolves
> to a controller already bound to another player`) — this is informational, not an error, and it
> doesn't repeat. If this happens, restart the scene (see above) with every controller already
> connected and pressed once, in player order.
>
> **Two players in the same scene must not author the same `gamepad_index`.** One physical
> controller would then drive both characters — flagged both as a scene-load warning and an
> `ironhold_cli validate` error (`duplicate_gamepad_index`), so this is caught long before a
> playtest. Give each player their own value.
>
> **Troubleshooting: `gamepad_index: 0` set but the controller does nothing.** On some Windows +
> Chrome/Edge + controller combinations (confirmed with an Xbox 360 controller), one physical
> controller can appear in the browser as **two separate gamepad entries** — one live, one an
> inert duplicate that always reports zero for every axis/button. Since `gamepad_index` is just
> "connection order," index `0` can land on the dead duplicate while the real controller sits at
> index `1`. If a controller is visibly connected (check the browser console for a
> `Gamepad ... connected` log line) but nothing responds, try `gamepad_index: 1` (or `2`) before
> assuming the feature itself is broken. This is a real, reproducible platform quirk, not a
> hypothetical. **Note:** since a controller is now assigned once and locked in, if the dead
> duplicate happens to get assigned first, don't expect it to later "fix itself" if the duplicate
> disappears — the affected player stays on keyboard for the rest of the session; restart the
> scene to re-attempt assignment.

**Valid gamepad button names** — governs `InputMap`'s `gamepad_jump`/`gamepad_run`/
`gamepad_interact`/`gamepad_target_next` fields, `global_unclaimed_gamepad_bindings`/
`scene_unclaimed_gamepad_bindings` (see [Gamepad-triggered hot join](#gamepad-triggered-hot-join) below),
and `ActionSlotDef.gamepad_key` (see `ActionBar` above) — same Xbox / PlayStation physical mapping
in every case. An unrecognized name is `warn!`-logged (and, for the bindings maps and
`gamepad_key`, also flagged by `ironhold validate`) and treated as unbound (no crash), mirroring
keyboard key-name validation:

| Name | Xbox | PlayStation |
|------|------|--------------|
| `"South"` | A | Cross |
| `"East"` | B | Circle |
| `"North"` | Y | Triangle |
| `"West"` | X | Square |
| `"LeftTrigger"` | LB | L1 |
| `"LeftTrigger2"` | LT | L2 |
| `"RightTrigger"` | RB | R1 |
| `"RightTrigger2"` | RT | R2 |
| `"Select"` | View/Back | Share |
| `"Start"` | Menu/Start | Options |
| `"LeftThumb"` | L3 (left stick click) | L3 |
| `"RightThumb"` | R3 (right stick click) | R3 |
| `"DPadUp"` / `"DPadDown"` / `"DPadLeft"` / `"DPadRight"` | D-pad | D-pad |

> **`"LeftTrigger"`/`"RightTrigger"` are the shoulder *bumpers* (LB/RB, L1/R1) — the digital
> click, not the analog trigger.** `"LeftTrigger2"`/`"RightTrigger2"` are the analog *triggers*
> (LT/RT, L2/R2) themselves, read here as a digital press/release past a threshold. Easy to get
> backwards from the name alone — the table above lists the correct physical button for each.

Example `inputs:` block wiring every gamepad field (movement/turn are always active when `gamepad_index` is set; the fields below only change which button/deadzone is used):
```ron
inputs: (
  forward: "KeyW", backward: "KeyS", left: "KeyA", right: "KeyD",
  strafe_left: "KeyQ", strafe_right: "KeyE", jump: "Space", run: "ShiftLeft",
  gamepad_index: 1,
  gamepad_jump: "South",
  gamepad_run: "East",
  gamepad_interact: "West",
  gamepad_target_next: "North",
  gamepad_deadzone: 0.15,
),
```

> **Camera pitch via right-stick-Y reuses `CameraConfig.look_speed`** (the same dial governing keyboard-held `look_up`/`look_down` — see below), so a gamepad-only player's `camera:` block referencing `look_speed` isn't a keyboard-only leftover left in by mistake. A full stick deflection reproduces the keyboard-hold rate; partial deflection scales proportionally.

> **Standing keyboard/gamepad parity gap: no gamepad camera yaw.** Right-stick-X already drives character turning; reusing it to *also* orbit the camera (full twin-stick free-look) is a deeper control-feel redesign, not attempted here. A keyboard split-screen player can freely yaw their camera (`look_left`/`look_right`); a gamepad player can only pitch. This is a permanent, deliberate limitation until a twin-stick vs. auto-face-on-move redesign is decided from real playtest feedback — not an oversight.

> **Gamepad camera pitch (like keyboard camera look) only reaches a per-player `OrbitCamera`** — a scene using the shared `party` camera (`CameraConfig.party`) or a dynamic-split scene while still merged has no single per-player camera to attribute the pitch to, so right-stick-Y does nothing there, silently, exactly like `look_up`/`look_down` already do today. Not a gamepad-specific gap — switch to `split` (or wait for a dynamic scene to actually split) for either input method's camera control to take effect.

> **Selection is proximity-based, not a pixel-perfect mesh hit.** Left-clicking selects the `click_selectable` entity whose on-screen position is nearest the cursor (within a fixed radius), resolved from the entity's transform — so thin or animated/skinned characters are easy to click and never "fall through" to the geometry behind them. Clicking with nothing nearby clears the current target. For combat-style play, set the player camera's `orbit_button: "Right"` so left-click is free for selection (see `3rd_person_game_demo`).

**Valid key name strings** — both the canonical form (`"KeyW"`) and the shorthand (`"W"`) are accepted for letters and digits:

| Category | Valid strings |
|----------|--------------|
| Letters | `"KeyA"`–`"KeyZ"` (or bare `"A"`–`"Z"`) |
| Digits | `"Digit0"`–`"Digit9"` (or bare `"0"`–`"9"`) |
| Numpad | `"Numpad0"`–`"Numpad9"` — physical numeric-keypad keys, unaffected by NumLock state |
| Function | `"F1"`–`"F12"` |
| Modifiers | `"ShiftLeft"`, `"ShiftRight"`, `"ControlLeft"`, `"ControlRight"`, `"AltLeft"`, `"AltRight"` |
| Common | `"Space"`, `"Escape"`, `"Enter"`, `"Tab"`, `"Backspace"`, `"Delete"` |
| Arrows | `"ArrowUp"`, `"ArrowDown"`, `"ArrowLeft"`, `"ArrowRight"` |
| Punctuation | `"Comma"`, `"Period"`, `"Semicolon"`, `"Quote"`, `"Slash"`, `"BracketLeft"`, `"BracketRight"`, `"Minus"`, `"Equal"` |

Invalid key strings produce a `warn!` at load time and that binding has no effect. Case is significant for multi-character names — `"space"` and `"shiftleft"` are not valid. **Exception:** a single bare letter (e.g. `"q"`) is case-insensitive and resolves the same as `"Q"`; only letter keys get this leniency.

**`MovementConfig` fields** (all optional — defaults apply when omitted):

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `walk_speed` | `f32` | `5.0` | Walking speed in m/s |
| `run_speed` | `f32` | `10.0` | Running speed in m/s (hold Shift) |
| `rot_speed` | `Option<f32>` | `3.0` | Yaw rotation speed in rad/s |
| `jump` | `Option<JumpConfig>` | own height | Jump height; see below |
| `double_jump` | `bool` | `false` | Enable a second jump while airborne |
| `double_jump_height` | `Option<JumpConfig>` | same as `jump` | Second-jump height |
| `collider_radius` | `Option<f32>` | `0.4` | Capsule collider radius (**GLB players only** — primitive players use `primitive.radius`) |
| `collider_height` | `Option<f32>` | `1.8` | Capsule total height (**GLB players only** — primitive players use `primitive.height`) |
| `idle_drag` | `f32` | `0.8` | Velocity decay multiplier on the XZ plane per physics tick when no input is given (0 = instant stop, 1 = no friction). The player collider's own `Friction` (fixed at `0.15` for every player regardless of model source, not per-prefab-authorable) is the primary defense against downhill creep on sloped terrain; lowering `idle_drag` further bounds any residual creep but cannot zero it on its own. Applies in the air as well as on the ground (no grounded check) — a very low value also cancels horizontal momentum immediately after releasing input mid-jump |
| `linear_damping` | `f32` | `0.5` | Rapier `linear_damping` on the player capsule rigid body |
| `angular_damping` | `f32` | `0.5` | Rapier `angular_damping` on the player capsule rigid body |
| `ground_cast_length` | `f32` | `0.3` | Distance (metres) the ground-detection sphere is swept downward each frame — increase for uneven terrain or fast vertical movement |

**`JumpConfig` variants:**
- `Fixed(height: <f32>)` — absolute world-space height in metres (e.g. `Fixed(height: 2.5)`)
- `RelativeToHeight(percent: <f32>)` — fraction of the player's own height (e.g. `RelativeToHeight(percent: 100)`)

**`CameraConfig` fields** (`components.camera` — omit the entire block to use engine defaults):

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `offset` | `(f32, f32, f32)` | `(0, 5, 10)` | Camera position relative to the player (right, up, back) |
| `look_at_offset` | `(f32, f32, f32)` | `(0, 2, 0)` | Point the camera looks at, relative to the player origin (use `(0, 1.5, 0)` to look at chest height) |
| `zoom_speed` | `f32` | `10.0` | Scroll-wheel zoom speed, in world units per second per normalized wheel "notch". One notch is one notch regardless of platform/browser/OS scroll settings — the engine normalizes the raw `MouseWheel` event before applying this multiplier, so a single click never blows past `min_radius`/`max_radius` in one step (see `capabilities/camera.rs`'s `normalized_wheel_delta`). Trackpad swipes still scroll continuously and proportionately, just below the one-notch clamp. |
| `orbit_speed` | `f32` | `0.5` | Mouse orbit speed (radians per pixel) |
| `min_radius` | `f32` | `2.0` | Minimum zoom distance in metres |
| `max_radius` | `f32` | `20.0` | Maximum zoom distance in metres |
| `min_pitch` | `f32` | `0.1` | Minimum pitch in radians (looking up limit) |
| `max_pitch` | `f32` | `0.9` | Maximum pitch in radians (looking down limit) |
| `orbit_button` | `String` | `"Either"` | Mouse button that orbits the camera: `"Left"`, `"Right"`, `"Either"`, or `"None"` to disable manual mouse-orbit entirely (fixed-angle auto-follow only). See the `"None"` note below. |
| `character_rotate_button` | `Option<String>` | `Some("Right")` | Mouse button that also rotates the character yaw while orbiting; set to `None` to disable |
| `initial_pitch` | `f32` | `0.5` | Camera pitch at scene start in radians |
| `initial_yaw` | `f32` | `0.0` | Camera yaw at scene start in radians |
| `party` | `Option<PartyZoomDef>` | `None` | **Local co-op only.** Only read from the **first** `"player"`-tagged entity in the scene's `entities` list — `party` on any later player is ignored. When the scene has 2+ players and this is set, the engine spawns one shared camera that frames the midpoint of all players instead of giving each player their own orbit camera. See [Shared party camera](#shared-party-camera-partyzoomdef-) below. Mutually exclusive with `split` — see below. |
| `split` | `Option<SplitScreenDef>` | `None` | **Local co-op only.** Only read from the **first** `"player"`-tagged entity in the scene's `entities` list. When the scene has 2+ players and this is set, the engine gives every player their own real camera, each locked to its own half of the window, instead of one shared camera. See [Split-screen camera](#split-screen-camera-splitscreendef-) below. Mutually exclusive with `party` — see below. |
| `look_speed` | `f32` | `2.0` | Angular rate (radians/sec) for keyboard-held camera look (`InputMap.look_left`/`look_right`/`look_up`/`look_down`) — roughly a full turn every ~3s at the default. A **separate** dial from `orbit_speed`, which is tuned as a mouse-pixel-delta multiplier and would be far too slow if reused as a keyboard-hold rate. Authored per-player, same as every other `CameraConfig` field. |
| `fov` | `f32` | `45.0` | Field of view in degrees — matches Bevy's own `PerspectiveProjection` default exactly, so omitting this field reproduces the framing every project had before this field existed. Applied once at spawn (see [Unified camera modes](#unified-camera-modes-camera_mode-) above); a `SetCameraMode` switch (v2) with a `transition:` interpolates it smoothly instead — see [`camera_modes:` registry and `SetCameraMode`](#camera_modes-registry-and-setcameramode-v2-) below. Note `Follow`/`Fixed`'s own `fov` default is `60.0`, not `45.0` — a different struct with its own default, not an inconsistency in this one. |
| `transition` | `Option<CameraTransition>` | `None` | **v2.** Has **no effect at spawn** — a camera has nothing to blend from on its first frame. Only takes effect later, when `Action::SetCameraMode(mode: "default")` restores this mode and finds a `transition:` authored here to blend with. See [`camera_modes:` registry and `SetCameraMode`](#camera_modes-registry-and-setcameramode-v2-) below for the field's shape (`duration_secs`, `ease: EaseKind`). |

> **2+ players without a `party` or `split` block:** if a scene has 2+ `"player"`-tagged entities and the first player's `camera.party`/`camera.split` are both unset, the engine logs a warning and falls back to a single orbit camera that follows only the first player. It never silently spawns two competing per-player cameras — you must opt in to a shared or split camera explicitly.

> **`party` and `split` are mutually exclusive.** Both are read only from the first player's `camera` block. If a designer sets both by mistake, the engine logs a warning and `split` wins (treated as the more specific/newer setting) — it does not silently pick one with no signal.

> **`orbit_button: "None"`** — unlike an actually-unrecognized string (which warns and falls back to `"Either"`), `"None"` is a deliberate, silent opt-out: no left-click or right-click binding at all. This exists for local co-op split-screen player cameras, where a single shared mouse would otherwise rotate/zoom every player's camera identically — `camera_orbit_system` has no per-viewport cursor check; it reads the mouse/keyboard state once per frame and applies it identically to every active `Orbit`-mode camera in its query, regardless of which viewport the cursor is actually over (confirmed via a live playtest diagnostic during `camera_modes.md` v1). Split-screen scenes must pair this with **both** `zoom_speed: 0.0` (scroll × 0 has no effect) **and** `character_rotate_button: None` (see below — easy to forget, since it's a separate switch from `orbit_button`) on every player's `camera` block, giving each player a fixed-angle camera with no **mouse** control at all. Each player can still independently turn their own camera via keyboard — see "Keyboard camera look" below.
>
> **`character_rotate_button` must also be set to `None` for split-screen — it is a separate switch from `orbit_button`.** `orbit_button: "None"` only disables the camera's own mouse-orbit; `character_rotate_button` (default `Some("Right")`) independently rotates the **character model** on RMB-drag, gated only on the global right-mouse-button state, with the same no-per-viewport-check limitation as `orbit_button`. Missing this on a split-screen prefab means holding RMB and moving the mouse spins **every** split player's character at once, from either viewport — a real bug found live during `camera_modes.md` v1 (all `local_coop_demo` split prefabs were missing this and have been fixed as part of that feature; any *new* split-screen project must set it explicitly).

> **Keyboard camera look for split-screen.** Since split-screen disables *mouse* orbit per camera (immediately above), each player's own `InputMap.look_left`/`look_right`/`look_up`/`look_down` (see the `InputMap` table above) is the per-player alternative — bound to that player's own camera only, independent of every other player's. A worked example, matching `local_coop_demo`'s actual per-scheme bindings (each pair chosen to sit near that scheme's own movement cluster, and disjoint across every scheme so a 4-way grid scene has no key collisions):
>
> | Scheme | Movement keys | `look_left` / `look_right` |
> |---|---|---|
> | WASD | W/A/S/D, Q/E strafe | `"KeyZ"` / `"KeyX"` |
> | Arrows | ↑/↓/←/→ | `"Comma"` / `"Period"` |
> | IJKL | I/J/K/L | `"KeyH"` / `"KeyP"` |
> | Numpad | 5/1/2/3 | `"Numpad7"` / `"Numpad9"` |
>
> ```ron
> inputs: (
>   forward: "KeyW", backward: "KeyS", left: "KeyA", right: "KeyD",
>   strafe_left: "KeyQ", strafe_right: "KeyE",
>   jump: "Space", run: "ShiftLeft",
>   strafe_mouse_button: None,
>   look_left: "KeyZ", look_right: "KeyX",
>   // Pitch is also supported (same mechanism) — bind if wanted:
>   // look_up: "KeyC", look_down: "KeyV",
> ),
> ```
>
> Party mode (`camera.party`, one shared party camera for every player) has no per-player
> viewport to attribute a look binding to — `look_left`/etc. are simply not authored there. See
> `local_coop_demo/prefabs/prefabs.ron`'s `player_p1`/`player_p2` for the reference example.

**Jump sound** — the player system emits `GameEvent::Trigger("player.jumped")` on every jump. Wire a sound to it in `logic/state_machine.ron`:
```ron
on: [
  (event: "player.jumped", do_actions: [PlaySound(key: "sfx_jump")]),
]
```

### Unified camera modes (`camera_mode:`) ✅

Added in `planning/features/camera_modes.md` v1. `components.camera_mode` is **the mode a camera
starts in** — it replaces the implicit "orbit if `tags: ["player"]`, flycam if `tags: ["flycam"]`"
dispatch with an explicit, designer-chosen mode, and is the modern way to author any of the camera
behavior documented on this page and below. **The legacy `camera:`/`flycam:` fields below still
work unchanged and are not deprecated** — every existing project keeps working with zero migration
required (see "Backward compatibility" below). New authoring should prefer `camera_mode:`.

> **Not to be confused with the plural `camera_modes:` registry** (a *scene-level* field, v2, see
> below) — that's the list of **switchable presets** `Action::SetCameraMode` can jump to at
> runtime. This singular field never changes at runtime on its own; it only describes the mode a
> camera has the moment it spawns.

**`CameraModeDef` variants** (`components.camera_mode`, one prefab authors exactly one):

| Variant | What it does | Payload | Valid in `camera_modes:` registry? (v2) |
|---|---|---|---|
| `Orbit` | Mouse/keyboard/gamepad-orbitable camera following a target at a variable radius — the same behavior `camera:` below describes | Reuses `CameraConfig` (the same struct `camera:` uses) | Yes |
| `Follow` | Fixed-offset follow camera, no free orbit — good for top-down/side-scrollers | `offset`, `look_at_offset`, `smoothing`, `rotation_smoothing`, `fov` | Yes |
| `FirstPerson` | Camera locked to the target's head position; mouse look controls pitch, yaw rotates the character directly | `eye_offset`, `sensitivity`, `min_pitch`, `max_pitch`, `fov` | Yes |
| `Fixed` | Static camera at a world position, looking at a fixed point or a named tracked entity (re-resolved every frame) | `position`, `look_at` or `look_at_entity`, `fov` | Yes — the mode most `camera_modes:` presets will use |
| `Flycam` | Free-flying, keyboard + mouse look, no target — the same behavior `flycam:` above describes | Reuses `FlyCamDef` (the same struct `flycam:` uses) | Yes |
| `Party` | Shared camera framing every local-coop target at once — in practice always spawned internally by the `split:`/`party:` sibling-field dispatch below, not authored directly (see note) | `look_at_offset`, `zoom_margin`, `min_radius`, `max_radius`, `orbit_speed`, `zoom_speed`, `orbit_button`, `allow_manual_zoom` | **No** — rejected with a load-time `warn!` and an `ironhold_cli validate` error; no per-camera meaning under a `SetCameraMode` fired at one camera |

**RON syntax gotcha (verified, not just written from the schema):** a `CameraModeDef` variant
wrapping a named-field struct (`Orbit`, `Follow`, `FirstPerson`, `Fixed`, `Flycam`) needs a
**double** layer of parens — `Orbit((field: value, ...))`, not `Orbit(field: value, ...)`. The
single-paren form fails `ironhold_cli validate` with `Expected struct CameraConfig but found
"offset"`. This is a real RON rule (a newtype enum variant deserializes its single value as one
complete RON value, and a struct's own RON form is itself `(field: value, ...)`), not a project
typo — every example below already has it right.

```ron
// Single-player orbit camera, camera_mode: form (equivalent to the legacy camera: block below)
components: (
  tags: ["player"],
  camera_mode: Orbit((
    offset:         (0.0, 2.5, 8.0),
    look_at_offset: (0.0, 1.0, 0.0),
    zoom_speed:     8.0,
    orbit_speed:    0.4,
    min_radius:     3.0,
    max_radius:     18.0,
    orbit_button:   "Right",
  )),
),
```

**Where `split:`/`party:` live under the new syntax:** local-coop viewport assignment is a
**sibling** of `camera_mode:`, not nested inside its payload — orthogonal to which camera-following
mode a player uses, same as it always conceptually was. `split:`/`party:` reuse the exact same
`SplitScreenDef`/`PartyZoomDef` types the legacy `camera.split`/`camera.party` fields already use
(see [Split-screen camera](#split-screen-camera-splitscreendef-) / [Shared party
camera](#shared-party-camera-partyzoomdef-) below for their full field tables — those are unchanged
by this section, only their authoring location moves).

```ron
// Local-coop split-screen player, camera_mode: form (equivalent to the legacy nested
// camera.split: form — see local_coop_demo/prefabs/prefabs.ron's player_p1_split_h for the
// live shipped example)
components: (
  tags: ["player"],
  camera_mode: Orbit((
    offset: (0.0, 4.5, 9.0), look_at_offset: (0.0, 1.2, 0.0),
    zoom_speed: 0.0, orbit_speed: 0.4, min_radius: 4.5, max_radius: 9.0,
    orbit_button: "None",
  )),
  split: ( orientation: Horizontal, own_viewport_only: true ),  // sibling, not nested
),
```

Applies identically to `kind: "primitive"` players (`local_coop_demo/room10`'s
`player_p2_primitive_split_ring` already authors a full camera block this way) — the migration
isn't limited to `kind: "actor"` players.

**`Party` is not normally authored directly.** In practice, a designer switches on the shared
party camera via the `party:` sibling field (or `split.dynamic`'s merged state) on the *first*
player only, exactly as `camera.party`/`camera.split.dynamic` already work today — the engine
builds the internal `Party`-mode camera from that dispatch. Authoring `camera_mode: Party(...)`
directly has no wired spawn path for a single player (it will warn and fall back to `Orbit`); it
exists in the schema for completeness and for a future scene-level `Cinematic`-style direct framing
use, not as the everyday co-op switch.

**Backward compatibility:** a prefab with no `camera_mode:` field falls back to the legacy fields —
keyed on the prefab's **tag**, not on whether `camera:`/`flycam:` are present (both are optional
tuning blocks that were *already* fully defaulted when omitted, before this feature existed):
`tags: ["player"]` synthesizes `Orbit` from `camera:` (or engine defaults if omitted); `tags:
["flycam"]` synthesizes `Flycam` from `flycam:` (or engine defaults if omitted). An explicit
`camera_mode:` always overrides both. `3rd_person_game_demo` (all three player prefabs) and
`local_coop_demo`'s room4 pair (`player_p1_split_h`/`player_p2_split_h`) are migrated to the new
syntax as living proof; every other project and room deliberately keeps the legacy form as the
compat path's own regression test.

**`fov:` at spawn is static** — applied once when a camera first spawns/resolves its mode, on every
variant that has the field (`Orbit` via `CameraConfig.fov`, `Follow`, `FirstPerson`, `Fixed`;
`Party`/`Flycam` have no `fov:` field). *Interpolating* `fov` smoothly during a **runtime** mode
switch is handled by `SetCameraMode`'s `transition:` — see the next section.

### `camera_modes:` registry and `SetCameraMode` (v2) ✅

Added in `planning/features/camera_modes.md` v2. `components.camera_mode` (above) is *the mode a
camera starts in* — this section is a different, plural thing: `GameSceneV2.camera_modes` is a
scene-level, named list of **switchable presets** that `Action::SetCameraMode` can jump a camera to
at runtime, from a `rules.ron`/`state_machine.ron` rule or FSM state. It does not change what any
camera starts as.

```ron
// In a .scene.ron file, alongside spawn_points: (same map shape, same nesting level)
spawn_points: {
  "hero_start": (0.0, 0.5, 0.0),
},
camera_modes: {
  "cutscene_fixed": Fixed((
    position: (20.0, 10.0, 0.0),
    look_at_entity: "boss_01",   // an id from this scene's entities: list, not a prefab key
    fov: 50.0,
    transition: (duration_secs: 0.6, ease: EaseIn),
  )),
  "topdown": Follow((
    offset: (0.0, 18.0, 0.1),
    look_at_offset: (0.0, 1.0, 0.0),
    smoothing: 10.0,
    fov: 55.0,
    transition: (duration_secs: 0.3, ease: EaseInOut),
  )),
},
```

```ron
// logic/rules.ron or state_machine.ron
( on: "ui.button_pressed:enter_cutscene", do_actions: [SetCameraMode(mode: "cutscene_fixed")] ),
( on: "dialogue.ended:dialogue/boss_intro.ron", do_actions: [SetCameraMode(mode: "default")] ),
// Local co-op: retarget only player 2's viewport — player 1 keeps playing uninterrupted.
( on: "entity.entered:puzzle_room", do_actions: [SetCameraMode(mode: "topdown", owner_player: 1)] ),
```

**`"default"` is a reserved key, not a preset you define.** `SetCameraMode(mode: "default")`
restores a camera to its *own* scene-authored starting mode (whatever `components.camera_mode` —
or the legacy `camera:`/`flycam:` tag-driven fallback — resolved to when that camera spawned). A
`camera_modes:` entry literally named `"default"` is rejected: a load-time `warn!` and an
`ironhold_cli validate` error, since it would never be reachable under that name anyway.

**`owner_player`** (`SetCameraMode`, and `CameraShake` — see the actions table below) targets one
local-coop player's camera instead of every active camera. `n` matches the `player_index:` field
on a player prefab (see `PrefabDef` above) — `local_coop_demo` room11's `player_p1_camera_switch`
sets `player_index: 0`, which is exactly what `SetCameraMode(mode: "cinematic_fixed", owner_player:
0)` targets:

| `owner_player` | Behavior |
|---|---|
| omitted | Every active camera except a shared Party camera (it can never round-trip back to `"default"`, so it's simply left alone — see `Some(n)`'s last row for why) |
| `Some(n)`, that player has a single-owner camera of their own | Only that camera (even in a `split.dynamic` scene, where the same player is also a member of the merged Party camera's ownership — that shared camera is skipped, not the whole action) |
| `Some(n)`, player hasn't joined yet, or `n` is out of range | `warn!` + no-op |
| `Some(n)`, but that player owns only a shared Party camera | `warn!` + no-op — no single per-player camera to retarget |

**`transition:`** (present on every `CameraModeDef` variant except `Party`) blends the switch
instead of cutting instantly:

```ron
transition: (
  duration_secs: 0.4,
  ease: EaseInOut,   // Linear | EaseIn | EaseOut | EaseInOut — unquoted, see note below
),
```

Absent `transition:` on the *target* mode is an instant cut. When present, the camera's position,
rotation, and FOV all blend from wherever it was the moment `SetCameraMode` fired toward the new
mode's live behavior over `duration_secs`, eased by `ease`. Recommended: keep blends ≤0.4s for
gameplay transitions — player input to the newly-active mode is not suppressed during the blend
(a deliberate simplification; see `planning/claude_suggestions.md` ▸ Camera), so a long blend risks
the player's own mouse/keyboard visibly perturbing the destination camera mid-transition.

> **`ease:` is a real enum, written unquoted** (`ease: EaseInOut`), matching every other
> designer-facing enum on this page (`velocity_curve: EaseOut`, `orientation: Vertical`) — not a
> quoted string. It is a **separate type from `assets.ron`'s `EffectDef`/`LayerDef` `velocity_curve`
> enum** despite sharing `Linear`/`EaseIn`/`EaseOut`: camera transitions need `EaseInOut` (no
> particle use for it) and particles need `Pulse` (no camera use for it), so the two enums don't
> fully overlap and stay distinct rather than one type carrying unused variants on either side.

**Validation:** `ironhold_cli validate` checks `SetCameraMode(mode:)` against `"default"` or a key
present in *some* scene's `camera_modes` (project-scoped rules vs. scene-scoped registries, so a
rule that only makes sense while a *different* scene is loaded isn't caught — the dominant real
mistake, a typo'd key, is), and checks a `Fixed` preset's `look_at_entity` against that same
scene's `entities:` list.

**Worked examples:** `entity_logic_demo/scenes/main.scene.ron` (single-player — press C/V to
round-trip through a `"birdseye"` `Fixed` preset and back to `"default"`) and
`local_coop_demo/scenes/room11.scene.ron` (split-screen — `Digit1`/`Digit2` retarget only player
1's camera via `owner_player: 0`, proving the switch is per-player, not scene-wide, while the
split viewport layout stays unaffected).

### Shared party camera (`PartyZoomDef`) ✅

This is **local, same-machine co-op** — all players share one keyboard/gamepad set on one screen and one camera. It is unrelated to (and does not require) networked multiplayer, which is a separate, planned system (`planning/features/networking_multiplayer.md`) not covered by anything on this page.

For local co-op scenes with two or more `"player"`-tagged entities, `camera.party` on the **first** player switches from one-orbit-camera-per-player to a single shared camera. The shared camera looks at the midpoint of all players and automatically zooms out as they spread apart: its distance is `clamp(max_pairwise_separation + zoom_margin, min_radius, max_radius)`, using the `min_radius`/`max_radius` already set on that same `camera` block.

**`PartyZoomDef` fields** (`components.camera.party`, authored on the first player only):

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `zoom_margin` | `f32` | required | Extra distance (metres) added on top of the players' current maximum separation, so they aren't framed edge-to-edge. Larger values keep more headroom around both players as they split apart. |
| `allow_manual_zoom` | `bool` | `false` | When `true`, the scroll wheel still nudges the camera distance as an offset on top of the distance-derived radius. When `false` (default), the radius is fully derived from player separation and manual scroll input has no effect — this matches "the camera zooms based on how far apart the players are" with no player fighting that behavior. |

> **Dynamic split-screen (`camera.split.dynamic`) reuses these same two fields** under the names `merged_zoom_margin`/`merged_allow_manual_zoom` — dynamic split spawns its own internal shared camera for its merged state (rather than requiring a separate `party:` block alongside `split:`), tuned identically to `zoom_margin`/`allow_manual_zoom` above. See [Dynamic split-screen](#dynamic-split-screen-dynamicsplitdef-) below.

> **Only the first `"player"`-tagged entity's `party` block is read.** If you author `party` on the second (or later) player instead of the first, it is silently ignored — there is no validation error. The engine always reads whichever `"player"`-tagged entity appears first in the scene's `entities` list.

**Example** — two-player scene where player 1 owns the shared camera:

```ron
// prefabs/prefabs.ron
"player_p1": (
  kind: Actor,
  model: "character_male",
  display_name: "Player 1",
  player_index: 0,
  components: (
    tags: ["player"],
    camera: (
      offset:         (0.0, 7.0, 14.0),
      look_at_offset: (0.0, 1.2, 0.0),
      zoom_speed:     8.0,
      orbit_speed:    0.4,
      min_radius:     8.0,
      max_radius:     28.0,
      orbit_button:   "Right",
      // Shared-camera zoom: radius = clamp(max player separation + zoom_margin, min, max).
      party: (
        zoom_margin: 6.0,
      ),
    ),
    inputs: (
      forward: "KeyW", backward: "KeyS", left: "KeyA", right: "KeyD",
      strafe_left: "KeyQ", strafe_right: "KeyE", jump: "Space", run: "ShiftLeft",
    ),
  ),
),

// player_p2 — no `camera` block at all; with 2+ players only player_p1's camera
// (the first "player"-tagged entity in the scene) is read.
"player_p2": (
  kind: Actor,
  model: "character_female",
  display_name: "Player 2",
  player_index: 1,
  components: (
    tags: ["player"],
    inputs: (
      forward: "ArrowUp", backward: "ArrowDown", left: "ArrowLeft", right: "ArrowRight",
      strafe_left: "ArrowLeft", strafe_right: "ArrowRight", jump: "Enter", run: "ShiftRight",
    ),
  ),
),
```

A full working example lives in `assets/projects/local_coop_demo/`.

### Split-screen camera (`SplitScreenDef`) ✅

This is **local, same-machine co-op** — same scope note as the shared party camera above: unrelated to (and does not require) networked multiplayer.

For local co-op scenes with two or more `"player"`-tagged entities, `camera.split` on the **first** player switches from "one shared camera framing everyone" to **one real camera per player**, each locked to its own rectangle of the window. Unlike `party`, split-screen does not derive a shared midpoint/zoom — every player gets a fully independent `OrbitCamera` built from their **own** `camera` block, so `offset`, `zoom_speed`, `orbit_button`, etc. need to be authored correctly on **every** player's prefab, not just player 1's. Only the `split` switch field itself is read exclusively from the first player.

**`SplitScreenDef` fields** (`components.camera.split`, authored on the first player only):

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `orientation` | `SplitOrientation` | `Vertical` | **Dual meaning, depending on `dynamic`.** When `dynamic` is **unset**, this is the fixed split axis used for the whole scene (`Vertical`/`Horizontal`/`Grid`). When `dynamic` **is** set (`Vertical`/`Horizontal` only — `dynamic` does not support `Grid`), the live split axis is instead chosen automatically every time the view splits, from the players' actual relative position — `orientation` becomes only a rare tie-break hint, used on the exact frame the two players are equally separated on both axes. Optional either way — omit it to get `Vertical`. |
| `dynamic` | `Option<DynamicSplitDef>` | `None` | When set, the view starts **merged** into one shared camera (like `party`) and automatically switches to a two-camera split once the players separate far enough, merging back when they come close again. See [Dynamic split-screen](#dynamic-split-screen-dynamicsplitdef-) below. |
| `own_viewport_only` | `bool` | `false` | When `true`, each player's target-indicator ring (see [Target indicator](#target-indicator-targetindicatordef) above) is visible only in **that player's own** split viewport, instead of every ring being visible in every viewport (today's default). See [Per-viewport target ring visibility](#per-viewport-target-ring-visibility-own_viewport_only) below. |

**`SplitOrientation` variants:**
- `Vertical` — the window is split down the middle into a left half and a right half, one player per half. Always exactly 2-way.
- `Horizontal` — the window is split down the middle into a top half and a bottom half, one player per half. Always exactly 2-way.
- `Grid` — N-way split for 2 to `MAX_SPLIT_PLAYERS` (4) players, laid out in a grid computed from the actual player count. Static only (no `dynamic` support). See [Grid split-screen](#grid-split-screen--n-way-splitorientationgrid-) below.
- All three variants recompute every frame from the window's actual size, so they stay correct across resizes and on HiDPI displays.

> **Only the first `"player"`-tagged entity's `split` block is read.** Same rule as `party` — if you author `split` on the second (or later) player instead of the first, it is silently ignored. The engine always reads whichever `"player"`-tagged entity appears first in the scene's `entities` list.

> **`split` and `party` are mutually exclusive.** If both are set on the first player's `camera` block by mistake, the engine logs a warning and `split` wins.

> **Every player's own `camera` block matters here — not just the first player's.** With `party`, only player 1's `camera` fields (besides `party` itself) are used, because there is only one shared camera. With `split`, each player gets their own real `OrbitCamera` built from their own config, so `offset`, `look_at_offset`, `zoom_speed`, `min_radius`/`max_radius`, `orbit_button`, etc. must be set on **every** player's `camera` block. Only `split` (and `party`) themselves stay first-player-only.

> **Disabling manual mouse camera control.** A single shared mouse would otherwise orbit/zoom every split-screen player's camera identically, which looks wrong. Split-screen scenes should set `orbit_button: "None"` (see the `CameraConfig` table above) and `zoom_speed: 0.0` on **every** player's `camera` block, giving each player a fixed-angle camera at their configured offset with no mouse control — each player can still independently turn their own camera via keyboard (`InputMap.look_left`/etc.), see the "Keyboard camera look" note above.

**Example** — two-player scene with a vertical split, both cameras fixed (no manual mouse control):

```ron
// prefabs/prefabs.ron
"player_p1_split": (
  kind: Actor,
  model: "character_male",
  display_name: "Player 1",
  player_index: 0,
  components: (
    tags: ["player"],
    camera: (
      offset:         (0.0, 4.5, 9.0),
      look_at_offset: (0.0, 1.2, 0.0),
      zoom_speed:     0.0,
      orbit_speed:    0.4,
      min_radius:     4.5,
      max_radius:     9.0,
      orbit_button:   "None",
      // Sole switch for split-screen — read only from this (the first) player.
      split: (
        orientation: Vertical,
      ),
    ),
    inputs: (
      forward: "KeyW", backward: "KeyS", left: "KeyA", right: "KeyD",
      strafe_left: "KeyQ", strafe_right: "KeyE", jump: "Space", run: "ShiftLeft",
      strafe_mouse_button: None,
    ),
  ),
),

// player_p2_split — no `split`/`party` here (only the first player's is read for those),
// but its OWN camera block still fully matters: split-screen spawns one real camera per
// player from their own config, unlike `party`'s single shared camera.
"player_p2_split": (
  kind: Actor,
  model: "character_female",
  display_name: "Player 2",
  player_index: 1,
  components: (
    tags: ["player"],
    camera: (
      offset:         (0.0, 4.5, 9.0),
      look_at_offset: (0.0, 1.2, 0.0),
      zoom_speed:     0.0,
      orbit_speed:    0.4,
      min_radius:     4.5,
      max_radius:     9.0,
      orbit_button:   "None",
    ),
    inputs: (
      forward: "ArrowUp", backward: "ArrowDown", left: "ArrowLeft", right: "ArrowRight",
      strafe_left: "ArrowLeft", strafe_right: "ArrowRight", jump: "Enter", run: "ShiftRight",
      strafe_mouse_button: None,
    ),
  ),
),
```

A full working example lives in `assets/projects/local_coop_demo/` — see `prefabs/prefabs.ron` (`player_p1_split`/`player_p2_split`) and `scenes/room3.scene.ron`.

#### Per-viewport target ring visibility (`own_viewport_only`)

By default (`own_viewport_only: false`, the implicit default when the field is omitted), every
split-screen player's target-indicator ring is visible in **every** player's viewport — a P2 ring
still shows up in P1's half of the screen and vice versa. Each ring is still tinted per-owner via
`PLAYER_LABEL_COLORS` whenever 2+ players are present (see [Per-player split-screen
targeting](#per-player-split-screen-targeting) above), so whose ring is whose is still visually
distinguishable — `own_viewport_only` is a separate, additional restriction on top of that tinting,
not a replacement for it.

Setting `own_viewport_only: true` on the **first** player's `camera.split` block (same
first-player-only convention as `orientation`/`dynamic` above) changes this: each player's ring
becomes visible only in their own viewport. Applies to `Vertical`/`Horizontal`/`Grid`/`dynamic`
split — anything with real per-player cameras. Since the field lives under `split:`, a genuinely
`party`-only scene has no `split:` block to author it on at all. The reachable inert case is a
scene that authors a `split:` block (with `own_viewport_only: true`) but ends up with fewer than 2
`"player"`-tagged entities at load — the same copy-pasted-split-block trap as any other `split:`
field: it's simply never read, no warning is logged, and this is a documented no-op, not a bug.

> **With `dynamic` split, the restriction only applies while the view is actually split.** In the
> merged state (one shared camera, players close together) every player's ring is visible in that
> single shared view again — there are no separate viewports left to restrict them to. Expect rings
> to appear and disappear as the view merges and splits; this is correct, not a bug.

> **This does not change ring colour/tint.** `own_viewport_only` only restricts *where* a ring
> renders. The `PLAYER_LABEL_COLORS` tinting described in [Per-player split-screen
> targeting](#per-player-split-screen-targeting) above (whenever 2+ players are present) and the
> single-player `indicator_color`/`indicator_category`/scene `color` precedence are completely
> unaffected either way. If you want each player to *only* see their own ring, set this field; if
> you want rings to stay visually distinguishable across viewports, that tinting already happens
> automatically once 2+ players are present — no extra field needed for that part.

> **Give every player a unique `player_index` (0 through 3).** Ring/camera visibility is keyed on
> `player_index % 4` — two players whose index collides under that modulo (a duplicate value, or
> any index of 4 or higher) end up sharing a reserved viewport, silently defeating this field for
> that pair (a scene-load warning is logged if this happens). `own_viewport_only: true` combined
> with a non-hot-join `Action::Spawn` of a new `"player"`-tagged prefab is also warned rather than
> silently mishandled: that spawn path's camera never restricts its own viewport, so the new player
> would otherwise see no rings at all, not even their own.

**Example** — same two-player vertical split as above, with rings restricted to each player's own viewport:

```ron
// prefabs/prefabs.ron
"player_p1_split_ring": (
  kind: Actor,
  model: "character_male",
  display_name: "Player 1",
  player_index: 0,
  components: (
    tags: ["player"],
    camera: (
      offset:         (0.0, 4.5, 9.0),
      look_at_offset: (0.0, 1.2, 0.0),
      zoom_speed:     0.0,
      orbit_speed:    0.4,
      min_radius:     4.5,
      max_radius:     9.0,
      orbit_button:   "None",
      split: (
        orientation: Vertical,
        own_viewport_only: true,  // P1 no longer sees P2's ring, and vice versa
      ),
    ),
    inputs: (
      forward: "KeyW", backward: "KeyS", left: "KeyA", right: "KeyD",
      strafe_left: "KeyQ", strafe_right: "KeyE", jump: "Space", run: "ShiftLeft",
      strafe_mouse_button: None,
    ),
  ),
),

// player_p2_split_ring — no `split` here (only the first player's is read for the switch);
// its ring visibility follows the first player's `own_viewport_only` automatically.
"player_p2_split_ring": ( /* identical shape to player_p2_split above */ ),
```

A full working example lives in `assets/projects/local_coop_demo/` — see `prefabs/prefabs.ron`
(`player_p1_split_ring`/`player_p2_split_ring`) and `scenes/room9.scene.ron`. For the same
`own_viewport_only` setup with a primitive-bodied P2 instead, see `scenes/room10.scene.ron` and
its `player_p2_primitive_split_ring` prefab.

### Dynamic split-screen (`DynamicSplitDef`) ✅

This is **local, same-machine co-op** — same scope note as the two sections above: unrelated to (and does not require) networked multiplayer.

`camera.split.dynamic` on the **first** player makes the view start **merged** — one shared camera framing both players, just like `party` — and automatically **split** into two independent per-player cameras once the players separate beyond `split_distance`. It automatically **merges back** once they come within `merge_distance` of each other. The two thresholds are deliberately different (hysteresis) so the view doesn't flicker back and forth for players hovering right at one boundary.

Dynamic mode is self-contained: it does **not** require also authoring a `party:` block. Internally it spawns its own shared camera for the merged state, tuned by `merged_zoom_margin`/`merged_allow_manual_zoom` — these mirror `PartyZoomDef.zoom_margin`/`PartyZoomDef.allow_manual_zoom` exactly (see [Shared party camera](#shared-party-camera-partyzoomdef-) above). This keeps `party` and `split` mutually exclusive as a simple either/or switch, even when `split` itself internally behaves like `party` part of the time.

**`DynamicSplitDef` fields** (`components.camera.split.dynamic`, authored on the first player only):

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `split_distance` | `f32` | required | Distance (metres) between the two players beyond which the merged view splits into two independent cameras. No built-in default — the right value depends on room size and player `walk_speed`, so it must be tuned per scene. |
| `merge_distance` | `f32` | required | Distance (metres) below which a split view merges back into the single shared camera. Must be smaller than `split_distance` — the gap between the two is what prevents flicker right at the boundary. If authored backwards, a warning is logged and the value is clamped just below `split_distance`. |
| `merged_zoom_margin` | `f32` | required | Extra distance (metres) added beyond the players' current separation while merged, so they aren't framed edge-to-edge. Same meaning as `PartyZoomDef.zoom_margin`. |
| `merged_allow_manual_zoom` | `bool` | `false` | When `true`, the scroll wheel still nudges the merged camera's distance as an offset on top of the distance-derived radius. When `false` (default), the radius is fully derived from player separation while merged. Same meaning as `PartyZoomDef.allow_manual_zoom`. |

> **The split axis is chosen automatically, not authored.** Unlike fixed-orientation `split` (where you set `orientation: Vertical` or `orientation: Horizontal` up front), dynamic mode picks the axis itself every time the view transitions from merged to split, based on whether the players are further apart left-right or front-back at that moment. It then holds that axis fixed for the rest of the split period — it will not flip mid-split even if the players' relative position changes. `orientation` on `SplitScreenDef` is only consulted as a tie-break on the rare frame where the separation is exactly equal on both axes.

> **All three cameras exist for the whole scene.** Dynamic mode does not spawn or despawn cameras as the view merges/splits — the shared camera and both per-player cameras are created once, up front, and the engine simply toggles which ones are active. This avoids any pop or snap when switching: the inactive cameras keep tracking their targets in the background the entire time.

> **Every player's own `camera` block still matters**, same as fixed-orientation `split` — each player needs their own `offset`, `zoom_speed`, `orbit_button`, etc. authored, since those drive the per-player cameras used once the view splits (`orbit_button: "None"` disables *mouse* orbit only — `InputMap.look_left`/etc. still works per player, see "Keyboard camera look" above). Only `split.dynamic` itself is read exclusively from the first player.

**Example** — two-player scene that starts merged and splits once the players are more than 10 m apart, merging back under 6 m:

```ron
// prefabs/prefabs.ron
"player_p1_dynamic": (
  kind: Actor,
  model: "character_male",
  display_name: "Player 1",
  player_index: 0,
  components: (
    tags: ["player"],
    camera: (
      offset:         (0.0, 4.5, 9.0),
      look_at_offset: (0.0, 1.2, 0.0),
      zoom_speed:     0.0,
      orbit_speed:    0.4,
      min_radius:     4.5,
      max_radius:     18.0,
      orbit_button:   "None",
      // Sole switch for dynamic split — read only from this (the first) player.
      split: (
        dynamic: (
          split_distance:           10.0,
          merge_distance:           6.0,
          merged_zoom_margin:       6.0,
          merged_allow_manual_zoom: false,
        ),
      ),
    ),
    inputs: (
      forward: "KeyW", backward: "KeyS", left: "KeyA", right: "KeyD",
      strafe_left: "KeyQ", strafe_right: "KeyE", jump: "Space", run: "ShiftLeft",
      strafe_mouse_button: None,
    ),
  ),
),

// player_p2_dynamic — no `split` here (only the first player's is read for the switch), but
// its OWN camera block still matters once the view splits — same rule as fixed-orientation split.
"player_p2_dynamic": (
  kind: Actor,
  model: "character_female",
  display_name: "Player 2",
  player_index: 1,
  components: (
    tags: ["player"],
    camera: (
      offset:         (0.0, 4.5, 9.0),
      look_at_offset: (0.0, 1.2, 0.0),
      zoom_speed:     0.0,
      orbit_speed:    0.4,
      min_radius:     4.5,
      max_radius:     18.0,
      orbit_button:   "None",
    ),
    inputs: (
      forward: "ArrowUp", backward: "ArrowDown", left: "ArrowLeft", right: "ArrowRight",
      strafe_left: "ArrowLeft", strafe_right: "ArrowRight", jump: "Enter", run: "ShiftRight",
      strafe_mouse_button: None,
    ),
  ),
),
```

### Grid split-screen — N-way (`SplitOrientation::Grid`) ✅

This is **local, same-machine co-op** — same scope note as the sections above: unrelated to (and does not require) networked multiplayer.

`orientation: Grid` generalizes fixed-orientation `split` (`Vertical`/`Horizontal`, always exactly 2-way) to any player count from 2 up to `MAX_SPLIT_PLAYERS` (4). Quadrant/cell layout is computed automatically from however many `"player"`-tagged entities the scene has: `cols = ceil(sqrt(count))`, `rows = ceil(count / cols)`. For `count == 4` this produces a clean 2×2 quadrant grid — the only player count this engine's example content actually ships. **Static only** — `Grid` does not support `dynamic` (no merge/split-by-distance for 3+ players).

**Quadrant assignment is entity order, not an authored field.** Slot `0` is the first `"player"`-tagged entity in the scene's `entities` list, slot `1` the second, and so on — same "first player wins for the switch field" rule as `party`/`Vertical`/`Horizontal`, extended here to determine every player's on-screen position too. Cells fill row-major: for a 4-player grid, slot `0` = top-left, `1` = top-right, `2` = bottom-left, `3` = bottom-right.

> **`count == 3` leaves one grid cell empty.** There is no special-cased 3-way layout (e.g. one wide top pane + two bottom panes) — a 3-player `Grid` scene still computes a 2×2 grid and simply never assigns a camera to the 4th cell, which renders as the window's clear color.

> **More than `MAX_SPLIT_PLAYERS` (4) players spawn cameraless.** Consistent with the existing (pre-`Grid`) behavior when a 3rd player exists in a `Vertical`/`Horizontal` scene — extra players beyond the cap simply don't get a `SplitViewportSlot` camera, they still spawn and can still move, just without their own rendered view.

> **Every player's own `camera` block matters**, same as fixed-orientation `split` — each of the (up to 4) players needs their own `offset`, `zoom_speed`, `orbit_button`, etc. authored, disabling manual *mouse* control the same way (`orbit_button: "None"`, `zoom_speed: 0.0`) so one shared mouse doesn't move every camera at once — each player can still turn their own camera via keyboard (`InputMap.look_left`/etc., see "Keyboard camera look" above). Only `split` itself is read exclusively from the first player.

**Example** — 4-player scene with a grid split, all 4 cameras fixed (no manual mouse control):

```ron
// prefabs/prefabs.ron
"player_p1_grid": (
  kind: Actor,
  model: "character_male",
  material: "tint_blue",   // solid-color tint — see `PrefabDef.material` / `AssetCatalog.materials` above
  display_name: "Player 1",
  player_index: 0,
  components: (
    tags: ["player"],
    camera: (
      offset:         (0.0, 4.5, 9.0),
      look_at_offset: (0.0, 1.2, 0.0),
      zoom_speed:     0.0,
      orbit_speed:    0.4,
      min_radius:     4.5,
      max_radius:     9.0,
      orbit_button:   "None",
      // Sole switch for split-screen — read only from this (the first) player. Player count
      // is read from the scene's entity count at load, not authored here.
      split: (
        orientation: Grid,
      ),
    ),
    inputs: (
      forward: "KeyW", backward: "KeyS", left: "KeyA", right: "KeyD",
      strafe_left: "KeyQ", strafe_right: "KeyE", jump: "Space", run: "ShiftLeft",
      strafe_mouse_button: None,
    ),
  ),
),

// player_p3_grid — a THIRD keyboard scheme (IJKL) sharing the same physical keyboard as
// player_p1_grid's WASD and player_p2_grid's arrow keys. No `split` here — only the first
// player's is read for the switch — but this player's own `camera` block still matters.
"player_p3_grid": (
  kind: Actor,
  model: "character_male",
  material: "tint_dark_green",
  display_name: "Player 3",
  player_index: 2,
  components: (
    tags: ["player"],
    camera: (
      offset:         (0.0, 4.5, 9.0),
      look_at_offset: (0.0, 1.2, 0.0),
      zoom_speed:     0.0,
      orbit_speed:    0.4,
      min_radius:     4.5,
      max_radius:     9.0,
      orbit_button:   "None",
    ),
    inputs: (
      forward: "KeyI", backward: "KeyK", left: "KeyJ", right: "KeyL",
      strafe_left: "KeyJ", strafe_right: "KeyL", jump: "KeyU", run: "KeyO",
      strafe_mouse_button: None,
    ),
  ),
),

// player_p4_grid — a FOURTH scheme using the numeric keypad (`Numpad0`-`Numpad9` — see the
// "Valid key name strings" table above). Deliberately the lower half of the numpad (5/2/1/3 + 0 + 4), not
// the more common 8/4/6/2 cluster — 8-and-2-with-5-in-the-middle is uncomfortable to play.
"player_p4_grid": (
  kind: Actor,
  model: "character_female",
  material: "tint_red",
  display_name: "Player 4",
  player_index: 3,
  components: (
    tags: ["player"],
    camera: (
      offset:         (0.0, 4.5, 9.0),
      look_at_offset: (0.0, 1.2, 0.0),
      zoom_speed:     0.0,
      orbit_speed:    0.4,
      min_radius:     4.5,
      max_radius:     9.0,
      orbit_button:   "None",
    ),
    inputs: (
      forward: "Numpad5", backward: "Numpad2", left: "Numpad1", right: "Numpad3",
      strafe_left: "Numpad1", strafe_right: "Numpad3", jump: "Numpad0", run: "Numpad4",
      strafe_mouse_button: None,
    ),
  ),
),
```

A full working example (all 4 players, including `player_p2_grid`) lives in `assets/projects/local_coop_demo/` — see `prefabs/prefabs.ron` and `scenes/room6.scene.ron`.

### Local co-op hot join ✅

Lets a new player join an **already `Grid`-split** running scene at runtime — no scene reload,
and the existing players' cameras/state are completely untouched. Growing *into* split-screen
from a single-player scene, or hot-**leaving**, are not supported in v1 — see
`planning/features/local_coop_hot_join_leave.md`.

**Only `Grid`-split scenes support hot-join.** `party`, `dynamic` split, fixed `Vertical`/
`Horizontal` split, and ordinary single-camera scenes all no-op (with a `warn!`) on
`Action::JoinPlayer` — the same "first player's `camera.split`" convention that gates every other
split-screen feature also gates this one.

**`join_prefab_keys: Vec<Option<String>>`** (scene-level field, see the fields table above) maps
each **absolute slot number** (0-based — the same numbering as `PlayerIndex`/`SplitViewportSlot`)
to the prefab a joiner into that slot should use. A scene's `entities:`-declared players already
occupy the low slots, so a 2-player scene that wants a 3rd/4th hot-join slot writes:

```ron
// scenes/my_scene.scene.ron
join_prefab_keys: [None, None, "player_p3_grid", "player_p4_grid"],
```

Slots `0`/`1` are `None` because those players are already scene-authored — only the *empty*
slots need an entry. A join into a slot with no entry (index past the end of the list, or an
explicit `None`) is a no-op with a `warn!` — nothing crashes, but nothing joins either.

**`Action::JoinPlayer`** (no payload) spawns the next joiner:

1. Computes the next slot from the live split-slot count, **plus** any `Action::JoinPlayer`s
   already queued (but not yet spawned) this same frame — two joins processed in one frame never
   collide on the same slot.
2. No-ops with a `warn!` if that slot would exceed `MAX_SPLIT_PLAYERS` (4) — emits
   `coop.lobby_full` on the join that brings the count *to* the cap, so a scene can hide a
   "press to join" prompt on that event (see the caveat below).
3. Resolves the joiner's prefab from `join_prefab_keys[next_slot]`.
4. Resolves the joiner's spawn position from `spawn_points["player_{next_slot + 1}_start"]` —
   1-based, matching the `player_1_start"`.."player_4_start"` naming `room6.scene.ron` already
   uses (`next_slot` itself, like `PlayerIndex`, is 0-based). `room6`'s own `spawn_points` are
   never actually read by anything — its players are placed via `transform`, not spawn points —
   so this is a fresh live consumer of that naming convention, not a pre-tested one; get the
   `+ 1` offset wrong and a joiner silently lands on an existing player's position instead of
   their own. Falls back to the primary player's current position + a small
   `(1.5 * slot, 0, 0)` offset if that spawn point isn't defined.
5. **Overrides `PlayerIndex` to `next_slot`**, regardless of whatever `player_index` the joined
   prefab itself declares. A prefab reused as a join target for a different slot in a different
   scene (e.g. `player_p3_grid`'s baked `player_index: 2`) always gets the slot's index at join
   time, not its own baked value — don't rely on the prefab's own `player_index` for a hot-joined
   player, only for one placed directly in `entities:`.

Typically bound via a scene-scoped key and a rule:

```ron
// scenes/my_scene.scene.ron
scene_key_bindings: {
  "KeyG": "join",   // avoid "KeyJ" if any join-target prefab's own inputs use it (see below)
},
```
```ron
// logic/rules.ron
( on: "ui.button_pressed:join", do_actions: [ JoinPlayer ] ),
```

> **Pick a join key that no join-target prefab's own `inputs:` also binds.** `global_input_system`
> reads the physical key unconditionally, independent of any player's `InputMap` — so if a scene's
> join key collides with an already-joined player's own movement/action key (e.g. binding join to
> `"KeyJ"` in a scene whose IJKL-scheme player uses `KeyJ` for `left`/`strafe_left`), that player's
> ordinary movement input will *also* re-trigger `Action::JoinPlayer` every time they press it —
> harmless once the lobby is full (a spurious `warn!` each press), but a real join can accidentally
> fire early otherwise. `assets/projects/local_coop_demo/scenes/room8.scene.ron` uses `"KeyG"` for
> exactly this reason.

> **`SetEntityVisible` cannot target a scene-authored UI `Label`** (verified during
> implementation) — `Action::SetEntityVisible` resolves its `entity` argument through the same
> `SpawnRegistry` that 3D `entities:` use, and UI nodes are never registered there. A "press to
> join" prompt therefore can't be hidden via `coop.lobby_full` + `SetEntityVisible` in v1 — author
> it as an always-visible `Label` instead (see `room8.scene.ron`'s `join_prompt`), or react to
> `coop.lobby_full` some other way (e.g. a bound `GameVariable` swapping the label's own text).

A full working example (2-player start, grows to 4 via hot-join) lives in
`assets/projects/local_coop_demo/scenes/room8.scene.ron`.

#### Gamepad-triggered hot join ✅

A new player can also join by pressing a button on an **unclaimed** physical gamepad — the
gamepad equivalent of the keyboard trigger above. `global_unclaimed_gamepad_bindings`/
`scene_unclaimed_gamepad_bindings` (see the fields tables above) map a gamepad button name to a
trigger name, same format and same project-base/scene-override merge rule as `global_key_bindings`/
`scene_key_bindings`:

```ron
// {name}.project.ron
global_unclaimed_gamepad_bindings: {
  "South": "join",   // Xbox A / PlayStation Cross
},
```
```ron
// scenes/my_scene.scene.ron — same shape as a scene_key_bindings override
scene_unclaimed_gamepad_bindings: {
  "South": "join",
},
```
```ron
// logic/rules.ron — same rule as the keyboard trigger, no gamepad-specific rule needed
( on: "ui.button_pressed:join", do_actions: [ JoinPlayer ] ),
```

> **These maps are for join-style triggers only — not a general gamepad analogue of
> `global_key_bindings`.** A button press is only ever detected on a pad **not currently assigned
> to** any live player — so binding something like
> `"Start": "toggle_pause"` here will simply never fire for an actual playing player; it only
> fires from a pad nobody is using yet. For an already-joined player's own in-game gamepad
> actions, use that player's own `InputMap` fields (`gamepad_jump`/`gamepad_run`/
> `gamepad_interact`/`gamepad_target_next`) instead — this pair of maps exists specifically so an
> unclaimed pad can signal "a new player wants in," nothing more general.

Binding `"South"` here is safe despite it being `gamepad_jump`'s own default — an already-joined
player's own South-button jump is on a *claimed* pad and never reaches this trigger, so there's no
collision the way there can be with the keyboard collision warning above. No separate "is this pad
actually live" prefilter exists or is needed — a phantom/dead duplicate gamepad entry (see the
phantom-pad troubleshooting note above) reports zero for every button forever, so it can never
produce the button-press edge this trigger requires; requiring that edge already excludes it.

**At most one (pad, button) match is serviced per frame, full stop.** If two unclaimed pads happen
to press in the exact same frame, only the one sorted first (by connection order) joins that
frame — the other's press is not serviced at all this frame (not queued, not delayed) and simply
needs a second press. This is a hard cap, not just a "which pad gets bound" tiebreak: it exists
because nothing downstream can tell which of two same-frame join presses a bound pad belongs to,
so allowing both through could produce a second player with no pad bound at all, permanently (v1
has no hot-leave to fix a bad join after the fact).

**The join trigger must be produced synchronously, in the same frame's rule match** — i.e. a
direct `( on: "ui.button_pressed:<trigger>", do_actions: [ JoinPlayer ] )` rule, not a chain that
routes through `EmitEvent` into a second rule or a delayed event. The pad identity is only valid
for the one frame it was captured; a `JoinPlayer` that fires a frame later still joins, just with
no gamepad bound (falls back to the join prefab's own authored default), which can look like a
confusing "sometimes gamepad join doesn't work" bug if a project's rules aren't wired directly.

**The first press counts — no priming press or hold needed.** A pad's very first button press
after connecting can be the join itself. One browser-specific caveat: Chrome/Edge don't expose a
freshly-connected gamepad to the page at all until it sends some input, so on a brand-new
controller the very first press may be consumed just making the pad visible to the browser rather
than reaching this system. That's a browser platform rule, not an engine behavior — if the very
first press seems to do nothing, press again.

**A gamepad only ever binds to a player at the moment they join by pressing it.** There is no way
to hand a controller to an already-joined player, or to a player placed directly in a scene's
`entities:` list — give those their own `gamepad_index` in the player prefab's `inputs:` block
instead (see the `InputMap` fields table above).

**`Action::JoinPlayer` additionally binds the pressing pad directly to the new player** when
triggered by a gamepad (a keyboard-triggered join never inherits a pad identity from an earlier
frame — the identity is reset every frame it isn't freshly captured) — mirrors the
`PlayerIndex`-override behavior above (step 5), but for the controller instead. This is the exact
same controller that pressed the join button, assigned immediately with no extra frame of delay —
not merely a `gamepad_index` seed that gets re-resolved later. **This does not disable that
player's keyboard scheme** — gamepad and keyboard input are checked additively (`||`), never
exclusively, everywhere in this engine, so a gamepad-joined player can freely mix their join
slot's authored keyboard scheme and the bound pad (confirmed against source during implementation,
not assumed). Unplugging and replugging this controller into the **same** port later resumes this
player's gamepad input automatically — see [How a controller gets assigned to a player](#how-a-controller-gets-assigned-to-a-player) above.

A full working example (gamepad join alongside the keyboard one) lives in
`assets/projects/local_coop_demo/scenes/room8.scene.ron`.

### Split-screen player HUD labels ✅

**Fully engine-automatic — no RON field to author.** Whenever a scene has real split-screen
cameras (`Vertical`/`Horizontal`/`Grid` — i.e. any camera tagged `SplitViewportSlot`), each one
automatically gets a colored "P1"/"P2"/"P3"/"P4" corner label in its own cell, top-right anchored,
that updates live as the window resizes and hides/shows correctly across a `dynamic` split's
merge/split transitions. Party-mode and single-player scenes never get a label — they have no
`SplitViewportSlot` camera to attach one to.

> **Label text is driven by `player_index`, not by scene entity/spawn order.** The label reads
> each split camera's target's `PlayerIndex` component (forwarded from `PrefabDef.player_index`)
> — it is NOT derived from the order entities appear in the scene's `entities:` list. Give each
> player prefab a distinct `player_index` that matches its intended quadrant (as every
> `local_coop_demo` example already does — `player_p1_grid` through `player_p4_grid` set
> `player_index: 0` through `3`); otherwise two players could show the same "P" number, or a
> label that doesn't match its actual quadrant.

> **Label color is a fixed engine palette, independent of `material:`.** The four label colors are
> hardcoded (`PLAYER_LABEL_COLORS` in `capabilities/camera.rs`) to visually match
> `local_coop_demo`'s room6 tints (`tint_blue`/`tint_pink`/`tint_dark_green`/`tint_red`), but they
> are **not** read from a player's actual `material:` field — rooms 3/4/5 use plain untinted
> models and still get colored labels. Re-tinting a player's `material` in RON does **not** move
> the label's color; the two are deliberately independent.

No other configuration exists for this feature today — no opt-out, no repositioning, no
controller-icon variant. See `crates/ironhold_core/src/CLAUDE.md` for the underlying
`SplitScreenPlayerLabel`/`LinkedPlayerLabel` component pattern.

> The per-viewport **target HUD readout** (`target_hud:`, see
> [Per-player split-screen targeting](#per-player-split-screen-targeting) above) follows this exact
> same "no opt-out, no repositioning" placement precedent, anchored bottom-left instead of
> top-right so the two never collide.

### NPC behaviour (`components.npc`) ✅

Set `components.npc` on any prefab to attach NPC AI. The engine spawns a dynamic Rapier capsule body and runs the behaviour system each physics tick.

> **GLB Actor capsule size:** For `kind: Primitive` NPCs the physics capsule is sized from the primitive's `radius`/`height` parameters. For `kind: Actor` (GLB model) NPCs the capsule defaults to **0.35 m radius, 1.6 m total height**; set `collider_radius` and `collider_height` in the `npc:` block to override these for non-humanoid creatures.

Events emitted:

- `npc.player_spotted:{id}` — player entered detection range and the alerted pause has elapsed
- `npc.player_reached:{id}` — NPC is within `approach_distance` of the player
- `npc.player_lost:{id}` — player left `chase_radius`; NPC enters the Return state

**`NpcDef` fields** (`components.npc`):

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `faction` | `NpcFaction` | — | `Friendly`, `Hostile`, or `Neutral` |
| `on_player_near` | `NpcOnPlayerNear` | — | `Chase`, `Interact`, `Flee`, or `Alert` |
| `detection_radius` | `f32` | — | Metres inside which the NPC enters the Alerted state |
| `chase_radius` | `f32` | — | Metres beyond which the NPC gives up and returns to patrol |
| `fov_degrees` | `Option<f32>` | `None` (360°) | Half-angle forward cone in degrees; `None` = no blind spot |
| `requires_los` | `bool` | `false` | Rapier ray cast must confirm unobstructed line of sight before detecting the player |
| `approach_distance` | `f32` | `2.0` | Stop approaching at this distance (interact / attack range) |
| `patrol_speed` | `f32` | `2.0` | m/s while walking the patrol route or returning to origin |
| `chase_speed` | `f32` | `4.5` | m/s while chasing or fleeing |
| `patrol_waypoints` | `Vec<(f32,f32,f32)>` | `[]` | Offsets relative to spawn position; empty = idle in place |
| `eye_height` | `f32` | `0.9` | Metres above the entity origin used for LOS ray origin (tune for short/tall NPCs) |
| `alerted_duration` | `f32` | `0.3` | Seconds the NPC pauses in the Alerted state before acting |
| `drag` | `f32` | `0.8` | Velocity decay multiplier per physics tick when not actively moving (0 = instant stop, 1 = no decay) |
| `waypoint_reach_radius` | `f32` | `0.5` | Metres from a waypoint at which the NPC advances to the next one |
| `interact_leave_factor` | `f32` | `1.5` | Multiplier on `approach_distance` defining the leave-interact threshold (`distance > approach_distance * factor` exits Interact state) |
| `home_arrival_radius` | `f32` | `0.5` | Metres from spawn origin at which the NPC considers itself home and ends Return state |
| `linear_damping` | `f32` | `0.5` | Rapier `linear_damping` on the NPC capsule rigid body |
| `angular_damping` | `f32` | `0.5` | Rapier `angular_damping` on the NPC capsule rigid body |
| `collider_radius` | `Option<f32>` | `None` (0.35 m) | Radius of the NPC's physics capsule; tune up for large creatures (e.g. dragon) or down for small ones (e.g. imp). Omit to keep the humanoid default. |
| `collider_height` | `Option<f32>` | `None` (1.6 m) | Total height of the NPC's physics capsule; tune for creatures significantly taller or shorter than a humanoid. Omit to keep the humanoid default. |
| `investigate_timeout_secs` | `f32` | `5.0` | Seconds the NPC walks toward the attacker's last-known position before giving up and returning to spawn. Resets on each subsequent hit — enabling kiting. See `npc.investigating` / `npc.investigation_failed` events. |

```ron
// Hostile patrol guard — full configuration
"orc_guard": (
  kind: Actor,
  model: "orc_glb",
  components: (
    npc: (
      faction: Hostile,
      on_player_near: Chase,
      detection_radius: 8.0,
      chase_radius: 20.0,
      fov_degrees: 110.0,
      requires_los: true,
      approach_distance: 1.5,
      patrol_speed: 2.5,
      chase_speed: 5.0,
      patrol_waypoints: [
        (5.0, 0.0, 0.0),
        (5.0, 0.0, 10.0),
      ],
      // alerted_duration / drag / waypoint_reach_radius omitted — use defaults
    ),
  ),
),

// Small creature — custom physics feel and tight waypoint radius
"rat": (
  kind: Primitive,
  model: "",
  shape: Capsule3d,
  primitive: ( radius: 0.15, height: 0.3 ),
  components: (
    npc: (
      faction: Neutral,
      on_player_near: Flee,
      detection_radius: 3.0,
      chase_radius: 8.0,
      eye_height: 0.15,
      drag: 0.5,
      waypoint_reach_radius: 0.2,
    ),
  ),
),
```

### Primitive shapes ✅

When `kind: Primitive`, no GLB model is loaded. Instead the runtime generates a procedural Bevy mesh from the `shape` field and the optional `primitive` parameters block. The `model` field must be `""` (empty string) for primitive prefabs.

**Supported `PrimitiveShapeKind` variants:**

| `shape` value | Shape | Key dimension fields |
|---|---|---|
| `Cuboid` | Box | `size: (x, y, z)` |
| `Sphere` | Sphere | `radius` |
| `Cylinder` | Cylinder | `radius`, `height` |
| `Capsule3d` | Capsule | `radius`, `height` (used as half_length) |
| `Cone` | Cone | `radius`, `height` |
| `Torus` | Torus / donut | `radius` (outer), `radius_top` (inner) |
| `ConicalFrustum` | Truncated cone | `radius` (bottom), `radius_top` (top), `height` |
| `Plane` | Flat plane | `size: (x, _, z)` (Y ignored) |

> **Physics collider support:** `Cuboid`, `Sphere`, `Cylinder`, and `Capsule3d` support physics/sensor colliders. `Cone`, `Torus`, `ConicalFrustum`, and `Plane` are visual-only — `physics: true` / `sensor: true` is ignored for these shapes.

**`PrimitiveParams` fields** (all optional — defaults apply when omitted):

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `size` | `Option<(f32,f32,f32)>` | `(3,3,3)` | Cuboid XYZ extents |
| `radius` | `Option<f32>` | shape-specific | Sphere/Cylinder/Capsule/Cone/Torus outer/Frustum bottom |
| `radius_top` | `Option<f32>` | shape-specific | ConicalFrustum top radius; Torus inner radius |
| `height` | `Option<f32>` | shape-specific | Cylinder/Capsule/Cone/ConicalFrustum height |
| `color` | `Option<(f32,f32,f32)>` | project default | Linear sRGB. Priority: this field → `primitive_default_color` in project → grey `(0.7,0.7,0.7)` |
| `roughness` | `Option<f32>` | `0.5` | PBR perceptual roughness (0 = mirror, 1 = fully rough) |
| `metallic` | `Option<f32>` | `0.0` | PBR metallic factor (0 = dielectric, 1 = full metal) |
| `physics` | `bool` | `false` | Spawn a static `RigidBody::Fixed` Rapier collider (supported: Cuboid, Sphere, Cylinder) |
| `sensor` | `bool` | `false` | Spawn a ghost `Sensor` collider that fires `GameEvent::Trigger` on overlap (takes precedence over `physics`; supported: same shapes) |

**Example:**
```ron
(
  prefabs: {
    "marker_cube": (
      kind: "primitive",
      model: "Cuboid",
      components: (),
      primitive: (
        size: (2.0, 2.0, 2.0),
        // color omitted — uses project primitive_default_color
        roughness: 0.4,
      ),
    ),
    "beacon_sphere": (
      kind: "primitive",
      model: "Sphere",
      components: (),
      primitive: (
        radius: 1.5,
        color: (0.9, 0.2, 0.2),  // red override
        roughness: 0.2,
        metallic: 0.3,
      ),
    ),
  }
)
```

### GLB prop colliders (`colliders`) ✅

To make a GLB prop solid (so the player can stand on it or bump into it), add a `colliders` list to the prefab. All shapes are combined into a single Rapier compound `RigidBody::Fixed` — one entry for simple props, multiple entries to approximate curved geometry (arches, barrels) or multi-part shapes (chest base + lid). Works identically whether the prefab is a top-level scene entity or nested inside a composite prefab; in either case the prop's collider is an independent static body.

**`ColliderDef` fields** (each entry in the `colliders` list):

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `shape` | `ColliderShapeKind` | — | `Cuboid`, `Sphere`, or `Cylinder` (bare enum variant, no quotes) |
| `size` | `Option<(f32,f32,f32)>` | `(1,1,1)` | Full extents (width, height, depth) for Cuboid |
| `radius` | `Option<f32>` | `0.5` | Radius for Sphere / Cylinder |
| `height` | `Option<f32>` | `1.0` | Total height for Cylinder |
| `offset` | `(f32,f32,f32)` | `(0,0,0)` | Local-space offset of this shape from the entity origin |
| `rotation_euler_deg` | `(f32,f32,f32)` | `(0,0,0)` | Euler rotation in degrees (XYZ order) for this shape's local orientation |

```ron
// Simple single-shape prop
"barrel": (
  kind: Prop,
  model: "barrel",
  components: (),
  colliders: [
    (shape: Cylinder, radius: 0.35, height: 0.9),
  ],
),

// Multi-shape prop: chest with separate base and lid colliders
"chest_01": (
  kind: Prop,
  model: "chest_01",
  components: (tags: ["loot"]),
  colliders: [
    (shape: Cuboid, size: (0.70, 0.55, 1.00), offset: (0.0, -0.125, 0.0)),
    (shape: Cuboid, size: (0.68, 0.28, 0.98), offset: (0.0,  0.275, 0.0)),
  ],
),

// Archway approximated with three boxes — diagonal brace uses rotation_euler_deg
"archway": (
  kind: Prop,
  model: "archway",
  components: (),
  colliders: [
    (shape: Cuboid, size: (0.4, 3.0, 0.4), offset: (-1.5, 1.5, 0.0)),
    (shape: Cuboid, size: (0.4, 3.0, 0.4), offset: ( 1.5, 1.5, 0.0)),
    (shape: Cuboid, size: (3.4, 0.4, 0.4), offset: ( 0.0, 3.2, 0.0)),
    (shape: Cuboid, size: (0.3, 2.0, 0.3), offset: ( 0.0, 1.5, 0.0), rotation_euler_deg: (0.0, 0.0, 45.0)),
  ],
),
```

**For composite primitive prefabs**, child shapes that have `physics: true` in their `PrimitiveParams` automatically add a collider to that child mesh and `RigidBody::Fixed` to the parent anchor — no `colliders` field is needed on the parent `PrefabDef`.

**Nested GLB props inside a composite** use their own `PrefabDef.colliders` field independently. The parent composite anchor gets `RigidBody::Fixed` only from inline primitive children that have `physics: true`; nested GLB props build their own separate static body. In practice both are `Fixed`, so the player interacts with each surface correctly.

### Composite prefabs (`children`) ✅

A primitive prefab with a non-empty `children` list spawns multiple child meshes under a single parent entity. Each child is a `ChildPrimitiveDef`, which is either an **inline primitive shape** or a **nested prefab reference** — set exactly one of `shape` or `prefab` per child.

**`ChildPrimitiveDef` fields:**

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `shape` | `Option<PrimitiveShapeKind>` | `None` | Inline primitive shape — write the bare variant: `Cuboid`, `Sphere`, etc. (`implicit_some` active). Leave omitted when using `prefab`. Mutually exclusive with `prefab`. |
| `primitive` | `PrimitiveParams` | defaults | Shape dimensions and colour. Only used when `shape` is set. |
| `offset` | `(f32,f32,f32)` | `(0,0,0)` | Translation offset from the parent entity's origin |
| `rotation_euler_deg` | `(f32,f32,f32)` | `(0,0,0)` | Euler rotation in degrees (XYZ order) |
| `scale` | `(f32,f32,f32)` | `(1,1,1)` | Scale for this child |
| `material` | `Option<String>` | `None` | Key into `AssetCatalog.materials` for the child's material (inline primitives only). |
| `prefab` | `Option<String>` | `None` | **Nested prefab reference** — key into `PrefabCatalog.prefabs`. Mutually exclusive with `shape`. See below. |

The `material` field accepts the same custom/standard/terrain keys as the top-level `material` field, including `Custom` materials with WGSL shaders.

### Nested prefab references ✅

A child can reference another named prefab by key instead of defining an inline shape. All three prefab kinds are supported as nested children — `kind: Primitive` (both composite and single-shape), `kind: Actor`, and `kind: Prop` (GLB meshes). Transforms compose **multiplicatively** (standard Bevy hierarchy), so rotation and scale inherit correctly at every nesting level.

```ron
"village": (
  kind: Primitive,
  model: "",
  components: (),
  children: [
    // Inline primitive
    (
      shape: Cuboid,
      material: "mat_stone_cobble",
      primitive: (size: (18.0, 0.02, 14.0)),
      offset: (0.0, 0.01, 0.0),
    ),
    // Nested composite prefab (kind: Primitive with children)
    (
      prefab: "well",
      offset: (5.0, 0.0, 0.0),
      rotation_euler_deg: (0.0, 45.0, 0.0),
    ),
    // Nested GLB prop (kind: "prop" — loads a .glb file)
    (
      prefab: "rock_deco",
      offset: (3.0, 0.0, -2.0),
      rotation_euler_deg: (0.0, 35.0, 0.0),
    ),
    // Nested single-shape primitive (kind: "primitive" with no children, just a model)
    (
      prefab: "beacon",
      offset: (-6.0, 0.0, -4.0),
    ),
  ],
),
```

**What spawns for each nested prefab kind:**

| Nested prefab `kind` | Has `children`? | Result |
|---|---|---|
| `Primitive` | yes | Anchor + all children spawned recursively |
| `Primitive` | no (single `shape`) | Anchor + one mesh child |
| `Actor` / `Prop` | — | GLB loaded via `spawn_prefab_instance`; the GLB root entity sits at the child `offset` |

**Rules and constraints:**

- `shape` and `prefab` are mutually exclusive. Setting both is a validation error.
- Every child must set exactly one of `shape` or `prefab`. Setting neither is a validation error.
- The referenced prefab key must exist in the same catalog — forward references are validated when the catalog loads.
- Circular references (`a → b → a`) are detected at validation time and rejected with an error.
- Nesting depth is capped at 8 levels in the spawner. Exceeding this logs an error and skips the deep child.
- If a nested `"actor"` / `"prop"` prefab's `model` key is missing from the asset catalog, a `load_errors` entry is emitted and the prefab is skipped — no panic.

**Transform composition (multiplicative):**

When a `well` prefab at `offset: (5, 0, 0)` has its own children (a cylinder at local `(0, 0.4, 0)` and a torus at `(0, 0.82, 0)`), those children land in the world at:
- cylinder → `village_origin + (5, 0, 0) + (0, 0.4, 0)` = `(5, 0.4, 0)` relative to village
- If the village anchor is rotated 45° around Y, all of the above rotates with it — including the well's inner parts.

**Scale inheritance caveat:**  Non-uniform scale on a parent entity (e.g., `scale: (2, 1, 1)`) combined with a rotation on a nested child causes shearing — the same limitation that applies in every 3D scene hierarchy. Keep scale uniform (or leave it at `(1, 1, 1)`) on any prefab that contains nested children.

---

### kind: Foliage ✅

A procedural foliage prefab: no GLB geometry is loaded for the canopy. Instead the engine builds leaf-card cluster meshes at scene load from the parameters below. An optional trunk GLB is loaded as a child entity. Leaf cards are camera-facing billboards (always face the player) shaded with a cel/toon lighting model.

```ron
// Minimal foliage — bush with no trunk
"my_bush": (
    kind: Foliage,
    foliage: (
        clusters: (
            count: 5,
            emitter_radius: 0.7,
            leaves_per_cluster: 20,
            leaf_scale_min: 0.25,
            leaf_scale_max: 0.45,
        ),
        material: (
            leaf_texture: "textures/foliage/leaf_brush_01",
            color_highlight: (0.38, 0.65, 0.20),
            color_midtone:   (0.22, 0.48, 0.10),
            color_shadow:    (0.08, 0.25, 0.04),
            toon_bands: 3,
            ao_intensity: 0.4,
        ),
    ),
),

// Full tree with trunk
"oak_tree": (
    kind: Foliage,
    foliage: (
        trunk: "models/plants/trunk_with_branches_01",
        clusters: (
            count: 7,
            emitter_radius: 1.6,
            leaves_per_cluster: 28,
            leaf_scale_min: 0.35,
            leaf_scale_max: 0.65,
            height_bias: 0.75,
            seed: 42,
        ),
        material: (
            leaf_texture: "textures/foliage/leaf_brush_01",
            color_highlight: (0.45, 0.72, 0.25),
            color_midtone:   (0.28, 0.55, 0.15),
            color_shadow:    (0.10, 0.30, 0.06),
            toon_bands: 3,
            ao_intensity: 0.45,
        ),
    ),
),
```

**`FoliageDef` fields:**

| Field | Type | Default | Description |
|---|---|---|---|
| `trunk` | `Option<String>` | `None` | Asset catalog model key for the trunk GLB. Omit for bushes or pure foliage. |
| `clusters` | `FoliageClustersDef` | see below | Controls how many clusters spawn and how leaf cards are sized. |
| `material` | `FoliageMaterialDef` | see below | Leaf card texture and toon shading colours. |
| `cast_shadows` | `bool` | `true` | `true` = alpha-clipped depth prepass produces leaf-shaped shadows. `false` = `NotShadowCaster` inserted — no shadows at all (cheaper; useful for dense bushes). |

**`FoliageClustersDef` fields:**

| Field | Type | Default | Description |
|---|---|---|---|
| `count` | `u32` | `6` | Number of foliage clusters to spawn. |
| `emitter_radius` | `f32` | `1.2` | Crown radius in metres — how far apart clusters are spread (not a particle emitter). |
| `crown_height` | `f32` | `0.0` | Lifts the emitter sphere above the entity origin, in metres. Set this to roughly where the trunk meets the branches. Bushes use `0.0`; trees typically need `1.5`–`2.5`. |
| `leaves_per_cluster` | `u32` | `24` | Number of leaf cards baked into each cluster mesh. Must be > 0. |
| `leaf_scale_min` | `f32` | `0.3` | Minimum leaf card size in metres. |
| `leaf_scale_max` | `f32` | `0.6` | Maximum leaf card size in metres. Cards are randomly sized between min and max. |
| `height_bias` | `f32` | `0.7` | Biases cluster placement toward the top of the sphere. `0.0` = full sphere (clusters appear below the trunk base); `1.0` = upper hemisphere only. `0.7` is a good default for trees; `0.5` works well for bushes. |
| `seed` | `u32` | `0` | Rotates the Fibonacci placement pattern. Two trees of the same prefab with different `seed` values will have different cluster arrangements. Change the seed to get a different silhouette. |

**`FoliageMaterialDef` fields:**

| Field | Type | Description |
|---|---|---|
| `leaf_texture` | `String` | Asset catalog texture key. Must be an **RGBA PNG** where the leaf shapes are opaque and the background is transparent (alpha = 0). RGB images have no transparency and render as solid rectangles. |
| `color_highlight` | `(f32, f32, f32)` | RGB colour for the brightest lit areas (facing the sun). Linear sRGB 0–1. Note: **RGB, not RGBA** — no alpha component. |
| `color_midtone` | `(f32, f32, f32)` | RGB colour for the mid-tone transition zone. |
| `color_shadow` | `(f32, f32, f32)` | RGB colour for the darkest shadowed areas (facing away from the sun). |
| `toon_bands` | `u8` | Number of discrete shading bands. **Must be 2, 3, or 4.** 3 is the standard anime look; 2 is more graphic; 4 adds a subtle extra highlight band. |
| `ao_intensity` | `f32` | How much ambient occlusion darkens the shadow side. `0.0` = no AO; `1.0` = maximum darkening. Values around `0.4`–`0.5` look natural. |

> **Validation:** the engine rejects a Foliage prefab at load time if `leaf_texture` is empty, `leaves_per_cluster` is 0, or `toon_bands` is outside 2–4.

See `assets/projects/foliage_demo/` for a working demo with oak trees, autumn trees, and bushes.

---

## `prefabs/animation/*.ron` — AnimationPolicy ✅

Defines the locomotion clips and override animations for a character type.

> **GLB animation requirement** — Bevy creates an `AnimationPlayer` on a GLTF scene root only
> when the GLB contains at least one animation clip. Every character model GLB **must** include
> at least one embedded clip (typically `Idle_Loop`). Without it, `animation_sources` retargeting
> silently does nothing — the character will load visually but will never animate.

> **Reserved override IDs — `jump_enter` / `jump_exit`.** `player_movement_system` fires these
> two override IDs automatically on every jump takeoff/landing, for **every** player prefab,
> regardless of policy. If your policy has no `overrides` entry for one of them, the resolver
> falls through to treating the ID as a literal glTF clip name, doesn't find it, and logs a
> `WARN ... falling back to idle` — harmless, but noisy console spam on every jump. A minimal
> locomotion-only policy (no combat/emotes) still needs these two overrides; see
> `assets/projects/local_coop_demo/prefabs/animation/player_locomotion.ron` for the smallest
> working example (just `jump_enter`/`jump_exit`, nothing else).

```ron
(
    default_transition_ms: 150,

    // Extra GLB catalog keys whose clips are merged into this character's animation graph.
    // A clip referenced in `clips:` or `overrides[].clip` must come from a GLB listed here,
    // or it silently won't play. The model GLB is always included automatically.
    // Last source wins on duplicate clip names. See player_policy_human.ron for a live example.
    animation_sources: [
        "anim_locomotion",
        "anim_melee",
        "anim_magic",
    ],

    base: (
        idle:      "Idle_Loop",
        walk:      "Walk_Loop",
        run:       "Sprint_Loop",
        jump_loop: "Jump_Loop",
    ),

    clips: {
        "dance": "Dance_Loop",   // semantic alias → glTF clip name
        "sit":   "Sitting_Idle_Loop",
    },

    overrides: [
        (
            id: "dance",
            clip: "Dance_Loop",
            priority: 50,
            looping: true,
            cancel_on_move: true,
            stop_action: "stop_dance",
        ),
        (
            id: "attack_light",   // semantic override id — use this in PlayAnimationOn(clip: "attack_light")
            clip: "Sword_Regular_A",  // exact glTF clip name from one of the animation_sources
            priority: 100,
            looping: false,
            duration: 0.4,
            cancel_on_move: false,
        ),
        (
            id: "attack_magic",
            clip: "Two-hand Blast",
            priority: 100,
            looping: false,
            duration: 0.5,
            cancel_on_move: false,
        ),
    ],
)
```

**Adding a new combat animation (step-by-step):**

1. Add the animation GLB to `assets.ron` as a catalog key: `"anim_magic": ( path: "shared/models/characters/character-animations/magic.glb#Scene0" )`
2. Add the catalog key to `animation_sources` in the character's `.ron` policy file
3. Add an `overrides` entry with a semantic `id` and the exact glTF clip name (check with `ironhold inspect glb <path>`)
4. Reference the semantic `id` in action RON: `PlayAnimationOn(target: "player_01", clip: "attack_magic")`

**`PlayAnimationOn(clip:)` resolution order:** override `id` → `clips:` alias → raw glTF clip name. The `clip:` argument is matched first against `overrides[].id`, then against `clips:` keys, then as a literal glTF animation name. Always use an override `id` for one-shot skills — only overrides support `duration` and `cancel_on_move`.

**AnimationOverrideDef fields:**

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `id` | `String` | — | Semantic ID used by `PlayAnimation("<id>")` |
| `clip` | `String` | — | glTF animation clip name |
| `priority` | `i32` | `0` | Higher priority wins |
| `looping` | `bool` | `true` | Whether to loop |
| `cancel_on_move` | `bool` | `false` | Cancel this override when the player moves |
| `stop_action` | `Option<String>` | — | `PlayAnimation` ID that cancels this override |
| `duration` | `Option<f32>` | — | Auto-expire after N seconds (one-shots). Omit to hold the last frame indefinitely. |
| `transition_ms` | `Option<u64>` | — | Per-override blend duration; overrides `default_transition_ms` |

> **Death-pose pattern** — to keep an NPC frozen in its death pose until it respawns, omit `duration` and set `stop_action` to the clip ID that fires on respawn. With `looping: false` and no `duration`, the override plays once and holds the final frame forever. The `stop_action` clip cancels it when the revive happens:
>
> ```ron
> // spider_policy.ron — death override holds pose until "npc_revive" is played
> (
>     id: "death",
>     clip: "Death",
>     priority: 150,
>     looping: false,
>     cancel_on_move: false,
>     stop_action: "npc_revive",
> ),
> ```
>
> ```ron
> // enemy_spider.behavior.ron — "alive" entry_actions clear the death pose on respawn
> entry_actions: [
>     PlayAnimationOn(target: "{self}", clip: "npc_revive"),
>     EmitEvent("npc.revive:{self}"),
>     SetEntityVisible(entity: "{self}", visible: true),
>     ...
> ],
> ```
>
> The `"npc_revive"` clip does not need to exist in the GLB — if it resolves to nothing, the animation system simply clears the override with no visual transition, which is the intended result for an instant-revive effect.

---

## `logic/rules.ron` — LogicRulesAsset ✅

Maps runtime events to action sequences. This is the primary place for data-driven game logic.

```ron
(
    schema_version: 2,
    rules: [
        (
            on: "ui.button_pressed:start_game",
            do_actions: [ Log("Starting"), LoadScene("scenes/main.scene.ron") ],
        ),
        (
            on: "ui.button_pressed:dance",
            do_actions: [ Log("Dance triggered"), PlayAnimation("dance") ],
        ),
        (
            on: "ui.button_pressed:quit",
            do_actions: [ Quit ],
        ),
    ],
)
```

**Event name format:** `"<domain>.<event_type>:<payload>"`. Available event names:

| Event name | Source |
|-----------|--------|
| `ui.button_pressed:<trigger>` | `UiEvent` — UI button or key binding; `<trigger>` is the button's `action` field with the `"ui."` prefix stripped |
| `<name>` (as-is) | `GameEvent::Trigger(name)` — gameplay capability (e.g. `"entity.collected:coin_01"`); the string is the full rule key, no prefix added |
| `scene.requested:<stem>` | Scene load initiated |
| `scene.loaded:<stem>` | RON asset deserialized; entities not yet spawned |
| `scene.ready:<stem>` | All entities spawned |
| `scene.unloading:<stem>` | Before a full scene replace |

`<stem>` is the filename without `.scene.ron` (e.g. `"main"` for `scenes/main.scene.ron`).

`GameEvent` naming convention: `"<category>.<verb>:<id>"` — e.g. `"entity.collected:coin_01"`, `"zone.entered:checkpoint_1"`.

**Available actions:**

| Action | Description |
|--------|-------------|
| `LoadScene("path")` | Load a `.scene.ron` file relative to the project root |
| `Spawn { prefab, id, position, spawn_point, yaw_deg }` | Enqueue a prefab spawn (max 2/frame); `id` auto-generated if omitted; `spawn_point` looks up a scene-defined named point; `yaw_deg` rotates around Y axis |
| `PreloadPrefab("key")` | Load a prefab's GLB early and cache the handle; fire on `scene.ready` to eliminate the first-spawn WASM decode stall |
| `PreloadGlb("key")` | Load a **model catalog** GLB (from `assets.ron` `models:`) early — use for animation-source GLBs that have no prefab entry. The full GLTF including all clips is decoded and cached; the handle is kept alive in `PreloadedGlbHandles` until the next scene load. Validates that the key exists in `assets.ron` |
| `Despawn("id")` | Remove a previously spawned entity by its spawn ID |
| `PlayAnimation("id")` | Broadcast an animation to **all** animated entities. Use for global emotes (dance). Resolves via AnimationPolicy — see override `id` / `clips:` alias / raw clip name order above |
| `PlayAnimationOn(target: "id", clip: "name")` | Play an animation on a **specific** spawned entity by spawn ID. Use this from skill-bar `do_actions`, FSM rules, and behavior files whenever you want to target one entity. `{self}` / `{target}` substitution applies in `target`. Clip resolves via the entity's AnimationPolicy (override `id` → `clips:` alias → raw glTF name) |
| `PlaySound(key: "key")` | Play a sound by audio catalog key (`.wav`, `.ogg`, `.mp3`); warns on missing key or unsupported format; optional `volume: f32` (0.0–1.0, default 1.0) multiplies the per-entry catalog volume |
| `PlayMusicLoop(key: "key")` | Start a looping background track by audio catalog key; stops any currently playing music; optional `volume: f32` multiplies the per-entry catalog volume |
| `Log("message")` | Emit an `info!` log line |
| `Quit` | Exit the application |
| `PreloadScene("path")` | Warm the asset cache for a `.scene.ron` before it is needed |
| `EnterState("name")` | Transition the interpreter to a named logic state; `""` returns to stateless |
| `SetVariable("key", "value")` | Write a named string variable into `GameVariables`; readable by data-bound UI labels |
| `IncrementVariable("key", i32)` | Parse the variable as `i32` and add the delta; missing or unparseable values default to `0` |
| `ModifyStat(key: "key", delta: f32)` | Add `delta` to a stat and clamp. **Dot-routing:** `"spawn_id.stat_name"` targets that entity's `StatMap`; no dot targets global `LoadedStats`. In behavior files, `{self}` in `key` is substituted with the entity's spawn ID. |
| `SetStat(key: "key", value: f32)` | Set a stat to an absolute value and clamp. Same dot-routing and `{self}` substitution as `ModifyStat`. |
| `ShowDamagePopup(entity: "id", amount: f32)` | Spawns a floating `+N` / `-N` label above the entity with the given spawn ID. Positive amounts show in heal colour, negative in damage colour. Uses `{self}` substitution in behavior files. Style (font size, duration, colours) is set via `damage_popup_style` in `.project.ron`. |
| `ShowFloatingText(entity: "id", text: "msg")` | Spawns a floating text label above the entity with the given spawn ID. Rises and fades using the same animation as `ShowDamagePopup`. Colour is warm yellow; use `ShowDamagePopup` for numeric health feedback. Uses `{self}` and `{target}` substitution. Optional `offset: (x, y, z)` overrides the default spawn height set by `damage_popup_style.spawn_offset` — useful when multiple floating texts fire at the same time and would otherwise overlap. Example: `ShowFloatingText(entity: "player_01", text: "You killed {self}!", offset: (0.0, -0.02, 0.0))` |
| `SetEntityVisible(entity: "id", visible: bool)` | Shows (`true`) or hides (`false`) a spawned entity by its spawn ID. The entity stays in the ECS — colliders and behavior FSM keep running. World-space labels tracking that entity (stat bar, stat label) auto-hide automatically. Uses `{self}` in behavior files. |
| `EmitEventAfterDelay(event: "name", delay_secs: f32)` | Fires a `GameEvent::Trigger("name")` after `delay_secs` seconds. One-shot — fires once then is removed. Cleared on `Action::LoadScene` so delayed events do not leak across scene transitions. Uses `{self}` substitution in behavior files. |
| `SpawnEffect(key: "key", position/entity)` | Spawn a particle burst from `assets.ron effects`. Quality multiplier and budget gating are applied at spawn time. See the Particle System section. |
| `ProjectDecal(key: "key", …)` | Spawn a flat ground-projected texture quad. See the Ground Decals section. |
| `SetParticleQuality(Level)` | Set the global quality tier (`High`, `Medium`, `Low`, `Minimal`). Persists across scene transitions. Affects all subsequent `SpawnEffect` calls. |
| `SetVolume(0–100)` | Set the global audio volume (percent). Scales against the project's `max_volume` ceiling — `SetVolume(100)` equals `max_volume`. Emits `audio.volume_changed`. |
| `ToggleMute` | Toggle muted state. Muting emits `audio.muted`; unmuting restores the previous volume and emits `audio.unmuted`. |
| `SyncAudioState` | Re-emit the current mute state (`audio.muted` or `audio.unmuted`) without changing it. Use in state `entry_actions` to initialise bound audio labels on first load — combine with a `global_on` bridge that maps the event to `SetVariable`. |
| `ToggleOwnNameplate` | Toggle the local player's own nameplate visibility as a runtime preference, independent of the scene's `show_player_nameplate` default. Does not persist across scene transitions (resets to the new scene's authored default). Has no effect on NPC/prop nameplates or when the player prefab has an explicit `nameplate: Some(true)`/`Some(false)` override (that always wins). Emits `nameplate.own_shown`/`nameplate.own_hidden`. |
| `ApplyModifier(modifier_key: "key")` | Apply a named stat modifier template to its target stat. |
| `RemoveModifier(modifier_key: "key")` | Remove all active instances of a named modifier. |
| `SetTarget("spawn_id")` | Set `CurrentTarget` to the given spawn ID. Emits `target.changed:{id}` and `target.changed`. |
| `ClearTarget` | Clear `CurrentTarget`. Emits `target.cleared`. Also cleared automatically on `LoadScene`. |
| `ResetToSpawn("{self}")` | Teleport an NPC entity to its scene-placed origin and zero its velocity. Call before `SetEntityVisible(visible: true)` in respawn entry_actions so the entity appears at its spawn point instead of where it died. Warns and no-ops for non-NPC entities. `{self}` is substituted in behavior files. |
| `CameraShake(duration_secs: f32, intensity: f32, owner_player: Option<u32>)` | Apply a procedural position shake to every active `Orbit`- or `Party`-mode camera (both split-screen's per-player cameras and a shared party camera shake correctly). `duration_secs` is the shake duration in seconds (typical range 0.2–0.8). `intensity` is the peak camera displacement in world-space metres (typical range 0.05–0.25 — scale with enemy weight: a snake might use 0.10, a heavy boss 0.25). Re-triggering while a shake is active replaces it with the new parameters. No-op (warning logged) in scenes that use a flycam instead — an orbit camera is created by a prefab tagged `"player"` (see the `player` tag description above). `owner_player` (v2, omit for the pre-v2 behavior: every active camera) targets one local-coop player's camera — see the `owner_player` targeting table under [`camera_modes:` registry and `SetCameraMode`](#camera_modes-registry-and-setcameramode-v2-) above, which this field follows identically. Example: `CameraShake(duration_secs: 0.4, intensity: 0.15)` |
| `SetCameraMode(mode: "preset_or_default", owner_player: Option<u32>)` | Switch a camera's active mode at runtime (v2). `mode` is either the reserved key `"default"` (restore the camera's own scene-authored starting mode) or a key from the current scene's `camera_modes:` registry. `owner_player` targets one local-coop player's camera; omitted targets every active camera. See [`camera_modes:` registry and `SetCameraMode`](#camera_modes-registry-and-setcameramode-v2-) above for the full targeting table, `transition:` blending, and validation rules. Examples: `SetCameraMode(mode: "cutscene_fixed")`, `SetCameraMode(mode: "default", owner_player: 1)` |
| `StartDialogue(npc_id: "id", dialogue_path: "dialogues/npc.dialogue.ron")` | Open the `DialoguePanel` UI for the given NPC and begin playing the `.dialogue.ron` conversation. Emits `dialogue.started:{npc_id}`. Auto-fired when the player interacts with an entity that has `PrefabDef.dialogue` set; can also be fired from `rules.ron` or `state_machine.ron`. |
| `AdvanceDialogue` | Advance the current dialogue to the next node. No-op when the current node has visible choices (player must click a choice button). |
| `EndDialogue` | Close the dialogue panel immediately. Emits `dialogue.ended:{path}`. Cleared automatically on `LoadScene`. |
| `JoinPlayer` | Hot-join a new player into an already `Grid`-split local co-op scene, growing the split-screen layout live (up to `MAX_SPLIT_PLAYERS`) with no scene reload. No-ops with a `warn!` outside a `Grid`-split scene, at the cap, or with no `join_prefab_keys` entry for the next slot. Emits `coop.lobby_full` on the join that reaches the cap. See [Local co-op hot join](#local-co-op-hot-join) above. |

---

## `dialogues/*.dialogue.ron` — DialogueDef ✅

Defines a branching NPC conversation tree. File extension must be `.dialogue.ron`.
Reference from a `PrefabDef` via the `dialogue` field (project-relative path); pair with `interactable` so the player can trigger the conversation.

**Fields:**

| Field | Type | Description |
|---|---|---|
| `schema_version` | `u32` | Must be `1` |
| `nodes` | `Vec<DialogueNodeDef>` | Ordered list of conversation nodes |

**`DialogueNodeDef` fields:**

| Field | Type | Default | Description |
|---|---|---|---|
| `id` | `String` | required | Stable node identifier used as a `jump_to` target. Must be unique within this file. |
| `speaker` | `String` | required | Display name shown in the speaker label. `{self}` is replaced with the NPC's spawn ID. |
| `portrait` | `Option<String>` | `None` | Reserved — parsed and stored but not yet rendered in v1. |
| `body` | `String` | required | Dialogue body text shown to the player. `{self}` is replaced with the NPC's spawn ID. |
| `advance_delay_secs` | `Option<f32>` | `None` | When set and the node has **no choices**, auto-advances after this many seconds. Ignored when choices are present. |
| `choices` | `Vec<DialogueChoiceDef>` | `[]` | Player response buttons. Empty = auto-advance node (waits for `advance_delay_secs` or an `AdvanceDialogue` action). |

**`DialogueChoiceDef` fields:**

| Field | Type | Default | Description |
|---|---|---|---|
| `label` | `String` | required | Button text shown to the player. `{self}` is replaced with the NPC's spawn ID. |
| `condition` | `Option<DialogueCondition>` | `None` | Choice is hidden if the condition evaluates to false. |
| `do_actions` | `Vec<Action>` | `[]` | Actions queued when this choice is selected. `{self}` is substituted with the NPC's spawn ID before each action is pushed. |
| `jump_to` | `Option<String>` | `None` | Node `id` to jump to after `do_actions` fire. `None` = advance to the next node. `"__end__"` = close the dialogue. |

**`DialogueCondition` variants:**

| Variant | Description |
|---|---|
| `HasVariable { key, value }` | Choice visible only when `GameVariables[key] == value`. |
| `VariableGte { key, min }` | Choice visible only when `GameVariables[key]` parses as `i32` and is `>= min`. |
| `StatAtLeast { stat_key, min }` | Choice visible only when the named global stat's effective value is `>= min`. |

**Pipeline events emitted:**

| Event | When |
|---|---|
| `dialogue.started:{npc_id}` | `StartDialogue` executed — panel opened |
| `dialogue.ended:{dialogue_path}` | `EndDialogue` executed or dialogue closed — panel hidden |

**Example:**

```ron
// dialogues/npc_intro.dialogue.ron
(
    schema_version: 1,
    nodes: [
        (
            id: "greeting",
            speaker: "Guard",
            body: "Hail, traveller! The undead are restless — stay sharp.",
            choices: [
                ( label: "Tell me more.", jump_to: "lore" ),
                ( label: "Any reward?",   jump_to: "reward" ),
                ( label: "Thanks.",       jump_to: "__end__" ),
            ],
        ),
        (
            id: "lore",
            speaker: "Guard",
            body: "A sorcerer was spotted near the northern ruins three nights ago.",
            choices: [
                ( label: "I'll investigate.",
                  do_actions: [ SetVariable("quest_ruins", "accepted") ],
                  jump_to: "accepted" ),
                ( label: "Good luck.", jump_to: "__end__" ),
            ],
        ),
        (
            id: "reward",
            speaker: "Guard",
            body: "The elder offers gold for proof the sorcerer has been dealt with.",
            // condition: only shows if the quest isn't already accepted
            choices: [
                ( label: "Consider it done.",
                  condition: HasVariable(key: "quest_ruins", value: ""),
                  do_actions: [ SetVariable("quest_ruins", "accepted") ],
                  jump_to: "accepted" ),
                ( label: "I'll think about it.", jump_to: "__end__" ),
            ],
        ),
        (
            id: "accepted",
            speaker: "Guard",
            body: "Good luck, warrior. The ruins lie north-east, past the stone bridge.",
            advance_delay_secs: 3.0,  // auto-closes after 3 s; no choices needed
        ),
    ],
)
```

**Wiring in `prefabs/prefabs.ron`:**

```ron
"guard_npc": (
    kind: Actor,
    model: "character_male",
    interactable: ( radius: 3.0, hint_text: "Talk" ),
    dialogue: "dialogues/npc_intro.dialogue.ron",
),
```

**Placing the panel in `scenes/main.scene.ron`:**

```ron
ui: [
    DialoguePanel((
        id: "npc_dialogue_panel",
        position: (16.0, 440.0),
        size: (1220.0, 200.0),
        background_color: (0.04, 0.04, 0.07, 0.93),
        speaker_font_size: 18.0,
        body_font_size: 15.0,
        choice_font_size: 13.0,
    )),
],
```

The panel is `Visibility::Hidden` at spawn (`initially_hidden: true` is the default). It becomes visible when `StartDialogue` is processed and returns to hidden on `EndDialogue` or `LoadScene`.

**Reacting to dialogue events in `rules.ron`:**

```ron
( on: "dialogue.started:npc_guard_01", do_actions: [ SetVariable("talking_to_guard", "true") ] ),
( on: "dialogue.ended:dialogues/npc_intro.dialogue.ron", do_actions: [ SetVariable("talking_to_guard", "") ] ),
```

---

## `overrides/model_fixes.ron` — ModelFixesAsset ✅

Per-asset transform corrections applied to every spawned instance of a model. Use to fix off-centre pivots, wrong-axis imports, or scale mismatches.

```ron
(
    schema_version: 2,
    model_fixes: {
        "shared/models/character-01.glb#Scene0": (
            pivot_offset: (0.0, 0.0, 0.0),
            rotation_deg: (0.0, 180.0, 0.0),
            scale: (1.0, 1.0, 1.0),
        ),
    },
)
```

**TransformFix fields** (all optional, sane defaults):

| Field | Default | Description |
|-------|---------|-------------|
| `pivot_offset` | `(0,0,0)` | Metres; applied as child local translation |
| `rotation_deg` | `(0,0,0)` | Euler degrees, order **YXZ** |
| `scale` | `(1,1,1)` | Local scale |

Instances are spawned with a parent (instance transform) + child (GLB scene). The fix is applied to the child so gameplay transforms remain clean.

**Convention — 180° Y rotation for Blender-origin character models**

Character models exported from Blender (and most other DCCs) face **+Z** by default. The engine moves characters in the **-Z** direction (Bevy's internal "forward"), so Blender-origin characters need a 180° Y flip to appear facing the right way in-game:

```ron
"shared/models/characters/character-male-01.glb#Scene0": (
    rotation_deg: (0.0, 180.0, 0.0),
),
```

This is expected and normal — nearly every character model in `shared/models/characters/` carries this fix. Props and creatures may differ (e.g. a treasure chest at 90° because it was authored facing sideways in the DCC).

---

## `logic/state_machine.ron` — StateMachineAsset ✅

Used when `state_machine_path` is set in the project config (schema v3). Replaces `rules.ron` for FSM-based projects. See `docs/30_runtime_events_and_logic.md` for detailed FSM semantics.

**Top-level fields:**

| Field | Type | Description |
|-------|------|-------------|
| `schema_version` | `u32` | Must be `1` |
| `initial_state` | `String` | Starting logic state; set immediately when the asset loads |
| `states` | `Vec<FsmState>` | Named states |
| `transitions` | `Vec<FsmTransition>` | State-change triggers |
| `global_on` | `Vec<FsmEventBinding>` | Event bindings that fire from any state without changing state |

**`FsmState` fields:**

| Field | Type | Description |
|-------|------|-------------|
| `name` | `String` | State identifier |
| `entry_actions` | `Vec<Action>` | Queued when entering this state |
| `exit_actions` | `Vec<Action>` | Queued when leaving this state |
| `on` | `Vec<FsmEventBinding>` | In-state event bindings (do not change state) |

**`FsmTransition` fields:**

| Field | Type | Description |
|-------|------|-------------|
| `from` | `Option<String>` | Source state; omit to match any current state |
| `on` | `String` | Event that triggers this transition |
| `to` | `String` | Target state |

Execution order on transition: `exit_actions` of old state → state change → `entry_actions` of new state.

---

## `stats.ron` — StatCatalog ✅

Named stat definitions for a project. Referenced via `stats_path` in `{name}.project.ron`. Optional — omitting it means no stat system for that project.

Stats persist across scene transitions (the `LoadedStats` resource is not cleared on scene load).

**Fields:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `schema_version` | `u32` | ✅ | Must be `1` |
| `stats` | `Map<String, StatDef>` | ✅ | Named stats keyed by stat ID |

**`StatDef` fields:**

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `base` | `f32` | ✅ | Starting value (must be within `[min, max]`) |
| `min` | `f32` | `0.0` | Minimum allowed value |
| `max` | `f32` | ✅ | Maximum allowed value |
| `regen_rate` | `f32` | `0.0` | Units per second added when regen is active. `0` = no regen |
| `regen_delay` | `f32` | `0.0` | Seconds after a decrease before regen resumes |
| `thresholds` | `Vec<StatThreshold>` | `[]` | Events to emit when a threshold is crossed |

**`StatThreshold` fields:**

| Field | Type | Description |
|-------|------|-------------|
| `when` | `ThresholdCondition` | Condition that triggers the event |
| `emit` | `String` | Event name emitted as `GameEvent::Trigger` on false→true crossing |

**`ThresholdCondition` variants:**

| Variant | Example | Fires when… |
|---------|---------|-------------|
| `BelowOrEqual(f32)` | `BelowOrEqual(0.0)` | `current <= value` |
| `AboveOrEqual(f32)` | `AboveOrEqual(80.0)` | `current >= value` |
| `BelowPercent(f32)` | `BelowPercent(0.25)` | `current / max < fraction` |
| `AtOrAbovePercent(f32)` | `AtOrAbovePercent(1.0)` | `current / max >= fraction` |

Threshold events are **edge-triggered**: they fire once when the condition transitions from false to true. They do not re-fire every frame while the condition remains true.

**Example:**
```ron
// stats/stats.ron
(
    schema_version: 1,
    stats: {
        "health": (
            base: 100.0,
            min: 0.0,
            max: 100.0,
            regen_rate: 0.0,
            regen_delay: 0.0,
            thresholds: [
                ( when: BelowOrEqual(0.0),    emit: "stat.health.depleted" ),
                ( when: BelowPercent(0.25),   emit: "stat.health.low" ),
                ( when: AtOrAbovePercent(1.0),emit: "stat.health.full" ),
            ],
        ),
        "mana": (
            base: 50.0,
            min: 0.0,
            max: 50.0,
            regen_rate: 2.0,
            regen_delay: 3.0,
            thresholds: [
                ( when: AtOrAbovePercent(1.0), emit: "stat.mana.full" ),
            ],
        ),
    },
)
```

**Project config reference:**
```ron
// {name}.project.ron
(
    schema_version: 2,
    ...
    stats_path: "stats/stats.ron",
)
```

**Reacting to threshold events in rules:**
```ron
// logic/rules.ron or state_machine.ron
( on: "stat.player_health.depleted", do_actions: [ LoadScene("scenes/game_over.scene.ron") ] ),
( on: "stat.player_health.low",      do_actions: [ PlaySound(key: "heartbeat") ] ),

// Play a death animation when the player's health hits zero (global stat, no {self}):
( on: "stat.player_health.depleted", do_actions: [ PlayAnimationOn(target: "player_01", clip: "death") ] ),
```

> The global-stat event name is `stat.<key>.<threshold-emit-value>` — no `{self}` substitution since
> global stats are not owned by a specific entity. Per-entity stat events (from `stat_templates`) do
> include `{self}` in the `emit` string; see the Instance Stats section below.

---

## Instance stats (`stat_templates`) ✅

While `stats.ron` holds **global** stats that persist across scene transitions (e.g. player health, score), `stat_templates` on a `PrefabDef` declare **per-entity** stats. Every spawned instance gets its own independent `StatMap` component — there is no shared state between instances of the same prefab.

**Player prefabs support `stat_templates` too**, exactly the same field and syntax as any NPC/prop prefab (`tags: ["player"]` doesn't change anything about how `stat_templates` is read). This is the mechanism behind per-player action-bar resource pools in split-screen: give each player prefab its own `stat_templates` entry (e.g. `"mana"`) and that player's `ActionBar` `SlotCost` checks resolve against their own pool instead of the shared global `LoadedStats` resource — see `SlotCost` and "Per-player action bars" above.

### Authoring

```ron
// prefabs/prefabs.ron
"goblin_guard": (
  kind: "primitive",
  model: "Capsule3d",
  behavior: "behaviors/goblin_guard.behavior.ron",
  stat_templates: [
    (
      key: "health",
      base: 60.0,
      min:  0.0,
      max:  60.0,
      thresholds: [
        ( when: BelowOrEqual(0.0), emit: "stat.{self}.health.depleted" ),
      ],
    ),
  ],
),
```

`{self}` in `emit` strings is replaced with the entity's spawn ID at spawn time, so two instances `goblin_01` and `goblin_02` get independent events `"stat.goblin_01.health.depleted"` and `"stat.goblin_02.health.depleted"`.

### StatTemplateDef fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `key` | `String` | — | Stat name within the entity (the key inside `StatMap`) |
| `base` | `f32` | — | Starting value |
| `min` | `f32` | `0.0` | Minimum allowed value |
| `max` | `f32` | — | Maximum allowed value |
| `regen_rate` | `f32` | `0.0` | Units per second added when regen is active |
| `regen_delay` | `f32` | `0.0` | Seconds after a decrease before regen resumes |
| `thresholds` | `Vec<StatThreshold>` | `[]` | Events to emit on threshold crossings (same schema as `stats.ron`; `{self}` is substituted) |

### Addressing instance stats in rules

Use **dot notation** to address instance stats: `"spawn_id.stat_name"`.

```ron
// In a behavior file — {self} is substituted with the entity's spawn ID
( event: "entity.interacted:{self}", do_actions: [
    ModifyStat(key: "{self}.health", delta: -35.0),
]),
( event: "stat.{self}.health.depleted", do_actions: [
    Despawn("{self}"),
    IncrementVariable("score", 50),
]),
```

```ron
// In state_machine.ron — address a specific instance by ID
ModifyStat(key: "goblin_01.health", delta: -10.0)
```

**Routing rules for `ModifyStat` / `SetStat`:**

| Key format | Routed to |
|---|---|
| `"player_health"` (no dot) | `LoadedStats` resource (global stat) |
| `"goblin_01.health"` (dot present) | `StatMap` component on the entity with `SpawnId("goblin_01")` |

---

## World-space stat widgets (`stat_label` and `world_stat_bar`) ✅

These fields on `PrefabDef` attach floating UI to a spawned entity. Both widgets track a live stat and update every frame via `resolve_stat` — the same routing that drives `StatBar` and `StatSpread`.

**Auto-hide:** When the tracked entity is hidden (`SetEntityVisible(visible: false)`), both widgets automatically hide. They restore automatically when the entity is shown again.

**Player prefabs support `stat_label`/`world_stat_bar` too**, exactly the same fields and syntax as any NPC/prop prefab (`tags: ["player"]` doesn't change anything about how they're read) — see `planning/features/player_stat_widgets.md`. A player-authored widget is queued through the same `DynamicStatUiQueue` mechanism `Action::Spawn`-created entities use, so it appears one frame after the player spawns; in split-screen it duplicates across viewports exactly like any other entity's widget (see the split-screen visibility note below).

**Stat key routing:**
- `"{self}.health"` — entity-local stat (requires `stat_templates`; `{self}` is resolved to the spawn ID at scene load)
- `"player_health"` — global stat (from `stats.ron`)

> **Global vs. `{self}` in local co-op.** A **global** key (no dot, e.g. `"player_health"`) reads the single shared `LoadedStats` resource — in a split-screen scene with 2+ players, every player's widget using that key shows and moves **identically**, which reads as a bug (both players sharing one value) rather than the intentional shared-state case it's meant for. Use `"{self}.<stat>"`, paired with a `stat_templates` entry on that specific player's own prefab (see [Instance stats](#instance-stats-stat_templates-) above), to give each player an independent readout — the same global-vs-per-player distinction `SlotCost` already has (see "Per-player action bars" above).

> **`{self}.<stat>` requires a matching `stat_templates` entry on that SAME prefab, or the widget silently renders empty forever with no further warning.** This is the identical footgun already documented above for nameplates (`{self}.stat` requires a per-entity `stat_templates` entry) — it applies the same way to `stat_label`/`world_stat_bar`, on any prefab kind including players. A scene-load `warn!` and an `ironhold_cli validate` check (`missing_stat_widget_template`) both catch this misconfiguration so it's never silent — the widget still renders empty (unchanged runtime behavior), but the mistake is now diagnosable instead of invisible.

### `StatLabelDef` fields (`stat_label`)

A numeric text label (e.g. `"85 / 100"`).

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `stat_key` | `String` | — | Stat key; supports `{self}` substitution |
| `offset` | `(f32,f32,f32)` | `(0, 2.5, 0)` | World-space offset from the entity's origin in metres |
| `screen_offset` | `(f32,f32)` | `(0, 0)` | Screen-space offset in pixels, applied AFTER projection. It does not grow/shrink with perspective the way a world-space `offset` does — but it IS multiplied by the scene's `label_depth_scale` factor when one is set, so a widget stack shrinks together as one unit (a 20px gap becomes 10px at `min_scale: 0.5`). Use this, not a slightly different `offset`, to stack this label against another widget tracking the same entity (e.g. a nameplate or `world_stat_bar`) — give them the SAME `offset` and use `screen_offset` for the pixel gap between them instead. Two widgets with nearly-but-not-quite-identical world `offset`s drift apart/together as the camera zooms, since perspective magnifies a fixed world gap more the closer the camera gets — this was a real bug (see [Label depth scaling](#label-depth-scaling-labeldepthscaledef)'s `screen_offset` note). Positive Y is up (matches `offset`'s own up-axis convention), negative Y stacks below. Available on `stat_label`/`world_stat_bar` only — scene `world_labels:` and entity `label:` have no `screen_offset` yet. |
| `font_size` | `f32` | `16.0` | Screen-space font size in pixels |
| `color` | `(f32,f32,f32,f32)` | `(0.2, 0.9, 0.2, 1.0)` | sRGB RGBA text colour |
| `show_max` | `bool` | `true` | When `true`, shows `"current / max"`; when `false`, shows `"current"` |

```ron
stat_label: (
  stat_key: "{self}.health",
  offset: (0.0, 2.1, 0.0),
  font_size: 14.0,
  color: (0.8, 0.8, 0.8, 0.85),
  show_max: true,
),
```

### `WorldStatBarDef` fields (`world_stat_bar`)

A floating stat bar above an entity. The visual style is set by the `style` field — `Ascii` (text characters), `Pixel` (solid-colour mesh quads), `Icon` (a row of per-cell sprite icons, e.g. hearts), or `Textured` (a 9-sliced continuous fill bar built from designer art). All four styles update every frame and auto-hide when the tracked entity is hidden.

**Shared top-level fields:**

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `stat_key` | `String` | — | Stat key; supports `{self}` substitution |
| `offset` | `(f32,f32,f32)` | `(0, 2.8, 0)` | World-space offset from the entity origin in metres |
| `screen_offset` | `(f32,f32)` | `(0, 0)` | Screen-space offset in pixels, applied AFTER projection — see `StatLabelDef.screen_offset` above for why this exists and how to use it (same convention, same purpose: stack this bar against another widget on the same entity using a shared `offset` plus a distinct `screen_offset`, instead of slightly different `offset`s that drift apart with zoom). |
| `fill_color` | `(f32,f32,f32,f32)` | bright green | Base fill colour; used when `color_bands` is empty or no band matches |
| `bg_color` | `(f32,f32,f32,f32)` | dark red-brown | Background track colour |
| `color_bands` | `Vec<(f32,(f32,f32,f32,f32))>` | `[]` | Threshold-based fill colour overrides. Each entry is `(above_ratio, (r,g,b,a))`. The **highest** `above_ratio` ≤ the current fill ratio wins. Ratios are 0.0–1.0. When empty, the `Ascii` style falls back to built-in adaptive green/yellow/red; `Pixel`/`Textured` use `fill_color` directly. Ignored by `Icon` (its filled/empty look comes from the atlas cells, not a colour). |
| `style` | `WorldStatBarStyle` | `Ascii()` | Visual style; see variants below |

`fill_color`, `bg_color`, and `color_bands` all have **no effect** on the `Icon` style — there is no fill tint and no background quad; the filled/empty look comes entirely from `filled_index`/`empty_index` in the atlas art itself. `Textured` reuses both colour fields as **multiply-tints**: `fill_color`/`color_bands` tint the fill layer, `bg_color` tints the empty/track layer — see the `Textured` section below.

**`WorldStatBarStyle::Ascii` fields** (use `style: Ascii(...)` or omit `style` entirely for the default):

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `cells` | `u8` | `10` | Total character width. Practical range: 1–32. |
| `font_size` | `f32` | `14.0` | Screen-space font size in pixels |

**`WorldStatBarStyle::Pixel` fields** (use `style: Pixel(...)`):

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `size` | `(f32,f32)` | `(64.0, 8.0)` | Bar width × height in screen pixels, at full size. Shrinks with camera distance when the scene sets `label_depth_scale` — Pixel bars used to be excluded from depth scaling (a v1 limitation), that's no longer true. |
| `border` | `f32` | `1.5` | Border thickness in pixels. Set to `0.0` to disable. |
| `border_color` | `(f32,f32,f32,f32)` | near-black `(0.05,0.05,0.05,1.0)` | Border quad colour |

**`WorldStatBarStyle::Icon` fields** (use `style: Icon(...)`):

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `icon_sheet` | `String` | — (required) | Catalog key into `AssetCatalog.textures` — the sprite sheet both `filled_index`/`empty_index` come from. Same convention as `ActionBarDef`/`ItemDef`. |
| `icon_cols` | `u32` | `8` | Columns in the icon atlas grid. The `8`×`8` default matches the engine's status-effect/item atlases; a dedicated pip sheet (like the shipped hearts) is usually smaller (e.g. `2`×`1`) — set both `icon_cols`/`icon_rows` to match your actual sheet, or the wrong cells will be sampled with no error. |
| `icon_rows` | `u32` | `8` | Rows in the icon atlas grid — see `icon_cols` note |
| `icon_cell_size` | `u32` | `64` | Pixel size of each square cell **in the source atlas image** |
| `filled_index` | `u32` | — (required) | Row-major index (`col + row * icon_cols`) of the "filled" cell, e.g. a solid heart |
| `empty_index` | `u32` | — (required) | Row-major index of the "empty" cell, e.g. a hollow heart outline |
| `cells` | `u8` | `5` | Total number of cells (pips) the bar represents. Practical range: 1–20. |
| `spacing` | `f32` | `4.0` | Gap between adjacent cell **edges**, in screen pixels — same convention as `ActionBarDef.slot_gap` (not centre-to-centre distance) |
| `size` | `(f32,f32)` | `(24.0, 24.0)` | Per-cell size in screen pixels |

Texture dimensions for `icon_sheet` do **not** need to be power-of-2 — that was a WebGL1-era constraint, not a WebGPU/WebGL2 one. Power-of-2 (matching the engine's other shipped icon sheets, e.g. 512×512) is still a reasonable default for mip-mapping/compression parity, but not required.

**`Icon` fill rounding — `ceil`, not `round`:** the number of filled cells is `filled = 0` only at exactly `ratio == 0.0`; otherwise `filled = max(1, ceil(ratio * cells))`. On a 5-cell bar this means 1% health always shows **at least 1** filled cell (never reads as dead while the entity is alive), and anything above 80% shows as **full** (`ceil(0.81 * 5) = 5`) — this is expected/idiomatic for a discrete pip display, not a bug. There is no partial-cell (half-heart) rendering.

**`WorldStatBarStyle::Textured` fields** (use `style: Textured(...)`):

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `texture_sheet` | `String` | — (required) | Catalog key into `AssetCatalog.textures` — ONE sheet containing both the fill and empty frames (see `fill_rect`/`empty_rect` below). Same catalog convention as `Icon.icon_sheet`. Both the fill and empty-track `Sprite` layers share this one image handle (resolved/loaded once, not twice). |
| `fill_rect` | `(f32,f32,f32,f32)` | — (required) | `(x, y, w, h)` in **texture pixels** — the sub-rect of `texture_sheet` containing the FULL/fill frame (the solid, "100% health" art). Cropped via `Sprite.rect`. |
| `empty_rect` | `(f32,f32,f32,f32)` | — (required) | `(x, y, w, h)` in **texture pixels** — the sub-rect of `texture_sheet` containing the EMPTY/track frame (the "0% health" / background art). |
| `size` | `(f32,f32)` | `(64.0, 12.0)` | Bar width × height in **screen pixels**, at full size. Shrinks with camera distance when the scene sets `label_depth_scale` — Textured bars used to be excluded from depth scaling (a v1 limitation, same as Pixel), that's no longer true. |
| `slice_border` | `(f32,f32,f32,f32)` | `(6.0, 6.0, 6.0, 6.0)` | 9-slice cap insets `(left, right, top, bottom)` in **texture pixels**, relative to each rect's own origin — the fixed corner/cap regions of your art that must not stretch as the fill grows/shrinks. Applied identically to both `fill_rect` and `empty_rect` (both frames are assumed to share one cap geometry). For a stadium/pill shape, cap width ≈ frame height / 2. **`left + right` must be less than the rect's own `w`, and `top + bottom` less than its own `h`** — for *both* `fill_rect` and `empty_rect` — or Bevy logs a console error and renders that layer un-sliced (a plain stretched quad, caps included) instead. There is no engine-side validation of this today; check it yourself when authoring. |

**What "9-slice" means, briefly:** the source rect is divided into a 3×3 grid — four fixed-size corners, four edges that stretch along one axis, and a center that stretches along both. As the bar resizes, only the edges/center stretch; the corners (and therefore your rounded caps/border) stay a constant pixel size. `slice_border` is what defines where those corner boundaries sit within your source art.

**Two different pixel spaces — don't mix them up:** `size` is on-screen pixels (how big the bar renders in the game); `fill_rect`/`empty_rect`/`slice_border` are all source-image pixels (where things live inside your sheet file). A 72×14 on-screen bar can crop from a 96×40 source sheet just fine — the two numbers are unrelated. Coordinates are measured from the sheet's **top-left corner**, with X increasing right and Y increasing **down** — the same readout your image editor shows when you hover the cursor. In the shipped sheet, `fill_rect` starts at `(0, 0)` (the top of the file) and `empty_rect` at `(0, 17)` (the frame directly below it).

**Getting the numbers for your own art:** open your sheet in any image editor and, for each frame, read off the pixel coordinates of its top-left corner and its width/height — those four numbers are the rect. There's no in-engine tool for this; it's manual measurement against the source file.

**Corners don't stay pixel-perfect if `size` doesn't match your source rect's dimensions.** The claim "corners stay round" means corners are scaled *uniformly* (never stretched into an oval) — not that they're rendered at a fixed pixel size regardless of `size`. Concretely: Bevy computes a single scale factor from the smaller of the horizontal and vertical fit ratios and applies it to both corner axes together. If your `size` height doesn't equal the source rect's height, corners will render slightly smaller than "native" even at 100% fill (matching whichever axis is more cropped) — round, not stretched, just not 1:1. This compounds at low fill, where the corners shrink further as the fill's on-screen width drops below the summed cap widths (documented below). For the tightest fidelity, pick `size` so its height matches your source rect's height at the render scale you're using.

**One sheet, two frames — not two separate textures.** Unlike an early draft of this feature, `Textured` does **not** take separate `fill_texture`/`empty_texture` catalog keys. Both layers crop from one shared `texture_sheet` via `Sprite.rect` — e.g. the shipped `rounded-healthbar-texture-sheet.png` is a single 48×48 PNG: rows 0–16 hold a solid filled pill, rows 17–33 hold a hollow track outline (with a couple of rows of transparent padding at the bottom to keep both frames the same 17px height — matching heights avoids the corner-scale mismatch described above). Author your own sheets the same way (any layout — top/bottom, side-by-side — as long as `fill_rect`/`empty_rect` point at the right sub-regions); matching frame heights isn't required, but makes the corner math simpler to reason about.

**Default tints are not neutral.** `Textured` reuses the shared `fill_color` (default bright green) and `bg_color` (default dark red-brown) as *multiply*-tints over your art — a bar that omits both will render as `art × green` (fill) and `art × dark-red-brown` (track), not your art's own colours. If your art is already fully coloured, set both to `(1.0, 1.0, 1.0, 1.0)` (white = no tint); if your art is greyscale/white, set your own `fill_color`/`color_bands`/`bg_color` for the look you want — don't rely on the defaults being neutral.

**Colour tinting — reuses `fill_color`/`bg_color`, no `Textured`-only field.** Both frames in the shipped reference sheet are flat, colourless mid-grey, authored specifically to be recoloured by multiply-tint: the fill layer's `Sprite.color` is set from `fill_color`/`color_bands` (identical selection logic to `Pixel`), and the empty/track layer's `Sprite.color` is set **once at spawn** from `bg_color` (it never changes after that, since the track doesn't animate). If you supply pre-coloured, opaque art instead and want zero tint, set both `fill_color: (1.0, 1.0, 1.0, 1.0)` and `bg_color: (1.0, 1.0, 1.0, 1.0)` (white = no tint).

**Low-fill behavior:** when the fill's on-screen width shrinks below the summed cap widths (very low health on a small bar), Bevy's 9-slicer scales the corner slices down proportionally rather than clipping or overlapping them — so near-empty, the rounded caps get tighter (a smaller effective radius) instead of glitching. This is benign and often looks fine; just be aware the corners "un-round" slightly at very low ratios on a tightly-sized bar.

> **Pixel/Icon/Textured bar depth scaling:** these three styles render at a fixed screen-pixel size regardless of camera distance. Depth-based scaling is not yet implemented for any of them (only `Ascii`/`stat_label` support it).

> **Split-screen visibility:** `stat_label` and all four `world_stat_bar` styles (`Ascii`, `Pixel`, `Icon`, `Textured`) correctly duplicate across simultaneously-visible split viewports (local co-op scenes with `camera.split` configured) — each active viewport gets its own correctly-positioned copy, same as portal room-name labels. Damage popups and nameplates do **not** duplicate — an entity's popup or nameplate shows in **at most one** viewport at a time. **This applies to co-op players too**: any `world_stat_bar` style works correctly on a split-screen player prefab — pick whichever look you want, duplication is not a factor in that choice.
>
> **`Ascii` is a prototyping/debug style; `Pixel`, `Icon`, and `Textured` are the production-quality choices.** `Ascii` is the silent default when `style` is omitted, so a `world_stat_bar` that doesn't set `style` explicitly is using the prototyping look by default — set `style: Pixel(...)` for a solid-fill bar, `style: Icon(...)` for a discrete pip/heart display, or `style: Textured(...)` for an art-directed continuous bar with rounded caps/borders. `Ascii` may be retired in a future version; existing bars with no `style` set will keep working until then.

```ron
// Minimal — omit style to get the default Ascii bar.
world_stat_bar: (
  stat_key: "{self}.health",
),

// Ascii bar — full configuration.
world_stat_bar: (
  stat_key: "{self}.health",
  offset: (0.0, 2.4, 0.0),
  fill_color: (0.15, 0.85, 0.15, 0.95),
  bg_color: (0.25, 0.08, 0.08, 0.75),
  color_bands: [
    (0.6,  (0.15, 0.85, 0.15, 0.95)),  // ≥ 60 % → green
    (0.3,  (1.0,  0.75, 0.10, 1.0)),   // ≥ 30 % → yellow
    (0.0,  (0.85, 0.10, 0.10, 1.0)),   // ≥  0 % → red
  ],
  style: Ascii( cells: 12, font_size: 14.0 ),
),

// Pixel bar — size and border are in screen pixels.
world_stat_bar: (
  stat_key: "{self}.health",
  offset: (0.0, 2.5, 0.0),
  fill_color: (0.15, 0.85, 0.15, 1.0),
  bg_color:   (0.20, 0.05, 0.05, 0.85),
  color_bands: [
    (0.0, (0.85, 0.12, 0.12, 1.0)),
    (0.3, (0.95, 0.75, 0.10, 1.0)),
    (0.6, (0.15, 0.85, 0.15, 1.0)),
  ],
  style: Pixel( size: (48.0, 6.0) ),
),

// Icon bar — a 5-heart lives/health display, reusing the standard icon-atlas convention.
world_stat_bar: (
  stat_key: "player_health", // a global LoadedStats key here, illustratively — {self}.health works too
  offset: (0.0, 2.3, 0.0),
  style: Icon(
    icon_sheet: "icons_hearts",
    icon_cols: 2, icon_rows: 1,
    filled_index: 0, empty_index: 1,
    cells: 5,
  ),
),

// Textured bar — a rounded, 9-sliced continuous health bar cropped from one shared sheet.
// This is the shipped 3rd_person_game_demo player bar (prefabs/prefabs.ron).
world_stat_bar: (
  stat_key: "player_health",
  offset: (0.0, 2.3, 0.0),
  bg_color: (0.15, 0.15, 0.15, 0.6),  // dim neutral track tint
  color_bands: [
    (0.0, (0.85, 0.12, 0.12, 1.0)),  // < 30% -> red
    (0.3, (0.95, 0.75, 0.10, 1.0)),  // >= 30% -> yellow
    (0.6, (0.15, 0.85, 0.15, 1.0)),  // >= 60% -> green
  ],
  style: Textured(
    texture_sheet: "healthbar_sheet",
    fill_rect:  (0.0, 0.0, 48.0, 17.0),   // solid pill frame, top of the sheet
    empty_rect: (0.0, 17.0, 48.0, 17.0),  // hollow outline frame, below it
    size: (72.0, 14.0),
    slice_border: (8.0, 8.0, 8.0, 8.0),
  ),
),
```

> **Tip:** Use `stat_label` for a compact numeric readout, `world_stat_bar` for a graphical bar. All three styles can coexist on the same prefab (useful for demos), but in production most designs pick one style per entity.

**Migration from the pre-style schema:** The old flat fields `cells` and `font_size` at the top level of `WorldStatBarDef` are no longer supported. Move them inside `style: Ascii(...)`:

```ron
// Old (no longer valid)
world_stat_bar: ( stat_key: "…", cells: 10, font_size: 16.0 )

// New
world_stat_bar: ( stat_key: "…", style: Ascii( cells: 10, font_size: 16.0 ) )
```

Entries with no `style` field continue to parse correctly — the default is `Ascii` with `cells: 10` and `font_size: 14.0`.

---

## `DamagePopupStyle`

Optional block in `{name}.project.ron` that controls how `Action::ShowDamagePopup` popups look. All fields have built-in defaults — omit the block entirely to use them.

```ron
// {name}.project.ron
(
    schema_version: 3,
    ...
    damage_popup_style: (
        font_size:     22.0,             // default
        duration_secs:  1.2,             // default
        rise_speed:     1.5,             // default — metres/second the label rises
        spawn_offset:  (0.0, 1.2, 0.0), // default — world-space offset from entity origin
        damage_color:  (0.95, 0.25, 0.20, 1.0),  // default red (RGBA)
        heal_color:    (0.20, 0.90, 0.20, 1.0),  // default green (RGBA)
    ),
)
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `font_size` | `f32` | `22.0` | Screen-space font size in pixels |
| `duration_secs` | `f32` | `1.2` | Seconds the popup is visible before fully fading |
| `rise_speed` | `f32` | `1.5` | Metres per second the label rises during its lifetime |
| `spawn_offset` | `(f32,f32,f32)` | `(0.0, 1.2, 0.0)` | World-space offset from the entity origin where the popup appears. Increase Y for tall entities. |
| `damage_color` | `(f32,f32,f32,f32)` | `(0.95, 0.25, 0.20, 1.0)` | sRGB RGBA colour for negative amounts (damage) |
| `heal_color` | `(f32,f32,f32,f32)` | `(0.20, 0.90, 0.20, 1.0)` | sRGB RGBA colour for positive amounts (healing) |

---

## `AudioConfig`

Optional block in `{name}.project.ron` that controls project-level audio volume. All fields have built-in defaults — omit the block entirely to use them.

```ron
// {name}.project.ron
(
    schema_version: 3,
    ...
    audio: (
        max_volume:    0.8,    // project ceiling; 0.0–1.0, default 1.0
        mute_on_start: false,  // default false
    ),
)
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `max_volume` | `f32` | `1.0` | Master volume ceiling (0.0–1.0). `SetVolume(100)` equals this value, not 1.0. Lets you tune overall project volume without touching individual audio source files. |
| `mute_on_start` | `bool` | `false` | Start the project muted. Equivalent to firing `ToggleMute` once immediately on project load. |

**Pipeline events** emitted by audio actions:

| Event | Trigger |
|-------|---------|
| `audio.muted` | `ToggleMute` transitions to muted, or `SyncAudioState` while already muted |
| `audio.unmuted` | `ToggleMute` transitions to unmuted, or `SyncAudioState` while not muted |
| `audio.volume_changed` | `SetVolume` changes the active fraction |

---

## `items.ron` — ItemCatalog ✅

Named item definitions for a project. Referenced via `items_path` in `{name}.project.ron`. Optional — omitting it means no inventory system for this project.

Items persist across scene transitions (the `PlayerInventory` resource is not cleared on scene load). Container `Inventory` components on entities reset on `LoadScene` because they are owned by `LevelEntity`.

**`ItemCatalog` fields:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `schema_version` | `u32` | ✅ | Must be `1` |
| `items` | `Map<String, ItemDef>` | ✅ | Named items keyed by item ID |

**`ItemDef` fields:**

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `display_name` | `String` | ✅ | Human-readable name shown in tooltip / shop panel |
| `icon_sheet` | `Option<String>` | `null` | Catalog texture key for the icon atlas this item's icon is on. Overrides the panel's `icon_sheet` default. Omit if the item is on the panel's default sheet. All sheets must share the panel's `icon_cols/rows/cell_size`. |
| `icon_index` | `u32` | `0` | Zero-based index into the icon atlas (row-major: `col + row * icon_cols`) |
| `icon_color` | `Option<(f32,f32,f32,f32)>` | `None` | sRGB RGBA multiplicative tint for the icon. See the tint note in the Action Bar section above |
| `stackable` | `bool` | `true` | Whether multiple units stack in a single slot |
| `max_stack` | `u32` | `99` | Maximum count per stack when `stackable: true` |
| `weight` | `f32` | `1.0` | Weight in game units (for future encumbrance mechanics) |
| `tags` | `Vec<String>` | `[]` | Arbitrary designer tags (e.g. `["consumable", "quest"]`) |
| `currency_stat` | `Option<String>` | `None` | When set, looting this item adds its count to the named global stat instead of occupying an inventory slot. Use for currency (e.g. `"gold"`). |

**`InventoryContainerDef` fields (on `PrefabDef.inventory`):**

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `max_slots` | `usize` | `9` | Maximum number of item slots this container entity has. Clamped to at least 4. |
| `initial_items` | `Vec<InitialItemEntry>` | `[]` | Items pre-placed at spawn time, in slot order. Excess is silently ignored when slots are full. |

**`InitialItemEntry` fields:**

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `item_key` | `String` | ✅ | Key of the item from `items.ron` |
| `count` | `u32` | `1` | How many to place |

```ron
// Place 3 health potions and 2 mana potions in chest_01 at spawn
inventory: (
    max_slots: 9,
    initial_items: [
        (item_key: "health_potion", count: 3),
        (item_key: "mana_potion",   count: 2),
    ],
),
```

The slot count label in `InventoryPanel` and `ContainerPanel` now shows `"N/MAX"` (e.g. `"3/10"`) for stackable items and nothing for non-stackable items.

**`MerchantDef` fields (on `PrefabDef.merchant`):**

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `stock` | `Vec<ShopEntry>` | ✅ | Items the merchant sells/buys |
| `currency_stat` | `String` | `"gold"` | Stat key used as currency (display-only in v1; buy/sell transactions are planned for v1.1) |

**`ShopEntry` fields:**

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `item_key` | `String` | ✅ | Key of the item from `items.ron` |
| `buy_price` | `u32` | ✅ | Price for the player to buy the item |
| `sell_price` | `u32` | ✅ | Price the merchant pays to buy the item from the player |
| `stock_count` | `Option<u32>` | `null` | If set, the merchant only has this many; displayed as `[N]` in the shop panel (deduction not yet implemented) |

**Example:**

```ron
// items/items.ron
(
    schema_version: 1,
    items: {
        // Uses panel's default icon_sheet (no override needed)
        "health_potion": (
            display_name: "Health Potion",
            icon_index: 2,          // row 0, col 2 — heart bottle
            stackable: true,
            max_stack: 10,
            weight: 0.5,
            tags: ["consumable"],
        ),
        // Uses a different sheet for this item only
        "iron_sword": (
            display_name: "Iron Sword",
            icon_sheet: "icons_weapons",   // overrides panel default for this item only
            icon_index: 5,                 // row 0, col 5 on icons_weapons sheet
            stackable: false,
            weight: 3.5,
            tags: ["weapon"],
        ),
        "key_iron": (
            display_name: "Iron Key",
            icon_index: 63,         // row 7, col 7
            stackable: false,
            weight: 0.3,
            tags: ["quest"],
        ),
    },
)
```

> **Multi-sheet note:** All sheets referenced by items in a project's item catalog (via `icon_sheet`) are pre-loaded at panel spawn time. All sheets must share the same grid dimensions (`icon_cols`, `icon_rows`, `icon_cell_size`) as defined on the `InventoryPanel`. The engine resolves per-item sheet key → panel default → skip icon, so items on the default sheet never need an explicit `icon_sheet` field.

```ron
// prefabs/prefabs.ron — container chest
"chest_small": (
    kind: Prop,
    model: "chest",
    interactable: (radius: 1.5, hint_text: "Open"),
    inventory: (max_slots: 9),
),

// prefabs/prefabs.ron — merchant NPC
"merchant_vendor": (
    kind: Actor,
    model: "npc_merchant",
    interactable: (radius: 2.0, hint_text: "Talk"),
    merchant: (
        stock: [
            (item_key: "health_potion", buy_price: 10, sell_price: 5),
            (item_key: "key_iron",      buy_price: 50, sell_price: 0, stock_count: 1),
        ],
    ),
),
```

**Pipeline events emitted by inventory actions:**

| Event | Trigger |
|-------|---------|
| `inventory.added:{entity}:{item_key}:{count}` | `AddItem` successfully adds items |
| `inventory.full:{entity}` | `AddItem` finds no room (all slots occupied) |
| `inventory.removed:{entity}:{item_key}:{count}` | `RemoveItem` removes items (count = actual removed) |
| `inventory.transferred:{from}:{to}:{item_key}` | `TransferItem` completes a move between entities |

> **v1 scope note:** `MerchantDef.buy_price`, `sell_price`, and `currency_stat` are display-only in v1. The shop panel shows pricing information but does not yet deduct or credit the `currency_stat`. Full buy/sell transaction support (stat deduction, item transfer) is planned for v1.1.

---

## Global environment lighting ✅

Can be set on `ProjectConfig.global_environment` and overrides per-scene lighting if a scene has no environment block.

```ron
global_environment: (
    intensity: 400.0,
    fallback: (
        top_color: (0.1, 0.2, 0.4),
        bottom_color: (0.01, 0.01, 0.01),
    ),
),
```

- `intensity` — IBL strength
- `fallback` — procedural gradient sky used when no `.ktx2` cubemap is specified
- `asset_path` (optional) — path to a `.ktx2` cubemap for full IBL

---

## Schema evolution

When adding fields to any data type:
1. Add the field as `#[serde(default)]` in the Rust struct to keep old files valid.
2. Add an example to the relevant project under `assets/projects/`.
3. Add or update a test in `ironhold_core/tests/ron_validation.rs`.
4. Update this document and `docs/STATUS.md`.

Breaking changes (removing or renaming fields) require a `schema_version` bump.

---

## Migrating `prefabs.ron` from schema_version 1 to 2

`PrefabCatalog` was bumped from version 1 to 2. Two sets of changes are required in every `prefabs.ron`:

### 1. `kind` field — quoted string → bare enum variant

```ron
// Before (v1)           // After (v2)
kind: "actor"      →     kind: Actor
kind: "prop"       →     kind: Prop
kind: "primitive"  →     kind: Primitive
```

### 2. Collider `shape` field — quoted string → bare enum variant

```ron
// Before (v1)           // After (v2)
shape: "Cuboid"    →     shape: Cuboid
shape: "Sphere"    →     shape: Sphere
shape: "Cylinder"  →     shape: Cylinder
```

### 3. Primitive shape — `model:` → `shape:` + `model: ""`

For top-level `kind: Primitive` prefabs that have a shape name in `model:`:

```ron
// Before (v1)
"my_cube": (
    kind: "primitive",
    model: "Cuboid",
    primitive: ( size: (2.0, 1.0, 2.0) ),
)

// After (v2)
"my_cube": (
    kind: Primitive,
    model: "",
    shape: Cuboid,
    primitive: ( size: (2.0, 1.0, 2.0) ),
)
```

### 4. Child `shape` — quoted string → bare enum variant

For `ChildPrimitiveDef` entries in `children:` lists:

```ron
// Before (v1)
( shape: "Sphere", primitive: (radius: 0.5), ... )

// After (v2)
( shape: Sphere, primitive: (radius: 0.5), ... )
```

Composite primitives (those that only have `children:`, no top-level `shape:`) do not need a `shape:` field — only single-mesh primitives do.

### 5. Bump `schema_version`

```ron
schema_version: 1  →  schema_version: 2
```
