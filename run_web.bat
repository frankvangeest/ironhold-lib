wasm-pack build crates/ironhold_web --target web --out-dir ../../pkg
python -m http.server 8000