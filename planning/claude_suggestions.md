# Claude Suggestions

> Raw development-time observations. Frank reviews and promotes good ones to `backlog.md`.
> Format: **title** _(observed at `<hash>` <YYYY-MM-DD>)_ — what + why, both one sentence.

---

## Performance

- **Audit all per-frame `Mut<T>` writes for unnecessary change-detection triggers** _(observed at `91cd464` 2026-04-29)_
  Found in `world_label_screen_pos_system`: writing `text_font.font_size` and `*vis` every frame — even to the same value — fired Bevy's change detection unconditionally, causing 35× text re-layout and glyph atlas uploads per frame. Any system that writes to a render-affecting component each frame should guard the write with a value comparison to avoid silent performance regressions.

- **Extend pipeline warmup to cover Text2d and UI pipelines** _(observed at `91cd464` 2026-04-29)_
  The current `pipeline_warmup_system` only queries `Mesh3d` entities; `Text2d` labels and UI buttons use a separate 2D render pipeline that is also compiled lazily on first use. If any of those pipelines compile during gameplay they would cause the same kind of WASM frame spike as the 3D pipelines did.

- **Consider discrete LOD steps for depth-scaled label font sizes instead of continuous scaling** _(observed at `91cd464` 2026-04-29)_
  Continuous `base_font_size * scale` produces a unique float every frame, making every camera move a glyph-cache miss even after the integer-rounding fix. Snapping to a small fixed set of sizes (e.g. 100 %, 75 %, 50 %, 25 %) would eliminate atlas misses entirely while still communicating depth, at the cost of a slight stepping artefact on zoom.

## Scene Loading

- **Staggered entity spawning to spread pipeline compilation across frames** _(observed at `91cd464` 2026-04-29)_
  Currently `spawn_scene_v2` spawns all entities in one frame; all unique pipelines compile synchronously that frame on WASM. Draining a `PendingEntitySpawns` queue at N entities/frame would turn one 1 400 ms spike into several ~200 ms frames, improving web INP even before the loading screen lands.
  _(Related backlog entry: Performance → Staggered entity spawning.)_
