---
name: validate-cross-file-blind-spots
description: Structural blind spots in ironhold_cli validate.rs — hardcoded stats.ron path, try_parse silent-None, convention-glob vs reference-driven discovery, substitution-token false positives, the docs "Checks performed" list, and open dialogue/JoinPlayer gaps
metadata:
  type: project
---

Recurring gaps to check whenever a new `ironhold_cli validate` cross-file check is added
(`crates/ironhold_cli/src/commands/validate.rs`). Established during the
`feature/cli-validate-hardening` review (2026-08-30, verdict ALIGNED).

**A validate-only change is the second-easiest ALIGNED verdict after a runtime-warn diagnostic**
(see [[diagnostic-only-feature-pattern]]): no new schema field, no `ironhold_core` change, purely
additive exit-1 diagnostics. The whole review reduces to "does the check's lookup match what the
runtime actually resolves against, and is the coverage complete?"

**Why:** every check in this file is a hand-written match arm or hand-written catalog walk, so
coverage drifts field-by-field rather than being enforced by the compiler.

**How to apply — the four recurring blind spots:**

1. **`do_validate` hardcodes `"stats/stats.ron"`** while `ProjectConfig.stats_path` is a real
   configurable `Option<String>` (schema/project.rs:~225). Any project with a custom `stats_path`
   gets `stat_catalog == None` → *every* stat-keyed check silently skips. `items_path` is loaded
   correctly from `ProjectConfig` — that is the pattern to copy, not the stats one. Latent only:
   all shipped projects use the convention path.
2. **`try_parse` returns `None` for a missing file with no `FileResult` pushed** (validate.rs:90-100).
   So `items_path: Some("items/itmes.ron")` (typo) → catalog `None` → the check it gates is silently
   skipped *and* no `missing_file` error is reported. Any new configurable-path catalog inherits this.
3. **Scene-path actions are covered piecemeal.** `LoadScene`/`LoadSceneOverlay`/`PreloadScene` are
   checked on disk; **`ToggleOverlay(String)` is also a scene path and is not** (actively used in
   `primitive_world/logic/state_machine.ron:48-49`), and **`ProjectConfig.initial_scene` is never
   checked at all** — the single most load-bearing designer-authored scene path in a project.
   `project_dir.join(path)` is the correct disk check: `resolve_project_path` (scene_manager/mod.rs:734)
   is literally `format!("{project_root}/{path}")`, and no shipped RON interpolates `{var}` into a
   scene path, so no false-positive risk.
4. ~~**`collect_actions` skips dialogue files.**~~ **CLOSED** by `feature/cli-validate-dialogues`
   (2026-09-04): `do_validate` now globs `dialogues/*.dialogue.ron` + `parse_file::<DialogueDef>`,
   and `collect_actions` takes a 4th `dialogues: &[(String, DialogueDef)]` param walking
   `nodes[].choices[].do_actions`. Parse parity with the runtime is genuine — both sides use
   IMPLICIT_SOME (`utils::ron_from_str` vs `ImplicitRonPlugin`). **But `query.rs::collect_logic` is a
   second, parallel walker (rules + state_machine + behaviors only) that was NOT extended** — so
   `query actions`/`query events` stay dialogue-blind and now disagree with `validate` about "all
   the project's actions" (`docs/60_contributing.md:309` enumerates the three old sources).

5. **There are now THREE parallel logic-file walkers**, not two: `validate::collect_actions`,
   `utils::collect_handled_events` (added by `feature/ui_trigger_reachability_check`), and
   `query::collect_logic`. `collect_handled_events` additionally re-reads the files from disk even
   though `do_validate` already has them parsed — so a `rules.ron` parse error yields zero handlers
   and buries the real error under one bogus `unreachable_trigger` per button. See
   [[ui-trigger-reachability-pattern]].

**THE false-positive class for any new string-key check: `{self}`/`{target}`/`{new_id}` substitution.**
Established reviewing `feature/spawn_point_reference_check` (2026-09-04). Before adding a
`contains_key`-style check on a designer-authored string, grep `message_interpreter.rs::rewrite_self`
/`rewrite_target` and `dialogue.rs::substitute_self_in_action` for that field — if the field is
`.replace("{self}", ..)`d there, the check must skip values containing `{`, or it will reject a
*working* project. Behavior FSMs and dialogue `do_actions` are both in `collect_actions`, and both
are exactly where `{self}` interpolation is used. Concrete live example: `Action::Spawn.spawn_point`
is `{self}`-substituted, and `3rd_person_game_demo` already names its points `zombie_01_spawn` /
`snake_01_spawn` — i.e. one behavior rule with `spawn_point: "{self}_spawn"` would collapse its 6
near-identical respawn transitions and then fail `validate`. Only one check in the file handles this
today (`stat_label`/`world_stat_bar`, via `strip_prefix("{self}.")`, validate.rs:~855) — copy that
posture. A false positive is worse than a miss here: a designer with no Rust knowledge cannot
suppress a `validate` error.

**Every new check must be added to the `docs/60_contributing.md` "Checks performed" /
"`--strict` flag" bullet lists (~lines 236-262).** That is the only designer-facing enumeration of
what `validate` catches, and it is otherwise well maintained (label_depth_scale, gamepad_index,
merchant, slope/coyote all present) — but the `Action::SetCameraMode` mode check is already missing
from it, so don't take "the neighbouring check didn't do it" as precedent.

**Sibling gap noted 2026-09-04: `Action::JoinPlayer`'s `spawn_points["player_{next_slot+1}_start"]`
is unchecked** (action_executor.rs:~1753) and its miss path is *even quieter* than
`Action::Spawn.spawn_point`'s — no `warn!` at all, just a silent fall back to the primary player's
position + `1.5 * next_slot` on X. Fully derivable at design time and genuinely scene-scoped (not
union): for each index `i` where `scene.join_prefab_keys[i].is_some()`, require
`scene.spawn_points["player_{i+1}_start"]`.

**Coverage model to keep in mind — convention-glob vs. reference-driven.** `do_validate` discovers
logic files by *convention* (`glob_dir(dir, subdir, suffix)`, non-recursive), but the runtime
resolves whatever project-relative path the designer authored, through a loader registered for
plain `&["ron"]`. So `dialogue: "conversations/npc.ron"` (or `behavior:` likewise) loads fine at
runtime and is never parse-checked, and a nested `dialogues/act1/x.dialogue.ron` is missed too.
Driving the parse pass off the *union* of the glob and the referenced paths
(`PrefabDef.dialogue`/`.behavior`, `Action::StartDialogue.dialogue_path`) would close this class.

**Open dialogue-adjacent gaps as of 2026-09-04** (all cheap, all now unblocked since the parsed
`DialogueDef`s are in hand):
- **`Action::StartDialogue { dialogue_path }` has no on-disk check** despite being the same
  project-relative-path shape as the `LoadScene|LoadSceneOverlay|PreloadScene|ToggleOverlay` arm
  (`resolve_project_path` = `format!("{root}/{path}")`, so `project_dir.join(path).exists()` is
  right). Runtime failure is *total silence* — `dialogue_assets.get() => None => return`, panel
  never opens, no message.
- **`PrefabDef.dialogue` has no on-disk check** even though `PrefabDef.behavior` does, ~6 lines
  above it in the same prefab loop. This is the auto-wire path (`DialoguePath` +
  `entity.interacted:{id}`), i.e. the dominant way dialogues are actually reached.
- **`jump_to` is not cross-checked against `nodes[].id`** (exclude the reserved `"__end__"`).
  Runtime `warn!`s and *closes the conversation mid-flow* — a visible, confusing failure, and the
  textbook runtime-warn/CLI-error twin. Duplicate node `id`s are also unenforced despite the
  "Must be unique" doc comment (`position()` = first wins, later node unreachable).
- **`DialogueCondition::StatAtLeast.stat_key` is unchecked** — identical shape to the
  `MerchantDef.currency_stat` check already in this file, resolving against the same global
  `LoadedStats`/stats.ron catalog; a typo silently hides that choice forever with no warn.
- **No shipped project exercises the dialogue half of `collect_actions`** —
  `3rd_person_game_demo/dialogues/npc_intro.dialogue.ron` has zero `do_actions`, so it's
  fixture-only coverage.

**Correct-lookup verification (the one substantive thing to actually check):** trace where the
runtime resolves the key. `MerchantDef.currency_stat` reads `scene_state.loaded_stats.0`
(action_executor.rs:~1372) = the global stats.ron catalog, **not** a per-player `StatMap` — so
`stat_catalog.stats.contains_key` is right. Had it been player-scoped, checking stats.ron would
false-positive against `stat_templates`-only stats (cf. [[per-player-stat-pools-pattern]]).

**"Union across all scenes" is now an established, twice-used scoping tier** (`SetCameraMode.mode`,
and `Action::Spawn.spawn_point` as of `feature/spawn_point_reference_check`): rules.ron /
state_machine.ron / behaviors are project-scoped but `camera_modes`/`spawn_points` are scene-scoped,
so "defined in scene A, fired only while scene B is active" is a deliberate false negative. Accept
it, but require the tradeoff be stated in a code comment at the loop, as both do. True per-scene
reachability needs `LoadScene`-graph reasoning and stays deferred — same bucket as
`Action::Spawn.at_entity` (which additionally needs live-entity reasoning).

**Scoping:** merchant checks are prefab-catalog-scoped (`source_file: "prefabs/prefabs.ron"`,
iterating `catalog.prefabs`) because `MerchantDef` is a prefab-local condition — same rule as
[[diagnostic-only-feature-pattern]]. Note `MerchantDef` lives at `PrefabDef.merchant`, not
`PrefabDef.components.merchant`.

**Sibling gap left open:** `ItemDef.currency_stat: Option<String>` (schema/items.rs:66) is a second
designer-authored global stat key, unchecked, whose runtime twin is a bare
`warn!("TakeAllFromContainer: currency_stat {:?} not found in stats")` (action_executor.rs:~1485) —
the exact runtime-warn-without-CLI-twin shape this file exists to close.
