---
name: validate-cross-file-blind-spots
description: Structural blind spots in ironhold_cli validate.rs — the 4 configurable catalog paths + load_configured_catalog fallback divergence, source_file-literal rule, try_parse silent-None, convention-glob discovery, substitution-token false positives, the docs "Checks performed" list, the full inventory of RON-authored disk paths (which are checked vs not), and open dialogue/JoinPlayer gaps
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

**Context plumbing is now `struct LoadedProject<'a>`** (`feature/loaded_project_refactor`,
2026-09-05): `cross_file_checks`/`check_ui_trigger_reachability`/`strict_checks` each take one
`Copy` struct and destructure it with `..` into the same local names their bodies always used, so
adding a new *input* to a check is now one struct field + one name in that check's destructure
(no 3-signature/positional-call-site churn). Practical consequence for review: the structural fix
for the `source_file`-literal lie below (thread each catalog's resolved path in) went from
"+4 params × 3 signatures" to a couple of lines — stop treating it as expensive. Only genuinely
swappable pair in the struct literal is `logic_files_parsed_cleanly`/`scenes_parsed_cleanly`
(both `bool`); every other field has a distinct type, so a mis-assignment can't compile.

**How to apply — the four recurring blind spots:**

1. ~~**`do_validate` hardcodes `"stats/stats.ron"`**~~ **CLOSED** by
   `feature/configurable_catalog_paths` (2026-09-04). Scope was wider than the backlog title:
   **FOUR** `ProjectConfig` fields are configurable catalog paths treated identically by
   `project_loader.rs:65-86` (`asset_catalog`, `prefab_catalog`, `stats_path`, `items_path` —
   each `.map()`'d into one `asset_server.load()`, **no convention fallback: unset ⇒ the runtime
   loads nothing at all for that catalog**). All four now go through one generic
   `load_configured_catalog<T>(project_dir, field, convention_path, field_name, results)`.
   Semantics: field set ⇒ that exact path, and configured-but-missing is a **hard error**
   (`"{field_name} in .project.ron does not exist on disk"`); field unset ⇒ falls back to the
   convention path via tolerant `try_parse`. That fallback is a **deliberate CLI-only divergence**
   from runtime "load nothing" (dropping it broke ~30/65 fixtures that omit `.project.ron`), and
   it is inert for shipped content — verified 2026-09-04 that every one of the 13 projects with a
   convention-path catalog file on disk also declares it. **The residual false negative to
   remember: unset field + convention file present on disk ⇒ validate passes but the runtime boots
   with an empty catalog.** A `--strict` "file exists but field unset" warning would close it
   without touching any fixture.
2. **`try_parse` returns `None` for a missing file with no `FileResult` pushed** (validate.rs:90-100).
   So `items_path: Some("items/itmes.ron")` (typo) → catalog `None` → the check it gates is silently
   skipped *and* no `missing_file` error is reported. Any new configurable-path catalog inherits this.
3. ~~**Scene-path actions are covered piecemeal**~~ **CLOSED**: `ToggleOverlay` is now in the
   scene-path arm and `ProjectConfig.initial_scene` has its own check (validate.rs:~454). The disk
   check is `project_dir.join(path).exists()`, matching `resolve_project_path`
   (scene_manager/mod.rs:734 = `format!("{project_root}/{path}")`).
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
from it, so don't take "the neighbouring check didn't do it" as precedent. **This is the single most
frequently missed step — recurred again in `feature/camera_mode_validation` (2026-09-06): two new
checks + a whole new prefab-level `camera_mode` surface shipped with line 249 untouched, and
`docs/20_data_formats.md`'s own per-feature "**Validation:**" paragraph (there is one per feature
area, e.g. camera_modes @~2556) left stale too. Check BOTH lists, not just 60_contributing.**

**Design question to ask of any check that mirrors an existing runtime `warn!`: is the warn itself
over-strict relative to the code?** Promoting a `warn!` to an exit-1 error raises the cost of a
false positive — a designer cannot suppress a `validate` error. Concrete case
(`feature/camera_mode_validation`): `FixedCameraDef`'s doc says "exactly one of
`look_at`/`look_at_entity`", but `fixed_camera_system` implements `look_at_entity ... .or(look_at)`,
a real working fallback. Read the *system*, not just the warn text, before mirroring.

**A helper shared across two call sites needs per-call-site remedy text, not just a per-call-site
`context` prefix.** Same feature: the nested-`split`/`party` message prescribes
`components: (camera_mode: ..., split: (...))`, which is meaningless for the `camera_modes:`
registry call site (no `components:` block exists there). Prefix-only parameterisation makes the
*subject* right and leaves the *remedy* wrong.

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
**`feature/scene_path_validity` (2026-09-06) applied exactly that union pattern for scenes** — the
reference template to copy for `behavior`/`dialogue`/`animation_policy` next. Three things that
review established and any repeat of the pattern must get right:
- **Dedup the union against `file_results`' rel_paths, NOT against the successfully-parsed vec.**
  The scenes version deduped against `scenes` (successes only), so a *broken* `scenes/x.scene.ron`
  that is also the `initial_scene` gets re-`try_parse`d and pushes a **second identical FileResult** —
  the designer sees the same parse error twice and `N files checked` double-counts. Same root cause
  makes a typo'd `ToggleOverlay("logic/rules.ron")` push a GameSceneV2 parse error under
  `rel_path == "logic/rules.ron"`, which flips `logic_files_parsed_cleanly` false and silently
  disables `check_ui_trigger_reachability` + `orphan_rule` while blaming a valid file. One fix for
  both. (Borrow note: the extra-path list must be re-owned into `Vec<String>` before pushing into
  the vec it deduped against — that's why the `.map(String::from)` line exists.)
- **Discovery is single-pass, not transitive**, and runs right after `all_actions` — a folded-in
  file's own actions are never collected, so an out-of-convention file reachable only from another
  out-of-convention file is missed.
- Verified there is **no `"scenes/"` prefix assumption anywhere downstream** — `rel_path` is only
  ever used as a `source_file` message string; validate never derives a scene-name event
  (`scene.ready:{stem}`) from it. Folding into the shared `scenes` vec really does give all ~15
  scene-walking checks the new file for free.

**`collect_actions` still misses TWO live Action sources** (so *every* action-driven check —
scene-path existence, item/effect/prefab keys, `spawn_point`, `{new_id}` — is blind to them):
`GameSceneV2`'s `ActionBarDef.slots[].do_actions` (scene_v2.rs:1054, scene-authored) and
`ProjectConfig.rules[].do_actions` (the V1 inline-rules field, project.rs:170/329 — still honored
at project_loader.rs:111 & 252 whenever `rules_path` is unset). Fixing `collect_actions` is the
single highest-leverage change in this file. Confirmed complete, though: the only scene-path-bearing
`Action` variants are `LoadScene`/`LoadSceneOverlay`/`PreloadScene`/`ToggleOverlay`, and
`initial_scene` is the only scene path on `ProjectConfig` (`GameSceneV2` has none).

**Scene paths are NOT `{self}`/`{target}`-substituted** — `rewrite_self`/`rewrite_target` have no
`LoadScene`-family arm, so a templated scene path is broken end-to-end at runtime *and* already a
hard exit-1 `missing_file` today. Don't add a `{`-skip guard there (unlike `spawn_point`); if a code
comment calls templated scene paths a "hypothetical future" form, that's misleading on both counts.

**`query scenes` (query.rs:326) and `stats` (stats.rs:83) still glob only `scenes/`** — whenever
validate's coverage goes reference-driven, those two stay convention-only and disagree with it.

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

~~**Sibling gap left open:** `ItemDef.currency_stat`~~ **CLOSED** by
`feature/item_key_reference_check` (2026-09-04), together with the `Action::AddItem`/`RemoveItem`/
`TransferItem`/`BuyItem` `item_key` arm and `PrefabDef.inventory.initial_items[].item_key`. All three
verified false-positive-free: `rewrite_self`/`rewrite_target` (message_interpreter.rs:275-285,
331-341) destructure and move `item_key` through **untouched** — only `entity`/`from`/`to` are
`.replace`d — and `dialogue.rs::substitute_self_in_action` has no item-action arm at all. `BuyItem`'s
`String` is the *item key* (`OpenShop`'s is the merchant id — don't confuse them).

**`source_file` literals must match how the file is actually located.** As of
`feature/configurable_catalog_paths` **all four catalogs are relocatable, so every hardcoded catalog
literal in this file is now potentially a lie** — the old "these literals are honest because the
path is hardcoded" justification is dead. Still outstanding after that feature: **11×
`source_file: "prefabs/prefabs.ron"` + 3× `source_file: "assets.ron"`** (the `--strict` unused-*
warnings), plus **12× message-body `"not found in prefabs.ron/assets.ron/stats.ron/items.ron"`**.
A project that relocates its catalog now gets a correct exit-1 pointing at a path that doesn't
exist in that project. Structural fix: make `load_configured_catalog` return
`Option<(String, T)>` (resolved rel path + value) and thread the path into
`cross_file_checks`/`strict_checks` as `source_file`. Note the resolved path is also the honest
value for checks whose `source_file` is currently `find_project_ron(project_dir)`.

**`find_project_ron` picks the *first* `*.project.ron` in `read_dir` order** and
`assets/projects/integration_tests/` has three (`integration_tests`, `test_terrain`,
`test_start_menu`). Pre-existing, but catalog resolution now depends on that arbitrary pick — inert
only because all three declare identical `asset_catalog`/`prefab_catalog` values.

**Runtime failure mode for a bad `item_key` is quieter than "no-op":** `inventory::add_to_slots`
(inventory.rs:167-177) falls back to `max_stack = 99` on a catalog miss with **no warn**, so the item
lands in the inventory as an unnamed, icon-less stack. Cite this when justifying design-time strictness.

**Inventory of RON-authored disk paths (established during `feature/path_case_check`, 2026-09-05).**
Only **6** call sites in validate.rs do an on-disk check, and `path_case_mismatch` now covers all 6
(`LoadScene|LoadSceneOverlay|PreloadScene|ToggleOverlay`, `StartDialogue.dialogue_path`,
`ProjectConfig.initial_scene`, `PrefabDef.behavior`, `PrefabDef.dialogue`,
`load_configured_catalog`). The other `exists()/is_file()` hits in the file are convention-path
literals (`try_parse`, the `--strict` unset-catalog check) or the CLI arg — correctly out of scope.
**But "every RON-authored disk-path reference" is a much bigger set than "every site that already
had an exists() check", and the rest are unchecked entirely** (no existence check, hence no case
check either):
- `PrefabDef.animation_policy` — project-relative (`"prefabs/animation/x.ron"`), resolved by
  `resolve_project_path` in entity_spawner.rs:182, sits **3 lines from the `behavior`/`dialogue`
  checks in the same prefab loop**. Failure mode is severe: the entity is spawned
  `Visibility::Hidden` pending the policy load, so a 404 = invisible-then-unanimated character.
  Same field on `PlayerConfig` (schema/player.rs:55, scene-authored).
- `ProjectConfig.rules_path` / `state_machine_path` / `model_fixes_path` — configurable, but
  validate still uses the hardcoded `"logic/rules.ron"`/`"logic/state_machine.ron"` literals
  (acknowledged at validate.rs:57-59). A relocated/mis-cased `rules_path` = zero logic in the browser.
- **assets-root-relative** (different base dir — `assets/`, not `project_dir`): every
  `AssetCatalog` entry `path` (models/textures/audio/effects/decals, schema/catalog.rs:602/608),
  `MaterialDef.shader` (the one designer-authored shader path), `TerrainConfigV2.heightmap`/
  `.splatmap`, `EnvironmentMapConfig.diffuse_path`/`.specular_path`. validate checks none of these;
  `tools/asset_checker/check.py` checks existence with Python `Path.exists()`, which is **equally
  case-blind on Windows** — so the largest population of HTTP-served paths still has the bug.
  `path_case_mismatch(base, rel)` already takes its base dir as a param, so extending it is cheap.

**`path_case_mismatch` false-positive class to remember: duplicate case-variant siblings.** The walk
does `read_dir(..).find(|e| e.file_name().eq_ignore_ascii_case(component))` — **first match in
arbitrary `read_dir` order, not exact-match-preferred**. On a case-sensitive FS where both
`Main.scene.ron` and `main.scene.ron` exist, an exactly-correct authored path can be reported as a
mismatch. One-line fix (prefer an exact match before falling back to the case-insensitive one).
Conservative in the other direction by luck: `eq_ignore_ascii_case` doesn't fold non-ASCII, and `.`/
`..`/leading-`/`/empty components make `find` return `None` ⇒ silent skip, never a false error.

**The check is a no-op on a case-sensitive FS** (its `else` branch is only reached after `exists()`
passed, which on Linux means the path already matched byte-exactly). Two consequences: the 3
message-asserting fixture tests (`wrong_case_scene_path_exits_1`, `backslash_scene_path_exits_1`,
`wrong_case_configured_catalog_path_exits_1`) assert Windows/macOS-only message text and would fail
on Linux (they'd get `missing_file` instead); and the highest-value Linux/macOS improvement is the
reverse — run the case walk **inside** the `missing_file` branch to append "did you mean {real}?",
which would also converge the messages across platforms and make those tests portable.

**`load_configured_catalog` returns `None` on a case mismatch**, so the catalog isn't parsed and
every downstream check that depends on it silently vanishes (fix the case → re-run → fresh wave of
unrelated errors). The file is readable; continuing to `try_parse` after recording the error would
be strictly better. Also: that site pushes a `FileResult` string, so it is the **1 of 6 sites with
no `error_type: "path_case_mismatch"`** — a `--json` consumer grepping that type misses catalog paths.

**Positive-path coverage for item checks already exists via the `validate_projects` smoke test:**
`3rd_person_game_demo` has 4 `BuyItem`s (state_machine.ron:157-160), `ItemDef.currency_stat: "gold"`
(items/items.ron:42), and 5 prefabs with `inventory.initial_items` — so those three arms have a real
no-false-positive gate. `AddItem`/`RemoveItem`/`TransferItem` in logic files are fixture-only.
