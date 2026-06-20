use bevy::prelude::*;

use crate::runtime::messages::{GameEvent, UiEvent};
use crate::runtime::actions::ActionQueue;
use crate::runtime::scene_manager::SpawnId;
use crate::schema::actions::Action;
use crate::schema::dialogue::{DialogueDef, DialogueChoiceDef, DialogueCondition};
use crate::GameVariables;
use crate::schema::stats::LoadedStats;

// ─── Components ───────────────────────────────────────────────────────────────

/// Marks a scene entity as having an associated dialogue file.
/// Inserted by the scene loader when `PrefabDef.dialogue` is `Some`.
/// The `dialogue_tick_system` uses this to auto-fire `Action::StartDialogue`
/// when `entity.interacted:{id}` is received for this entity.
#[derive(Component, Debug, Clone)]
pub struct DialoguePath(pub String);

/// Root marker for the dialogue panel UI node.
/// Spawned by the scene loader via `UiNodeDef::DialoguePanel`.
/// `choice_font_size` is stored here so `dialogue_tick_system` can use it
/// when dynamically spawning choice buttons.
#[derive(Component)]
pub struct DialoguePanelMarker {
    pub choice_font_size: f32,
}

impl Default for DialoguePanelMarker {
    fn default() -> Self { Self { choice_font_size: 13.0 } }
}

/// Identifies which text role a `Text` entity inside the panel plays.
/// Combined marker+role avoids two `Query<&mut Text>` parameters in the same system.
#[derive(Component)]
pub struct DialogueTextMarker(pub DialogueTextRole);

#[derive(Debug, Clone, PartialEq)]
pub enum DialogueTextRole { Speaker, Body }

/// Marks the flex column that receives dynamically spawned choice buttons.
#[derive(Component)]
pub struct DialogueChoicesContainer;

/// Marks a dynamically spawned choice button. Holds the 0-based index into the
/// current dialogue node's `choices` array for routing `UiEvent::ButtonPressed`.
#[derive(Component)]
pub struct DialogueChoiceBtn(pub usize);

// ─── Resource ─────────────────────────────────────────────────────────────────

/// Tracks the currently open dialogue conversation.
/// Cleared when no dialogue is active (`handle.is_none()`).
#[derive(Resource, Default, Clone)]
pub struct ActiveDialogue {
    pub npc_id: String,
    pub dialogue_path: String,
    pub current_node_index: usize,
    pub auto_advance_timer: Option<f32>,
    pub handle: Option<Handle<DialogueDef>>,
    /// Index of the last node that was fully rendered to the panel.
    /// Set to `None` when the index changes so `dialogue_tick_system` re-renders.
    pub last_rendered_node: Option<usize>,
}

impl ActiveDialogue {
    pub fn is_active(&self) -> bool { self.handle.is_some() }
    pub fn clear(&mut self) { *self = Self::default(); }
}

// ─── System ───────────────────────────────────────────────────────────────────

/// Manages the active dialogue conversation each frame.
///
/// Responsibilities:
/// 1. **Auto-wire** — detects `entity.interacted:{id}` for entities with `DialoguePath`
///    and pushes `Action::StartDialogue` so the executor sets `ActiveDialogue`.
/// 2. **Node rendering** — when `active_dialogue.last_rendered_node` doesn't match
///    `current_node_index`, updates the panel text and choice buttons.
/// 3. **Auto-advance** — ticks `auto_advance_timer` and advances when it expires.
/// 4. **Choice clicks** — reads `UiEvent::ButtonPressed("dialogue_choice:{n}")` and
///    navigates to the next node, a named jump target, or closes the dialogue.
///
/// Ordering: runs `.after(button_system).after(interactable_system).before(message_interpreter_system)`.
pub fn dialogue_tick_system(
    mut active: ResMut<ActiveDialogue>,
    dialogue_assets: Res<Assets<DialogueDef>>,
    mut action_queue: ResMut<ActionQueue>,
    mut ui_events: MessageReader<UiEvent>,
    mut game_events: MessageReader<GameEvent>,
    game_vars: Res<GameVariables>,
    loaded_stats: Option<Res<LoadedStats>>,
    time: Res<Time>,
    entity_dialogue_q: Query<(&SpawnId, &DialoguePath)>,
    mut panel_q: Query<(&mut Visibility, &DialoguePanelMarker)>,
    mut text_q: Query<(&mut Text, &DialogueTextMarker)>,
    choices_q: Query<Entity, With<DialogueChoicesContainer>>,
    choice_btn_q: Query<Entity, With<DialogueChoiceBtn>>,
    mut commands: Commands,
) {
    // ── Collect events upfront (avoids mid-system borrow conflicts) ───────────
    let interacted_ids: Vec<String> = game_events.read()
        .filter_map(|GameEvent::Trigger(name)| {
            name.strip_prefix("entity.interacted:").map(|s| s.to_string())
        })
        .collect();

    let choice_clicks: Vec<usize> = ui_events.read()
        .filter_map(|UiEvent::ButtonPressed(t)| {
            t.strip_prefix("dialogue_choice:").and_then(|s| s.parse::<usize>().ok())
        })
        .collect();

    // ── Hide panel and handle auto-wire when no dialogue is active ────────────
    if !active.is_active() {
        for (mut vis, _) in &mut panel_q {
            if *vis != Visibility::Hidden {
                *vis = Visibility::Hidden;
            }
        }
        for id in &interacted_ids {
            for (spawn_id, path) in &entity_dialogue_q {
                if &spawn_id.0 == id {
                    action_queue.push(Action::StartDialogue {
                        npc_id: id.clone(),
                        dialogue_path: path.0.clone(),
                    });
                    break;
                }
            }
        }
        return;
    }

    // ── Get the current dialogue definition ───────────────────────────────────
    let handle = match active.handle.clone() { Some(h) => h, None => return };
    let def = match dialogue_assets.get(&handle) { Some(d) => d, None => return };

    // ── Choice click handling (takes priority over auto-advance) ──────────────
    if let Some(choice_idx) = choice_clicks.first().copied() {
        let node_idx = active.current_node_index;
        if let Some(node) = def.nodes.get(node_idx) {
            if let Some(choice) = node.choices.get(choice_idx) {
                let jump_to = choice.jump_to.clone();
                let npc_id = active.npc_id.clone();
                for action in &choice.do_actions {
                    action_queue.push(substitute_self_in_action(action.clone(), &npc_id));
                }
                match jump_to.as_deref() {
                    Some("__end__") => {
                        active.clear();
                        set_panel_hidden(&mut panel_q);
                        despawn_choice_buttons(&choice_btn_q, &mut commands);
                        return;
                    }
                    Some(target_id) => {
                        let tid = target_id.to_string();
                        match def.nodes.iter().position(|n| n.id == tid) {
                            Some(idx) => {
                                active.current_node_index = idx;
                                active.last_rendered_node = None;
                                active.auto_advance_timer = None;
                            }
                            None => {
                                warn!("Dialogue: jump_to \"{}\" not found in '{}'; closing", tid, active.dialogue_path);
                                active.clear();
                                set_panel_hidden(&mut panel_q);
                                despawn_choice_buttons(&choice_btn_q, &mut commands);
                                return;
                            }
                        }
                    }
                    None => {
                        active.current_node_index += 1;
                        active.last_rendered_node = None;
                        active.auto_advance_timer = None;
                    }
                }
            }
        }
    }

    // ── Auto-advance timer ────────────────────────────────────────────────────
    if let Some(ref mut timer) = active.auto_advance_timer {
        *timer -= time.delta_secs();
        if *timer <= 0.0 {
            active.auto_advance_timer = None;
            active.current_node_index += 1;
            active.last_rendered_node = None;
        }
    }

    // ── Node rendering ────────────────────────────────────────────────────────
    let idx = active.current_node_index;
    if active.last_rendered_node != Some(idx) {
        if idx >= def.nodes.len() {
            active.clear();
            set_panel_hidden(&mut panel_q);
            despawn_choice_buttons(&choice_btn_q, &mut commands);
            return;
        }

        let node = &def.nodes[idx];
        let npc_id = active.npc_id.clone();
        let speaker_text = node.speaker.replace("{self}", &npc_id);
        let body_text = node.body.replace("{self}", &npc_id);

        // Show the panel.
        for (mut vis, _) in &mut panel_q {
            if *vis != Visibility::Visible {
                *vis = Visibility::Visible;
            }
        }

        // Update speaker and body text labels.
        for (mut text, role) in &mut text_q {
            match &role.0 {
                DialogueTextRole::Speaker => {
                    if text.0 != speaker_text {
                        *text = Text::new(speaker_text.clone());
                    }
                }
                DialogueTextRole::Body => {
                    if text.0 != body_text {
                        *text = Text::new(body_text.clone());
                    }
                }
            }
        }

        // Rebuild choice buttons.
        despawn_choice_buttons(&choice_btn_q, &mut commands);

        let choice_font_size = panel_q.iter()
            .next()
            .map(|(_, m)| m.choice_font_size)
            .unwrap_or(13.0);

        let visible_choices: Vec<(usize, DialogueChoiceDef)> = node.choices.iter()
            .enumerate()
            .filter(|(_, c)| evaluate_condition(c.condition.as_ref(), &game_vars, loaded_stats.as_deref()))
            .map(|(i, c)| (i, c.clone()))
            .collect();

        if let Ok(container_entity) = choices_q.single() {
            let npc_id_c = npc_id.clone();
            let fsize = choice_font_size;
            commands.entity(container_entity).with_children(|parent| {
                for (original_idx, choice) in &visible_choices {
                    let label = choice.label.replace("{self}", &npc_id_c);
                    let trigger = format!("dialogue_choice:{}", original_idx);
                    parent.spawn((
                        Name::new(format!("Choice:{}", original_idx)),
                        Button,
                        Node {
                            width: Val::Percent(100.0),
                            padding: UiRect::axes(Val::Px(10.0), Val::Px(5.0)),
                            border: UiRect::all(Val::Px(1.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.10, 0.10, 0.14, 0.90)),
                        BorderColor::from(Color::srgba(0.28, 0.30, 0.40, 0.75)),
                        crate::schema::ui::UiAction::Trigger(trigger),
                        DialogueChoiceBtn(*original_idx),
                    ))
                    .with_children(|p| {
                        p.spawn((
                            Name::new("ChoiceText"),
                            Text::new(label),
                            TextFont { font_size: fsize, ..default() },
                            TextColor(Color::srgba(0.85, 0.88, 0.95, 1.0)),
                        ));
                    });
                }
            });
        }

        active.auto_advance_timer = if node.choices.is_empty() { node.advance_delay_secs } else { None };
        active.last_rendered_node = Some(idx);
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn set_panel_hidden(panel_q: &mut Query<(&mut Visibility, &DialoguePanelMarker)>) {
    for (mut vis, _) in panel_q.iter_mut() {
        *vis = Visibility::Hidden;
    }
}

fn despawn_choice_buttons(
    choice_btn_q: &Query<Entity, With<DialogueChoiceBtn>>,
    commands: &mut Commands,
) {
    for e in choice_btn_q.iter() {
        commands.entity(e).despawn();
    }
}

/// Replaces `{self}` with `npc_id` in the string fields of common action variants.
/// Mirrors the interpreter's `{self}` substitution so that dialogue choice `do_actions`
/// behave consistently with behavior-file `do_actions`.
fn substitute_self_in_action(action: Action, npc_id: &str) -> Action {
    let s = |st: &str| st.replace("{self}", npc_id);
    match action {
        Action::Despawn(id)            => Action::Despawn(s(&id)),
        Action::ResetToSpawn(id)       => Action::ResetToSpawn(s(&id)),
        Action::Log(msg)               => Action::Log(s(&msg)),
        Action::EmitEvent(ev)          => Action::EmitEvent(s(&ev)),
        Action::SetTarget(id)          => Action::SetTarget(s(&id)),
        Action::SetVariable(k, v)      => Action::SetVariable(s(&k), s(&v)),
        Action::IncrementVariable(k, d) => Action::IncrementVariable(s(&k), d),
        Action::EmitEventAfterDelay { event, delay_secs } =>
            Action::EmitEventAfterDelay { event: s(&event), delay_secs },
        Action::PlayAnimationOn { target, clip } =>
            Action::PlayAnimationOn { target: s(&target), clip },
        Action::ModifyStat { key, delta } =>
            Action::ModifyStat { key: s(&key), delta },
        Action::SetStat { key, value } =>
            Action::SetStat { key: s(&key), value },
        Action::ShowDamagePopup { entity, amount } =>
            Action::ShowDamagePopup { entity: s(&entity), amount },
        Action::ShowFloatingText { entity, text, offset } =>
            Action::ShowFloatingText { entity: s(&entity), text: s(&text), offset },
        Action::SetEntityVisible { entity, visible } =>
            Action::SetEntityVisible { entity: s(&entity), visible },
        Action::SpawnEffect { key, position, entity } =>
            Action::SpawnEffect { key, position, entity: entity.map(|e| s(&e)) },
        Action::ProjectDecal { key, entity, position, radius, duration_secs, color, pulse_speed } =>
            Action::ProjectDecal { key, entity: entity.map(|e| s(&e)), position, radius, duration_secs, color, pulse_speed },
        other => other,
    }
}

fn evaluate_condition(
    condition: Option<&DialogueCondition>,
    game_vars: &GameVariables,
    loaded_stats: Option<&LoadedStats>,
) -> bool {
    match condition {
        None => true,
        Some(DialogueCondition::HasVariable { key, value }) => {
            game_vars.0.get(key.as_str()).map_or(false, |v| v == value)
        }
        Some(DialogueCondition::VariableGte { key, min }) => {
            game_vars.0.get(key.as_str())
                .and_then(|v| v.parse::<i32>().ok())
                .map_or(false, |n| n >= *min)
        }
        Some(DialogueCondition::StatAtLeast { stat_key, min }) => {
            loaded_stats
                .and_then(|s| s.0.get(stat_key.as_str()))
                .map_or(false, |stat| stat.effective >= *min)
        }
    }
}
