use bevy::prelude::*;
use ironhold_core::*; // your public re-exports

pub fn test_app_with_core(minimal_project: Option<schema::ProjectConfig>) -> App {
    let mut app = App::new();

    // Prefer the same plugin set your native runner uses,
    // or a known-good headless subset you document for tests.
    app.add_plugins(DefaultPlugins);

    app.add_plugins(GamePlugin); // your top-level core plugin

    if let Some(project) = minimal_project {
        app.insert_resource(project);
    } else {
        // or call a "load_project_from_assets(...)" helper if you have one
    }
    app
}

// To use, at the top of your test:
// use ironhold_core::tests::support;
// let mut app = support::test_app_with_core(Some(minimal_project()));