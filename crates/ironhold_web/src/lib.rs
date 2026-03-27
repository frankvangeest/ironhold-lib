use wasm_bindgen::prelude::*;
use ironhold_core::start_app;

#[wasm_bindgen(start)]
pub fn start() {
    start_app(read_url_project_param());
}

/// Reads `?project=<name>` from the page URL and converts it to a project-config path.
/// Returns `None` if the param is absent (the engine will load its default project).
fn read_url_project_param() -> Option<String> {
    let window = web_sys::window()?;
    let search = window.location().search().ok()?;
    if search.is_empty() || search == "?" {
        return None;
    }
    let params = web_sys::UrlSearchParams::new_with_str(&search).ok()?;
    let name = params.get("project")?;
    Some(format!("projects/{}/{}.project.ron", name, name))
}
