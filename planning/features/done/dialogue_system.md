# Feature: Dialogue System

_Status: Draft_
_Planned at: `af1b004` (2026-06-02)_

---

> ## Pre-implementation checklist
>
> - [ ] **Decide: `DialogueDef` as standalone asset file vs. inline on `PrefabDef`.** Options: (a) standalone `.dialogue.ron` files loaded by `AssetServer`, referenced by path in `PrefabDef.dialogue`; (b) inline `DialogueDef` struct on `PrefabDef`. Recommendation: **standalone `.dialogue.ron` files** — conversations can be long; reusable across NPCs (an innkeeper in multiple towns uses the same welcome script); matches the pattern of `.behavior.ron` (also a standalone asset). Path stored as `dialogue: Some("dialogues/merchant_intro.dialogue.ron")` on `PrefabDef`.
>
> - [ ] **Decide: UI layout.** Options: (a) full-width bottom panel with portrait, speaker name, body text, and stacked choice buttons; (b) speech bubble above the NPC in world space (requires world-space UI projection, complex); (c) designer-configurable via a `DialoguePanel` scene UI node. Recommendation: **`DialoguePanel` UI node in scene RON** — same pattern as `ActionBar`; each scene declares its dialogue UI layout once; the dialogue system writes to it. Default layout: lower-third panel, portrait on the left, name + text on the right, choices below the text.
>
> - [ ] **Decide: condition evaluation for v1.** The backlog mentions `quest.state:{id}` conditions but the quest system does not exist yet. For v1, limit conditions to: `HasVariable { key, value }`, `VariableGte { key, min }` (reads `GameVariables`), and `StatThreshold { key, min }` (reads global `LoadedStats`). Quest conditions deferred — they will be added when the quest system ships.
>
> - [ ] **Decide: branching / jump-to semantics.** Options: (a) linear only — nodes advance in order, no jumps; (b) `jump_to: node_id` on each choice — jump to any named node; (c) `next_node: Option<String>` on each node (default = next in list). Recommendation: **`jump_to: Option<String>` on `DialogueChoiceDef`** — supports branches and loops; `None` = advance to next node in list; special value `"__end__"` = close dialogue. Nodes are accessed by string `id` field.
>
> - [ ] **Decide: auto-advance nodes (no choices).** A node with no `choices` list should advance automatically after `advance_delay_secs`. For manual-advance nodes (cut-scenes where the player clicks to proceed), use a single empty-label choice or set `advance_delay_secs: None`. Recommendation: **`advance_delay_secs: Option<f32>` on `DialogueNodeDef`** — `None` = wait for `AdvanceDialogue` action; `Some(2.5)` = auto-advance after 2.5 s.
>
> - [ ] **Confirm: `DialoguePanel` UI node won't conflict with `ActionBar` or `LoadSceneOverlay`.** Dialogue panels should stack with overlays cleanly — the dialogue UI is a persistent scene entity (not an overlay), so it is part of the scene's UI section and survives overlay load/unload.

---

## What

RON-defined conversation trees between the player and NPCs (or between two NPCs, or as a monologue). A `DialogueDef` asset contains an ordered list of nodes; each node has a speaker name, body text, optional portrait, and optional player choices. Choices fire pipeline actions and can jump to any named node.

`StartDialogue` opens a `DialoguePanel` UI node declared in the scene RON. All text and branching is data-driven; the engine provides only the rendering layer and event plumbing.

---

## Why

Without a dialogue system, the only way to convey NPC speech is through `world_label` annotations or in-world text. There is no mechanism for branching conversations, quest accept/decline, or shop-gating behind a conversation. The dialogue system provides the minimal viable RPG conversation layer and unblocks quest triggers, merchant interactions, and cutscene-style monologues.

Unblocks: quest accept/decline flow (via `do_actions` on choices), merchant conversation-gating, lore delivery, tutorial guidance without code changes.

---

## Asset format

### `.dialogue.ron`

```ron
// dialogues/merchant_intro.dialogue.ron
(
    schema_version: 1,
    nodes: [
        (
            id: "greeting",
            speaker: "Old Merchant",
            portrait: Some("portraits/merchant"),
            body: "Ah, a traveller! Welcome to my humble shop.",
            advance_delay_secs: None,
            choices: [
                ( label: "What are you selling?", jump_to: Some("shop_offer") ),
                ( label: "Tell me about this town.", jump_to: Some("town_lore") ),
                ( label: "Goodbye.", jump_to: Some("__end__") ),
            ],
        ),
        (
            id: "shop_offer",
            speaker: "Old Merchant",
            body: "Fine blades and potions, the best this side of the mountains!",
            choices: [
                ( label: "I'd like to browse.",
                  do_actions: [ EndDialogue, OpenShop("merchant_01") ] ),
                ( label: "Maybe later.", jump_to: Some("__end__") ),
            ],
        ),
        (
            id: "town_lore",
            speaker: "Old Merchant",
            body: "They say the old keep is haunted. I wouldn't go there if I were you.",
            advance_delay_secs: Some(3.0),
            // No choices — auto-advances after 3 seconds, then falls through to next node.
        ),
        (
            id: "lore_followup",
            speaker: "Old Merchant",
            body: "But adventurers never listen, do they?",
            choices: [
                ( label: "I'm not afraid.", jump_to: Some("__end__") ),
                ( label: "You're right, I'll be careful.", jump_to: Some("__end__") ),
            ],
        ),
    ],
)
```

### New `DialogueDef`, `DialogueNodeDef`, `DialogueChoiceDef` (`schema/dialogue.rs`)

```rust
#[derive(Deserialize, Asset, TypePath, Debug, Clone)]
pub struct DialogueDef {
    pub schema_version: u32,
    pub nodes: Vec<DialogueNodeDef>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct DialogueNodeDef {
    /// Stable identifier for jump targets. Must be unique within this dialogue.
    pub id: String,
    /// Display name shown in the speaker label.
    pub speaker: String,
    /// Texture catalog key for a portrait image. None = no portrait.
    #[serde(default)]
    pub portrait: Option<String>,
    /// Main body text. Supports `{self}` (NPC spawn ID) and `{target}` substitution.
    pub body: String,
    /// Seconds before auto-advancing when `choices` is empty. None = wait for AdvanceDialogue.
    #[serde(default)]
    pub advance_delay_secs: Option<f32>,
    /// Player choices. Empty = auto-advance node (uses `advance_delay_secs`).
    #[serde(default)]
    pub choices: Vec<DialogueChoiceDef>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct DialogueChoiceDef {
    /// Button label shown to the player.
    pub label: String,
    /// Optional condition. Hidden (not shown) when condition is false.
    #[serde(default)]
    pub condition: Option<DialogueCondition>,
    /// Actions fired through the pipeline when this choice is selected.
    #[serde(default)]
    pub do_actions: Vec<Action>,
    /// Node `id` to jump to after actions fire. None = advance to next node in list.
    /// Use `"__end__"` to close the dialogue.
    #[serde(default)]
    pub jump_to: Option<String>,
}

/// v1 conditions — quest conditions deferred until quest system ships.
#[derive(Deserialize, Debug, Clone)]
pub enum DialogueCondition {
    /// Choice visible only when `GameVariables[key] == value`.
    HasVariable { key: String, value: String },
    /// Choice visible only when `GameVariables[key]` parses as i32 >= min.
    VariableGte { key: String, min: i32 },
    /// Choice visible only when the named global stat's current value >= min.
    StatAtLeast { stat_key: String, min: f32 },
}
```

### `PrefabDef` addition (`schema/catalog.rs`)

```ron
// prefabs/prefabs.ron
"merchant_npc": (
    kind: "actor",
    model: "characters/merchant",
    dialogue: Some("dialogues/merchant_intro.dialogue.ron"),  // NEW
    interactable: ( radius: 2.5 ),
    // ...
)
```

```rust
// schema/catalog.rs — in PrefabDef
/// Project-relative path to a `.dialogue.ron` file.
/// When set and the entity is interacted with (interactable trigger), `StartDialogue` fires automatically.
#[serde(default)]
pub dialogue: Option<String>,
```

### Scene RON — `DialoguePanel` UI node

```ron
// scenes/town.scene.ron
ui: [
    ( id: "dialogue_panel", kind: DialoguePanel(
        position: (0.0, 0.0),         // anchor: bottom-left of the panel
        size: (800.0, 220.0),
        portrait_size: (180.0, 180.0),
        speaker_font_size: 16.0,
        body_font_size: 14.0,
        choice_font_size: 13.0,
        background_color: (0.05, 0.05, 0.1, 0.9),
        initially_hidden: true,       // shown only when dialogue is active
    )),
]
```

---

## New actions (`schema/actions.rs`)

```ron
// Start dialogue from a specific NPC entity (substitutes {self} in node text)
StartDialogue(npc_id: "merchant_01", dialogue_path: "dialogues/merchant_intro.dialogue.ron")

// Advance to the next node or confirm a choice (fired by choice buttons internally)
AdvanceDialogue

// Close the dialogue panel
EndDialogue
```

```rust
/// Open a dialogue. `npc_id` provides `{self}` substitution context; may be empty string.
/// `dialogue_path` is a project-relative path to a `.dialogue.ron` asset.
/// No-op with warning if no `DialoguePanel` UI node exists in the current scene.
StartDialogue { npc_id: String, dialogue_path: String },
/// Advance to the next node in the active dialogue. No-op when no dialogue is active.
AdvanceDialogue,
/// Close the active dialogue panel. Emits `dialogue.ended:{path}`.
EndDialogue,
```

---

## New pipeline events

```ron
dialogue.started:{path}           // dialogue opened
dialogue.node:{path}:{node_id}    // new node displayed (fires on each node, including first)
dialogue.choice:{path}:{index}    // player selected choice at index (0-based)
dialogue.ended:{path}             // dialogue closed (by EndDialogue, __end__ jump, or last node)
```

---

## Runtime

### `ActiveDialogue` resource (`capabilities/dialogue.rs`)

```rust
#[derive(Resource, Default)]
pub struct ActiveDialogue {
    pub npc_id: String,
    pub dialogue_path: String,
    pub current_node_index: usize,
    pub auto_advance_timer: Option<f32>,   // countdown for auto-advance nodes
    pub handle: Option<Handle<DialogueDef>>,
}
```

### `dialogue_tick_system` (`capabilities/dialogue.rs`)

Runs in `Update`. Only active when `ActiveDialogue.handle` is `Some`.

1. **Poll asset load**: if handle not yet resolved in `Assets<DialogueDef>`, skip.
2. **Auto-advance timer**: if current node has `advance_delay_secs` and no choices, decrement timer. On ≤ 0: advance to next node (or close if last).
3. **Render current node**: write speaker name, body text (with `{self}` / `{target}` substitution), and visible choices (filtered by condition evaluation) to the `DialoguePanel` UI entity. Only write when node index changes (change-detection guard).

### `StartDialogue` executor arm

```rust
Action::StartDialogue { npc_id, dialogue_path } => {
    let handle = asset_server.load::<DialogueDef>(&resolved_path);
    active_dialogue.handle = Some(handle);
    active_dialogue.npc_id = npc_id;
    active_dialogue.dialogue_path = dialogue_path.clone();
    active_dialogue.current_node_index = 0;
    game_events.write(GameEvent::Trigger(format!("dialogue.started:{}", dialogue_path)));
    // Show DialoguePanel — set Visibility::Visible on the panel entity
}
```

### Choice selection

When a player clicks a choice button in the `DialoguePanel`:
1. The button emits `UiEvent::ButtonPressed("dialogue_choice:{index}")`.
2. The `dialogue_tick_system` intercepts this, fires `do_actions` from the choice through the pipeline, emits `dialogue.choice:{path}:{index}`.
3. If `jump_to: Some("__end__")` → close dialogue.
4. If `jump_to: Some(id)` → find node by `id`, set `current_node_index`.
5. If `jump_to: None` → advance to `current_node_index + 1`; if past end → close.

---

## Interactable integration

When a prefab has both `dialogue: Some(...)` and `interactable: (...)`, the scene loader
auto-registers a rule in the entity's behavior file (or via a synthesized local rule):

```ron
( on: "entity.interacted:{self}", do_actions: [
    StartDialogue(npc_id: "{self}", dialogue_path: "dialogues/merchant_intro.dialogue.ron")
] ),
```

The designer does not need to write this manually — `PrefabDef.dialogue` being set implies automatic interaction wiring. Override by writing an explicit behavior rule for `entity.interacted:{self}`.

---

## New Rust changes

- `schema/dialogue.rs` (new file) — `DialogueDef`, `DialogueNodeDef`, `DialogueChoiceDef`, `DialogueCondition`.
- `schema/catalog.rs` — `dialogue: Option<String>` on `PrefabDef`.
- `schema/scene_v2.rs` — `DialoguePanel(DialoguePanelDef)` variant in the `UiNodeKind` enum.
- `schema/actions.rs` — `StartDialogue`, `AdvanceDialogue`, `EndDialogue`.
- `capabilities/dialogue.rs` (new file) — `ActiveDialogue`, `dialogue_tick_system`, condition evaluation.
- `capabilities/mod.rs` — register module + system + `ImplicitRonPlugin::<DialogueDef>`.
- `runtime/scene_manager/action_executor.rs` — handle `StartDialogue`, `AdvanceDialogue`, `EndDialogue`.
- `runtime/scene_manager/scene_loader.rs` — auto-wire `entity.interacted:{self}` rule when `PrefabDef.dialogue` is set; spawn `DialoguePanel` UI entity.
- `runtime/scene_manager/mod.rs` — clear `ActiveDialogue` on `LoadScene`.
- `lib.rs` — register `DialogueDef` as an asset type.

---

## Tasks

- [ ] Decisions from pre-implementation checklist resolved
- [ ] `schema/dialogue.rs` — `DialogueDef` + all types; `ImplicitRonPlugin` registered
- [ ] `dialogue: Option<String>` on `PrefabDef`
- [ ] `DialoguePanel(DialoguePanelDef)` UI node variant + scene loader spawn
- [ ] `ActiveDialogue` resource; cleared on `LoadScene`
- [ ] `dialogue_tick_system` — asset poll, auto-advance, node render, choice filtering
- [ ] `StartDialogue` / `AdvanceDialogue` / `EndDialogue` executor arms
- [ ] `{self}` + `{target}` substitution in `body` and `label` strings
- [ ] Condition evaluation for `HasVariable`, `VariableGte`, `StatAtLeast`
- [ ] Auto-wire `entity.interacted` → `StartDialogue` when `PrefabDef.dialogue` is set
- [ ] Pipeline events: `dialogue.started`, `dialogue.node`, `dialogue.choice`, `dialogue.ended`
- [ ] Demo: add a dialogue NPC to `entity_logic_demo` with branching choices; one choice fires `EmitEvent` to unlock a door
- [ ] Integration tests: node sequence, auto-advance timing, jump_to by id, `__end__` closes, condition hides choice, events fire correctly
- [ ] Docs: `.dialogue.ron` format in `docs/20_data_formats.md`; `StartDialogue` + events in `docs/30_runtime_events_and_logic.md`

---

## Open questions

- **Quest conditions**: `DialogueCondition::QuestState { key, state }` deferred until quest system ships. When it does, add the variant to `DialogueCondition` — no structural changes to the dialogue system needed.
- **`{npc_name}` substitution in body text**: the `speaker` field is a static string. If the designer wants the body to include the NPC's display name, they write it directly. Dynamic name substitution is not in v1.
- **Dialogue during combat**: no lock-out. If a designer wants dialogue blocked during a hostile encounter, they wire a `GameVariables` check (`HasVariable { key: "in_combat", value: "true" }`) on the interactable or add a condition to the opening choice.
- **Multiple concurrent dialogues**: not supported. `StartDialogue` while another is active replaces it (with a warning). Multiplayer concurrent dialogues are Tier 2 / networking scope.
- **Localisation**: `body` and `label` are plain strings in v1. A future pass could make these keys into a string table. Design the schema so `body` is a plain string — localisation would swap it out at load time via a pre-processor, not a runtime system.

---

## Acceptance criteria

- Given a prefab with `dialogue: Some("dialogues/intro.dialogue.ron")` and `interactable`, pressing the interact key while near the NPC opens the dialogue panel and displays the first node.
- Given a node with `advance_delay_secs: Some(2.0)` and no choices, the panel advances to the next node after 2 seconds.
- Given a choice with `jump_to: Some("lore_node")`, selecting it displays the node with `id: "lore_node"`.
- Given a choice with `jump_to: Some("__end__")`, the panel closes and `dialogue.ended:{path}` is emitted.
- Given a choice with `condition: HasVariable { key: "met_merchant", value: "true" }` and the variable is not set, that choice does not appear.
- Given `do_actions: [EmitEvent("quest.accepted:main_quest")]` on a choice, the event fires through the pipeline when the choice is selected.
- Given `body: "Hello, {self}!"` and `npc_id: "merchant_01"`, the rendered text reads "Hello, merchant_01!".
- Given a scene transition (`LoadScene`), the dialogue panel is hidden and `ActiveDialogue` is cleared.
- Given `StartDialogue` with no `DialoguePanel` node in the scene, a warning is logged and no panel is shown.
