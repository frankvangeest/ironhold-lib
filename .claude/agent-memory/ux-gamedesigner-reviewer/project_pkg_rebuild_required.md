---
name: pkg/ web build must be rebuilt for designers to use new features
description: The hosted WASM build in pkg/ is the designer's only runtime; new schema/action features ship to designers only after a fresh wasm-pack build and commit of pkg/
type: project
---

Designers do not have access to Rust source. They use the prebuilt WASM artifact in `pkg/` (or whatever is hosted at the public URL). Schema or action changes in `crates/ironhold_core/` reach them only after:

1. `wasm-pack build crates/ironhold_web --target web --out-dir ../../pkg`
2. The resulting `pkg/ironhold_web.js` and `pkg/ironhold_web_bg.wasm` are committed and pushed.

**Why:** if a new Action variant is documented in `docs/` and exemplified in `assets/projects/...`, but `pkg/` is stale, the designer will hit a deserialization error or silent no-op when they try to use it. The `.ron` files load against the WASM-compiled schema, not the Rust source.

**How to apply:** when reviewing staged feature changes, always check `git status pkg/`. If the wasm-bound code (schema, actions, executor, capabilities) has new variants but `pkg/` is unchanged, this is a blocker for the designer experience — even if everything else is perfect.
