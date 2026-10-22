use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use ironhold_core::start_app;

#[wasm_bindgen(start)]
pub fn start() {
    let (project, scene) = read_url_params();
    install_pointer_lock_reporting();
    start_app(project, scene);
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

/// Reads `?project=<name>` and `?scene=<path>` from the page URL.
/// - `project` is converted to a project-config asset path.
/// - `scene`   is passed through as-is and used as an `InitialSceneOverride`.
fn read_url_params() -> (Option<String>, Option<String>) {
    let window = match web_sys::window() {
        Some(w) => w,
        None => return (None, None),
    };
    let search = match window.location().search() {
        Ok(s) => s,
        Err(_) => return (None, None),
    };
    if search.is_empty() || search == "?" {
        return (None, None);
    }
    let params = match web_sys::UrlSearchParams::new_with_str(&search) {
        Ok(p) => p,
        Err(_) => return (None, None),
    };
    let project = params
        .get("project")
        .map(|name| format!("projects/{}/{}.project.ron", name, name));
    let scene = params.get("scene");
    (project, scene)
}
