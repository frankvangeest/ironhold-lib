# Memory Index

- [Docs lag the action schema](project_docs_lag_actions.md) — docs/20_data_formats.md, docs/30_runtime_events_and_logic.md, docs/STATUS.md consistently miss new Action variants when added
- [pkg/ web build must be rebuilt](project_pkg_rebuild_required.md) — staged schema/action changes do not reach designers until wasm-pack build + commit of pkg/
- [Color tuples vary RGB vs RGBA](project_color_tuple_inconsistency.md) — DamagePopupStyle uses 3-tuple RGB while StatLabelDef/WorldStatBarDef use 4-tuple RGBA in the same prefab block
- [{self} substitution pattern](project_self_substitution_pattern.md) — Entity-targeted actions accept {self} in .behavior.ron; canonical example is primitive_world/behaviors/attack_dummy.behavior.ron
