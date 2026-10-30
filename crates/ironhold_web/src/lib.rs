mod nakama;

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use ironhold_core::{start_app_with, AppOptions, RunMode};

#[wasm_bindgen(start)]
pub fn start() {
    let options = read_url_params();
    install_pointer_lock_reporting();
    start_app_with(options);
}

/// Tells `ironhold_core` the real pointer-lock state. winit's web backend requests the lock
/// fire-and-forget and never listens for `pointerlockchange`, so the engine cannot otherwise know
/// whether the browser granted the lock or the user released it with Escape (see
/// `ironhold_core::capabilities::cursor_capture` and `planning/features/done/first_person_starter.md`,
/// "Spike A result"). Core stays free of the DOM: the runner only pushes the boolean.
fn install_pointer_lock_reporting() {
    let Some(document) = web_sys::window().and_then(|w| w.document()) else {
        return;
    };
    ironhold_core::capabilities::runner_reports_pointer_lock();

    let doc = document.clone();
    let on_change = Closure::<dyn FnMut()>::new(move || {
        ironhold_core::capabilities::report_pointer_lock(doc.pointer_lock_element().is_some());
    });
    let _ = document
        .add_event_listener_with_callback("pointerlockchange", on_change.as_ref().unchecked_ref());
    on_change.forget(); // lives for the whole page, like the app itself

    // A refused request (no user activation, Chrome's re-lock delay) fires `pointerlockerror`
    // and no change event: report "not locked" so the engine retries on the next click.
    let on_error = Closure::<dyn FnMut()>::new(|| {
        ironhold_core::capabilities::report_pointer_lock(false);
    });
    let _ = document
        .add_event_listener_with_callback("pointerlockerror", on_error.as_ref().unchecked_ref());
    on_error.forget();
}

/// Reads `?project=<name>`, `?scene=<path>`, `?nakama_key=<server key>` (the online backend adapter; only used when `backend.ron` has no `server_key`, and without either a real endpoint runs offline), `?backend=fake://...` (replaces `backend.ron`'s endpoint with a test backend)
/// and `?static=1` from the page URL.
/// - `project` is converted to a project-config asset path.
/// - `scene`   is passed through as-is and used as an `InitialSceneOverride`.
/// - `static=1` holds the simulation from the first frame (browser-test harness) and is honoured **only together with
///   `testing=1`**, so a shared link can never leave a player in a frozen-looking game.
fn read_url_params() -> AppOptions {
    let mut options = AppOptions::default();
    let Some(window) = web_sys::window() else { return options };
    let Ok(search) = window.location().search() else { return options };
    // Always a factory (even with no address parameters) so a real endpoint without a key says so on screen.
    let params = if search.is_empty() || search == "?" { None } else { web_sys::UrlSearchParams::new_with_str(&search).ok() };
    options.backend_factory = nakama::backend_factory(params.as_ref());
    let Some(params) = params else { return options };
    options.project_path = params
        .get("project")
        .map(|name| format!("projects/{}/{}.project.ron", name, name));
    options.scene_override = params.get("scene");
    // A page link must never choose which server receives a player's device id (it is their account credential), so the page
    // can only pick the test backend; a real endpoint comes from the project's own backend.ron.
    options.backend_endpoint = params.get("backend").filter(|e| !e.trim().is_empty()).filter(|e| {
        let ok = e.starts_with("fake://");
        if !ok {
            web_sys::console::warn_1(&"ignoring ?backend=: the page address can only select a fake:// backend".into());
        }
        ok
    });
    if params.get("static").as_deref() == Some("1") {
        if params.get("testing").as_deref() == Some("1") {
            options.run_mode = RunMode::Static;
        } else {
            web_sys::console::warn_1(&"ignoring ?static=1: it is only honoured together with ?testing=1".into());
        }
    }
    options
}
