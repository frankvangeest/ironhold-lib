// Regression test for the panel-blocker click-through bug (panel_backdrop feature).
//
// Builds the exact entity topology from scene_loader.rs (UI Root with a child
// Button at z=0, plus a sibling-root full-screen blocker at GlobalZIndex(98))
// and runs the real Bevy 0.18 UI focus pipeline.
//
// It asserts directly on the dance button's `Interaction` after a click (the
// faithful signal — `button_system` only fires on `Changed<Interaction>`, which
// stale state can mask, so we read the value, not a fired-count).
//
// FINDING: with the default FocusPolicy (Pass), a visible blocker does NOT stop
// ui_focus_system from also pressing the button beneath it. Adding
// FocusPolicy::Block to the blocker fixes it. Both cases are asserted below.

use bevy::prelude::*;
use bevy::ui::{FocusPolicy, Interaction};

#[derive(Component)]
struct DanceButton;

#[derive(Resource, Default)]
struct InjectClick(bool);

// Inject just_pressed AFTER the input-clear system but BEFORE ui_focus_system,
// so the click is live during the focus pass. Mirrors a real mouse-down frame.
fn inject_click(mut inject: ResMut<InjectClick>, mut mb: ResMut<ButtonInput<MouseButton>>) {
    if inject.0 {
        mb.press(MouseButton::Left);
        inject.0 = false;
    }
}

// Headless harness has no camera view, so VisibilityPropagate leaves
// InheritedVisibility=false on every UI node. Force it to mirror a rendered
// frame: inherited-visible unless the node's own Visibility is Hidden.
fn force_inherited_visibility(mut q: Query<(&Visibility, &mut InheritedVisibility)>) {
    for (vis, mut inh) in &mut q {
        let target = if *vis != Visibility::Hidden {
            InheritedVisibility::VISIBLE
        } else {
            InheritedVisibility::HIDDEN
        };
        inh.set_if_neq(target);
    }
}

fn build_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(bevy::transform::TransformPlugin)
        .add_plugins(bevy::input::InputPlugin)
        .add_plugins(bevy::window::WindowPlugin {
            primary_window: Some(Window {
                resolution: (1280u32, 720u32).into(),
                ..default()
            }),
            ..default()
        })
        .add_plugins(bevy::a11y::AccessibilityPlugin)
        .add_plugins(bevy::asset::AssetPlugin::default())
        .add_plugins(bevy::text::TextPlugin)
        .add_plugins(bevy::picking::DefaultPickingPlugins)
        .add_plugins(bevy::ui::UiPlugin::default())
        .init_asset::<Image>()
        .init_asset::<TextureAtlasLayout>();
    app.init_resource::<InjectClick>();
    app.add_systems(
        PreUpdate,
        (force_inherited_visibility, inject_click)
            .after(bevy::input::InputSystems)
            .before(bevy::ui::UiSystems::Focus),
    );
    app
}

fn set_cursor(app: &mut App, x: f64, y: f64) {
    let world = app.world_mut();
    let mut win = world.query::<&mut Window>();
    let mut w = win.single_mut(world).unwrap();
    w.set_physical_cursor_position(Some((x, y).into()));
}

fn dance_interaction(app: &mut App) -> Interaction {
    let world = app.world_mut();
    let mut q = world.query_filtered::<&Interaction, With<DanceButton>>();
    *q.single(world).unwrap()
}

// Click at the button centre with the blocker in `blocker_policy`, and return
// the dance button's Interaction immediately after the focus pass.
fn click_through_blocker(blocker_policy: Option<FocusPolicy>) -> Interaction {
    let mut app = build_app();

    app.world_mut().spawn((
        Camera {
            order: 1000,
            viewport: Some(bevy::camera::Viewport {
                physical_position: bevy::math::UVec2::ZERO,
                physical_size: bevy::math::UVec2::new(1280, 720),
                ..default()
            }),
            ..default()
        },
        bevy::camera::RenderTarget::Window(bevy::window::WindowRef::Primary),
    ));

    app.world_mut()
        .spawn((
            Name::new("UI Root"),
            Node { width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
        ))
        .with_children(|p| {
            p.spawn((
                Name::new("Dance Button"),
                Button,
                DanceButton,
                Node {
                    width: Val::Px(200.0),
                    height: Val::Px(44.0),
                    position_type: PositionType::Absolute,
                    left: Val::Px(16.0),
                    top: Val::Px(176.0),
                    ..default()
                },
            ));
        });

    // Panel blocker — separate root, GlobalZIndex(98), VISIBLE for this test.
    let mut blocker = app.world_mut().spawn((
        Name::new("Panel Blocker"),
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        GlobalZIndex(98),
        Interaction::default(),
        Visibility::Visible,
    ));
    if let Some(fp) = blocker_policy {
        blocker.insert(fp);
    }

    for _ in 0..6 {
        app.update();
    }

    set_cursor(&mut app, 116.0, 198.0);
    app.update();
    app.update();

    app.world_mut().resource_mut::<InjectClick>().0 = true;
    app.update();

    dance_interaction(&mut app)
}

#[test]
fn blocker_with_default_pass_policy_does_not_block() {
    // Default Node FocusPolicy is Pass: focus iteration continues past the
    // blocker and ALSO presses the button beneath it. This is the bug.
    let interaction = click_through_blocker(None);
    eprintln!("default-policy blocker -> dance Interaction = {:?}", interaction);
    assert_eq!(
        interaction,
        Interaction::Pressed,
        "with default Pass policy the blocker should NOT block (bug repro)"
    );
}

#[test]
fn blocker_with_block_policy_blocks() {
    // FocusPolicy::Block makes the blocker capture the interaction: iteration
    // stops, the button beneath is never pressed. This is the fix.
    let interaction = click_through_blocker(Some(FocusPolicy::Block));
    eprintln!("block-policy blocker -> dance Interaction = {:?}", interaction);
    assert_ne!(
        interaction,
        Interaction::Pressed,
        "with FocusPolicy::Block the button beneath must not be pressed"
    );
}

// Regression test for the overlay-backdrop click-through gap (same root cause
// as the panel blocker above): scene_loader.rs's "Overlay Backdrop" node must
// carry FocusPolicy::Block + Interaction, or a base-scene button beneath it
// remains pressable regardless of GlobalZIndex or screen coverage.
fn click_through_overlay_backdrop(backdrop_has_block: bool) -> Interaction {
    let mut app = build_app();

    app.world_mut().spawn((
        Camera {
            order: 1000,
            viewport: Some(bevy::camera::Viewport {
                physical_position: bevy::math::UVec2::ZERO,
                physical_size: bevy::math::UVec2::new(1280, 720),
                ..default()
            }),
            ..default()
        },
        bevy::camera::RenderTarget::Window(bevy::window::WindowRef::Primary),
    ));

    app.world_mut()
        .spawn((
            Name::new("UI Root"),
            Node { width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
        ))
        .with_children(|p| {
            p.spawn((
                Name::new("Dance Button"),
                Button,
                DanceButton,
                Node {
                    width: Val::Px(200.0),
                    height: Val::Px(44.0),
                    position_type: PositionType::Absolute,
                    left: Val::Px(16.0),
                    top: Val::Px(176.0),
                    ..default()
                },
            ));
        });

    // Mirrors scene_loader.rs's "Overlay Backdrop" spawn exactly.
    let mut backdrop = app.world_mut().spawn((
        Name::new("Overlay Backdrop"),
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        GlobalZIndex(100),
    ));
    if backdrop_has_block {
        backdrop.insert((FocusPolicy::Block, Interaction::default()));
    }

    for _ in 0..6 {
        app.update();
    }

    set_cursor(&mut app, 116.0, 198.0);
    app.update();
    app.update();

    app.world_mut().resource_mut::<InjectClick>().0 = true;
    app.update();

    dance_interaction(&mut app)
}

#[test]
fn overlay_backdrop_without_focus_policy_does_not_block() {
    let interaction = click_through_overlay_backdrop(false);
    eprintln!("backdrop without FocusPolicy::Block -> dance Interaction = {:?}", interaction);
    assert_eq!(
        interaction,
        Interaction::Pressed,
        "without FocusPolicy::Block the base-scene button is clickable through the overlay backdrop (bug repro)"
    );
}

#[test]
fn overlay_backdrop_with_focus_policy_blocks() {
    let interaction = click_through_overlay_backdrop(true);
    eprintln!("backdrop with FocusPolicy::Block -> dance Interaction = {:?}", interaction);
    assert_ne!(
        interaction,
        Interaction::Pressed,
        "with FocusPolicy::Block the base-scene button beneath the overlay backdrop must not be pressed"
    );
}
