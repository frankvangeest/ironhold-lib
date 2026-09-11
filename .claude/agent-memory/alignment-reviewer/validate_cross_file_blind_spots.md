---
name: validate-cross-file-blind-spots
description: Structural blind spots in ironhold_cli validate.rs — the 6 configured paths + load_configured_catalog fallback divergence, source_file-literal rule, try_parse silent-None, convention-glob discovery, substitution-token false positives, the docs "Checks performed" list, the RON-authored-disk-path and texture-key inventories, and the 8 schema/ validate() methods (2 wired, 3 dead)
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

**SECOND false-positive class, established `feature/cli_validate_small_wins` (2026-09-07): a field
the *runtime overwrites* after reading the prefab.** Before copying the now-standard
"scene `entities:` + `join_prefab_keys`" scoping template (`duplicate_gamepad_index` is the
original), check whether the JoinPlayer executor keeps the authored value for THAT field.
`gamepad_index` is genuinely read from the join prefab (only a *gamepad*-triggered join bypasses it,
via `PendingJoinGamepad`), but `player_index` is unconditionally clobbered —
`action_executor.rs:~1775 player_config.player_index = next_slot;` — so the `join_prefab_keys` half
of a `duplicate_player_index`-shaped check flags working content (same join prefab in two slots, or
a scene-placed player prefab reused as a join prefab). Only `local_coop_demo/room8` authors
`join_prefab_keys` at all, and it happens to use two distinct prefabs, so this class of false
positive won't be caught by the `validate_projects` smoke test.

**Check WHICH vec the push goes into against what the docs claim.** Same feature shipped
`duplicate_player_index` into `cross_file_checks` (→ `errors`, unconditional exit 1) while
`docs/20_data_formats.md` advertised it as `validate --strict`. `cross_file_checks` +
`check_ui_trigger_reachability` always run; only `strict_checks` is gated on the flag
(validate.rs:~2820-2823). Severity is invisible at the push site — the only tell is which function
you're inside — so read the enclosing `fn`, not the comment above the check.

**Runtime-warn parity for `player_index` specifically: there is NO general duplicate-`player_index`
warn.** `scene_loader.rs` only has `warn_duplicate_gamepad_index`; the player_index warn lives in
`spawn_players_and_camera` and fires **only when `own_viewport_only == true`**, and on a
`% MAX_SPLIT_PLAYERS` collision (so 0-vs-4 counts, which an equality-only CLI check misses). The
HUD label is `format!("P{}", player_index.0 + 1)` (camera.rs:~813) — a message that prints
`"P{index}"` is off by one. `MAX_SPLIT_PLAYERS` is fully `pub` in `capabilities::camera` and
`capabilities` is a `pub mod`, so "the CLI can only reach `schema::`" is a convention, not a
constraint — relevant whenever a check wants an engine constant (`GRAVITY` is the exception: real
`pub(crate)` in `runtime::scene_manager::scene_loader`, so it gets *mirrored* in validate.rs with a
keep-in-sync comment and no test guarding the two; the clean fix is moving it + `JumpConfig`'s
height resolution into `schema/catalog.rs`, which would also de-duplicate validate.rs's own
`resolve_height` copy).

**Deterministic-order sweep is still incomplete.** As of 2026-09-07 the two prefab loops at
validate.rs:~1093/~1475 sort, but two error-emitting `HashMap` walks do not: merchant
`currency_stat` (~822) and merchant `stock[].item_key` (~869). **Correction (2026-09-11): the
`camera_modes:` registry loop needs no sort — `GameSceneV2.camera_modes` is a `BTreeMap`
(scene_v2.rs:66), so it iterates in key order already.** Don't re-flag it.

**Font-glyph lints (`non_ascii_dash_in_text`) must enumerate text surfaces by hand.** The lint
walks parsed values (so the ~860 em-dashes in RON *comments* correctly don't fire) but covers only
scene `ui:` `Label`/`Button`, entity `label:`, and `world_labels:`. Uncovered designer-authored
strings that render through the same embedded font: `DialogueNodeDef.speaker`/`.body` +
`ChoiceDef.label` (dialogue.rs:22/29/44 — prose, the highest-risk surface, and `LoadedProject`
already carries the parsed dialogues), `ItemDef.display_name` (items.rs:37), `PrefabDef.display_name`
(catalog.rs:858), `ActionSlotDef.label` (scene_v2.rs:1066). No font-family field exists anywhere in
`schema/`, so such a lint cannot false-positive against a project supplying its own font.

**Every new check must be added to the `docs/60_contributing.md` "Checks performed" /
"`--strict` flag" bullet lists (~lines 236-262).** That is the only designer-facing enumeration of
what `validate` catches, and it is otherwise well maintained (label_depth_scale, gamepad_index,
merchant, slope/coyote all present) — but the `Action::SetCameraMode` mode check is already missing
from it, so don't take "the neighbouring check didn't do it" as precedent. **This is the single most
frequently missed step — recurred AGAIN in `feature/cli_validate_small_wins` (2026-09-07): three
new checks, `docs/20_data_formats.md` updated in three places, `60_contributing.md` untouched.
Also recurred in `feature/camera_mode_validation` (2026-09-06): two new
checks + a whole new prefab-level `camera_mode` surface shipped with line 249 untouched, and
`docs/20_data_formats.md`'s own per-feature "**Validation:**" paragraph (there is one per feature
area, e.g. camera_modes @~2556) left stale too. **And AGAIN in `feature/cli_validate_batch3`
(2026-09-11): four new check families, `docs/60_contributing.md:240-262` untouched.** Check BOTH
lists, not just 60_contributing. Treat this as a near-certain finding on any validate.rs feature —
grep `docs/60_contributing.md` for one of the new `error_type` strings before writing the review.**

**Design question to ask of any check that mirrors an existing runtime `warn!`: is the warn itself
over-strict relative to the code?** Promoting a `warn!` to an exit-1 error raises the cost of a
false positive — a designer cannot suppress a `validate` error. Concrete case
(`feature/camera_mode_validation`): `FixedCameraDef`'s doc says "exactly one of
`look_at`/`look_at_entity`", but `fixed_camera_system` implements `look_at_entity ... .or(look_at)`,
a real working fallback. Read the *system*, not just the warn text, before mirroring.

**The camera-mode vocabulary surface has FOUR RON entry points; validate covers two.**
Established `feature/cli_validate_batch3` (2026-09-11), which added `camera_mode_vocab_problems`
(`orbit_button`/`character_rotate_button` ∈ Left/Right/Either/None; `look_button` ∈
Left/Right/Either — **no None**; `FlyCamDef`'s six movement keys via `InputMap::parse_key`, whose
miss path is `unwrap_or(KeyCode::KeyW)` with *no warn at either design time or runtime* — the
quietest failure in the camera area). Covered: `PrefabDef.components.camera_mode` (both the
`is_player()` and `is_flycam()` gates) and `GameSceneV2.camera_modes`. **Uncovered: the legacy
`PrefabDef.components.camera` (`Option<CameraConfig>`) and `.flycam` (`Option<FlyCamDef>`)** —
both still live (scene_loader.rs:272) and still shipped (`local_coop_demo` authors ~10 legacy
`camera:` blocks; `foliage_demo`/`dynamic_animation_control` author legacy `flycam:`). If anyone
retrofits, apply ONLY the vocab helper — `camera_mode_nested_split_party_problem` must never touch
`components.camera`, where nested `split`/`party` is the *correct* legacy location. `PlayerConfig`
is not `Deserialize` (player.rs:26), so its `camera`/`camera_mode` fields are not a fifth surface.
No false-positive risk from `FlyCamDef` defaults: they are `"KeyW"`/`"Space"`/`"KeyQ"` etc., all of
which `parse_key` accepts.

**`scene_loader.rs` does NOT silently discard a flycam-tagged prefab's `camera_mode`.** Recurring
wrong claim (appeared in `claude_suggestions.md`, a validate.rs comment, and a test doc comment).
Reality: `scene_loader.rs:273-274` passes `components.camera_mode` straight through as `mode`; the
flycam-spawn match at `:824-836` rejects any non-`Flycam(_)` value with its own `warn!` ("has no
defined behavior for a standalone flycam-tagged prefab… falling back to Flycam defaults") and
`FlyCamDef::default()`. Only `model`/`shape`/`primitive`/`children` (and the ~20 other fields in
[[diagnostic-only-feature-pattern]]) are truly discarded. So the CLI check for this is a
design-time twin of an existing warn, not a first-ever diagnostic.

**A helper shared across two call sites needs per-call-site remedy text, not just a per-call-site
`context` prefix.** Same feature: the nested-`split`/`party` message prescribes
`components: (camera_mode: ..., split: (...))`, which is meaningless for the `camera_modes:`
registry call site (no `components:` block exists there). Prefix-only parameterisation makes the
*subject* right and leaves the *remedy* wrong.

~~**Sibling gap noted 2026-09-04: `Action::JoinPlayer`'s `spawn_points["player_{next_slot+1}_start"]`
is unchecked**~~ **CLOSED** 2026-09-11 (action_executor.rs:~1753) — its miss path was *even quieter* than
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
As of `feature/configurable_logic_paths` this is no longer hypothetical: `query rules`/`actions`/
`events` (query.rs:437-438, 646, 663) and `stats` (stats.rs:96,100) still `silent_parse` the two
hardcoded logic convention paths, so `ironhold query rules assets/projects/3rd_person_game_demo`
(the exact command documented at 60_contributing.md:302) lists that project's runtime-dead
`logic/rules.ron` as live while `validate` no longer sees it at all.

**SIX configured paths now, not four.** `feature/configurable_logic_paths` (2026-09-06) added
`resolve_logic_files` + `ResolvedLogicFiles { rules, rules_source, state_machine,
state_machine_source }` (validate.rs:~430-519), mirroring `project_loader.rs::check_project_loaded`
exactly: `.project.ron` present ⇒ `rules_path`/`state_machine_path` are the ONLY source for their
half (unset `rules_path` ⇒ synthesize a `LogicRulesAsset` from inline V1 `config.rules`, attributed
to the project.ron's own filename; unset `state_machine_path` ⇒ `None`, no fallback); no
`.project.ron` ⇒ both convention paths, the same deliberate divergence `load_configured_catalog`
already makes. Two things a repeat of this pattern must get right, both missed here:
- **`parse_configured` (the new helper) skips the `path_case_mismatch` step that
  `load_configured_catalog` (validate.rs:189-199) performs.** So a mis-cased
  `rules_path: "Logic/rules.ron"` validates clean on Windows and 404s in the browser = zero logic.
  60_contributing.md:254 still says "the four configured catalog paths" — it's six now, two uncovered.
- **`strict_checks`'s `unset_catalog_path_with_convention_file` array (validate.rs:~2080-2085) was
  not extended.** `3rd_person_game_demo`/`terrain_demo` each ship a now-invisible dead
  `logic/rules.ron`; before the change they at least got (wrong) `orphan_rule` warnings pointing at
  them. Adding the two fields needs *different* message text than the catalog one (that message
  ends "...even though this validate run just checked it via the convention-path fallback" — for
  logic paths validate did NOT check it).

**Source-path attribution is display-only, verified safe.** No check in validate.rs branches on a
logic file's source string; the only structural uses are `r.rel_path == rules_source` in
`logic_files_parsed_cleanly` and `source_file` message strings, and the only path-shaped predicates
are `starts_with("behaviors/")`/`starts_with("scenes/")`. So attributing inline `config.rules` to
the `*.project.ron` filename (not a `logic/*.ron` path) misbehaves nowhere, and the synthesized
`schema_version: 2` is inert (validate never calls `LogicRulesAsset::validate()` anywhere).

**`schema/` `validate()` methods are a separate coverage axis from validate.rs's hand-written checks.**
There are **8** of them; `feature/cli_validate_gap_closures` (2026-09-11) wired the first 2 into
`cross_file_checks` (`AssetCatalog`, `PrefabCatalog`, `source_file` = hardcoded literals). Still
runtime-only (called at `project_loader.rs:319`/`:342`, not by the CLI): `StatCatalog`,
`ItemCatalog`. Called **nowhere in either crate** (dead code): `GameSceneV2::validate()`,
`LogicRulesAsset::validate()`, `ModelFixesAsset::validate()`. Also runtime-only: `ProjectConfig`
(`:41`), `StateMachineAsset` (`:260`). Three things to check whenever another one gets wired:
- **They are fail-fast over a `HashMap`** (`return Err` on first problem), so exactly one violation
  surfaces per catalog per run and *which* one is nondeterministic — the opposite posture from every
  hand-written check here. Any fixture with >1 violation is flaky; the two added in that batch are
  single-entry deliberately.
- **`AssetCatalog::validate()`'s empty-`models[].path`/`decals[]` arms are now DOUBLE-reported**,
  since `check_asset_catalog_path` already flags `""` via `is_file()` (with a misleading
  "relative to the asset root" remedy). The new fixture hides this by having no `assets`-named
  ancestor. The wiring's unique value is the `schema_version` + `EffectDef` particle/flipbook
  invariants only.
- **`GameSceneV2::validate()` is the highest-value unwired one**: duplicate scene entity `id`,
  duplicate UI element `id`, duplicate `world_labels[].id` — i.e. it generalises the
  dialogue-specific duplicate-`id` check that batch hand-wrote.
Runtime severity for the wired pair is `error!`-and-continue (degraded), CLI is exit 1 — the normal
design-time-stricter posture. But note the CLI's convention-path fallback means an *unset*
`asset_catalog`/`prefab_catalog` field + a stale catalog on disk now exits 1 on schema invariants in
a file the runtime never loads (new instance of the documented `load_configured_catalog` divergence).

**Texture-key checks are now a family — CLOSED by `feature/cli_validate_batch3` (2026-09-11).**
`fn check_texture_key(asset_catalog, source_file, context, key, errors)` (validate.rs:~390,
`error_type: "missing_catalog_key"`) now covers `InventoryPanelDef.icon_sheet`,
`ContainerPanelDef.icon_sheet`, `ActionBarDef.icon_sheet`, `ActionSlotDef.icon`, and
`PrefabDef.world_stat_bar`'s `Icon.icon_sheet`/`Textured.texture_sheet`. The two pre-existing
sibling checks (`FoliageMaterialDef.leaf_texture`, `ItemDef.icon_sheet`) were deliberately NOT
retrofitted — each has its own nuance (empty-string guard; relocation-aware `assets.ron` name) —
so three shapes of the same check coexist, with three different `error_type`s
(`missing_catalog_key` / `missing_catalog_key` / `missing_reference`).
**`TargetIndicatorDef.texture` is NOT a `textures` key — it is a `decals` key** (verified twice:
scene_loader.rs:1125 `params.asset_catalog.0.decals.get(&def.texture)` + its own "unknown decal
key" warn). An earlier memory/review claim that it was a `.textures` lookup was wrong; it now has
its own inline check, not a `check_texture_key` call.
**Miss-path severity differs per field — don't copy one blanket "silent" claim across all six.**
Genuinely silent (no warn anywhere): `InventoryPanel`/`ContainerPanel`/`ActionBar` icon sheets,
`ActionSlotDef.icon`, and `WorldStatBarStyle::Icon.icon_sheet` (stat_display.rs:698-700,
`unwrap_or_default()` ⇒ blank handle). NOT silent: `WorldStatBarStyle::Textured.texture_sheet`
(stat_display.rs:775-781 warns *and* skips the whole bar) and `target_indicator.texture`
(warns + disables the indicator).

**Dialogue/JoinPlayer/animation_policy gaps: CLOSED by `feature/cli_validate_gap_closures`
(2026-09-11)** — `PrefabDef.animation_policy` (existence + `path_case_mismatch`, third arm of the
same prefab loop as `behavior`/`dialogue`), duplicate `DialogueNode.id` + unresolved `jump_to`
(excluding `"__end__"`), `DialogueCondition::StatAtLeast.stat_key`, `ItemDef.icon_sheet`, and
`Action::JoinPlayer`'s `player_{slot+1}_start`. Verification facts worth reusing:
- `jump_to` is **not** substituted anywhere (`dialogue.rs:166` matches the literal id; only
  `do_actions` go through `substitute_self_in_action`) — so no `{`-skip guard is needed, unlike
  `spawn_point`.
- `StatAtLeast` resolves against global `LoadedStats` (`dialogue.rs:382-385`,
  `map_or(false, ..)` ⇒ choice permanently hidden on a miss), so `stat_catalog.stats.contains_key`
  is the right lookup — same as `MerchantDef.currency_stat`.
- `ItemDef.icon_sheet` really is a `textures` key (`scene_loader.rs:2303` builds
  `LoadedInventoryUi.icon_atlases` keyed by catalog key), not a path.
- The JoinPlayer index mapping is exactly right: `join_prefab_keys[next_slot]` is 0-based and
  `spawn_points["player_{next_slot + 1}_start"]` is 1-based (`action_executor.rs:1749-1753`), and
  both live on one `GameSceneV2` ⇒ genuinely scene-scoped, no union approximation. Accepted narrow
  over-coverage: a `Some(..)` entry in a slot below the scene's initial player count can never be
  hot-joined, so the demanded spawn point is unused — same over-coverage the pre-existing
  player-tagged/GLB-only `join_prefab_keys` check already has.
- `PlayerConfig.animation_policy` needs **no** separate check: it is runtime-assembled from the
  prefab by `assemble_player_config`, never RON-authored (the only `animation_policy` hits outside
  `prefabs.ron` in all 15 shipped projects are RON *comments*).

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
- ~~`jump_to` / duplicate node `id` / `StatAtLeast.stat_key`~~ **CLOSED** 2026-09-11 — see the
  `feature/cli_validate_gap_closures` section above.
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
- ~~`PrefabDef.animation_policy`~~ **CLOSED** 2026-09-11 — it is now the third arm of the same
  prefab loop as `behavior`/`dialogue`, so there are **7** on-disk check sites with
  `path_case_mismatch`, not 6. (`PlayerConfig.animation_policy`, schema/player.rs:55, needs no
  check — runtime-assembled from the prefab, never RON-authored.)
- ~~`ProjectConfig.rules_path` / `state_machine_path`~~ **CLOSED** by
  `feature/configurable_logic_paths` (2026-09-06) — see the "SIX configured paths" section below.
  **`model_fixes_path` is the one still hardcoded** (`try_parse(project_dir,
  "overrides/model_fixes.ron")`, validate.rs:~2536, assigned to `_model_fixes` and consumed by no
  check — only its parse `FileResult` matters). 8 shipped projects set it, all at the convention
  path. Two live inconsistencies: a relocated `model_fixes_path` is never parse-checked, and a
  *dead* `overrides/model_fixes.ron` (field unset) can still exit-1 on a file the runtime never
  loads. Fix is one call: route it through `load_configured_catalog` + add it to the strict array.
- ~~**assets-root-relative**~~ **CLOSED** by `feature/asset_catalog_path_check` (2026-09-10) — see
  the "assets-root-relative paths" section at the end of this file for the full key-vs-path
  inventory and the `find_assets_root` heuristic's known holes.

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

**assets-root-relative paths (`feature/asset_catalog_path_check`, 2026-09-10).** A second base dir
now exists in validate.rs: `find_assets_root(project_dir)` = `project_dir.parent().parent()` gated
on a `shared/` sibling being a dir, and `check_asset_catalog_path(assets_root, source_file, context,
authored_path, errors)` does exists + `path_case_mismatch` on one raw path (stripping a `#Scene0`
fragment). Now covered: `AssetCatalog.models[k].path`/`.textures[k]`/`.audio[k].path`/`.decals[k]`,
all 3 `MaterialKind` variants' nested paths, `ProjectConfig.global_environment.diffuse_path`/
`.specular_path`, per-scene `terrain.heightmap`/`.splatmap`/`.material_paths`.

**The key-vs-path inventory (verified exhaustively — reuse instead of re-deriving).** RAW paths
(assets-root-relative, straight to `asset_server.load`): the 4 `AssetCatalog` value/`.path` fields,
`MaterialDef`'s nested texture/shader/splatmap/layers, `EnvironmentMapConfig.*_path`,
`TerrainConfigV2.heightmap`/`.splatmap`/`.material_paths`. CATALOG KEYS (must NOT be path-checked):
`FoliageMaterialDef.leaf_texture`, `EffectDef`/`LayerDef.sprite`/`.sprites`, `ItemDef.icon_sheet`,
`InventoryPanelDef.icon_sheet`, `IconButtonDef.icon_on`/`.icon_off`, `TargetIndicatorDef.texture`,
`PrefabDef.material`, `PrefabDef.model`, `FoliageDef.trunk`, `AnimationPolicy.animation_sources`.
Two structural facts that make this stable: `MaterialDef` exists **only** inside
`AssetCatalog.materials` and `EnvironmentMapConfig` **only** at `ProjectConfig.global_environment`
(no inline scene/prefab variants), and there is **no raw-path fallback** on a failed
`models.get()` anywhere (scene_loader.rs:720 pushes a load_error and `continue`s) — so checking the
catalog's own paths transitively covers every key-based consumer.

**`find_assets_root`'s two holes.** (1) It is purely textual — no `canonicalize`/`absolute` — so
`ironhold validate .` or `cd assets/projects && ironhold validate quick_scene` yields
`parent()` = `""`, `parent()` = `None` ⇒ **all these checks silently vanish, no diagnostic**. Same
for `ironhold watch .`. One-line fix: `let root = project_dir.canonicalize().ok()?;` first.
(2) The runtime's real invariant is `utils.rs::find_assets_folder` = "nearest ancestor dir literally
named `assets`" and does **not** require `shared/`; the `shared/`-probe is a proxy for "am I in the
repo layout" chosen so `tests/fixtures/asset_paths/{shared,projects}/` works. A downstream game with
no `assets/shared/` gets silent skips. `root.file_name() == "assets" || root.join("shared").is_dir()`
covers both. Fragility to remember: if anyone ever creates `crates/ironhold_cli/tests/shared/`, all
~65 bare `tests/fixtures/{name}/` fixtures start resolving asset paths against `tests/` at once.

**`tools/asset_checker/check.py` is the Python twin and diverges deliberately.** It got the same
byte-exact-then-case-insensitive `find_case_mismatch` and now exits 1 on a mismatch, but it globs
`PROJECTS_DIR.rglob("assets.ron")` and regex-scans for quoted strings ending in an asset extension —
so it is blind to `terrain.*` and `global_environment.*` (which validate.rs now checks) and to a
relocated `asset_catalog`. Conversely it reports **line numbers**, which validate.rs cannot (RON
spans are lost post-parse). Neither tool subsumes the other; say so when either is extended.

**Positive-path coverage for item checks already exists via the `validate_projects` smoke test:**
`3rd_person_game_demo` has 4 `BuyItem`s (state_machine.ron:157-160), `ItemDef.currency_stat: "gold"`
(items/items.ron:42), and 5 prefabs with `inventory.initial_items` — so those three arms have a real
no-false-positive gate. `AddItem`/`RemoveItem`/`TransferItem` in logic files are fixture-only.
