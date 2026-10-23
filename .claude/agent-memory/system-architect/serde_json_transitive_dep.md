---
name: serde-json-transitive-dep
description: serde_json is already in ironhold_core's normal (incl. wasm32) dep tree via bevy_gltf/gltf-json, so adding it directly costs ~no binary size
metadata:
  type: project
---

Verified 2026-10-23 (`cargo tree -p ironhold_core -e normal -i serde_json --target wasm32-unknown-unknown`):
serde_json 1.0.149 is pulled in by bevy_gltf, gltf, gltf-json — on native AND wasm32. ironhold_core only
lists it as a `[dev-dependencies]` (debug_state_json round-trip test); ironhold_cli uses `serde_json = "1"`
directly; there is no `[workspace.dependencies]` entry for it.

**Why:** the "hand-rolled vs serde_json" question for small JSON emitters (e.g. `#debug-state` in lib.rs)
keeps coming up; the WASM-size argument against serde_json is void here — only a monomorphized
Serialize impl would be added.

**How to apply:** don't reject serde_json in core on binary-size grounds; judge on code clarity. A
hand-rolled escaper is acceptable only if a test round-trips it through a real parser. See [[wasm-pitfalls]].
Also: web-sys `Element` feature implies `Node` (web-sys Cargo.toml `Element = ["EventTarget","Node"]`),
so `Node::set_text_content` needs no extra feature.
