# Memory Index — alignment-reviewer

- [Recurring anti-patterns in ironhold_core](recurring_anti_patterns.md) — repeated designer-reachability blockers found during full-core review; consult before reviewing changes to capabilities/, runtime/, or schema/
- [UiMaterial-driven UI node pattern](uimaterial_ui_node_pattern.md) — five-touchpoint pattern (schema, capability, SceneMaterialParams, scene_loader, lib.rs) that StatRadar/terrain follow; consult when reviewing new shader-backed UI nodes
- [Entity-targeted action pattern](entity_targeted_action_pattern.md) — six-touchpoint checklist for new Action variants that reference an entity by spawn ID; rewrite_self omission is the most common silent break
