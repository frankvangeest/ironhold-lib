---
name: world-icon-stat-bar
description: WorldStatBarStyle::Icon — engine's first bevy_sprite Sprite render path; sprite pipeline NOT covered by pipeline_warmup_system (Mesh3d-only)
metadata:
  type: project
---

`WorldStatBarStyle::Icon` (catalog.rs) renders a stat as N discrete `Sprite`+`TextureAtlas` cells parented under an invisible anchor, one anchor+children set per split-screen rank (≤ MAX_SPLIT_PLAYERS=4). Impl in `capabilities/stat_display.rs`: `WorldIconBar` component + `world_icon_bar_update_system` (registered Update in lib.rs).

**First-ever `bevy_sprite` Sprite/TextureAtlas render path.** Everything else uses Mesh2d/ColorMaterial (Pixel bars) or UI ImageNode (action bar/inventory). `TextureAtlasLayout`/`TextureAtlas` *data* were already used via ImageNode, but the `Sprite` *component* drives bevy_sprite's SpritePipeline — a genuinely new GPU pipeline.

**Why (perf profile, confirmed good):**
- Per-frame: `resolve_stat` called ONCE per anchor (not per cell — correct; dotted-key lookup is O(entities-with-StatMap)). Per-cell loop does only index-compare + conditional usize write. No per-frame Vec/String/format!. Worst case cells=20 × 4 ranks = 80 sprites, trivial.
- Asset sharing: `Handle<Image>` + `TextureAtlasLayout` built ONCE per bar instance outside the `for rank` loop, `.clone()`d (Arc bump) into every rank/cell. 4-way split 5-cell = 20 Sprite entities but 1 layout asset + 1 image load. Correct.
- Binary size: zero new deps/features — `bevy_sprite` already transitively compiled (bevy default features). New assets iconsheet-hearts-01.png/.json ≈ 1 KB, served over HTTP not embedded in .wasm.
- WebGL2: sprites are basic quad+texture, fully compatible (no compute/storage).

**How to apply:**
- **Sprite pipeline warmup GAP**: `pipeline_warmup_system` (lib.rs) queries `With<Mesh3d>` ONLY — it does NOT pre-warm the sprite pipeline. If an Icon bar's tracked entity starts off-screen (frustum-culled) at scene load and is revealed later by camera movement, expect a one-time WebGPU first-draw compile stall (~300–1000 ms). Mitigation: keep the tracked entity visible at load so the compile folds into startup. This is the item to watch in `python test_web.py`. Documented mesh-based fallback exists in the feature plan if sprite pipeline stalls on WASM.
- **Change-detection nit**: `sprite.texture_atlas.as_mut()` DerefMuts `Mut<Sprite>` every frame, flagging Sprite changed regardless of the inner `index != want` guard. Harmless (sprite extraction runs every frame anyway; ≤80 entities), doc comment overstates the guard. Use `map_unchanged`/read-first only if it ever matters.
- **Panic nit**: Icon arm `.expect()`s on `ctx.atlas_layouts/asset_server/asset_catalog` (all Option), unlike the UI atlas path which `if let Some`s gracefully. `Assets<TextureAtlasLayout>` is always present under DefaultPlugins so won't panic in practice, but on WASM a panic aborts.

See [[project_stat_widget_split_duplication]] (Pixel/Ascii split duplication) and [[project_pixel_world_stat_bar]] for the sibling render paths.
