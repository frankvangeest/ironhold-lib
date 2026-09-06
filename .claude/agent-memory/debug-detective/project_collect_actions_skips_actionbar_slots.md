---
name: collect-actions-skips-actionbar-slots
description: validate.rs collect_actions() ingests rules/state_machine/behaviors/dialogues but NOT scene ui ActionBar slot do_actions, so every action-level cross-file check silently skips them
metadata:
  type: project
---

`collect_actions()` in `crates/ironhold_cli/src/commands/validate.rs` builds the `all_actions`
list from exactly four sources: `rules.ron`, `state_machine.ron`, `behaviors/*.behavior.ron`, and
`dialogues/*.dialogue.ron`. It does **not** walk scenes. But `ActionSlotDef.do_actions`
(`schema/scene_v2.rs`, inside a scene's `ui: [ ActionBar((slots: [...])) ]`) is a real
`Vec<Action>` that fires through the normal pipeline.

Consequence: **every** check that iterates `all_actions` — missing scene file, missing dialogue
file, prefab-key/effect-key/audio-key/modifier-key references, `{new_id}` token placement,
`spawn_point` references, and the new `path_case_mismatch` — is blind to any action authored in an
action-bar slot. Reproduced empirically (2026-09-05): a scene with
`ActionBar slots: [ (key: "2", do_actions: [ LoadScene("scenes/does_not_exist_at_all.scene.ron") ]) ]`
validates **exit 0, zero errors**.

Note validate.rs *does* iterate `bar.slots` in several places (~lines 835, 912, 956, 992) — but only
for `key`/`gamepad_key`/cost-stat checks, never for `slot.do_actions`. Seeing those loops makes it
easy to wrongly assume slot actions are covered.

**Why:** the action-bar feature added a second authoring surface for `Action` values without
extending the one collector every action check funnels through.

**How to apply:** whenever reviewing or extending any `all_actions`-driven check in validate.rs,
state explicitly that action-bar slots are out of scope, or fix `collect_actions` to take
`scenes: &[(String, GameSceneV2)]` too. Also treat "the CLI validates every authored X" claims in
commit messages as untrue for action-bar slots until this is fixed. Related:
[[validate-reference-checks-token-blind]], [[reverse-reachability-false-positive-asymmetry]].
