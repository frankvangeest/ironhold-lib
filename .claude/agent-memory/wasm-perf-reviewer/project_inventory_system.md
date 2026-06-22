---
name: inventory-system
description: Inventory/item/shop system web-perf characteristics — inventory_ui_system, OpenShop despawn+respawn, slot entity count
metadata:
  type: project
---

Inventory & item system (added ~2026-06). Files: `capabilities/inventory.rs`, `schema/items.rs`, panel spawn in `scene_loader.rs` (~1836–1903), executor arms in `action_executor.rs` (~883–952), system registered in `lib.rs:258` (Update).

`inventory_ui_system` (Update): gated `if !player_inv.is_changed() { return; }` so idle frames are a single resource-change check (near-free). On change, iterates panel Children + `slot_q.get_mut(child)`, builds `format!`/`String` per slot, writes `text.0` ONLY when `text.0 != label`. Change-detection discipline is correct — no spurious render-layout retrigger. Allocation only on inventory mutation, not per-frame. Footgun: `is_changed()` fires for the whole PlayerInventory resource on ANY mutation (even an unrelated slot), so all N slots get re-formatted on any change — fine at default 20 slots, watch if max_slots grows large.

`OpenShop` (executor, event-driven): `despawn_children()` then re-spawns N text children per stock entry. NOT a held-key risk — triggered by `entity.interacted:{id}` which is `just_pressed` edge-triggered in `interactable.rs:34`. Max once per keypress. No per-frame stall.

Slot entities: InventoryPanel spawns 1 panel + columns*rows slot text nodes (default 5x4=20) at scene load (per-scene-load cost, not per-frame). 20 UI text nodes is negligible for UI layout/frame time.

Binary size: ItemCatalog/ItemDef/ItemStack use only serde + bevy Asset/TypePath + std HashMap — ZERO new deps. ImplicitRonPlugin::<ItemCatalog> is one more monomorphization of an existing generic plugin (trivial). [[project_wasm_size]] unaffected.

ItemStack holds `item_key: String` (heap) per stack — inventory clones strings on add/transfer, but only on item events, never per-frame.

Icon support (added ~2026-06-20): `inventory_ui_system` gained a 3rd query `icon_q: Query<(&InventorySlotIconMarker, &mut ImageNode, &mut Visibility)>`. Both text and icon loops sit AFTER the single `is_changed()` early-return, so idle frames remain one resource-change check (zero icon cost). Icon loop is equality-guarded on `ta.index` AND `*vis` before writing — change-detection-correct, no per-frame atlas re-upload or layout retrigger. `icon_index` resolved from `LoadedItemCatalog` (designer-reachable, not hardcoded). Texture path resolved via `asset_catalog.textures.get(key)` (catalog key, no fabricated path) — rule-compliant.

Slot icon spawn (scene_loader.rs ~1854-1925): ONE `TextureAtlasLayout` + ONE `asset_server.load(path)` Handle created before the row/col loop; both `.clone()`d per slot (Handle clone = Arc bump, cheap). Adds ~20 extra ImageNode entities at scene load (per-scene-load, not per-frame). All 20 ImageNodes share the SAME Image handle + SAME atlas layout => ONE GPU texture, ONE WebGPU UI-image pipeline compile (atlas index is per-draw, not a new pipeline). Spawned `Visibility::Hidden` so they don't draw until an item lands.

Atlas PNG default 11x11 @ 114px = 1254x1254. `asset_server.load::<Image>()` decodes async off main thread in WASM (Bevy async asset pipeline) — non-blocking, no startup stall. One-time fetch+decode of a single ~MB PNG; warm in cache after first load.

`InventorySlotIconMarker { slot_index: usize }` adds ZERO deps and one trivial component monomorphization — [[project_wasm_size]] unaffected.

Icon tint (added ~2026-06-21): `ItemDef.icon_color: Option<(f32,f32,f32,f32)>` resolved in the icon loop (inventory.rs ~182-185) → `Color::linear_rgba(..)` or `Color::WHITE` fallback. Write guarded by `if img_node.color != icon_color` — change-detection-correct, no per-frame material rebind. Pure scalar math (Color::linear_rgba is a const-ish struct build, no alloc, WASM-safe). Filled-slot visibility changed `Visible`→`Inherited` (also equality-guarded `*vis != Visibility::Inherited`) so the panel's Hidden state propagates — correct, no extra cost. All work is behind the `is_changed()` early-return → zero idle-frame cost; runs only on inventory mutation (pickup/drop), ≤32 slots. ZERO new deps. [[project_wasm_size]] unaffected.
