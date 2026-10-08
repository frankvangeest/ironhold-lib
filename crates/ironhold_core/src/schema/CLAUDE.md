# schema/ — RON data types

Rules for editing `crates/ironhold_core/src/schema/`. Loaded when you touch a file in this folder; the
crate-wide rules are in `../CLAUDE.md`. Designer-facing reference for every type here is
`docs/20_data_formats.md`.

<!-- b:35 -->
## Adding or changing an `Action`, and the `deny_unknown_fields` consequences

Add the variant to `actions.rs` with a doc comment, add its arm to `action_executor_system`
(`runtime/scene_manager/action_executor.rs`; see that folder's `CLAUDE.md`), and document it where it has
non-obvious semantics (`docs/20_data_formats.md`).

`Action` carries `#[serde(deny_unknown_fields)]`, so for a new struct variant:
- **Never use `#[serde(flatten)]`** on a field (incompatible with `deny_unknown_fields`); duplicate shared fields
  rather than flattening a shared struct in.
- **Renaming or removing a field on an existing variant is a hard break** for every project's RON, not a silent
  default: bump `schema_version` and document the migration ("Schema evolution" in `docs/20_data_formats.md`).

The same attribute is on every FSM container an `Action` lives in (`StateMachineAsset`, `FsmState`,
`FsmTransition`, `FsmEventBinding`) and on the dialogue schema (`DialogueDef`, `DialogueNodeDef`,
`DialogueChoiceDef`, `DialogueCondition`): a typo anywhere in that chain is a parse error, not an empty action list.

<!-- b:54 -->
## RON syntax follows the variant shape

A struct variant (`SpawnEffect { key, entity, position }`) takes named fields: `SpawnEffect(key: "hit_spark",
entity: "{self}")`. A tuple variant (`SetVariable(String, String)`) takes positional values:
`SetVariable("score", "0")`; named fields on a tuple variant are a parse error. Check the variant in `actions.rs`.

## `{self}` / `{target}` substitution has four sites

A new `Action` field that holds an entity id, an event name or a spawn id must be handled at **all four** places,
or `{self}`/`{target}` stays a literal string in that field with no error:
`rewrite_self` and `rewrite_target` (`runtime/scene_manager/action_substitution.rs`), `substitute_self_in_action`
(`capabilities/dialogue.rs`, for dialogue-choice actions) and `action_needs_target` (`capabilities/action_bar.rs`,
decides whether an action-bar slot needs a target). `{new_id}` is the one exception: it exists only on
`Spawn.id` and is resolved by the executor, not here (see `runtime/scene_manager/CLAUDE.md`).

<!-- b:108.ref -->
## `scene.ui` is walked with `walk_ui_nodes`

(The one-line rule is in the crate parent `../CLAUDE.md`; this is the detail.) Use
`schema::scene_v2::walk_ui_nodes`, or `walk_ui_nodes_pathed` for diagnostics, and never loop the top-level `Vec`
flat: a flat loop silently misses any nested `ui:` node (a `StatRadar` inside a `Group` gets no material, an
`ActionBar` escapes every duplicate-key warning). The walker is pre-order and depth-capped at `MAX_UI_DEPTH` (16;
a top-level node is depth 1), so diagnostics cover exactly the nodes that spawn. It yields nodes only: `.enumerate()`
it if you need a per-node tag. The one deliberate exception is the spawn loops in `scene_loader.rs`, which recurse
structurally. See `planning/features/ui_flex_group.md`.

<!-- b:1213 -->
## A new rendering `PrefabDef` field must be checked against the player path

`PrefabDef.material` is not applied automatically to players. `spawn_prefab_instance` (the generic
Actor/Prop/NPC path) reads `prefab.material` and inserts `PendingMaterialOverride`, but `spawn_player_entity_core`
is separate and needs `PlayerConfig.material`, forwarded by `assemble_player_config`. Any future `PrefabDef` field
meant to affect rendering or visuals must be forwarded the same way, for GLB and primitive players alike (see the
player-construction sites in `docs/dev/player-spawn-sites.md`).

<!-- b:1589 -->
## Duplicate `gamepad_index` / `player_index` checks are scoped to `entities:` on purpose

Do not widen them. `warn_duplicate_gamepad_index` (`scene_loader.rs`) and the matching `ironhold_cli validate`
error cover only the scene's instantiated `entities:`, not the prefab catalog: `local_coop_demo` legitimately
reuses a `gamepad_index` across rooms' player variants that are never co-instantiated. The sibling
`duplicate_player_index` (`validate --strict`) is also `entities:`-only and is deliberately not extended to
`join_prefab_keys`: `Action::JoinPlayer` overwrites a hot-joined player's `player_index` with the join slot, so a
join prefab's authored value is dead data and checking it would false-positive on prefabs reused across slots.

<!-- b:769 -->
## `ProjectConfig.max_frame_delta_secs`

`ProjectConfig.max_frame_delta_secs` (`project.rs`) caps how many catch-up ticks one frame's `FixedUpdate` may run
after a genuine stall (asset load, GC pause, shader hitch). Lowering it bounds the worst case after a hitch; it does
not reduce the routine 60Hz/144Hz multi-tick frames. Authoring reference: `docs/20_data_formats.md#max_frame_delta_secs`;
measurement: `planning/investigations/fixed_timestep_max_delta.md`.
