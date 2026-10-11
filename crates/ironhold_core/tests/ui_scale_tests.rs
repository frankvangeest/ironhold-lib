//! `ProjectConfig.ui_scale` (responsive_ui Phase 2): the reference-resolution `UiScale` system and
//! the schema/validation that feed it. Headless: no `UiPlugin`, just the `UiScale` resource, a
//! primary `Window`, a `ProjectConfig` asset and `ui_scale_system` itself.

use bevy::prelude::*;
use bevy::ui::UiScale;
use bevy::window::PrimaryWindow;
use ironhold_core::capabilities::ui_scale::ui_scale_system;
use ironhold_core::schema::project::{ProjectConfig, ProjectConfigHandle, UiScaleConfig, UiScaleFit};

/// Counts the frames on which `UiScale` was marked changed (any write, even of an equal value):
/// the real cost of a write is a whole-UI relayout, so "no write when unchanged" is asserted here.
#[derive(Resource, Default)]
struct UiScaleWrites(u32);

fn count_ui_scale_writes(ui_scale: Res<UiScale>, mut writes: ResMut<UiScaleWrites>) {
    if ui_scale.is_changed() {
        writes.0 += 1;
    }
}

fn ron_config(ui_scale_block: &str) -> ProjectConfig {
    let text = format!(
        r#"#![enable(implicit_some)]
        (
            schema_version: 3,
            initial_scene: "scenes/main.scene.ron",
            {ui_scale_block}
        )"#
    );
    ron::de::from_str(&text).unwrap_or_else(|e| panic!("project RON must parse: {e}\n{text}"))
}

fn build_app(config: ProjectConfig, window: (u32, u32)) -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(bevy::asset::AssetPlugin::default())
        .init_asset::<ProjectConfig>()
        .init_resource::<UiScale>()
        .init_resource::<UiScaleWrites>()
        .add_systems(Update, (ui_scale_system, count_ui_scale_writes).chain());
    let handle = app.world_mut().resource_mut::<Assets<ProjectConfig>>().add(config);
    app.world_mut().insert_resource(ProjectConfigHandle(handle));
    app.world_mut().spawn((
        Window { resolution: window.into(), ..default() },
        PrimaryWindow,
    ));
    app
}

fn set_window(app: &mut App, size: (u32, u32)) {
    let mut q = app.world_mut().query_filtered::<&mut Window, With<PrimaryWindow>>();
    let mut w = q.single_mut(app.world_mut()).unwrap();
    w.resolution = size.into();
}

fn ui_scale(app: &App) -> f32 {
    app.world().resource::<UiScale>().0
}

fn writes(app: &App) -> u32 {
    app.world().resource::<UiScaleWrites>().0
}

// ── ui_scale_system ─────────────────────────────────────────────────────────────

#[test]
fn project_without_a_ui_scale_block_keeps_exactly_1_and_pulls_back_to_1() {
    let mut app = build_app(ron_config(""), (1920, 1080));
    app.update();
    assert_eq!(ui_scale(&app), 1.0, "no block = off, whatever the window size");

    // A hot-reload that removed the block while a scale was applied must reset to exactly 1.0.
    app.world_mut().resource_mut::<UiScale>().0 = 1.5;
    app.update();
    assert_eq!(ui_scale(&app), 1.0);
}

#[test]
fn fit_scales_with_the_window_and_snaps_down() {
    let mut app = build_app(ron_config("ui_scale: (reference: (1280.0, 720.0)),"), (1920, 1080));
    app.update();
    assert_eq!(ui_scale(&app), 1.5);

    set_window(&mut app, (1024, 768)); // raw 0.8: snaps DOWN to 0.75 (nearest would be 1.0 and overflow)
    app.update();
    assert_eq!(ui_scale(&app), 0.75);

    set_window(&mut app, (800, 600)); // raw 0.625 -> 0.5
    app.update();
    assert_eq!(ui_scale(&app), 0.5);

    set_window(&mut app, (3840, 2160)); // raw 3.0, clamped to max_scale 2.0
    app.update();
    assert_eq!(ui_scale(&app), 2.0);
}

#[test]
fn floor_raises_the_scale_and_never_panics_on_a_phone_sized_window() {
    let mut app = build_app(ron_config("ui_scale: (reference: (1280.0, 720.0)),"), (390, 844));
    app.update();
    assert_eq!(ui_scale(&app), 0.5, "390/1280 needs 0.25 but min_scale (0.5) is the floor");
}

#[test]
fn unchanged_scale_is_not_rewritten() {
    let mut app = build_app(ron_config("ui_scale: (reference: (1280.0, 720.0)),"), (1920, 1080));
    app.update();
    let after_first = writes(&app);
    for _ in 0..5 {
        app.update();
    }
    assert_eq!(writes(&app), after_first, "a steady window must not touch UiScale (each write = a full UI relayout)");

    // A different window size that snaps to the SAME scale must not write either.
    set_window(&mut app, (1950, 1100)); // raw min(1.523, 1.528) = 1.52 -> snaps down to 1.5, as before
    app.update();
    assert_eq!(ui_scale(&app), 1.5);
    assert_eq!(writes(&app), after_first, "same applied value, so no write");
}

#[test]
fn minimized_window_keeps_the_last_scale_and_writes_nothing() {
    let mut app = build_app(ron_config("ui_scale: (reference: (1280.0, 720.0)),"), (1920, 1080));
    app.update();
    assert_eq!(ui_scale(&app), 1.5);
    let before = writes(&app);

    for degenerate in [(0u32, 0u32), (0, 720), (1280, 0)] {
        set_window(&mut app, degenerate);
        app.update();
        assert_eq!(ui_scale(&app), 1.5, "a {degenerate:?} window carries no information");
    }
    assert_eq!(writes(&app), before, "minimize must not write UiScale");

    set_window(&mut app, (1920, 1080)); // restore
    app.update();
    assert_eq!(ui_scale(&app), 1.5, "restore returns to exactly the pre-minimize scale");
    assert_eq!(writes(&app), before, "and does not rewrite it either");
}

#[test]
fn fit_width_and_fit_height_use_one_axis() {
    let mut app = build_app(ron_config("ui_scale: (reference: (1280.0, 720.0), fit: FitWidth),"), (2560, 720));
    app.update();
    assert_eq!(ui_scale(&app), 2.0);

    let mut app = build_app(ron_config("ui_scale: (reference: (1280.0, 720.0), fit: FitHeight),"), (2560, 720));
    app.update();
    assert_eq!(ui_scale(&app), 1.0);
}

#[test]
fn snap_zero_disables_snapping() {
    let mut app = build_app(
        ron_config("ui_scale: (reference: (1000.0, 1000.0), snap: 0.0),"),
        (880, 880),
    );
    app.update();
    assert!((ui_scale(&app) - 0.88).abs() < 1e-6);
}

// ── Schema + validation ─────────────────────────────────────────────────────────

#[test]
fn ui_scale_block_parses_with_defaults() {
    let config = ron_config("ui_scale: (reference: (1280.0, 720.0)),");
    let cfg = config.ui_scale.as_ref().expect("block present");
    assert_eq!(cfg.reference, (1280.0, 720.0));
    assert_eq!(cfg.fit, UiScaleFit::Fit);
    assert_eq!((cfg.min_scale, cfg.max_scale, cfg.snap), (0.5, 2.0, 0.25));
    assert!(config.validate().is_ok());
}

#[test]
fn absent_block_is_none_and_valid() {
    let config = ron_config("");
    assert!(config.ui_scale.is_none());
    assert!(config.validate().is_ok());
}

#[test]
fn unknown_fields_and_missing_reference_are_parse_errors() {
    let parse = |block: &str| -> Result<ProjectConfig, _> {
        ron::de::from_str(&format!(
            "#![enable(implicit_some)]\n(schema_version: 3, initial_scene: \"s\", {block})"
        ))
    };
    assert!(parse("ui_scale: (reference: (1280.0, 720.0), fitt: Fit),").is_err(), "typo'd field");
    assert!(parse("ui_scale: (fit: Fit),").is_err(), "reference is required");
    assert!(parse("ui_scale: (reference: (1280.0, 720.0), fit: Cover),").is_err(), "unknown fit mode");
}

fn invalid(cfg: UiScaleConfig) -> String {
    let config = ProjectConfig { schema_version: 3, ui_scale: Some(cfg), ..Default::default() };
    config.validate().expect_err("must be rejected")
}

fn base() -> UiScaleConfig {
    UiScaleConfig { reference: (1280.0, 720.0), fit: UiScaleFit::Fit, min_scale: 0.5, max_scale: 2.0, snap: 0.25 }
}

#[test]
fn validation_names_the_field_and_the_fix() {
    for bad in [(0.0, 720.0), (1280.0, 0.0), (-1.0, 720.0), (f32::NAN, 720.0), (f32::INFINITY, 720.0)] {
        let msg = invalid(UiScaleConfig { reference: bad, ..base() });
        assert!(msg.contains("ui_scale.reference"), "{msg}");
    }
    let msg = invalid(UiScaleConfig { min_scale: 1.5, max_scale: 1.0, ..base() });
    assert!(msg.contains("ui_scale.min_scale (1.5) > ui_scale.max_scale (1): swap them"), "{msg}");
    assert!(invalid(UiScaleConfig { min_scale: 0.0, ..base() }).contains("ui_scale.min_scale"));
    assert!(invalid(UiScaleConfig { min_scale: 1e-30, ..base() }).contains("ui_scale.min_scale"), "a near-zero floor is rejected too");
    assert!(invalid(UiScaleConfig { max_scale: f32::NAN, ..base() }).contains("ui_scale.max_scale"));
    assert!(invalid(UiScaleConfig { snap: -0.25, ..base() }).contains("ui_scale.snap"));
    assert!(invalid(UiScaleConfig { snap: f32::NAN, ..base() }).contains("ui_scale.snap"));
    assert!(ProjectConfig { schema_version: 3, ui_scale: Some(UiScaleConfig { snap: 0.0, ..base() }), ..Default::default() }
        .validate().is_ok(), "snap 0.0 (off) is valid");
}

// ── Invalid configs must never panic the per-frame system ───────────────────────

#[test]
fn an_invalid_block_is_treated_as_off_and_never_panics() {
    // The runtime only logs a failed `ProjectConfig::validate()`; the config stays loaded and this
    // system reads it every frame. `f32::clamp` would panic on min > max (killing a web tab).
    for block in [
        "ui_scale: (reference: (1280.0, 720.0), min_scale: 1.5, max_scale: 1.0),",
        "ui_scale: (reference: (0.0, 720.0), fit: FitWidth),",
        "ui_scale: (reference: (0.0, 0.0)),",
        "ui_scale: (reference: (1280.0, 720.0), snap: -1.0),",
    ] {
        let mut app = build_app(ron_config(block), (1920, 1080));
        app.world_mut().resource_mut::<UiScale>().0 = 1.7; // a leftover value must be pulled back too
        app.update();
        assert_eq!(ui_scale(&app), 1.0, "invalid block must act as off: {block}");
    }
}

#[test]
fn a_floor_that_is_not_a_multiple_of_snap_is_not_reported_as_overflow() {
    use ironhold_core::capabilities::ui_scale::compute_ui_scale;
    let cfg = UiScaleConfig { reference: (1000.0, 1000.0), fit: UiScaleFit::Fit, min_scale: 0.6, max_scale: 2.0, snap: 0.25 };
    // Needs 0.65: snaps down to 0.5, is raised to the 0.6 floor, and still fits (0.6 < 0.65).
    let r = compute_ui_scale(&cfg, 650.0, 650.0);
    assert_eq!(r.applied, 0.6);
    assert!(!r.floored(&cfg), "0.65 needed >= 0.6 floor: it fits, so no overflow warning");
    // Needs 0.55: below the floor, the UI really overflows.
    assert!(compute_ui_scale(&cfg, 550.0, 550.0).floored(&cfg));
}

#[test]
fn whole_numbers_are_accepted_for_the_reference() {
    let config = ron_config("ui_scale: (reference: (1280, 720)),");
    assert_eq!(config.ui_scale.unwrap().reference, (1280.0, 720.0));
}

#[test]
fn snap_larger_than_max_scale_is_rejected() {
    let msg = invalid(UiScaleConfig { snap: 3.0, ..base() });
    assert!(msg.contains("ui_scale.snap"), "{msg}");
}