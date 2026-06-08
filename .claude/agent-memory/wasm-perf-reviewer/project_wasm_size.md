---
name: project-wasm-size
description: WASM size limits and the fact the targeting change added zero new dependencies
metadata:
  type: project
---

Release `pkg/ironhold_web_bg.wasm` ~90.7 MB as of 2026-06; warn at 95 MB, GitHub Pages hard-blocks at 100 MB.

**Why:** GitHub Pages serves the web build; very little headroom.

**How to apply:**
- DEV builds balloon (~190 MB observed for the targeting branch dev build) — ignore dev size entirely, only the `--release` (cargo clean + wasm-pack release) size counts toward the limit.
- The targeting capability + {target}/rewrite changes added NO new dependencies (root `Cargo.toml` and crate Cargo.toml files unchanged in `git diff HEAD`). targeting.rs uses only `bevy::prelude` + `bevy::window`, already pulled in by DefaultPlugins. Net binary-size impact: negligible (a couple small monomorphized systems).
- Note: bevy_picking is still linked via DefaultPlugins even though the targeting capability deliberately avoids mesh-raycast picking; the change did not remove that dep, just chose not to use raycast for selection.
