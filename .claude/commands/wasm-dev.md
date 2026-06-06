Build a development WASM bundle and report binary size.

Run:
```
wasm-pack build crates/ironhold_web --target web --out-dir ../../pkg --dev
```

After the build completes, check the size of `pkg/ironhold_web_bg.wasm` and report it.

- If size < 95 MB: confirm it is within the safe range and state the exact size.
- If size is 95–100 MB: warn Frank clearly — this is approaching the GitHub Pages hard limit of 100 MB.
- If size ≥ 100 MB: alert Frank that this exceeds the GitHub Pages limit and the build cannot be deployed.

**Important:** Always remind Frank that this is a dev build and must NOT be committed. The `pkg/` directory should never be committed after a `--dev` build. A release build (`cargo clean && wasm-pack build ... ` without `--dev`) is required before committing.
