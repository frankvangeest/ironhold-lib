# Memory Index — alignment-reviewer

- [Recurring anti-patterns in ironhold_core](recurring_anti_patterns.md) — repeated designer-reachability blockers found during full-core review; consult before reviewing changes to capabilities/, runtime/, or schema/
- [UiMaterial-driven UI node pattern](uimaterial_ui_node_pattern.md) — five-touchpoint pattern (schema, capability, SceneMaterialParams, scene_loader, lib.rs) that StatRadar/terrain follow; consult when reviewing new shader-backed UI nodes
- [Entity-targeted action pattern](entity_targeted_action_pattern.md) — six-touchpoint checklist for new Action variants that reference an entity by spawn ID; rewrite_self omission is the most common silent break
- [Particle quality/budget pattern](particle_quality_budget_pattern.md) — six-touchpoint checklist for global-state Actions that mutate persistent resources, plus backward-compat rules for adding fields to EffectDef/LayerDef
- [PrefabDef markers need all 3 spawn paths](prefab_marker_three_spawn_paths.md) — spawn-time marker fields wired only into spawn_prefab_instance silently break primitive/composite prefabs; grep scene_loader.rs for the field
- [Targeting capability + {target} pattern](targeting_capability_pattern.md) — click/Tab selection, screen-space not raycast, 3-spawn-path markers, and the SetTarget-vs-capability GameVariable asymmetry footgun
- [Audio state pattern](audio_state_pattern.md) — AudioConfig/AudioState/SetVolume/ToggleMute six touchpoints; two project_loader insert sites; dual-write-to-GlobalVolume footgun
- [NPC GLB Actor capsule pattern](npc_glb_actor_pattern.md) — components.npc works on GLB Actors via entity_spawner.rs; capsule dims now data-driven (NpcDef.collider_radius/height); npc.rs emits GameEvent not ActionQueue (correct)
- [stat_overrides flow](stat_overrides_pattern.md) — SceneEntityDef.stat_overrides correctly covers all 3 non-player spawn paths (positive reference); StatMap-build is triplicated (refactor candidate)
- [WorldLabel stat UI pattern](world_label_stat_ui_pattern.md) — stat_label/world_stat_bar dynamic-spawn route via DynamicStatUiQueue; depth_scale:None on dynamic spawns is accepted (popup precedent); widget-spawn block triplicated
