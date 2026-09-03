# Feature: LOD — Pre-baked Mesh Swap

_Status: Draft_
_Planned at: `32df2ec` (2026-06-02)_

---

> ## Pre-implementation checklist
>
> - [ ] **Decide: LOD level schema shape.** Options: (a) parallel arrays — `lod_distances: [20.0, 50.0]` + `lod_models: ["creatures/orc_lod1", "creatures/orc_lod2"]`; (b) a combined struct list — `lod_levels: [(distance: 20.0, model: "creatures/orc_lod1"), ...]`. Recommendation: **combined struct list** (`lod_levels`). It keeps the pairing explicit, allows per-level `model: None` (hide at that distance), and avoids silent mismatches from mismatched array lengths.
>
> - [ ] **Decide: LOD swap mechanism.** GLB actors in Bevy spawn as `SceneRoot(Handle<Scene>)`. To show a different mesh: (a) swap `SceneRoot` to a pre-loaded handle (Bevy rebuilds GLB child hierarchy — fast if handle is hot, causes one-frame entity hierarchy rebuild); (b) spawn all LOD levels as siblings and toggle `Visibility` (no hierarchy rebuild, but wastes ECS memory for hidden levels); (c) use Bevy's planned `Lod` component (not in 0.18 stable). Recommendation: **swap `SceneRoot`** — memory-efficient (only one active GLB in ECS at a time), standard Bevy API, one-frame rebuild is acceptable at LOD boundary crossings. Pre-load all handles at spawn time to avoid asset-decode latency on swap.
>
> - [ ] **Decide: LOD distance measurement.** Camera position or player entity position? Recommendation: **active camera position** — matches visual expectation (a telescoped view could see an object clearly while the player is far away).
>
> - [ ] **Decide: hysteresis.** Without hysteresis, an entity at exactly the LOD boundary swaps every frame as the camera oscillates. Recommendation: **add a 10% hysteresis margin** — switch UP (to lower detail) at `distance * 1.05` and switch DOWN (to higher detail) at `distance * 0.95`. Prevents flicker at the boundary.
>
> - [ ] **Decide: hidden-beyond-last behavior.** When the camera is beyond the last LOD distance: (a) always hide; (b) stay at last LOD (never hide); (c) designer controls via `model: None` at the final level. Recommendation: **`model: None` opt-in** — if the last `lod_level` has `model: None`, the entity hides beyond that distance; otherwise it stays at the last LOD. This is explicit and handles both use cases.
>
> - [ ] **Confirm: `SceneRoot` swap stability.** Verify in Bevy 0.18 that swapping `SceneRoot` on a running entity correctly despawns old GLB children and spawns new ones without dangling component queries in other systems (animation, stat bars, etc.). The animation system walks `Children` of the `SceneRoot` entity — confirm it handles a mid-frame hierarchy rebuild cleanly.

---

## What

Distance-based LOD switching for GLB actor and prop prefabs using pre-baked lower-detail GLB files. The designer declares `lod_levels` on a `PrefabDef` — each level specifies a distance threshold and a catalog model key. A single system watches camera distance and swaps `SceneRoot` when crossing a threshold.

No runtime mesh decimation. No web workers. No geometry shaders. Fully WASM-compatible.

---

## Why

High-poly GLB models rendered at full detail at 80m cost the same as at 2m. For open-world or crowd-heavy scenes, swapping to pre-decimated LOD GLBs (generated once offline in Blender or `meshopt`) dramatically reduces vertex count without visible quality loss at distance.

This is the standard LOD approach for data-driven engines: the heavy work (mesh simplification) is done at asset-authoring time; the runtime only decides which handle to bind.

---

## Asset workflow (offline, not part of this feature)

1. Designer opens `orc.glb` in Blender.
2. Uses the Decimate modifier to produce `orc_lod1.glb` (~50% poly) and `orc_lod2.glb` (~15% poly).
3. Exports both and places them alongside `orc.glb`.
4. Registers both in `assets.ron`:

```ron
models: {
    "creatures/orc":      ( path: "models/creatures/orc.glb" ),
    "creatures/orc_lod1": ( path: "models/creatures/orc_lod1.glb" ),
    "creatures/orc_lod2": ( path: "models/creatures/orc_lod2.glb" ),
},
```

The Ironhold runtime is unaware of the relationship between these keys — it only knows the `PrefabDef` references them as LOD levels.

---

## Schema

### `PrefabDef` — new field (`schema/catalog.rs`)

```ron
// prefabs/prefabs.ron
"orc_enemy": (
    kind: "actor",
    model: "creatures/orc",           // LOD0 — always the highest-detail base model
    lod_levels: [                     // NEW — optional LOD1, LOD2, ...
        ( distance: 20.0, model: Some("creatures/orc_lod1") ),
        ( distance: 50.0, model: Some("creatures/orc_lod2") ),
        ( distance: 100.0, model: None ),  // hidden beyond 100m
    ],
    // ...
)

"barrel_prop": (
    kind: "prop",
    model: "props/barrel",
    lod_levels: [
        ( distance: 40.0, model: Some("props/barrel_lod1") ),
        // beyond 40m: stays at barrel_lod1 (no None entry = never hide)
    ],
)
```

```rust
// schema/catalog.rs — in PrefabDef
/// Optional LOD levels. The base `model` is always LOD0.
/// Each entry defines the camera distance at which to switch to a lower-detail model.
/// Entries must be sorted by `distance` ascending (validated at load).
/// `model: None` at a level means hide the entity beyond that distance.
#[serde(default)]
pub lod_levels: Vec<LodLevelDef>,
```

### New `LodLevelDef` (`schema/catalog.rs`)

```rust
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct LodLevelDef {
    /// Camera distance (world units) at which this level becomes active.
    /// Switch-up: distance > threshold * 1.05.
    /// Switch-down: distance < threshold * 0.95.
    pub distance: f32,

    /// Model key from AssetCatalog.models to use beyond this distance.
    /// None = hide the entity beyond this distance.
    pub model: Option<String>,
}
```

### Validation in `PrefabCatalog::validate()`

- `lod_levels` distances are in ascending order.
- Each `model: Some(key)` exists in `AssetCatalog.models`.
- At most one `model: None` entry (must be the last level).

---

## Runtime

### Components (`capabilities/lod.rs`)

```rust
/// Attached to any entity whose prefab has `lod_levels`. Tracks current LOD state.
#[derive(Component)]
pub struct LodLevels {
    /// Pre-loaded scene handles in level order: index 0 = LOD0 (base model), 1 = LOD1, etc.
    /// None handle = hide at that level (from `LodLevelDef.model: None`).
    pub handles: Vec<Option<Handle<Scene>>>,

    /// Distance thresholds from PrefabDef.lod_levels, in ascending order.
    /// `distances[0]` is the LOD0→LOD1 boundary. Length = handles.len() - 1.
    pub distances: Vec<f32>,

    /// Active LOD index (0 = full detail). Used for change detection.
    pub current: usize,

    /// Tracks whether the entity is currently hidden (model: None active).
    pub hidden: bool,
}
```

### Spawn-time setup

In `entity_spawner.rs`, when spawning a prefab with non-empty `lod_levels`:

1. Resolve handles for all LOD levels:
   - Index 0: `asset_server.load::<Scene>(&lod0_path)` (the base model, already loaded as `SceneRoot`)
   - Index 1..N: `asset_server.load::<Scene>(&lod_n_path)` for each entry with `model: Some(...)`
   - `model: None` entries → `None` in the handles vec
2. Insert `LodLevels { handles, distances, current: 0, hidden: false }` on the entity.

Pre-loading via `asset_server.load()` at spawn time ensures all LOD handles are in the cache before the camera could reach the boundary. No on-demand HTTP fetch latency on swap.

### `lod_swap_system` (`capabilities/lod.rs`)

Runs in `Update`. Reads active camera `GlobalTransform`.

For each entity with `LodLevels` + `GlobalTransform` + `SceneRoot`:

1. Compute `dist = camera_pos.distance(entity_pos)`.
2. Determine target LOD index:
   - Start at 0. For each `(i, threshold)` in `distances`:
     - If `dist > threshold * 1.05` (with hysteresis) → target = i + 1
     - Else break.
3. If `target == current`: skip (no change needed).
4. If `target != current`:
   - If `handles[target]` is `None`: set `Visibility::Hidden`, set `current = target`, set `hidden = true`.
   - Else: `commands.entity(e).insert(SceneRoot(handles[target].clone()))`, set `Visibility::Inherited` if was hidden, set `current = target`, set `hidden = false`.

**Change-detection guard**: only write to ECS when `target != current`. Do not compare `Handle` equality every frame — track the index.

---

## Worked example

```
orc_enemy spawned at (10.0, 0.0, 10.0)
Camera at (10.0, 5.0, 10.0) → distance ≈ 5m → LOD0 (orc.glb) active

Camera moves to (10.0, 5.0, 35.0) → distance ≈ 32m → 32 > 20 * 1.05 = 21 → switch to LOD1
  system: commands.entity(orc).insert(SceneRoot(orc_lod1_handle))
  current: 1

Camera moves to (10.0, 5.0, 15.0) → distance ≈ 19m → 19 < 20 * 0.95 = 19 → switch to LOD0
  system: commands.entity(orc).insert(SceneRoot(orc_handle))
  current: 0

Camera at (10.0, 5.0, 120.0) → distance ≈ 110m → 110 > 100 * 1.05 = 105 → level 3 = None
  system: Visibility::Hidden; current: 3
```

---

## Interaction with other systems

- **Animation system**: the animation system walks GLB children via `Children`. When `SceneRoot` is swapped, Bevy despawns old children and spawns new ones. The animation system should gracefully handle a frame where the animated child entity no longer exists — confirm it queries `Option<&AnimationPlayer>` or similar (check `capabilities/animation.rs` before implementation).
- **`world_stat_bar` / `stat_label`**: these follow the entity's `GlobalTransform`, not its mesh children. A LOD swap does not affect stat bar tracking.
- **`NameplateTag`**: same as stat bars — nameplate anchor follows `GlobalTransform`. No special handling needed.
- **Physics colliders**: Rapier colliders are attached to the root entity, not GLB children. LOD swap does not affect physics.
- **`Despawn`**: the `SpawnRegistry` and `SpawnId` components are on the root entity. LOD swap does not affect despawn.

---

## Tooling note

For `ironhold_cli validate --strict`: extend the orphan checker to warn when an `assets.ron` model key with `_lod1` / `_lod2` suffix is not referenced in any `PrefabDef.lod_levels`. This catches stale LOD files after a prefab is updated.

---

## New Rust changes

- `schema/catalog.rs` — `LodLevelDef` struct; `lod_levels: Vec<LodLevelDef>` on `PrefabDef`; `PrefabCatalog::validate()` checks distance order and model key existence.
- `capabilities/lod.rs` (new file) — `LodLevels` component, `lod_swap_system`.
- `capabilities/mod.rs` — register module + system.
- `runtime/scene_manager/entity_spawner.rs` — at spawn time, if `lod_levels` non-empty, pre-load handles and insert `LodLevels`.

---

## Tasks

- [ ] Decisions from pre-implementation checklist resolved
- [ ] `LodLevelDef` struct + `lod_levels: Vec<LodLevelDef>` on `PrefabDef`
- [ ] `PrefabCatalog::validate()` — distance order + model key cross-check
- [ ] `LodLevels` component in `capabilities/lod.rs`
- [ ] Spawn-time handle pre-loading + `LodLevels` insertion in `entity_spawner.rs`
- [ ] `lod_swap_system` — distance compute, hysteresis, `SceneRoot` swap, `Visibility` toggle
- [ ] Change-detection guard (only write on level change)
- [ ] Verify animation system handles mid-run hierarchy rebuild (read `capabilities/animation.rs`)
- [ ] CLI `--strict` orphan hint for `_lod1`/`_lod2` models
- [ ] Demo: add LOD levels to 2–3 entities in `terrain_demo` or `3rd_person_game_demo`; verify swap at boundary
- [ ] Integration tests: LOD index advances at threshold, reverts on approach, hidden at `None` level, no swap when camera is static
- [ ] Docs: `lod_levels` field in `docs/20_data_formats.md`; asset workflow note in tools section

---

## Open questions

- **Animation on LOD-swapped entities**: if LOD1 GLB has a simplified rig (fewer bones), the animation clip names might differ from LOD0. v1 assumption: all LOD levels use the same animation clips or are non-animated. Animated LODs with different rig complexity are out of scope — document the constraint clearly.
- **LOD for primitive prefabs**: `kind: "primitive"` prefabs use CPU-built `Mesh` handles, not `SceneRoot`. LOD swap as designed only works for `kind: "actor"` and `kind: "prop"`. Exclude primitives from LOD for v1 (validate and warn if `lod_levels` is set on a primitive prefab).
- **Max LOD levels**: no hard limit. Practically, 2 LOD levels (LOD1 + hide) is the common case. 3+ levels are valid but rarely needed for the distances ironhold targets.
- **LOD bias for quality settings**: a future `SetLodBias(f32)` action could multiply all distances (e.g. 0.5× for high-quality mode uses full detail at greater range). Not in v1.

---

## Acceptance criteria

- Given a prefab with `lod_levels: [(distance: 20.0, model: Some("...lod1"))]`, the entity renders its `lod1` model when the camera is > 21m away (20m × 1.05 hysteresis).
- Given the camera returns within 19m (20m × 0.95), the entity reverts to the LOD0 base model.
- Given `lod_levels: [..., (distance: 80.0, model: None)]`, the entity becomes invisible beyond 84m and reappears within 76m.
- Given `lod_levels` is empty, no `LodLevels` component is inserted and the entity renders at full detail at all distances.
- Given a prefab with `lod_levels` where a model key does not exist in `AssetCatalog.models`, `PrefabCatalog::validate()` returns an error.
- Given a scene load with LOD-enabled prefabs, all LOD handles are pre-loaded at spawn time with no visible stall on first swap.
- Given `lod_levels` on a `kind: "primitive"` prefab, validation logs a warning and the field is ignored at runtime.
