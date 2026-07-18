---
name: world-textured-stat-bar
description: WorldStatBarStyle::Textured — 9-sliced SpriteImageMode::Sliced fill bar; shares SpritePipeline with Icon/plain sprites (no distinct pipeline variant); update system zero per-frame alloc
metadata:
  type: project
---

`WorldStatBarStyle::Textured` (catalog.rs enum arm) renders a continuous fill health bar as TWO 9-sliced `Sprite` layers per rank — a static empty/track (z=2, tinted by `bg_color`, never updated) and a fill (z=3, `custom_size.x` + `color` driven per frame). Both crop ONE shared `Handle<Image>` via static `Sprite.rect` and use `SpriteImageMode::Sliced(TextureSlicer)`. Impl in `capabilities/stat_display.rs`: spawn arm in `spawn_world_stat_bar_widget` (~line 722), `WorldTexturedBarFillMarker` component, `world_textured_bar_update_system` (registered Update in lib.rs alongside the other bar update systems). In 3rd_person_game_demo this REPLACED the Icon hearts bar on player_male/player_female prefabs (prefabs.ron); Pixel bars remain on NPCs.

**Sprite-pipeline warmup — confirmed NOT worsened, and sliced shares the pipeline:**
- `SpriteImageMode::Sliced` does NOT compile a distinct render pipeline variant. In bevy_sprite 9-slicing is CPU-side geometry expansion (`ComputedTextureSlices` → multiple `ExtractedSprite` quads); all sprites (plain, atlas, sliced) go through the same `SpritePipeline`, keyed on HDR/MSAA/texture-format/tonemapping — not on slicing or atlas. So any prior sprite warmup covers sliced sprites. The Icon→Textured swap keeps the sprite-pipeline situation identical (same pipeline, first compiled on first sprite draw).
- `pipeline_warmup_system` (lib.rs) remains `With<Mesh3d>`-ONLY — still no sprite NoFrustumCulling warmup (pre-existing gap from Icon, see [[world-icon-stat-bar]]). Textured does NOT make it materially worse: spawns 2 sprites/rank (empty+fill) vs Icon's `cells` (default 5), so FEWER sprites per bar. Player prefabs are visible when spawned (character-select Action::Spawn, once — not per-frame/wave), so the sprite pipeline compile folds into that spawn frame; low first-draw-stall risk.

**Per-frame cost profile (world_textured_bar_update_system) — clean:**
- Iterates ALL `WorldTexturedBarFillMarker` entities every frame regardless of Visibility (4 ranks/bar in split-screen). No change-detection filter, but body is cheap.
- ZERO per-frame heap allocation: no format!/String/Vec. `resolve_stat` per fill entity; for the shipped `player_health` (non-dotted global key) it's an O(1) LoadedStats HashMap lookup, NOT the O(entities) linear StatMap scan (that path only triggers for dotted `{self}.x` keys).
- `color_bands` selection is filter+max_by over a tiny borrowed Vec (≤3 entries) — no alloc. Width write guarded by 0.5px epsilon; color write guarded by `!=`. Correct.

**Binary size:** zero new deps/features — bevy_sprite already compiled. New asset healthbar_sheet.png served over HTTP, not embedded in .wasm.

See [[world-icon-stat-bar]] (sibling sprite path it replaced) and [[project_stat_widget_split_duplication]].
