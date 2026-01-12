use wasm_bindgen::prelude::*;
use ironhold_core::start_app;

#[wasm_bindgen(start)]
pub fn start() {
    start_app(None);
}
