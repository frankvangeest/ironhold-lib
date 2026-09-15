Build a development WASM bundle and report binary size.

Run:
```
wasm-pack build crates/ironhold_web --target web --out-dir ../../pkg --dev --features webgpu --features inspector
```

`inspector` is a standing default for every dev build, not a judgment call based on what's being tested — it makes F9 (physics collider wireframes) and `` ` `` (full egui world inspector) available in-browser without having to predict ahead of time whether a given playtest will need them. Never add it to a release build — production must never ship inspector tooling.

After the build completes, check the size of `pkg/ironhold_web_bg.wasm` and report it, but treat it as a rough sanity check only, not a proxy for the release build's size — an unoptimized `--dev` build, further inflated here by `inspector`'s `bevy_egui`/`bevy_inspector_egui` dependencies (which never ship), is routinely far larger than the optimized release build. The GitHub Pages 95/100 MB thresholds apply to the actual release build (`cargo clean && wasm-pack build --features webgpu`, no `--dev`, no `inspector`) — check size there, not here, before concluding anything about deployability.

**Important:** Always remind Frank that this is a dev build and must NOT be committed. The `pkg/` directory should never be committed after a `--dev` build. A release build (`cargo clean && wasm-pack build --features webgpu` without `--dev`) is required before committing.
