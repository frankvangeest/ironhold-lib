# Feature: Per-prefab `select_aim_height` for click targeting

_Status: Active_
_Planned at: `9455db6` (2026-06-18)_

## What

Add a `select_aim_height: f32` field (default `1.0`) to `PrefabDef` so each prefab can declare
its own vertical offset for click-selection hit detection. Ground-hugging creatures (snake, spider)
currently use the global 1.0 m constant, which puts the click target a metre above their heads.

## Why

`targeting.rs` uses a global `SELECT_AIM_HEIGHT: f32 = 1.0` applied to every entity's world origin
before projecting to screen space. That is correct for human-sized characters (~1.8 m capsule) but
breaks for low-profile enemies:

- `enemy_snake`: `collider_height: 0.8`, body centre ≈ 0.4 m — constant overshoots by 0.6 m
- `enemy_spider`: `collider_height: 1.2`, body centre ≈ 0.6 m — overshoots by 0.4 m

With `select_aim_height` unset, all existing prefabs keep the current behaviour (default = 1.0).

## Approach

**Schema** (`schema/catalog.rs` — `PrefabDef`)

```rust
/// Vertical offset (metres) added to the entity world origin when projecting it to
/// screen space for click-selection. Defaults to 1.0 (humanoid body centre).
#[serde(default = "default_select_aim_height")]
pub select_aim_height: f32,
fn default_select_aim_height() -> f32 { 1.0 }
```

**Component** (`capabilities/targeting.rs`)

```rust
#[derive(Component)]
pub struct SelectAimHeight(pub f32);
```

**Spawner** (`runtime/scene_manager/entity_spawner.rs`)

In `spawn_prefab_instance` (or the shared `tag_spawned_entity` helper), insert
`SelectAimHeight(prefab.select_aim_height)` alongside the existing `ClickSelectable` marker.

**Systems** (`capabilities/targeting.rs`)

`click_select_system` and `debug_selectables_system` both query
`Option<&SelectAimHeight>` and use `.map_or(SELECT_AIM_HEIGHT, |h| h.0)` per entity.
The global constant becomes the documented default fallback and can eventually be removed
once all prefabs are explicit.

**RON changes** (`3rd_person_game_demo/prefabs/prefabs.ron`)

```ron
"enemy_snake": (
  ...
  select_aim_height: 0.4,   // collider_height 0.8 / 2 = body centre
  ...
),
"enemy_spider": (
  ...
  select_aim_height: 0.6,   // collider_height 1.2 / 2 = body centre
  ...
),
```

## Tasks

- [ ] Add `select_aim_height: f32` (serde default 1.0) to `PrefabDef`
- [ ] Add `SelectAimHeight(f32)` component to `targeting.rs`
- [ ] Insert component in spawner alongside `ClickSelectable`
- [ ] Update `click_select_system` and `debug_selectables_system` to read per-entity height
- [ ] Set `select_aim_height: 0.4` on `enemy_snake`, `0.6` on `enemy_spider` in prefabs.ron
- [ ] Run tests; update integration test fixture if schema snapshot is checked

## Acceptance criteria

- Clicking at ground level on a snake or spider selects it reliably.
- Clicking a metre above a snake does NOT select it (the gizmo sphere should appear at body centre).
- Human-scale entities (orc, zombie, player) behave identically to before (no `select_aim_height` → default 1.0).
- `debug_target_hitboxes` gizmo sphere appears at body centre for all three creature types.
