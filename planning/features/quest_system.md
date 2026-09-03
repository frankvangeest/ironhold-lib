# Feature: Quest System

_Status: Draft_
_Planned at: `e9a421e` (2026-06-02)_
_Soft deps: Inventory (Collect objectives), Dialogue (accept/turn-in flow), Stat templates (stat rewards — shipped)_

## Phases

| Phase | Backlog item | Status | Completed |
|---|---|---|---|
| v1 | Core loop — catalog, `QuestLog`, objectives, rewards; testable via events alone | Queued | — |
| v2 | Presentation layer — `QuestTracker` UI node, quest-giver nameplate indicator, `DialogueCondition::QuestState` | Queued | — |

---

> ## Pre-implementation checklist
>
> - [ ] **Decide: `KillCount` death detection mechanism.** Options: (a) listen for `entity.died:{spawn_id}` via `EventReader<GameEvent>` — requires knowing the prefab key from the spawn ID; (b) watch `StatMap` health ≤ 0 directly each frame in a quest system. Recommendation: **`EventReader<GameEvent>` with `PrefabKey` component lookup** — the quest system reads all `GameEvent::Trigger` events each frame, pattern-matches `entity.died:*`, resolves the spawn ID to an entity, reads its `PrefabKey` component, and increments the matching `KillCount` objective counter. Requires `PrefabKey` component (designed in inventory feature file) to be in place first.
>
> - [ ] **Decide: `Collect` objective detection.** Listen for `inventory.added:{entity}:{item_key}:{count}` via `EventReader<GameEvent>` — on matching event, check if the player's total held count satisfies the objective. Requires inventory system shipped first.
>
> - [ ] **Decide: `QuestLog` vs. per-entity component.** Quest progress tracks per-player, not per-entity. `QuestLog` is a `Resource` (like `PlayerInventory`), not a component. It persists across scene transitions. Confirm: one global `QuestLog` per session; no per-entity quest tracking in v1.
>
> - [ ] **Decide: auto-complete vs. manual turn-in.** When all objectives are satisfied, two options: (a) `auto_complete: true` fires rewards immediately; (b) `auto_complete: false` (default) enters `ReadyToTurnIn` state — `CompleteQuest` must be called explicitly (typically from a dialogue choice on the quest-giver). Recommendation: **per-quest flag** `auto_complete: bool` — leave the choice to the designer.
>
> - [ ] **Decide: quest catalog file location.** `quests/quests.ron` per project, parallel to `items/items.ron` and `groups/groups.ron`. Confirmed — separate file.
>
> - [ ] **Decide: branching/prerequisite chains for v1.** `prerequisites: Vec<String>` (list of quest keys that must be `Completed` before this quest becomes `Inactive → Available`). A quest in `Inactive` state is not available to `AcceptQuest`. This is linear prerequisite chaining, not full directed graph branching — sufficient for v1.

---

## What

RON-defined quest catalog (`quests/quests.ron`) with objectives, prerequisites, and rewards. `QuestLog` resource tracks per-quest and per-objective progress. The quest system intercepts pipeline events to auto-advance countable objectives (`KillCount`, `Collect`). Trigger-based objectives (`ReachLocation`, `TalkTo`, `Custom`) are event-driven. Rewards dispatch through the existing action executor.

---

## Why

The whole progression loop — explore, fight, gather, talk, be rewarded — requires a quest system to give players structured goals. Without it, the engine can only react to events; it cannot track multi-step goal completion across scenes.

Unblocks: Quest-giver indicator icons (nameplate extension), quest-gated shop access, conditional dialogue choices using quest state, save/load quest progress (v2 save system extension).

---

## Schema

### `quests/quests.ron`

```ron
(
    schema_version: 1,
    quests: {
        "gather_herbs": (
            title: "Gather Medicinal Herbs",
            description: "Collect 5 herbs from the forest.",
            prerequisites: [],
            auto_complete: true,
            objectives: [
                ( id: "collect_herbs", kind: Collect(item_key: "herb", count: 5) ),
            ],
            rewards: [
                GiveItem(item_key: "health_potion", count: 2),
                GiveStat(stat_key: "gold", amount: 50.0),
            ],
        ),
        "clear_bandits": (
            title: "Clear the Bandit Camp",
            description: "Eliminate all bandits threatening the village.",
            prerequisites: ["gather_herbs"],
            auto_complete: false,  // must talk to quest-giver to claim reward
            objectives: [
                ( id: "kill_bandits", kind: KillCount(prefab_key: "bandit_grunt", count: 5) ),
                ( id: "kill_captain", kind: KillCount(prefab_key: "bandit_captain", count: 1) ),
            ],
            rewards: [
                GiveItem(item_key: "iron_sword", count: 1),
                GiveStat(stat_key: "gold", amount: 200.0),
                UnlockQuest("escort_merchant"),
            ],
        ),
        "reach_shrine": (
            title: "Find the Ancient Shrine",
            prerequisites: [],
            auto_complete: true,
            objectives: [
                ( id: "find_it", kind: ReachLocation(trigger_zone_id: "shrine_zone") ),
            ],
            rewards: [ GiveStat(stat_key: "max_health", amount: 10.0) ],
        ),
    },
)
```

### New types (`schema/quests.rs`)

```rust
#[derive(Deserialize, Asset, TypePath, Debug, Clone)]
pub struct QuestCatalog {
    pub schema_version: u32,
    pub quests: HashMap<String, QuestDef>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct QuestDef {
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub prerequisites: Vec<String>,
    #[serde(default)]
    pub auto_complete: bool,
    pub objectives: Vec<QuestObjectiveDef>,
    #[serde(default)]
    pub rewards: Vec<QuestReward>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct QuestObjectiveDef {
    pub id: String,
    pub kind: QuestObjectiveKind,
    /// Optional display hint (e.g. "Kill 3/5 bandits"). Default: derived from kind.
    #[serde(default)]
    pub display: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
pub enum QuestObjectiveKind {
    /// Kill N entities with the given prefab key.
    KillCount { prefab_key: String, count: u32 },
    /// Hold at least N of an item in the player's inventory.
    Collect { item_key: String, count: u32 },
    /// Player enters the named trigger zone.
    ReachLocation { trigger_zone_id: String },
    /// A dialogue with the given NPC path ends.
    TalkTo { dialogue_path: String },
    /// Satisfied when the given event pattern fires. Supports prefix wildcards.
    Custom { event_pattern: String },
}

#[derive(Deserialize, Debug, Clone)]
pub enum QuestReward {
    GiveItem { item_key: String, #[serde(default = "one")] count: u32 },
    GiveStat { stat_key: String, amount: f32 },
    UnlockQuest(String),      // move named quest from Inactive → Available
    RunActions(Vec<Action>),  // arbitrary pipeline actions
}
```

### `ProjectConfig` addition

```ron
quests_path: Some("quests/quests.ron"),
```

### `PrefabDef` addition (`schema/catalog.rs`)

```ron
"merchant_npc": (
    // ...
    quest_giver: Some([         // NEW — quests this NPC offers/accepts
        "gather_herbs",
        "clear_bandits",
    ]),
)
```

```rust
// schema/catalog.rs — in PrefabDef
/// Quest keys this entity offers/accepts as a quest giver.
#[serde(default)]
pub quest_giver: Option<Vec<String>>,
```

---

## Runtime

### `QuestLog` resource (`capabilities/quests.rs`)

```rust
#[derive(Resource, Default)]
pub struct QuestLog {
    pub quests: HashMap<String, QuestProgress>,
}

pub struct QuestProgress {
    pub state: QuestState,
    pub objective_progress: HashMap<String, u32>,  // obj_id → current count
}

#[derive(Debug, Clone, PartialEq)]
pub enum QuestState {
    Inactive,        // prerequisites not met; not available
    Available,       // prerequisites met; can be accepted
    Active,
    ReadyToTurnIn,   // all objectives done, auto_complete: false
    Completed,
    Failed,
    Abandoned,
}
```

`QuestLog` persists across scene transitions (not cleared on `LoadScene`).

### `quest_event_system` (`capabilities/quests.rs`)

Runs in `Update`. Reads `EventReader<GameEvent>`. For each `GameEvent::Trigger(event_name)`:

1. **`entity.died:{spawn_id}`** — resolve entity via `SpawnRegistry`, read `PrefabKey` component. For each `Active` quest with a `KillCount { prefab_key, count }` objective whose `prefab_key` matches: increment progress. If progress ≥ count: mark objective complete, emit `quest.objective_complete:{quest_key}:{obj_id}`.
2. **`inventory.added:{entity}:{item_key}:{count}`** — for each `Active` quest with `Collect { item_key: k, count: n }` where `k` matches: check current player inventory count. If ≥ n: mark complete.
3. **`entity.entered:{zone_id}`** — for each `Active` quest with `ReachLocation { trigger_zone_id: z }` where `z` matches: mark complete.
4. **`dialogue.ended:{path}`** — for each `Active` quest with `TalkTo { dialogue_path }` where path matches: mark complete.
5. **Custom**: regex/prefix match on event name.

After any objective completes, check if all objectives are complete. If so:
- `auto_complete: true` → dispatch rewards, transition to `Completed`, emit `quest.completed:{key}`.
- `auto_complete: false` → transition to `ReadyToTurnIn`, emit `quest.ready_to_turn_in:{key}`.

### New actions (`schema/actions.rs`)

```ron
AcceptQuest("gather_herbs")
AbandonQuest("gather_herbs")
CompleteQuest("clear_bandits")       // manual turn-in; dispatches rewards + transitions to Completed
FailQuest("gather_herbs")
CompleteObjective(quest_key: "clear_bandits", obj_id: "kill_bandits")  // manual force
```

```rust
AcceptQuest(String),
AbandonQuest(String),
CompleteQuest(String),
FailQuest(String),
CompleteObjective { quest_key: String, obj_id: String },
```

### `AcceptQuest` executor logic

1. Verify quest is in `Available` state. If `Inactive`, warn and no-op.
2. Set state to `Active`. Init all objective progress counters to 0.
3. Emit `quest.accepted:{key}`.

### Reward dispatch

```rust
fn dispatch_rewards(quest_key: &str, def: &QuestDef, action_queue: &mut ActionQueue, /* ... */) {
    for reward in &def.rewards {
        match reward {
            QuestReward::GiveItem { item_key, count } =>
                action_queue.push(Action::AddItem { entity: "player".into(), item_key: item_key.clone(), count: *count }),
            QuestReward::GiveStat { stat_key, amount } =>
                action_queue.push(Action::ModifyStat { key: stat_key.clone(), delta: *amount }),
            QuestReward::UnlockQuest(key) => {
                if let Some(p) = quest_log.quests.get_mut(key) {
                    if p.state == QuestState::Inactive { p.state = QuestState::Available; }
                }
            }
            QuestReward::RunActions(actions) =>
                for a in actions { action_queue.push(a.clone()); }
        }
    }
}
```

### New pipeline events

```ron
quest.accepted:{key}
quest.objective_progress:{key}:{obj_id}:{count}    // emitted on each progress increment
quest.objective_complete:{key}:{obj_id}
quest.ready_to_turn_in:{key}           // auto_complete: false, all objectives done
quest.completed:{key}
quest.failed:{key}
quest.abandoned:{key}
```

---

## Quest-giver indicator (nameplate extension)

When an entity has `quest_giver: Some([...])`, the nameplate system reads `QuestLog` to determine the indicator icon:
- Any offered quest is `Available` + not yet `Active` → yellow `!` (quest available)
- Any accepted quest is `ReadyToTurnIn` → yellow `?` (turn in available)
- All offered quests are `Completed` → no indicator

This is a small extension to the nameplate system added when the quest system ships.

### `QuestTracker` UI node

```ron
( id: "quest_tracker", kind: QuestTracker((
    position: (10.0, 10.0),
    max_visible: 3,
    font_size: 12.0,
    show_objectives: true,
))),
```

Renders a live list of `Active` quests with title and objective progress (e.g. "Kill Bandits: 2/5").

---

## Tasks

- [ ] Decisions from pre-implementation checklist resolved
- [ ] `schema/quests.rs` — `QuestCatalog`, `QuestDef`, `QuestObjectiveDef`, `QuestObjectiveKind`, `QuestReward`
- [ ] `quests_path` in project config; `quest_giver: Option<Vec<String>>` on `PrefabDef`
- [ ] `QuestLog` + `QuestProgress` + `QuestState` resource; NOT cleared on `LoadScene`
- [ ] `PrefabKey` component dependency confirmed (from inventory feature)
- [ ] `quest_event_system` — death/collect/location/talk/custom detection
- [ ] `AcceptQuest`, `AbandonQuest`, `CompleteQuest`, `FailQuest`, `CompleteObjective` actions
- [ ] Reward dispatch: `GiveItem`, `GiveStat`, `UnlockQuest`, `RunActions`
- [ ] Prerequisite enforcement in `AcceptQuest`
- [ ] Auto-complete vs. `ReadyToTurnIn` logic
- [ ] `QuestTracker` UI node variant + update system
- [ ] Quest-giver nameplate indicator (extension patch to nameplate system)
- [ ] `DialogueCondition::QuestState` + `ObjectiveProgress` patch to dialogue system
- [ ] Pipeline events: all `quest.*` variants
- [ ] Demo: two-quest chain in `3rd_person_game_demo` — gather quest auto-complete, bandit quest with NPC turn-in
- [ ] Integration tests: accept/progress/complete/reward cycle; prerequisite blocks accept; `ReadyToTurnIn` does not fire rewards until `CompleteQuest`; `QuestLog` persists across scene load
- [ ] Docs: `quests.ron` format, `quest_giver`, `QuestTracker`, all quest actions + events

---

## Acceptance criteria

- Given `AcceptQuest("gather_herbs")`, state transitions to `Active` and `quest.accepted:gather_herbs` is emitted.
- Given `AcceptQuest` on a quest with an unmet prerequisite, a warning is logged and state stays `Inactive`.
- Given `KillCount(prefab_key: "bandit_grunt", count: 5)` and 5 bandits killed, `quest.objective_complete` fires and (with `auto_complete: true`) rewards are granted and `quest.completed` fires.
- Given `auto_complete: false`, reaching all objectives emits `quest.ready_to_turn_in`; rewards fire only after `CompleteQuest`.
- Given `UnlockQuest("escort_merchant")` reward, the `escort_merchant` quest transitions from `Inactive` to `Available`.
- Given a scene transition, `QuestLog` retains all quest states and progress.
