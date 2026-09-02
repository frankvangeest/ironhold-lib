---
name: project-wasm-size
description: WASM size limits and the fact the targeting change added zero new dependencies
metadata:
  type: project
---

Release `pkg/ironhold_web_bg.wasm` measured **30.6 MB on `integration`, 2026-09-01** — roughly half the previous figure (58.4 MB 2026-08-09; 58.1 MB 2026-07-31; ~58 MB 2026-07-06). The drop coincides with `0ff54d5 chore(pkg): rebuild release WASM with wasm-opt re-enabled` — i.e. **wasm-opt had been silently disabled for a long stretch** and re-enabling it halved the blob. Warn at 95 MB, GitHub Pages hard-blocks at 100 MB. The "~90.7 MB" figure quoted in some agent prompts is stale/wrong by 3x — always measure `ls -l pkg/ironhold_web_bg.wasm` rather than quoting it.

**Why:** GitHub Pages serves the web build; headroom is very large (~64 MB below warn). Binary size is effectively a non-issue for ordinary feature work; only a genuinely new heavy dependency or embedded asset could move the needle.

**Watch:** if a future measurement jumps back toward ~58 MB with no dep change, suspect wasm-opt being skipped in that build, not real code growth.

**How to apply:**
- DEV builds balloon (~190 MB observed for the targeting branch dev build) — ignore dev size entirely, only the `--release` (cargo clean + wasm-pack release) size counts toward the limit.
- The targeting capability + {target}/rewrite changes added NO new dependencies (root `Cargo.toml` and crate Cargo.toml files unchanged in `git diff HEAD`). targeting.rs uses only `bevy::prelude` + `bevy::window`, already pulled in by DefaultPlugins. Net binary-size impact: negligible (a couple small monomorphized systems).
- Note: bevy_picking is still linked via DefaultPlugins even though the targeting capability deliberately avoids mesh-raycast picking; the change did not remove that dep, just chose not to use raycast for selection.
