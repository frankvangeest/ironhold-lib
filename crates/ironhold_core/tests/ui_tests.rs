use bevy::prelude::*;
use bevy::ecs::system::RunSystemOnce;
use ironhold_core::GameVariables;
use ironhold_core::runtime::{UiEvent, ActionQueue, LoadedRules, SceneHandleV2};
use ironhold_core::schema::{AppState, Action, ProjectConfig, ProjectConfigHandle, LogicRule, GameSceneV2};

mod support;
use support::setup_test_app;

use ironhold_core::{IconButtonBind, IconShadowBind, IconButtonRoot};
use ironhold_core::schema::UiAction;

/// Spawns an IconButton root + foreground icon child (mirrors the real hierarchy built by
/// `scene_loader.rs`'s `UiNodeDef::IconButton` arm) and returns (root_entity, icon_child_entity,
/// icon_on_handle, icon_off_handle).
fn spawn_icon_button(
    app: &mut App,
    key: &str,
    root_interaction: Interaction,
) -> (Entity, Entity, Handle<Image>, Handle<Image>) {
    let (icon_on, icon_off) = {
        let mut images = app.world_mut().resource_mut::<Assets<Image>>();
        (images.add(Image::default()), images.add(Image::default()))
    };

    let root = app.world_mut().spawn((
        Button,
        Node::default(),
        BackgroundColor(Color::NONE),
        UiAction::Trigger("test_icon_toggle".to_string()),
        IconButtonRoot,
        root_interaction,
    )).id();

    let icon_child = app.world_mut().spawn((
        Node::default(),
        ImageNode { image: icon_off.clone(), color: Color::WHITE, ..default() },
        IconButtonBind {
            key: key.to_string(),
            icon_on: icon_on.clone(),
            icon_off: icon_off.clone(),
            icon_color: Color::srgba(1.0, 1.0, 1.0, 1.0),
            active_color: Color::srgba(0.0, 1.0, 0.0, 1.0),
            hover_color: Color::srgba(1.0, 1.0, 0.0, 1.0),
            click_color: Color::srgba(1.0, 0.0, 0.0, 1.0),
        },
        ChildOf(root),
    )).id();

    (root, icon_child, icon_on, icon_off)
}

#[test]
fn test_ui_button_to_load_scene_action() {
    let mut app = setup_test_app();
       
    // 1. Run once to process Startup (setup)
    app.update();
    
    // Override ProjectConfig with test-specific rules
    {
        let mut configs = app.world_mut().resource_mut::<Assets<ProjectConfig>>();
        let rules = vec![
            LogicRule {
                on: "ui.button_pressed:test_load".to_string(),
                when: None,
                do_actions: vec![Action::LoadScene("scenes/tests/test_scene.scene.ron".to_string())],
            }
        ];
        let config_handle = configs.add(ProjectConfig {
            schema_version: 1,
            initial_scene: "scenes/tests/test_scene.scene.ron".to_string(),
            rules: rules.clone(),
            ..Default::default()
        });
        app.world_mut().insert_resource(ProjectConfigHandle(config_handle));
        app.world_mut().insert_resource(LoadedRules(rules));
    }

    // 2. Simulate Button Press Message
    app.world_mut().resource_mut::<Messages<UiEvent>>().write(UiEvent::ButtonPressed("test_load".to_string()));
    
    // 3. Run systems (Interpreter + Executor will run)
    app.update();
    
    // 4. Run once more to process state transition
    app.update();
    
    // 5. Verify side effects
    // The executor should have inserted a SceneHandleV2 resource
    assert!(app.world().contains_resource::<SceneHandleV2>());
    
    // And state should be LoadingScene
    let state = app.world().resource::<State<AppState>>();
    assert_eq!(*state.get(), AppState::LoadingScene);
}

#[test]
fn test_ui_button_to_quit_action() {
    let mut app = setup_test_app();
    
    // 1. Run once to process Startup (setup)
    app.update();
    
    // Override ProjectConfig with test-specific rules
    {
        let mut configs = app.world_mut().resource_mut::<Assets<ProjectConfig>>();
        let rules = vec![
            LogicRule {
                on: "ui.button_pressed:test_quit".to_string(),
                when: None,
                do_actions: vec![Action::Quit],
            }
        ];
        let config_handle = configs.add(ProjectConfig {
            schema_version: 1,
            initial_scene: "scenes/tests/test_scene.ron".to_string(),
            rules: rules.clone(),
            ..Default::default()
        });
        app.world_mut().insert_resource(ProjectConfigHandle(config_handle));
        app.world_mut().insert_resource(LoadedRules(rules));
    }

    // 2. Simulate Quit Message
    app.world_mut().resource_mut::<Messages<UiEvent>>().write(UiEvent::ButtonPressed("test_quit".to_string()));
    
    // 3. Run systems (Interpreter + Executor will run)
    app.update();
    
    // 4. Verify side effects
    // The executor should have queued Action::Quit which sends AppExit
    // We can check if ActionQueue has it or just verify it doesn't crash
    let action_queue = app.world().resource::<ActionQueue>();
    assert!(action_queue.0.is_empty()); // Should be empty because it was popped and executed
}

/// Golden path: bound value is "true", root Interaction::None -> icon shows icon_on
/// with active_color (regression guard for the icon_on/icon_off inversion bug).
#[test]
fn test_icon_button_shows_icon_on_and_active_color_when_bound_true() {
    let mut app = setup_test_app();
    app.update();

    let (_root, icon_child, icon_on, _icon_off) =
        spawn_icon_button(&mut app, "some_key", Interaction::None);

    app.world_mut().resource_mut::<GameVariables>().0.insert("some_key".to_string(), "true".to_string());

    app.update();

    let image = app.world().get::<ImageNode>(icon_child).unwrap();
    assert_eq!(image.image, icon_on, "bound=true must show icon_on, not icon_off");
    assert_eq!(image.color, Color::srgba(0.0, 1.0, 0.0, 1.0), "bound=true + Interaction::None must show active_color");
}

/// Absent GameVariables key (never set) resolves to the "false" path: icon_off + icon_color.
#[test]
fn test_icon_button_shows_icon_off_and_icon_color_when_bound_missing() {
    let mut app = setup_test_app();
    app.update();

    let (_root, icon_child, _icon_on, icon_off) =
        spawn_icon_button(&mut app, "some_key", Interaction::None);
    // Deliberately do not set GameVariables["some_key"] at all.

    app.update();

    let image = app.world().get::<ImageNode>(icon_child).unwrap();
    assert_eq!(image.image, icon_off, "missing bind key must fall back to icon_off");
    assert_eq!(image.color, Color::srgba(1.0, 1.0, 1.0, 1.0), "missing bind key + Interaction::None must show icon_color");
}

/// Hover state overrides the resting color regardless of the bound value.
#[test]
fn test_icon_button_hover_color_overrides_resting_color() {
    let mut app = setup_test_app();
    app.update();

    let (_root, icon_child, _icon_on, _icon_off) =
        spawn_icon_button(&mut app, "some_key", Interaction::Hovered);
    app.world_mut().resource_mut::<GameVariables>().0.insert("some_key".to_string(), "true".to_string());

    app.update();

    let image = app.world().get::<ImageNode>(icon_child).unwrap();
    assert_eq!(image.color, Color::srgba(1.0, 1.0, 0.0, 1.0), "Interaction::Hovered must show hover_color even when bound=true");
}

/// Pressed state overrides the resting color regardless of the bound value.
#[test]
fn test_icon_button_click_color_overrides_resting_color() {
    let mut app = setup_test_app();
    app.update();

    let (_root, icon_child, _icon_on, _icon_off) =
        spawn_icon_button(&mut app, "some_key", Interaction::Pressed);
    // bound value absent this time — click_color must still win over icon_color.
    app.update();

    let image = app.world().get::<ImageNode>(icon_child).unwrap();
    assert_eq!(image.color, Color::srgba(1.0, 0.0, 0.0, 1.0), "Interaction::Pressed must show click_color regardless of bound value");
}

/// The optional shadow child follows the same icon_on/icon_off true/false logic as the
/// foreground icon, but its color is never touched by icon_button_sync_system.
#[test]
fn test_icon_shadow_follows_icon_swap_but_not_color() {
    let mut app = setup_test_app();
    app.update();

    let (icon_on, icon_off) = {
        let mut images = app.world_mut().resource_mut::<Assets<Image>>();
        (images.add(Image::default()), images.add(Image::default()))
    };
    let shadow_color = Color::srgba(0.0, 0.0, 0.0, 0.5);

    let root = app.world_mut().spawn((
        Button,
        Node::default(),
        BackgroundColor(Color::NONE),
        UiAction::Trigger("test_icon_toggle".to_string()),
        IconButtonRoot,
        Interaction::Hovered, // deliberately non-None to prove shadow color ignores it too
    )).id();

    let shadow_child = app.world_mut().spawn((
        Node::default(),
        ImageNode { image: icon_off.clone(), color: shadow_color, ..default() },
        IconShadowBind {
            key: "shadow_key".to_string(),
            icon_on: icon_on.clone(),
            icon_off: icon_off.clone(),
        },
        ChildOf(root),
    )).id();

    // Case 1: bound value false/missing -> icon_off, color untouched.
    app.update();
    {
        let image = app.world().get::<ImageNode>(shadow_child).unwrap();
        assert_eq!(image.image, icon_off);
        assert_eq!(image.color, shadow_color, "shadow color must never change");
    }

    // Case 2: flip bound value to true -> icon_on, color still untouched.
    app.world_mut().resource_mut::<GameVariables>().0.insert("shadow_key".to_string(), "true".to_string());
    app.update();
    {
        let image = app.world().get::<ImageNode>(shadow_child).unwrap();
        assert_eq!(image.image, icon_on, "shadow must follow the same true/false swap as the foreground icon");
        assert_eq!(image.color, shadow_color, "shadow color must remain untouched after the swap");
    }
}

/// icon_button_click_system fires UiEvent::ButtonPressed exactly once when an IconButtonRoot
/// entity's Interaction becomes Pressed.
#[test]
fn test_icon_button_click_fires_ui_event_on_press() {
    let mut app = setup_test_app();
    app.update();

    let root = app.world_mut().spawn((
        Button,
        Node::default(),
        BackgroundColor(Color::NONE),
        UiAction::Trigger("icon_toggle".to_string()),
        IconButtonRoot,
        Interaction::None,
    )).id();

    // First update settles Changed<Interaction> for the initial insert; no press yet.
    app.update();

    *app.world_mut().get_mut::<Interaction>(root).unwrap() = Interaction::Pressed;
    app.update();

    let fired: Vec<_> = app.world_mut().run_system_once(|mut ui_events: MessageReader<UiEvent>| {
        ui_events.read().cloned().collect::<Vec<_>>()
    }).unwrap();

    let press_count = fired.iter().filter(|e| matches!(e, UiEvent::ButtonPressed(t) if t == "icon_toggle")).count();
    assert_eq!(press_count, 1, "expected exactly one ButtonPressed event for the icon button trigger");
}

/// A plain Button (no IconButtonRoot) pressed alongside an IconButtonRoot button must only
/// fire once per press via its own path (button_system), proving icon_button_click_system
/// does not double-process ordinary buttons.
#[test]
fn test_icon_button_click_system_ignores_plain_buttons() {
    let mut app = setup_test_app();
    app.update();

    let plain_button = app.world_mut().spawn((
        Button,
        Node::default(),
        BackgroundColor(Color::NONE),
        UiAction::Trigger("plain_button_trigger".to_string()),
        Interaction::None,
        // Deliberately no IconButtonRoot marker.
    )).id();

    app.update();

    *app.world_mut().get_mut::<Interaction>(plain_button).unwrap() = Interaction::Pressed;
    app.update();

    let fired: Vec<_> = app.world_mut().run_system_once(|mut ui_events: MessageReader<UiEvent>| {
        ui_events.read().cloned().collect::<Vec<_>>()
    }).unwrap();

    let press_count = fired.iter().filter(|e| matches!(e, UiEvent::ButtonPressed(t) if t == "plain_button_trigger")).count();
    assert_eq!(press_count, 1, "plain Button must still fire exactly once via button_system, not be double-fired by icon_button_click_system");
}

/// Drives a real Replace-mode scene load (mirrors `scene_lifecycle_tests.rs`'s
/// `drive_replace_load` pattern) with a `ui:` block exercising `Label`/`Button`'s new
/// `font_size`/`clip` fields, both set and omitted.
fn load_scene_with_ui(app: &mut App, ui_ron: &str) {
    let config_handle = app
        .world_mut()
        .resource_mut::<Assets<ProjectConfig>>()
        .add(ProjectConfig {
            schema_version: 1,
            initial_scene: "scenes/t.ron".to_string(),
            ..Default::default()
        });
    app.world_mut().insert_resource(ProjectConfigHandle(config_handle));

    let ron_text = format!("(schema_version: 2, entities: [], ui: [{ui_ron}])");
    let scene: GameSceneV2 = ron::de::from_str(&ron_text).unwrap();
    let scene_handle = app.world_mut().resource_mut::<Assets<GameSceneV2>>().add(scene);
    app.world_mut().insert_resource(SceneHandleV2(scene_handle));

    app.world_mut().resource_mut::<NextState<AppState>>().set(AppState::LoadingScene);
    app.update();
    app.update();
    app.update();
}

fn text_font_sizes_by_name(app: &mut App) -> std::collections::HashMap<String, f32> {
    let mut query = app.world_mut().query::<(&Name, &TextFont)>();
    query
        .iter(app.world())
        .map(|(name, font)| (name.as_str().to_string(), font.font_size))
        .collect()
}

fn node_overflow_and_align_by_name(app: &mut App) -> std::collections::HashMap<String, (Overflow, AlignItems)> {
    let mut query = app.world_mut().query::<(&Name, &Node)>();
    query
        .iter(app.world())
        .map(|(name, node)| (name.as_str().to_string(), (node.overflow, node.align_items)))
        .collect()
}

#[test]
fn test_label_and_button_font_size_override() {
    let mut app = setup_test_app();
    app.update();

    load_scene_with_ui(&mut app, r#"
        Label((id: "custom_label", text: "Custom", position: (0.0, 0.0), font_size: 14.0)),
        Button((id: "custom_button", text: "Go", action: "ui.go", position: (0.0, 40.0), font_size: 18.0)),
    "#);

    let sizes = text_font_sizes_by_name(&mut app);
    assert_eq!(sizes.get("Text: Custom"), Some(&14.0), "Label.font_size must override the hardcoded default");
    assert_eq!(sizes.get("Text: Go"), Some(&18.0), "Button.font_size must override the hardcoded default");
}

#[test]
fn test_label_and_button_font_size_default_unchanged() {
    let mut app = setup_test_app();
    app.update();

    load_scene_with_ui(&mut app, r#"
        Label((id: "default_label", text: "Default", position: (0.0, 0.0))),
        Button((id: "default_button", text: "Ok", action: "ui.ok", position: (0.0, 40.0))),
    "#);

    let sizes = text_font_sizes_by_name(&mut app);
    assert_eq!(sizes.get("Text: Default"), Some(&22.0), "omitting font_size must reproduce the pre-feature Label default exactly");
    assert_eq!(sizes.get("Text: Ok"), Some(&26.0), "omitting font_size must reproduce the pre-feature Button default exactly");
}

#[test]
fn test_label_and_button_clip_defaults_to_off_and_preserves_center_alignment() {
    let mut app = setup_test_app();
    app.update();

    load_scene_with_ui(&mut app, r#"
        Label((id: "plain_label", text: "Plain", position: (0.0, 0.0))),
        Button((id: "plain_button", text: "Ok", action: "ui.ok", position: (0.0, 40.0))),
    "#);

    let nodes = node_overflow_and_align_by_name(&mut app);
    let (label_overflow, label_align) = nodes.get("Label: Plain").expect("label node must exist");
    let (btn_overflow, btn_align) = nodes.get("Button: Ok").expect("button node must exist");
    assert_eq!(*label_overflow, Overflow::visible(), "clip: false (the default) must leave overflow visible, matching every existing scene's shipped behavior");
    assert_eq!(*label_align, AlignItems::Center, "clip: false must not touch the default vertical centering");
    assert_eq!(*btn_overflow, Overflow::visible(), "clip: false (the default) must leave overflow visible, matching every existing scene's shipped behavior");
    assert_eq!(*btn_align, AlignItems::Center, "clip: false must not touch the default vertical centering");
}

#[test]
fn test_label_and_button_clip_true_enables_clipping_and_top_anchors() {
    let mut app = setup_test_app();
    app.update();

    load_scene_with_ui(&mut app, r#"
        Label((id: "clipped_label", text: "Clipped", position: (0.0, 0.0), clip: true)),
        Button((id: "clipped_button", text: "Ok", action: "ui.ok", position: (0.0, 40.0), clip: true)),
    "#);

    let nodes = node_overflow_and_align_by_name(&mut app);
    let (label_overflow, label_align) = nodes.get("Label: Clipped").expect("label node must exist");
    let (btn_overflow, btn_align) = nodes.get("Button: Ok").expect("button node must exist");
    assert_eq!(*label_overflow, Overflow::clip(), "clip: true must enable clipping");
    assert_eq!(*label_align, AlignItems::FlexStart, "clip: true must top-anchor content so overflow is trimmed only from the bottom, not sliced out of every line");
    assert_eq!(*btn_overflow, Overflow::clip(), "clip: true must enable clipping");
    assert_eq!(*btn_align, AlignItems::FlexStart, "clip: true must top-anchor content so overflow is trimmed only from the bottom, not sliced out of every line");
}
