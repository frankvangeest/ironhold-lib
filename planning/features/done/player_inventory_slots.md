# Feature: Player bag size from data (item system S2b)

_Status: Done (2026-11-15)_
_Plan-reviewed 2026-11-15: system-architect and ux-gamedesigner-reviewer; both returned Ready once their corrections were in (all folded in above)._
_Planned at: `29cce26` (2026-11-15)_

Slice S2b of `planning/features/item_system_review.md` (Frank's decision 6 there: project-level, default 20). Split off from S2a
(`planning/features/item_transaction_primitive.md`, done). S1, S2a and S3 are done. S4 (sell, destroy, menu) and S5/S6 (action bar item
slots) are NOT part of this slice.

## What
The player's bag has 20 slots today, fixed in the engine. A project can show any grid it likes in its `InventoryPanel` (`columns * rows`),
so a 4x3 panel over a 20-slot bag silently hides 8 slots of items, and a 6x5 panel shows 10 slots that can never hold anything. This slice
lets the project decide: `player_inventory: (max_slots: 12)` in the `.project.ron`, a validate check that the panel and the bag agree, and
slots the bag does not have drawn dimmed instead of looking usable.

## Why
The bag size is a core game-design knob (a survival game wants 8, an RPG 40). It is the only inventory number a designer cannot reach from
RON, and a mismatch today fails silently (hidden items, dead slots). It also unblocks later "bigger bag" features.

## Approach
1. **Schema** (`schema/project.rs`): `ProjectConfig.player_inventory: PlayerInventoryDef`, `#[serde(default)]`, struct form so starting items
   can join it later: `PlayerInventoryDef { max_slots: usize }` with `deny_unknown_fields` and `#[serde(default = "default_player_slots")]`
   on the field (so `player_inventory: ()` parses). **`Default` for both `PlayerInventoryDef` and `PlayerInventory` is hand-written**
   (`DEFAULT_PLAYER_SLOTS` = 20), never derived: about 45 test literals use `ProjectConfig { ..Default::default() }`, and a derived default
   would give them a 0-slot bag. `DEFAULT_PLAYER_SLOTS` and `MAX_PLAYER_SLOTS` (256) live in `schema/` and are re-exported from
   `capabilities/inventory.rs`. Additive field, no `schema_version` bump. `ProjectConfig::validate()` rejects 0 and above 256.
2. **Runtime sizing.** The runtime only *logs* a failed `validate()` (`project_loader.rs:64-66`), so the loader clamps: insert
   `PlayerInventory::new(max_slots.clamp(1, MAX_PLAYER_SLOTS))` once, in the shared tail of the loader (`project_loader.rs:358-363`, where
   `max_frame_delta_secs` is applied; it runs on both the with-files and the no-files path), with a `warn!` naming the field when it clamps
   (`player_inventory.max_slots 0 is out of range (1-256); using 1`). A web-only designer sees a working game and a console line, not a stuck
   loading screen. Exactly one `ProjectConfig` loads per app lifetime, so this sizes the bag once at boot; the bag persists across
   `LoadScene` as today. The three lazy `if max_slots == 0 { resize(20) }` sites (`action_executor.rs:1238`, `:1489`,
   `item_actions.rs:33-38`) are removed. `PlayerInventory::resize` stays `pub` (a test uses it; a future `SetInventorySize` would).
3. **Dead slots.** An `InventoryPanel` slot with `slot_index >= max_slots` is drawn **faded** (slot background at about 35% of its normal
   opacity, no icon, no count label) and ignores clicks. Dim, not hide: hiding would leave holes in the authored layout, and faded slots
   read as "locked", which a later bag-upgrade action needs. Implementation: `inventory_ui_system` gets a
   `Query<(&InventorySlotMarker, &mut BackgroundColor)>` and writes the colour both ways, guarded (about 20 comparisons a frame, no WASM
   concern); one shared constant pair for the normal and the faded colour (normal is `srgba(0.12, 0.12, 0.16, 0.85)` in `scene_loader.rs`).
   Click ignoring already holds (`slots.get(idx)` is `None` past `len`); it gets an explicit `idx >= max_slots` guard and a test.
4. **One pure check, two callers** (CLI/runtime mirror pattern): `inventory_panel_bag_issue(panel_slots, bag_slots) -> Option<..>` in
   `schema/` (next to `ui_layout_diagnostics`). The CLI walks every scene's panels with `walk_ui_nodes` (nested ones too); the scene loader
   calls it in its InventoryPanel arm and logs a `warn!` (a designer without the CLI otherwise never learns items are hidden):
   - panel smaller than the bag -> CLI **error** `inventory_panel_smaller_than_bag` + runtime `warn!`;
   - panel larger than the bag -> CLI **strict warning** `inventory_panel_larger_than_bag` + no runtime warning (the faded slots show it);
   - no `InventoryPanel` anywhere, with `player_inventory` set -> no message (a bag can be driven by actions alone). The check does not look at
     `items_path`: a panel smaller than the bag is a latent bug even in a project without items. Skipped when the `.project.ron` fails to parse.
   The error text names the scene file, the panel `id`, `columns x rows = N`, the bag size and whether it is the default ("player_inventory
   not set, default 20"), and both fixes ("set `player_inventory: (max_slots: N)` in your .project.ron, or resize the panel"). A unit test
   pins the default panel (`default_inv_columns() * default_inv_rows()`) equal to `DEFAULT_PLAYER_SLOTS`.
5. **Runtime resize is out of scope.** No action changes the bag size, so "shrinking below occupied slots refuses" has nothing to guard
   here; a `SetInventorySize` backlog item owns it (and the save-file question of a save holding more stacks than `max_slots`).
6. **Co-op:** one resource, one panel: the bag is shared by every local player (documented; per-player bags would be a future
   prefab-level field). **Not to be confused with** `PrefabDef.inventory: (max_slots: 9)`, which is a container's own inventory: the docs
   row says so.

### RON authoring example
```ron
(
    schema_version: 3,
    initial_scene: "scenes/main.scene.ron",
    items_path: "items/items.ron",
    player_inventory: ( max_slots: 12 ),   // optional, default 20; match columns * rows of your InventoryPanel
)
```

## Tasks
- [x] (added at the playtest, Frank) footer label `Slots: used/total` left of the gold label, orange when full
- [ ] `PlayerInventoryDef` + `ProjectConfig.player_inventory` + `validate()` range check; hand-written Defaults; constants in `schema/`
- [ ] Loader tail inserts the clamped, sized bag with a `warn!`; remove the three lazy resize sites
- [ ] Dead-slot fading in `inventory_ui_system` (shared colour constants); explicit click guard
- [ ] `inventory_panel_bag_issue` in `schema/`; CLI error + strict warning; scene-load `warn!`
- [ ] Demo: `3rd_person_game_demo.project.ron` gets `player_inventory: ( max_slots: 20 )` with a comment; rename its decorative panel id
      `player_inventory` to `bag_panel` (also in the docs example); optional commented line in `blank_project`
- [ ] Tests: core (config parse incl. `()`, default 20 from a `..Default::default()` literal, clamp 0 and 300, small bag refuses a fifth item,
      dead-slot click ignored, faded colour both ways); CLI (both directions, nested panel, default-bag message wording, range error)
- [ ] Docs: `docs/20_data_formats.md` (ProjectConfig row incl. "shared by all players, not the prefab `inventory:`", InventoryPanel note, a
      "The player's bag" paragraph: default 20, full-bag behaviour, persists across scenes, cannot change at runtime yet, faded slots),
      `docs/15_authoring_tools.md` (error row + `--strict:` row), `runtime/scene_manager/CLAUDE.md` one line (bag sized once in the loader
      tail, never lazily)
- [ ] Rebuild the cached `tools/bin/ironhold` after the schema change (the old binary rejects the demo's new field)

## Open questions (for Frank)
Both reviewers recommend the plan's defaults: (1) a panel larger than the bag is a `--strict` warning, not an error; (2) the cap stays 256.

## Acceptance criteria
- Given no `player_inventory` field, then the bag has 20 slots and nothing changes for existing projects.
- Given `player_inventory: (max_slots: 4)`, when the player picks up or buys a fifth distinct item, then it is refused with `inventory.full:player`
  / `shop.no_room:{item}` as for any full bag.
- Given `max_slots: 0` or 300, then `ironhold validate` exits 1 naming the field, and at runtime the game still loads, logs a warning naming
  the field and uses 1 or 256.
- Given a 5x4 panel and `max_slots: 12`, then slots 13-20 are drawn faded (no icon or count) and do not react to clicks.
- Given a panel of 12 slots and `max_slots: 20`, then the console shows one warning naming the panel and the hidden slots.
- Given a panel of 12 slots and `max_slots: 20`, then `ironhold validate` exits 1 naming the panel, its slots and the bag size.
- Given a panel of 20 slots and `max_slots: 12`, then `validate` passes and `--strict` reports the warning.
