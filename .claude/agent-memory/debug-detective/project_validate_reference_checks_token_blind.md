---
name: project-validate-reference-checks-token-blind
description: "validate.rs's action reference checks compare raw authored strings against catalogs, so any field that is also a {self}/{target} substitution target turns a legitimate template into a hard exit-1 error; only the {new_id} check is token-aware"
metadata:
  type: project
---

Every `missing_reference` check in `ironhold_cli`'s `cross_file_checks` compares the **raw authored
string** against a catalog/map. That is safe only for fields the interpreters never rewrite.

Fields validated today that are **not** substitution targets (safe): `SpawnEffect.key`,
`ProjectDecal.key`, `PlaySound`/`PlayMusicLoop.key`, `Spawn.prefab`, `PreloadPrefab`,
`SetCameraMode.mode` (absent from both `rewrite_self` and `rewrite_target` — falls through
`other => other`).

Fields that **are** substitution targets and would false-positive if validated raw:
`Spawn.spawn_point`, `Spawn.id`, `Spawn.at_entity`, `Despawn`, `SetDespawnTimer.entity`,
`PlayAnimationOn.target`, `ResetToSpawn`, `ModifyStat`/`SetStat.key`,
`AddItem`/`RemoveItem`/`TransferItem.entity`, `OpenShop`/`OpenContainer`, `EmitEvent`.
`{self}` is pervasive in `assets/projects/*/behaviors/*.behavior.ron`, which `collect_actions`
does collect — so the raw string reaching a check really can be `"{self}_spawn"`.

The one existing token-aware check is the `{new_id}` misplacement check (`validate.rs`, in the same
`for (source, action)` loop): it explicitly reasons about which `Action::Spawn` field a token
resolves in. Mirror that, not the `SetCameraMode` check, when adding a reference check on any field
in the second list — a token guard (`if s.contains('{') { continue }`, or better, resolve the
literal prefix/suffix) is required.

**Why:** the checks were each added by copying the previous one, and the copied template
(`SetCameraMode`) happens to sit on a non-substituted field, so the hazard is invisible at the copy
site.

**How to apply:** whenever a diff adds a `contains_key(...)`/`.any(...)` lookup on a string pulled
out of an `Action` in `validate.rs`, grep `rewrite_self`/`rewrite_target`
(`runtime/scene_manager/message_interpreter.rs`) and `substitute_self_in_action`
(`capabilities/dialogue.rs`) for that field name before accepting it.

Related: [[project_self_target_substitution_coverage]], [[project_action_ron_typos_are_silent]].
