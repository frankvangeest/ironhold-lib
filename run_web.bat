set RUSTFLAGS=--cfg=web_sys_unstable_apis
wasm-pack build crates/ironhold_web --target web --out-dir ../../pkg
python serve.py