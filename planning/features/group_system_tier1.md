# Feature: Group System — Tier 1 (Factions, Teams, Parties)

_Status: Draft_
_Planned at: `fcc53aa` (2026-06-02)_

---

> ## Pre-implementation checklist
>
> - [ ] **Decide: membership on entity, global resource, or both.** Options: (a) entity-only — `GroupMembership` component, no global list; (b) global-only — `LoadedGroups` resource holds member entity IDs; (c) both — component for fast per-entity queries, resource for fast group-wide queries (e.g. "how many members in this group?"). Recommendation: **both** — the component is queried by AI systems and stance checks each frame; the resource is updated lazily on membership changes and queried by wave/encounter systems for group-size events. The resource does NOT hold `Entity` handles (they go stale on despawn) — it holds `SpawnId` strings.
>
> - [ ] **Decide: group catalog location.** Options: (a) new `groups.ron` per project; (b) `groups` map in `assets.ron`. Recommendation: **separate `groups.ron`** — group definitions are gameplay/faction data, not asset references; keeping them separate avoids further inflating `assets.ron` and allows the CLI validator to give clearer error messages scoped to group data.
>
> - [ ] **Decide: stance between groups.** The existing `NpcFaction` (Friendly/Hostile/Neutral) is per-NPC, hardcoded in the prefab. The group system should generalize this. Options: (a) stance declared per `GroupDef` as a list of `(other_group, stance)` pairs; (b) a separate `stances` block in `groups.ron` with `(a, b, stance)` triples. Recommendation: **`stances` block in `groups.ron`** — avoids circular references (A declares stance toward B, and B declares it back inconsistently); a shared neutral table is easier to read.
>
> - [ ] **Decide: relationship to existing `NpcFaction`.** Both will coexist for a transition period. Targeting system and nameplate system use `NpcFaction` as a v1 approximation. When Group system ships, they should switch to `get_stance()`. Plan: add a `group_id: Option<String>` field on `NpcDef` — when set, the NPC uses group stance for targeting; when `None`, falls back to `NpcFaction`. Deprecate `NpcFaction` but do not remove it in this milestone.
>
> - [ ] **Decide: `max_members` enforcement.** When a group is full and `AddToGroup` fires: (a) silently no-op; (b) log a warning; (c) emit `group.full:{id}` and no-op. Recommendation: **emit `group.full:{id}` and no-op** — gives designers a hook to react (e.g., spawn a replacement group, trigger difficulty scaling). `max_members: 0` = unlimited.

---

## What

A generic RON-defined group system for factions, combat teams, and NPC parties. Groups are declared in `groups.ron` with membership actions and stance rules. Any entity can belong to one or more groups. `get_stance(a, b)` answers "how does group A regard group B?" — used by AI, targeting, and nameplate filtering.

Tier 1 covers: group definitions, membership management, stance lookup, and the membership event pipeline. Tier 2 (guild hierarchy, chat channels, raid roles) is deferred to the Beta 0.6 networking milestone.

---

## Why

`NpcFaction` is hard-wired per-prefab and only has three values. It cannot express "the player's party", "a neutral faction that becomes hostile after a quest", or "team red vs. team blue in an arena". The group system replaces all of these with one composable mechanism.

Unblocks: proper faction filter in targeting system and nameplates, quest-gated faction changes, arena team logic, party-based buff routing.

---

## Schema

### New `groups.ron` per project

```ron
// groups/groups.ron
(
    schema_version: 1,
    groups: {
        "enemies":  ( kind: Faction, max_members: 0, default_stance: Hostile ),
        "player_party": ( kind: Party,  max_members: 4, default_stance: Friendly ),
        "merchants": ( kind: Faction, max_members: 0, default_stance: Neutral ),
        "arena_red": ( kind: Team,    max_members: 5, default_stance: Neutral ),
        "arena_blue": ( kind: Team,   max_members: 5, default_stance: Neutral ),
    },
    stances: [
        // (from_group, to_group, stance) — asymmetric stances are allowed
        ( from: "enemies",    to: "player_party", stance: Hostile ),
        ( from: "player_party", to: "enemies",    stance: Hostile ),
        ( from: "arena_red",  to: "arena_blue",   stance: Hostile ),
        ( from: "arena_blue", to: "arena_red",    stance: Hostile ),
        // merchants → player: Neutral (covered by default_stance)
    ],
)
```

### New structs (`schema/groups.rs`)

```rust
#[derive(Deserialize, Asset, TypePath, Debug, Clone)]
pub struct GroupCatalog {
    pub schema_version: u32,
    pub groups: HashMap<String, GroupDef>,
    #[serde(default)]
    pub stances: Vec<StanceEntry>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct GroupDef {
    pub kind: GroupKind,             // Faction | Team | Party
    /// Maximum members. 0 = unlimited.
    #[serde(default)]
    pub max_members: u32,
    /// Stance this group takes toward any group not listed in `stances`.
    #[serde(default)]
    pub default_stance: GroupStance,
}

#[derive(Deserialize, Debug, Clone, Default)]
pub enum GroupKind {
    #[default]
    Faction,
    Team,
    Party,
}

#[derive(Deserialize, Debug, Clone, Default, PartialEq)]
pub enum GroupStance {
    Hostile,
    #[default]
    Neutral,
    Friendly,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct StanceEntry {
    pub from: String,
    pub to: String,
    pub stance: GroupStance,
}
```

### `NpcDef` addition (`schema/catalog.rs`)

```ron
// prefabs/prefabs.ron
"orc_enemy": (
    kind: "actor",
    // ...
    components: (
        npc: Some((
            faction: Hostile,       // kept for backwards compatibility
            group_id: Some("enemies"),  // NEW — when set, stance queries use LoadedGroups
            // ...
        )),
    ),
)
```

```rust
// schema/catalog.rs — in NpcDef
/// When set, this NPC's targeting and stance use the Group system.
/// Overrides `faction` for all group-aware queries. `faction` is kept for backwards compat.
#[serde(default)]
pub group_id: Option<String>,
```

### Project config addition

```ron
// {name}.project.ron
groups_path: Some("groups/groups.ron"),
```

---

## Runtime

### Resources (`capabilities/groups.rs` or `runtime/scene_manager/mod.rs`)

```rust
/// Loaded from groups.ron. None when project has no groups file.
#[derive(Resource, Default)]
pub struct LoadedGroups(pub Option<GroupCatalog>);

/// Global membership index — SpawnId strings per group.
/// Cleared on LoadScene; rebuilt by AddToGroup / RemoveFromGroup actions.
#[derive(Resource, Default)]
pub struct GroupMembership(pub HashMap<String, Vec<String>>);  // group_id → [spawn_ids]
```

### ECS component

```rust
/// On any entity that belongs to one or more groups.
#[derive(Component, Default)]
pub struct GroupMember {
    pub group_ids: Vec<String>,
}
```

### Stance query helper (`capabilities/groups.rs`)

```rust
/// Returns the stance that `from_group` has toward `to_group`.
/// Checks explicit stances first; falls back to `from_group.default_stance`.
pub fn get_stance(
    from_group: &str,
    to_group: &str,
    catalog: &GroupCatalog,
) -> GroupStance {
    for entry in &catalog.stances {
        if entry.from == from_group && entry.to == to_group {
            return entry.stance.clone();
        }
    }
    catalog.groups
        .get(from_group)
        .map(|g| g.default_stance.clone())
        .unwrap_or(GroupStance::Neutral)
}
```

### `AddToGroup` executor arm

```rust
Action::AddToGroup { entity, group_id } => {
    if let Some(catalog) = &groups_catalog.0 {
        if let Some(def) = catalog.groups.get(&group_id) {
            let members = membership.0.entry(group_id.clone()).or_default();
            // Enforce max_members
            if def.max_members > 0 && members.len() >= def.max_members as usize {
                game_events.write(GameEvent::Trigger(format!("group.full:{}", group_id)));
                return;
            }
            if !members.contains(&entity) {
                members.push(entity.clone());
            }
            // Also update GroupMember component on the entity
            if let Some(ecs_entity) = registry.entities.get(&entity) {
                commands.entity(*ecs_entity).entry::<GroupMember>()
                    .or_default()
                    .and_modify(|m| { if !m.group_ids.contains(&group_id) { m.group_ids.push(group_id.clone()); }});
            }
            game_events.write(GameEvent::Trigger(
                format!("group.joined:{}:{}", group_id, entity)
            ));
        }
    }
}
```

---

## New actions (`schema/actions.rs`)

```ron
AddToGroup(entity: "orc_01", group_id: "enemies")
RemoveFromGroup(entity: "orc_01", group_id: "enemies")
DisbandGroup("arena_red")       // removes all members; emits group.disbanded:{id}
```

```rust
AddToGroup { entity: String, group_id: String },
RemoveFromGroup { entity: String, group_id: String },
DisbandGroup(String),
```

---

## New pipeline events

```ron
group.joined:{group_id}:{entity_id}   // entity added to group
group.left:{group_id}:{entity_id}     // entity removed from group
group.full:{group_id}                 // AddToGroup blocked by max_members
group.disbanded:{group_id}            // DisbandGroup fired
group.empty:{group_id}                // last member left the group
```

---

## Integration with existing systems

### Targeting system (nameplate + tab targeting)

Replace the `has NpcAgent` v1 faction approximation with a group-based query:

```rust
// Was: entity has NpcAgent with faction Hostile
// Now:
fn is_hostile_to_player(entity: &GroupMember, player_group: &str, catalog: &GroupCatalog) -> bool {
    entity.group_ids.iter().any(|g| get_stance(g, player_group, catalog) == GroupStance::Hostile)
}
```

The player entity should be added to `"player_party"` group at spawn time (or via `AddToGroup` on `scene.ready`).

### NPC AI (`capabilities/npc.rs`)

`npc_movement_system` currently checks `NpcFaction::Hostile` to decide whether to chase the player. When `NpcDef.group_id` is set, substitute the `get_stance()` check.

---

## Worked example — quest-triggered faction change

```ron
// groups/groups.ron
stances: [
    ( from: "bandits", to: "player_party", stance: Hostile ),
    ( from: "player_party", to: "bandits", stance: Hostile ),
]

// logic/rules.ron
( on: "quest.completed:bandit_truce",
  do_actions: [ /* There is no direct "change stance" action — instead rebuild groups */ ] ),
```

Dynamic stance changes are handled by transitioning entities between groups:

```ron
// When truce quest completes: move bandits to "reformed" group (which has Neutral stance)
( on: "quest.completed:bandit_truce", do_actions: [
    RemoveFromGroup(entity: "bandit_chief", group_id: "bandits"),
    AddToGroup(entity: "bandit_chief", group_id: "reformed_bandits"),
] ),
```

This approach avoids mutable stance tables — stance is always derived from the group catalog (immutable). Dynamic changes are achieved by moving entities between groups.

---

## New Rust changes

- `schema/groups.rs` (new file) — `GroupCatalog`, `GroupDef`, `GroupKind`, `GroupStance`, `StanceEntry`.
- `schema/catalog.rs` — add `group_id: Option<String>` to `NpcDef`.
- `schema/project.rs` (or project config struct) — add `groups_path: Option<String>`.
- `capabilities/groups.rs` (new file) — `GroupMember` component, `LoadedGroups`, `GroupMembership`, `get_stance()`.
- `runtime/scene_manager/project_loader.rs` — load `groups.ron` if `groups_path` is set.
- `runtime/scene_manager/action_executor.rs` — handle `AddToGroup`, `RemoveFromGroup`, `DisbandGroup`.
- `runtime/scene_manager/mod.rs` — clear `GroupMembership` on `LoadScene`; init `GroupMember` for player entity on scene load.
- `capabilities/npc.rs` — use `get_stance()` when `NpcDef.group_id` is set.
- `capabilities/mod.rs` — register module.

---

## Tasks

- [ ] Decisions from pre-implementation checklist resolved
- [ ] `schema/groups.rs` — `GroupCatalog` + all supporting types
- [ ] `groups_path: Option<String>` on project config; loaded by `project_loader`
- [ ] `LoadedGroups` + `GroupMembership` resources; cleared on `LoadScene`
- [ ] `GroupMember` component
- [ ] `get_stance()` helper
- [ ] `AddToGroup`, `RemoveFromGroup`, `DisbandGroup` actions + executor arms
- [ ] Pipeline events: `group.joined`, `group.left`, `group.full`, `group.disbanded`, `group.empty`
- [ ] `group_id: Option<String>` on `NpcDef`; NPC AI stance check updated
- [ ] Targeting system: replace `has NpcAgent` with `get_stance()` when `LoadedGroups` is `Some`
- [ ] Nameplate system: same stance update
- [ ] Demo: add `groups.ron` to `3rd_person_game_demo`; wire `AddToGroup` on scene ready and a quest-truce pattern
- [ ] Integration tests: `AddToGroup` adds to resource and component; `RemoveFromGroup` removes; max_members emits `group.full`; `get_stance` returns correct value; hostile NPC chases player via group stance
- [ ] Docs: `groups.ron` schema in `docs/20_data_formats.md`; group actions + events in `docs/30_runtime_events_and_logic.md`

---

## Open questions

- **Player group initialisation**: should the player always be in a `"player_party"` group, or is it opt-in per scene? Opt-in via `AddToGroup` in `scene.ready` rules gives the designer control; mandatory auto-join is simpler but may be unexpected for non-RPG projects.
- **Multi-group stance conflict**: if an entity belongs to `"enemies"` (Hostile toward player) and `"truce_faction"` (Friendly), which stance wins? v1: most-hostile stance wins (conservative / safe). Document this rule.
- **Saving group membership**: `GroupMembership` is not persisted by `SaveGame` v1 — groups reset on scene load. For v2 save, add `group_memberships` to `SaveState`. Document as a known limitation.
- **Tier 2 scope**: guild hierarchy, chat channels, raid roles — explicitly out of scope and tied to the Beta 0.6 networking milestone. Do not add these fields to `GroupDef` as placeholders.

---

## Acceptance criteria

- Given `groups.ron` with two groups and a stance entry, `get_stance("enemies", "player_party")` returns `Hostile`.
- Given `AddToGroup(entity: "orc_01", group_id: "enemies")`, `GroupMembership.0["enemies"]` contains `"orc_01"` and the entity has `GroupMember.group_ids = ["enemies"]`.
- Given `max_members: 2` and two members already added, a third `AddToGroup` emits `group.full:enemies` and does not add the entity.
- Given `DisbandGroup("arena_red")`, all members are removed, `group.disbanded:arena_red` is emitted, and all formerly-member entities have their `GroupMember.group_ids` updated.
- Given `NpcDef.group_id: Some("enemies")` and a `stances` entry marking enemies as `Hostile` toward `"player_party"`, the NPC chases the player.
- Given a scene transition (`LoadScene`), `GroupMembership` is cleared.
