use wasm_bindgen::prelude::*;
use ironhold_core::start_app;

#[wasm_bindgen(start)]
pub fn start() {
    let (project, scene) = read_url_params();
    start_app(project, scene);
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
