# Feature: Grid System

_Status: Draft_
_Planned at: `2504768` (2026-06-02)_

---

> ## Pre-implementation checklist
>
> - [ ] **Decide: per-scene grid or project-level grid catalog.** Most grid games have one grid per scene (the map IS the grid). Recommendation: **`grid: Option<GridDef>` on `GameSceneV2`** — one grid per scene, optional. A multi-grid scene (e.g. a chess game with two boards) is out of scope for v1; if needed, the designer places two scenes.
>
> - [ ] **Decide: coordinate representation in RON actions.** All three grid types use 2D coordinates in the actions schema (`cell: (col, row)`). For hex, these are axial coordinates `(q, r)`. For triangles, parity (upward vs. downward face) is derived as `(col + row) % 2 == 0` → up. Designers never write a third component. Confirm this is sufficient before coding.
>
> - [ ] **Decide: grid plane orientation.** The grid lives on the XZ plane at `y = grid_origin.y`. Entities on the grid stand at that Y height; their XZ positions are snapped to cell centers. Confirm there is no use case for a vertical grid (wall-mounted) in v1 — if so, defer.
>
> - [ ] **Decide: bounded vs. unbounded grids.** Options: (a) always bounded — `dimensions: (cols, rows)` required, out-of-bounds cells are invalid; (b) unbounded — infinite grid, cells lazily allocated. Recommendation: **bounded only** for v1 — all three target game types (RTS, strategy, puzzle) use bounded maps; unbounded requires sparse storage and complicates the overlay renderer.
>
> - [ ] **Decide: pathfinding sync vs. async.** A* on a 256×256 grid visits up to 65536 cells per call. On WASM, this blocks the main thread. Options: (a) synchronous with a hard node-visit cap (e.g. 2048 nodes — enough for most game-sized grids); (b) async via `AsyncComputeTaskPool`. Recommendation: **synchronous with cap** for v1 — simpler, sufficient for game-scale grids (≤ 64×64 typical); note the cap in docs so designers can tune `max_path_nodes` in `GridDef`. `AsyncComputeTaskPool` degrades to blocking on WASM anyway, so there is no practical benefit to async in v1.
>
> - [ ] **Decide: diagonal movement for square grids.** Options: (a) 4-neighbor only (NSEW, Manhattan); (b) 8-neighbor (with diagonals, Chebyshev). Recommendation: **configurable via `neighbors: Cardinal | Diagonal`** on `GridDef`. Default `Cardinal` (4-way) for classic RTS feel; `Diagonal` for chess-style or roguelikes.

---

## What

A data-driven grid system for square, hexagonal, and triangular cell layouts. Defined in scene RON, the grid divides the XZ world plane into discrete addressable cells. Entities snap to cell centers. The engine provides coordinate conversion, neighbor queries, A* pathfinding, and a debug overlay. Designers drive all grid behaviour through actions and events — no grid-specific scripting required.

**Target game genres**: RTS (Age of Empires, Command & Conquer), turn-based strategy (Civilization, XCOM), board games (chess, Catan), puzzle games (Sokoban, Hexcells).

---

## Why

Without a grid system, implementing any of the above genres requires designers to manually track world positions, implement proximity snapping, and write their own distance/neighbor logic in game variables — a significant and error-prone burden. A data-driven grid moves all of that into the engine layer and exposes clean designer-facing actions and events.

The grid is purely a logical and visual overlay over the existing 3D world — it does not replace physics or the existing transform system.

---

## Schema

### Scene RON — `grid` field on `GameSceneV2`

```ron
// scenes/strategy_map.scene.ron
(
    grid: Some((
        kind: Square,
        origin: (-20.0, 0.0, -15.0),   // world XZ position of cell (0,0) corner
        cell_size: 2.0,                 // metres per cell edge
        dimensions: (20, 15),           // cols × rows (bounded grid)
        neighbors: Cardinal,            // Cardinal (4) | Diagonal (8); Square only
        hex_orientation: PointyTop,     // PointyTop | FlatTop; Hex only, ignored otherwise
        max_path_nodes: 2048,           // A* node cap per pathfinding call
        show_overlay: false,            // debug grid line rendering
        overlay_color: (0.4, 0.6, 1.0, 0.25),
    )),
    // ...
)
```

### New `GridDef` and related types (`schema/scene_v2.rs`)

```rust
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct GridDef {
    pub kind: GridKind,

    /// World-space position of the grid's (0,0) corner (not center). Default: origin.
    #[serde(default)]
    pub origin: (f32, f32, f32),

    /// Edge length / cell width in metres. Default: 1.0.
    #[serde(default = "default_cell_size")]
    pub cell_size: f32,

    /// Grid extent as (cols, rows). Required.
    pub dimensions: (u32, u32),

    /// Square grids only: include diagonal neighbours. Default: Cardinal.
    #[serde(default)]
    pub neighbors: GridNeighborMode,

    /// Hex grids only: flat or pointy orientation. Default: PointyTop.
    #[serde(default)]
    pub hex_orientation: HexOrientation,

    /// Maximum A* nodes visited per pathfinding call. Default: 2048.
    #[serde(default = "default_max_path_nodes")]
    pub max_path_nodes: u32,

    /// Render grid lines via Gizmos (debug use). Default: false.
    #[serde(default)]
    pub show_overlay: bool,

    /// Overlay line color as linear RGBA. Default: (0.5, 0.5, 0.5, 0.3).
    #[serde(default = "default_overlay_color")]
    pub overlay_color: (f32, f32, f32, f32),
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
pub enum GridKind {
    Square,
    Hex,
    Triangle,
}

#[derive(Deserialize, Debug, Clone, Default)]
pub enum GridNeighborMode {
    #[default]
    Cardinal,  // 4-way (N/S/E/W for square; always 6 for hex; always 3 for tri)
    Diagonal,  // 8-way for square only
}

#[derive(Deserialize, Debug, Clone, Default)]
pub enum HexOrientation {
    #[default]
    PointyTop,
    FlatTop,
}
```

### `SceneEntityDef` — grid placement

```ron
// scenes/strategy_map.scene.ron
entities: [
    ( id: "unit_warrior",  prefab: "warrior",
      transform: ( translation: (0.0, 0.0, 0.0) ),
      grid_cell: Some((3, 5)) ),   // NEW — overrides transform XZ, snaps to cell center
]
```

```rust
// schema/scene_v2.rs — in SceneEntityDef
/// Optional grid placement. When set, overrides the entity's XZ transform
/// to the cell center at load time. Y is taken from the grid origin.
#[serde(default)]
pub grid_cell: Option<(i32, i32)>,
```

---

## Coordinate systems

### Square `(col, row)`

Cell center:
```
world.x = origin.x + (col + 0.5) * cell_size
world.z = origin.z + (row + 0.5) * cell_size
```

Neighbors (Cardinal): `(±1, 0)`, `(0, ±1)`. Diagonal adds `(±1, ±1)`.

Distance (Cardinal): `|Δcol| + |Δrow|` (Manhattan).

---

### Hex `(q, r)` — axial coordinates

Standard axial system following the [redblobgames hex grid reference](https://www.redblobgames.com/grids/hexagons/). `s = -q - r` is always derivable.

**Pointy-top** cell center:
```
world.x = origin.x + cell_size * (√3 * q  +  √3/2 * r)
world.z = origin.z + cell_size * (3/2 * r)
```

**Flat-top** cell center:
```
world.x = origin.x + cell_size * (3/2 * q)
world.z = origin.z + cell_size * (√3/2 * q  +  √3 * r)
```

6 neighbors: the 6 axial direction vectors `(+1,0), (-1,0), (0,+1), (0,-1), (+1,-1), (-1,+1)`.

Distance: `(|q| + |r| + |q+r|) / 2` (cube distance formula).

---

### Triangle `(col, row)` — parity from sum

Row `row` contains `cols` alternating up/down triangles. Parity: `(col + row) % 2 == 0` → upward face.

**Upward triangle** center:
```
world.x = origin.x + (col + 0.5) * (cell_size / 2)
world.z = origin.z + (row + 1.0/3.0) * (cell_size * √3/2)
```

**Downward triangle** center (same col, same row, opposite parity):
```
world.x = origin.x + (col + 0.5) * (cell_size / 2)
world.z = origin.z + (row + 2.0/3.0) * (cell_size * √3/2)
```

3 neighbors per triangle: left sibling, right sibling, and the opposing triangle (the downward-pointing one below an upward triangle, or vice versa).

Distance: BFS hop count (no closed-form formula; precompute via BFS if needed).

---

## Runtime

### Resources (`capabilities/grid.rs`)

```rust
/// Loaded from scene RON. None when scene has no grid.
#[derive(Resource, Default)]
pub struct GridConfig(pub Option<GridDef>);

/// Passability and per-cell metadata. Keyed by (col, row).
#[derive(Resource, Default)]
pub struct GridCellData(pub HashMap<(i32, i32), CellFlags>);

#[derive(Default, Clone, Copy)]
pub struct CellFlags {
    pub passable: bool,  // default true for all cells in bounds
}

/// Which entity (SpawnId) currently occupies each cell.
#[derive(Resource, Default)]
pub struct GridOccupancy(pub HashMap<(i32, i32), String>);  // cell → spawn_id

/// Active grid movement state per entity.
#[derive(Resource, Default)]
pub struct ActiveGridMoves(pub HashMap<String, GridMoveState>);

pub struct GridMoveState {
    pub path: Vec<(i32, i32)>,    // remaining waypoints
    pub step_timer: f32,           // seconds until next step
    pub step_duration: f32,        // 1.0 / speed
}
```

### `GridPosition` component

```rust
/// Attached to entities placed on the grid. Tracks their current cell.
#[derive(Component, Debug, Clone)]
pub struct GridPosition(pub i32, pub i32);
```

---

## New actions (`schema/actions.rs`)

```ron
// Snap entity to cell center (instant; no path)
PlaceOnGrid(entity: "unit_01", cell: (3, 5))

// Find path and move entity step by step (speed = cells/second)
StartGridMove(entity: "unit_01", target: (7, 8), speed: 2.0)

// Stop movement mid-path (stays at current cell)
StopGridMove("unit_01")

// Mark a cell as passable or blocked
SetCellPassable(cell: (4, 6), passable: false)

// Query path without moving (result in GridPathResult resource; emits grid.path_found or grid.path_blocked)
FindPath(entity: "unit_01", target: (7, 8))
```

```rust
PlaceOnGrid { entity: String, cell: (i32, i32) },
StartGridMove { entity: String, target: (i32, i32), #[serde(default="default_grid_speed")] speed: f32 },
StopGridMove(String),
SetCellPassable { cell: (i32, i32), passable: bool },
FindPath { entity: String, target: (i32, i32) },
```

---

## New pipeline events

```ron
grid.cell_entered:{entity}:{col}:{row}    // entity moved into a cell
grid.cell_exited:{entity}:{col}:{row}     // entity left a cell (before entering next)
grid.move_complete:{entity}              // entity reached its target cell
grid.move_blocked:{entity}              // path was broken mid-move (cell became impassable)
grid.path_found:{entity}:{steps}        // FindPath succeeded; steps = path length
grid.path_blocked:{entity}             // FindPath found no valid path
grid.cell_passable_changed:{col}:{row}:{passable}   // SetCellPassable fired
```

---

## Systems (`capabilities/grid.rs`)

**`grid_move_tick_system`** — runs in `Update`. For each `ActiveGridMoves` entry:
1. Decrement `step_timer` by `delta_secs`. When ≤ 0:
2. Pop next waypoint from `path`. Emit `grid.cell_exited`. Update `GridOccupancy`. Set entity `Transform.translation` to new cell center. Update `GridPosition`. Emit `grid.cell_entered`.
3. If path empty: emit `grid.move_complete`, remove from `ActiveGridMoves`.

Uses change-detection guard on `Transform` — only writes if position actually changes.

**`grid_overlay_system`** — runs in `Update` when `GridConfig.show_overlay == true`. Draws grid lines via `Gizmos`. Runs only in debug/native builds (gated by `#[cfg(debug_assertions)]` or a `show_overlay` flag — works on WASM too but is purely visual).

**`PlaceOnGrid` executor arm** — resolves world pos from cell, sets `Transform.translation`, inserts `GridPosition`, updates `GridOccupancy`.

**`StartGridMove` executor arm** — runs A* (see below), populates `ActiveGridMoves` entry.

---

## Pathfinding (A*)

```rust
pub fn astar(
    start: (i32, i32),
    goal: (i32, i32),
    cell_data: &GridCellData,
    occupancy: &GridOccupancy,
    config: &GridDef,
    max_nodes: u32,
) -> Option<Vec<(i32, i32)>> {
    // Standard A* with heuristic matched to grid type:
    //   Square Cardinal: Manhattan distance
    //   Square Diagonal: Chebyshev distance
    //   Hex: cube distance
    //   Triangle: BFS hop count (h = 0 for Dijkstra fallback)
    // Stops and returns None if node count exceeds max_nodes.
    // Treats out-of-bounds and non-passable cells as walls.
    // Goal cell does not need to be passable (allows moving to occupied destination).
}
```

The pathfinder returns `None` when no path exists or `max_path_nodes` is exceeded (logs a warning in the latter case — designers should increase `max_path_nodes` or reduce grid size).

---

## Worked examples

### RTS unit control (square grid)
```ron
// scene RON
grid: Some(( kind: Square, cell_size: 2.0, dimensions: (32, 32), neighbors: Cardinal ))

// rules.ron — move selected unit on right-click (needs targeting system for {target_cell})
( on: "input.grid_click:{col}:{row}", do_actions: [
    StartGridMove(entity: "{selected_unit}", target: ({col}, {row}), speed: 3.0),
] ),
( on: "grid.move_complete:{selected_unit}", do_actions: [
    PlaySound(key: "unit_arrived"),
] ),
```

### Hex strategy (Civilization-style)
```ron
grid: Some((
    kind: Hex,
    hex_orientation: FlatTop,
    cell_size: 3.0,
    dimensions: (15, 10),
))

// Terrain blocking
( on: "scene.ready:world_map", do_actions: [
    SetCellPassable(cell: (3, 4), passable: false),  // mountain hex
    SetCellPassable(cell: (7, 2), passable: false),  // ocean hex
] ),
```

### Puzzle game (Sokoban-style, square)
```ron
grid: Some(( kind: Square, cell_size: 1.0, dimensions: (8, 8), neighbors: Cardinal ))
entities: [
    ( id: "crate_a", prefab: "wooden_crate", transform: (...), grid_cell: Some((2, 3)) ),
    ( id: "goal_a",  prefab: "goal_marker",  transform: (...), grid_cell: Some((5, 6)) ),
]

// When crate reaches goal
( on: "grid.cell_entered:crate_a:5:6", do_actions: [
    EmitEvent("puzzle.crate_on_goal"),
    PlaySound(key: "crate_placed"),
] ),
```

### Board game (chess/Catan, hex)
```ron
grid: Some(( kind: Hex, hex_orientation: PointyTop, cell_size: 2.0, dimensions: (7, 7) ))

// Catan-style resource placement — designer places settlement props on vertices
// (vertices are intermediate points; for v1, settlements snap to nearest cell center)
```

---

## New Rust changes

- `schema/scene_v2.rs` — `GridDef`, `GridKind`, `GridNeighborMode`, `HexOrientation`; `grid: Option<GridDef>` on `GameSceneV2`; `grid_cell: Option<(i32, i32)>` on `SceneEntityDef`.
- `schema/actions.rs` — `PlaceOnGrid`, `StartGridMove`, `StopGridMove`, `SetCellPassable`, `FindPath`.
- `capabilities/grid.rs` (new file) — `GridConfig`, `GridCellData`, `GridOccupancy`, `ActiveGridMoves`, `GridPosition`, `grid_move_tick_system`, `grid_overlay_system`, coordinate conversion functions, `astar`.
- `capabilities/mod.rs` — register module + systems.
- `runtime/scene_manager/action_executor.rs` — all new grid action arms.
- `runtime/scene_manager/scene_loader.rs` — populate `GridConfig` from scene RON; apply `grid_cell` snapping at entity spawn; clear grid resources on `LoadScene`.

---

## Tasks

- [ ] Decisions from pre-implementation checklist resolved
- [ ] `GridDef` + all enum types in `schema/scene_v2.rs`
- [ ] `grid_cell: Option<(i32, i32)>` on `SceneEntityDef`
- [ ] `GridConfig`, `GridCellData`, `GridOccupancy`, `ActiveGridMoves`, `GridPosition`
- [ ] Coordinate conversion: `cell_to_world` and `world_to_cell` for all three types
- [ ] `neighbors(cell, config)` function — correct per grid type
- [ ] `distance(a, b, config)` function — correct per grid type
- [ ] `astar` pathfinder with `max_path_nodes` cap
- [ ] `PlaceOnGrid`, `StartGridMove`, `StopGridMove`, `SetCellPassable`, `FindPath` actions + executor arms
- [ ] `grid_move_tick_system` — step through path at speed, emit events
- [ ] `grid_overlay_system` — Gizmos grid line rendering for all three types
- [ ] Scene loader: populate `GridConfig`; apply `grid_cell` snapping at spawn
- [ ] All grid resources cleared on `LoadScene`
- [ ] New demo project **`grid_demo`** with three stations: square unit move, hex terrain map, triangle puzzle
- [ ] Integration tests: `cell_to_world` round-trips via `world_to_cell`; A* finds shortest path; impassable cell is avoided; out-of-bounds cell returns None from neighbors; `grid.cell_entered` fires on each step
- [ ] Docs: `GridDef` fields, coordinate system diagrams, action reference in `docs/20_data_formats.md` + `docs/30_runtime_events_and_logic.md`

---

## Open questions

- **Vertex and edge addressing**: Catan-style games need to place settlements on hex vertices and roads on edges, not just cell centers. v1 has no vertex/edge API — designers approximate by placing entities at the nearest cell centers or using offset world positions. Full vertex/edge support deferred to v2.
- **Multi-layer grids**: some games use stacked grids (underground tunnels under a surface map). Not in v1 — one grid per scene.
- **Grid serialization for save/load**: `GridCellData` (passability overrides) and `GridOccupancy` should be included in the save state. Deferred to v2 save system extension — note it in the save/load feature file.
- **Large grid pathfinding**: `max_path_nodes: 2048` is a hard cap. A 64×64 map has 4096 cells — paths across the full diagonal may be cut off. Designers can raise the cap but must accept a WASM main-thread stall. Document the tradeoff clearly. Async pathfinding via `AsyncComputeTaskPool` is the v2 fix (noting it blocks on WASM anyway until Bevy resolves WASM threading).
- **Grid input**: clicking a cell to select it (as in the RTS example) requires mapping a mouse world-ray hit to a grid cell. `world_to_cell(hit_pos)` does this, but the click event needs to include the world position. This wires naturally with the targeting system (`On<Pointer<Click>>` on a ground-plane entity) — designers emit `input.grid_click:{col}:{row}` from a rule. Not part of this feature — defer to integration with targeting system docs.

---

## Acceptance criteria

- Given `kind: Square, dimensions: (10, 10), cell_size: 2.0`, `cell_to_world((0, 0))` returns the center of the first cell (1.0, origin.y, 1.0).
- Given `PlaceOnGrid(entity: "unit", cell: (3, 5))`, the entity's `Transform.translation` snaps to that cell's world center and `GridPosition(3, 5)` is inserted.
- Given `StartGridMove(entity: "unit", target: (7, 8), speed: 2.0)`, the entity moves cell by cell and `grid.cell_entered` fires at each step; `grid.move_complete` fires on arrival.
- Given a cell marked `SetCellPassable(cell: (5, 5), passable: false)`, A* routes around it.
- Given `FindPath` to an unreachable target, `grid.path_blocked` is emitted.
- Given `kind: Hex` with `hex_orientation: PointyTop`, each cell has exactly 6 neighbors.
- Given `kind: Triangle`, each cell has exactly 3 neighbors.
- Given `show_overlay: true`, grid lines are rendered via Gizmos each frame.
- Given a scene transition, all grid resources (`GridConfig`, `GridCellData`, `GridOccupancy`, `ActiveGridMoves`) are cleared and repopulated from the new scene.
