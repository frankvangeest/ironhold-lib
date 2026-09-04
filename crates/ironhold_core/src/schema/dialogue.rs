use bevy::prelude::*;
use serde::Deserialize;
use crate::schema::actions::Action;

pub const DIALOGUE_SCHEMA_VERSION: u32 = 1;

/// A loaded `.dialogue.ron` asset. Contains an ordered list of conversation nodes.
#[derive(Deserialize, Asset, TypePath, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct DialogueDef {
    pub schema_version: u32,
    pub nodes: Vec<DialogueNodeDef>,
}

/// One step in a conversation tree.
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct DialogueNodeDef {
    /// Stable ID used as a jump target. Must be unique within this dialogue.
    pub id: String,
    /// Display name shown in the speaker label.
    pub speaker: String,
    /// Reserved for a future NPC portrait image (not yet rendered in v1).
    /// Set the field to any asset catalog texture key; it is parsed and stored but has no visual
    /// effect until portrait rendering is implemented.
    #[serde(default)]
    pub portrait: Option<String>,
    /// Body text shown to the player. Supports `{self}` (NPC spawn ID) substitution.
    pub body: String,
    /// Seconds before auto-advancing when `choices` is empty.
    /// `None` = wait for player input (`AdvanceDialogue` action or a choice click).
    #[serde(default)]
    pub advance_delay_secs: Option<f32>,
    /// Player choices displayed as buttons. Empty = auto-advance node (uses `advance_delay_secs`).
    #[serde(default)]
    pub choices: Vec<DialogueChoiceDef>,
}

/// One choice button displayed during a dialogue node.
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct DialogueChoiceDef {
    /// Button label shown to the player. Supports `{self}` substitution.
    pub label: String,
    /// Optional visibility condition. Choice is hidden when the condition evaluates to false.
    #[serde(default)]
    pub condition: Option<DialogueCondition>,
    /// Actions fired through the pipeline when this choice is selected.
    #[serde(default)]
    pub do_actions: Vec<Action>,
    /// Node `id` to jump to after actions fire.
    /// `None` = advance to the next node in declaration order.
    /// `"__end__"` = close the dialogue panel.
    #[serde(default)]
    pub jump_to: Option<String>,
}

/// v1 conditions evaluated when filtering which choices to display.
/// Quest conditions are deferred until the quest system ships.
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub enum DialogueCondition {
    /// Choice visible only when `GameVariables[key] == value`.
    HasVariable { key: String, value: String },
    /// Choice visible only when `GameVariables[key]` parses as `i32` and is `>= min`.
    VariableGte { key: String, min: i32 },
    /// Choice visible only when the named global stat's effective value is `>= min`.
    StatAtLeast { stat_key: String, min: f32 },
}
