# Feature: Target Indicator Color by Category and Per-Prefab Override

_Status: Ready_
_Planned at: `1d67762` (2026-06-18)_

## What

Make the target-indicator ground-ring colour depend on what is targeted, instead of
being a single scene-wide colour. Resolution is layered, highest precedence first:

1. **Prefab `indicator_color: Option<(f32,f32,f32,f32)>`** — direct per-prefab RGBA override.
2. **Prefab `indicator_category: Option<String>`** — a key looked up in the scene-level
   `target_indicator.named_colors` map (e.g. `"enemy"`, `"ally"`, `"loot"`).
3. **Scene-level `target_indicator.color`** — existing fallback when a prefab declares neither.

All authored in RON; no recompile. Designers get red rings on enemies, green on allies,
gold on loot, etc.

## Why

The selected-target ring currently uses one baked colour per scene
(`ResolvedTargetIndicator.color`). In a mixed scene (enemies, NPCs, pickups) the ring
gives no read on *what kind* of thing is selected. Category-driven colour is a standard
ARPG affordance and is naturally data-driven: it is purely cosmetic, so it does not need
to flow through the Message→Action pipeline. It fits the existing
`capabilities/target_indicator.rs` reactive-cosmetic model (a pure side-effect of
`CurrentTarget` state).

## Approach

The targeting capability already attaches a `PrefabKey(String)` component to every
addressable entity via `tag_spawned_entity` (single source of truth). The indicator
system already resolves `CurrentTarget` (spawn id) → `Entity` via `SpawnRegistry`. The
colour for a target is resolved at **target-switch time** by reading that entity's
`PrefabKey`, looking the prefab up in `PrefabCatalog`, and applying the precedence chain —
**without** adding any new spawn-time plumbing or new components.

Material is rebuilt only when the resolved colour actually differs from the last one used;
resolved (colour → material) handles are memoised in a small map so alternating between
two targets of the same colour never reallocates a `StandardMaterial`.

### Schema changes

**`schema/scene_v2.rs` — `TargetIndicatorDef`** (add one field; keep `deny_unknown_fields`):

```rust
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct TargetIndicatorDef {
    pub texture: String,
    #[serde(default = "default_indicator_radius")]
    pub radius: f32,
    #[serde(default = "default_indicator_color")]
    pub color: (f32, f32, f32, f32),
    #[serde(default = "default_indicator_offset_y")]
    pub offset_y: f32,
    /// Named colour palette for `indicator_category` lookups on prefabs.
    /// Key = category string authored on a prefab's `indicator_category`;
    /// value = RGBA tint. A prefab whose category is absent from this map
    /// falls through to `color`. Default: empty.
    #[serde(default)]
    pub named_colors: std::collections::HashMap<String, (f32, f32, f32, f32)>,
}
```

**`schema/catalog.rs` — `PrefabDef`** (add two flat fields; keep `deny_unknown_fields`).
Flat placement mirrors existing `click_selectable` / `targetable` fields in the same
targeting cluster. Do NOT nest under a sub-struct — these are conceptually part of the
same group and a sub-struct adds an unnecessary discovery hop for designers.

```rust
    /// Per-prefab target-indicator ring colour override (RGBA), highest precedence.
    /// When set, used directly regardless of `indicator_category` or scene `target_indicator.color`.
    /// Only meaningful when the prefab is `targetable` or `click_selectable`.
    #[serde(default)]
    pub indicator_color: Option<(f32, f32, f32, f32)>,
    /// Category key looked up in the scene's `target_indicator.named_colors` map to pick
    /// the ring colour. Ignored if `indicator_color` is set. Falls through to scene
    /// `target_indicator.color` if the key is absent from the map.
    #[serde(default)]
    pub indicator_category: Option<String>,
```

### Runtime changes

**`runtime/scene_manager/mod.rs` — `ResolvedTargetIndicator`** carries the resolved
default colour plus the named palette (texture/radius/offset stay scene-global):

```rust
pub struct ResolvedTargetIndicator {
    pub texture_path: String,
    pub radius: f32,
    pub color: (f32, f32, f32, f32),          // scene-level fallback
    pub offset_y: f32,
    pub named_colors: std::collections::HashMap<String, (f32, f32, f32, f32)>,
}
```

Populate `named_colors` from `TargetIndicatorDef.named_colors` at scene load alongside
the existing colour/radius/offset resolution.

**`capabilities/target_indicator.rs` — `target_indicator_system`**

Replace the single cached material handle with a per-colour memo keyed by `[u32; 4]`
(raw `f32::to_bits()` of each RGBA channel — `f32` is not `Eq`, so the tuple cannot be
used directly as a map key). Add `PrefabKey` query and `PrefabCatalog` resource.

- New `Local` caches (cleared on scene change alongside the mesh):
  ```rust
  mut cached_mesh: Local<Option<Handle<Mesh>>>,
  mut cached_mats: Local<HashMap<[u32; 4], Handle<StandardMaterial>>>,
  ```

- Additional system parameters:
  ```rust
  prefab_keys: Query<&PrefabKey>,
  prefab_catalog: Res<LoadedPrefabCatalog>,   // whichever resource Action::Spawn uses
  ```

- Colour resolution helper (pure fn, called at target-switch time):
  ```rust
  fn resolve_indicator_color(
      target_entity: Entity,
      prefab_keys: &Query<&PrefabKey>,
      catalog: &LoadedPrefabCatalog,
      cfg: &ResolvedTargetIndicator,
  ) -> (f32, f32, f32, f32) {
      let Ok(PrefabKey(key)) = prefab_keys.get(target_entity) else {
          return cfg.color;
      };
      let Some(prefab) = catalog.get(key) else {
          return cfg.color;
      };
      if let Some(c) = prefab.indicator_color {
          return c;                                   // (1) direct override
      }
      if let Some(cat) = prefab.indicator_category.as_deref() {
          if let Some(c) = cfg.named_colors.get(cat) {
              return *c;                              // (2) category palette
          }
      }
      cfg.color                                       // (3) scene fallback
  }
  ```

- Material memo lookup at spawn time:
  ```rust
  let rgba = resolve_indicator_color(target_entity, &prefab_keys, &prefab_catalog, cfg);
  let key = [rgba.0.to_bits(), rgba.1.to_bits(), rgba.2.to_bits(), rgba.3.to_bits()];
  let mat_handle = cached_mats.entry(key).or_insert_with(|| {
      let texture = asset_server.load(cfg.texture_path.clone());
      materials.add(StandardMaterial {
          base_color_texture: Some(texture),
          base_color: Color::srgba(rgba.0, rgba.1, rgba.2, rgba.3),
          alpha_mode: AlphaMode::Blend,
          unlit: true,
          depth_bias: 64.0,
          double_sided: true,
          cull_mode: None,
          ..default()
      })
  }).clone();
  ```

The per-frame XZ-tracking branch is unchanged. The "scene changed" branch clears both
`cached_mesh` and `cached_mats`.

> **Note:** All rings share one mesh (radius-driven, colour-independent) and the same
> alpha-blend `StandardMaterial` pipeline variant — only `base_color` differs between
> material instances, so there is no new WebGPU pipeline compile and no warmup stall.

### RON examples

`scenes/*.scene.ron` — scene-level palette:

```ron
target_indicator: (
    texture: "ring_decal",
    radius: 1.2,
    color: (0.3, 0.8, 1.0, 0.75),
    offset_y: 0.05,
    named_colors: {
        "enemy": (1.0, 0.15, 0.15, 0.85),
        "ally":  (0.2, 1.0, 0.3,  0.85),
        "loot":  (1.0, 0.85, 0.2, 0.85),
    },
),
```

`prefabs/prefabs.ron` — category on an enemy, direct override on a boss:

```ron
"enemy_orc_melee": (
    kind: Actor,
    model: "orc",
    targetable: true,
    click_selectable: true,
    indicator_category: "enemy",
),
"boss_dragon": (
    kind: Actor,
    model: "dragon",
    targetable: true,
    indicator_color: (0.8, 0.0, 0.9, 0.95),   // unique purple, ignores palette
),
```

Existing RON with no `named_colors` / `indicator_color` / `indicator_category` is
fully backwards-compatible — all fields are `#[serde(default)]`.

## Tasks

- [ ] Add `named_colors` to `TargetIndicatorDef` (`schema/scene_v2.rs`) with `#[serde(default)]`
- [ ] Add `indicator_color` + `indicator_category` to `PrefabDef` (`schema/catalog.rs`) with `#[serde(default)]`
- [ ] Add `named_colors` to `ResolvedTargetIndicator` (`runtime/scene_manager/mod.rs`); populate at scene-load resolution site
- [ ] Rework `target_indicator_system` cache: single mesh handle + per-colour material memo (`HashMap<[u32;4], Handle<StandardMaterial>>`); clear both on scene change
- [ ] Add `resolve_indicator_color` helper, `PrefabKey` query, and prefab-catalog resource to the system
- [ ] Wire `named_colors` into `3rd_person_game_demo` — add palette to `playing.scene.ron` and `indicator_category` to enemy prefabs
- [ ] `cargo check -p ironhold_cli` — additive schema changes; confirm no breakage
- [ ] Integration test: scene with `named_colors` + two prefabs (one category, one direct override); assert resolved colour per target and no extra `StandardMaterial` allocation on repeat switches
- [ ] Docs: `docs/20_data_formats.md` (TargetIndicatorDef + PrefabDef reference tables); `crates/ironhold_core/src/CLAUDE.md` (target-indicator section — note colour is per-target via `PrefabKey` lookup, not spawn-time plumbing)

## Open questions

- **Material handle accumulation** — `cached_mats` grows by one handle per distinct colour
  seen this scene. The palette is designer-bounded (handful of categories). If a project
  authors dozens of unique `indicator_color` overrides this could grow, but there is no
  eviction concern at realistic scale — cleared per scene either way.
- **`indicator_color`/`indicator_category` on non-selectable prefabs** — harmless dead
  data (indicator only spawns for selected targets). A future `--strict` orphan pass could
  warn if `indicator_category` references a key absent from every scene's `named_colors`,
  but that is a follow-up enhancement.

## Acceptance criteria

- A prefab with `indicator_color` shows that exact RGBA ring regardless of palette or scene colour.
- A prefab with `indicator_category` matching a `named_colors` key shows the palette colour.
- A prefab with `indicator_category` *not* in `named_colors` falls back to scene `color`.
- A prefab with neither shows scene `color` (existing behaviour, unchanged).
- Existing scenes/prefabs with none of the new fields load and render identically to before.
- Switching repeatedly between two same-coloured targets creates no new `StandardMaterial` after the first of each colour.
- WASM build succeeds; no new pipeline-variant stalls (verified: all rings share the alpha-blend pipeline — only `base_color` uniform differs).
