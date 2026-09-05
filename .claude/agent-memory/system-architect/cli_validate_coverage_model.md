---
name: cli-validate-coverage-model
description: ironhold_cli validate silently skips checks when a catalog is absent; only two severity tiers exist (CrossFileError=hard, StrictWarning=--strict-only); configurable catalog paths now honored in validate.rs but not query.rs/stats.rs, and the unset-field convention fallback masks a real authoring error; scene-path coverage is partial; dialogue parse gate landed but its referential checks and query.rs's collector did not; join_prefab_keys union checks can false-positive on runtime-unreachable low slots
metadata:
  type: project
---

`ironhold_cli validate`'s cross-file checks are **all conditional on the relevant catalog having
parsed**, and `try_parse()` returns `None` with *no diagnostic* when the file simply doesn't exist.
Net effect: a typo in a *configured catalog path* makes every check that depends on that catalog
silently vanish — validate exits 0 and reports nothing.

**There are exactly two severity tiers, and no mid-tier "warning" in the default run.**
`CrossFileError` (pushed in `cross_file_checks`) is *always* a hard error → exit 1. The only softer
signal is `StrictWarning`, produced by `strict_checks` and surfaced **only** under `--strict`
(still exit 1 when present). Any proposed check described as "a warning, not an error" therefore
belongs in `strict_checks`, not as a `CrossFileError` push — plan docs routinely get this
backwards. `jump_cannot_clear_ground_sensor` / `negative_coyote_time_secs` are the precedent (both
are `--strict` warnings even though `crates/ironhold_core/src/CLAUDE.md` calls them "errors").

**Plumbing is a solved problem as of `feature/loaded_project_refactor` (2026-09-05).** All three
of `cross_file_checks` / `check_ui_trigger_reachability` / `strict_checks` now take a single
`project: LoadedProject` (a private `#[derive(Clone, Copy)] struct LoadedProject<'a>` in
`validate.rs`, ~13 fields of `&`/`Option<&>`/`&[]`/`bool`, built once in `do_validate`), and each
destructures with a trailing `..`. Practical consequence when scoping any *future* check: "which
params does it already receive?" is no longer a design question — every one of them sees
`project_dir`, all four catalogs, `scenes`, `actions`, `rules`/`state_machine` (as
`Option<(source_path, &asset)>`), `behaviors`, and both parse-cleanliness bools for free, and
adding a new field costs one line at the struct + one at construction with **zero** consumer
churn (the `..` absorbs it). So never propose "…but that would need plumbing" as a cost against a
check in this file again; do still note that `..` means a newly added field silently reaches no
consumer until one destructures it. Note also `dialogues` is parsed by `do_validate` but is
deliberately *not* a struct field yet — the first of items 4's dialogue referential checks to land
should add it rather than take a parameter.

Three concrete asymmetries that keep resurfacing when reviewing `crates/ironhold_cli/src/commands/validate.rs`:

1. **Hardcoded vs configured catalog paths — FIXED in `validate.rs`, still live in `query.rs`/`stats.rs`.**
   `feature/configurable_catalog_paths` (2026-09-04) added
   `load_configured_catalog<T>(project_dir, field, convention_path, field_name, results)` to
   `validate.rs`; all four configurable catalogs (`asset_catalog`, `prefab_catalog`, `stats_path`,
   `items_path` — `project_loader.rs` treats all four identically) now resolve through it.
   `resolve_project_path()` (scene_manager/mod.rs) is a plain `format!("{root}/{path}")`, so the
   CLI's `project_dir.join(path)` is a faithful mirror of runtime resolution — no shared-asset
   special-casing to worry about.
   **Two things the fix deliberately did *not* do, both worth re-raising if they bite:**
   (a) the helper is private to `validate.rs`, so `query.rs` (3 sites) and `stats.rs` (2 sites)
   still hardcode `"prefabs/prefabs.ron"`/`"assets.ron"` — `query prefabs` on a relocated-catalog
   project hard-errors "prefabs/prefabs.ron not found". The helper belongs in `commands/utils.rs`
   next to `silent_parse`/`glob_dir`.
   (b) when a field is *unset*, validate falls back to the convention path rather than mirroring
   the runtime's "load nothing at all". This was forced by ~46 of 63 CLI fixtures having a
   convention catalog and no `.project.ron` at all. It is inert for shipped content today
   (verified: every convention-path `assets.ron`/`prefabs.ron`/`stats.ron`/`items.ron` under
   `assets/projects/` is declared by its project) but it structurally *masks the likeliest
   authoring error in this area*: add `stats/stats.ron` (or `items/items.ron`), forget the
   `stats_path`/`items_path` line, and the CLI happily cross-checks a catalog the runtime never
   loads. `docs/20_data_formats.md` states the runtime semantics explicitly ("omitting it means no
   stat system for that project"), so the divergence is documented-against. The cheap mitigation
   is a `StrictWarning` when the field is unset *and* the convention file exists — fixture-safe
   because `--strict` isn't the default gate.

2. **Scene-path existence coverage is partial.** Four `Action` variants carry a project-relative
   `.scene.ron` path: `LoadScene`, `LoadSceneOverlay`, `PreloadScene`, `ToggleOverlay`. Plus
   `ProjectConfig.initial_scene`. Any of these not covered by an existence check fall through
   `cross_file_checks`'s `_ => {}` catch-all silently.

3. **`source_file` is always a hardcoded literal**: `"prefabs/prefabs.ron"` (~12 sites),
   `"assets.ron"` (3), `"items.ron"` (1). `"items.ron"` points at a path that exists nowhere (the
   real convention path is `items/items.ron`), in both the human output and the `--json`
   `"source"` field. The resolved path *is* known at the `do_validate` call site; it just isn't
   plumbed into `cross_file_checks`. **This got worse, not better, once relocation actually
   works** (see item 1): before, a relocated `prefab_catalog` was silently never loaded, so the
   literal was never printed for it; now the catalog *is* loaded and every error it produces is
   attributed to a `prefabs/prefabs.ron` that doesn't exist on that project's disk. Plumbing the
   four resolved paths out of `load_configured_catalog` into `cross_file_checks` is the fix.

3b. **Item-key/currency-stat reference coverage is complete as of
   `feature/item_key_reference_check` (2026-09-04).** All four item-key-bearing `Action` variants
   (`AddItem`/`RemoveItem`/`TransferItem`/`BuyItem`, schema/actions.rs:364-404 — no others exist,
   and no condition type references item keys), both `Deserialize` structs with an `item_key`
   (`ShopEntry`, `InitialItemEntry`), and both `currency_stat` fields (`MerchantDef`, `ItemDef`)
   are now checked. Hard-error severity is safe here because `item_key` is never a `{self}`/
   `{target}` substitution target (`message_interpreter::rewrite_self` rewrites only entity fields).
   Worth knowing when judging the value of these checks: an unknown item key is **not** rejected at
   runtime — `capabilities/inventory.rs::add_to_slots` creates the stack unconditionally
   (`max_stack` falls back to 99) and the panel renders it at `icon_index` 0 of the default sheet,
   so a typo yields a phantom slot with the wrong icon. `entity_spawner.rs`'s `initial_items` path
   additionally passes `None` for the catalog, so prefab starting items ignore `stackable`/
   `max_stack` entirely (separate latent bug).

4. **Dialogue coverage: the parse half landed, the *referential* half did not.**
   `feature/cli-validate-dialogues` (2026-09-04) added `glob_dir("dialogues", ".dialogue.ron")` +
   `parse_file::<DialogueDef>` to `do_validate` and extended `collect_actions` to walk
   `nodes[].choices[].do_actions` — so `collect_actions` now covers **all five** `Action`-bearing
   schema surfaces (grep `Vec<Action>` in `schema/`: dialogue.rs:49, project.rs:131/134/154/316),
   and no `Action` variant nests another `Action`, so no recursion is needed. What is still
   missing: **no existence check on `PrefabDef.dialogue`** (contrast `def.behavior`, checked at
   `validate.rs`~821 with the `bad_behavior_file` fixture) and **no arm for
   `Action::StartDialogue { dialogue_path }`** in `cross_file_checks` (falls through `_ => {}`).
   Runtime failure mode for a typo'd path is bad: `action_executor.rs`~1135 sets `ActiveDialogue`
   active and `asset_server.load()`s a handle that never resolves — panel opens and never
   closes, no ironhold-side `warn!`. Also unchecked despite dialogues now being parsed: `jump_to`
   node-id validity (which *does* have a runtime `warn!` at `capabilities/dialogue.rs`~165 — a
   textbook [[cli-runtime-mirror-check-pairs]] gap, and the only check that would flag anything in
   shipped content, since `npc_intro.dialogue.ron` has 8 `jump_to`s and zero `do_actions`),
   duplicate node ids, `portrait` texture-catalog key, and `DialogueCondition::StatAtLeast.stat_key`.

4b. **`query.rs` and `validate.rs` disagree on what "all the project's actions" means.**
   `query.rs::collect_logic` globs `scenes` and `behaviors` but **not** `dialogues`, so after the
   fix above `validate` walks dialogue `do_actions` while `query actions`/`query events` still
   don't. Whenever a new `Action` authoring surface is added, both collectors need it — they are
   two independent enumerations of the same surface set with no shared helper. As of
   `feature/ui_trigger_reachability_check` (2026-09-04) there is a **third**:
   `utils::collect_handled_events`, an independent re-walk of the same logic files for the *event*
   half. A seam that would serve both without changing `query events`' output shape does exist and
   was not taken: an inherent `StateMachineAsset` method yielding
   `(event: &str, do_actions: &[Action], is_transition: bool)` covers `query.rs`'s `EventRecord`
   exactly and validate's needs by projection — no index-zip required. See
   [[ui-trigger-source-enumeration]] for the re-parse hazard that placement introduced.

5. **The standing `cargo test -p ironhold_cli` gate covers only 9 of the 15 project dirs.**
   `crates/ironhold_cli/tests/validate_projects.rs` hardcodes one `#[test]` per project and is
   missing `camera_modes`, `dynamic_animation_control`, `foliage_demo`, `stats_demo`,
   `blank_project`, and `integration_tests`. `test_web.py`'s `PROJECTS` list (14, everything but
   `integration_tests`) is the only broad gate, and it only runs on `integration` batches. So a
   "manual sweep of all 14 projects validated clean" claim is real but **not reproducible at
   feature-branch speed** — adding the missing one-liners to `validate_projects.rs` is the cheap fix
   any reviewer should recommend when a change's safety argument rests on such a sweep.

6. **`find_project_ron` picks the *first* `*.project.ron` in `read_dir` order** and
   `assets/projects/integration_tests/` holds three (`integration_tests`, `test_start_menu`,
   `test_terrain`). This was near-harmless while catalogs were hardcoded — it only affected
   `items_path`. After `feature/configurable_catalog_paths` it decides **all four** catalog paths,
   so validate's view of that directory is now `read_dir`-order-dependent where the runtime's is
   explicit (`--project`/test harness names one config by hand). All three currently declare the
   same `assets.ron`/`prefabs/prefabs.ron`, so it's latent; it stops being latent the moment one
   of them relocates a catalog. Multi-config project dirs arguably want validate to iterate every
   `*.project.ron` rather than pick one.

7. **`join_prefab_keys` is now a second "scene-reachable player prefab" surface, and unioning it
   with `scene.entities` has one non-obvious false-positive edge.** Three checks now scan it
   (`unsupported_join_prefab` player-tag/GLB-only guards; `label_depth_scale`'s
   `widen_prefab_if_player` union; `duplicate_gamepad_index`, extended 2026-09-04). But at runtime
   `Action::JoinPlayer` computes `next_slot = ActiveSplitSlotCount + queued_hot_joins`
   (`action_executor.rs`~1683), so **slots already occupied by `entities:`-placed players are never
   joined into** — `scene_v2.rs`'s doc tells authors to write `None` there. A union check that
   treats every non-`None` slot as co-instantiable therefore hard-errors on a *dead* low-slot entry
   (e.g. `join_prefab_keys[0]` naming the same prefab as the scene-placed P1, both with
   `gamepad_index: 0`). Latent today (no shipped join prefab authors a `gamepad_index`; only
   `local_coop_demo/room8` uses the field at all, slots 2/3). Widening is the safe direction for
   `label_depth_scale` (a `--strict` band) but not for a hard `CrossFileError`. If it ever bites,
   the runtime-faithful fix is to skip slots whose index is below the count of player-tagged scene
   entities — and a *separate* "dead join slot" diagnostic is the better authoring signal anyway.
   Note also: hot-join is Grid-split-only at runtime; none of the three checks gate on that.

**Why:** these gaps are invisible by construction — the failure mode is *absence* of output, so
they don't show up in test runs or in "all projects validate clean" verification.

**How to apply:** when reviewing any new `validate.rs` check, ask (a) what happens when its catalog
is missing vs malformed (malformed is loud via `parse_file`; missing is silent), and (b) whether
the path it resolves matches the runtime's resolution. When a change adds a check over a *set* of
enum variants or config fields, enumerate the full set and confirm none were missed.
