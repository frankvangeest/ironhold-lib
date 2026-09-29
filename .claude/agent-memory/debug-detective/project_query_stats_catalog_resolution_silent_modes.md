---
name: query-stats-catalog-resolution-silent-modes
description: query/stats resolve ProjectConfig.asset_catalog/prefab_catalog through silent_parse with no validate() and no empty-string guard, so a mis-pointed catalog field silently hides a working convention-path catalog behind an exit-0 zero
metadata:
  type: project
---

`feature/cli_query_stats_paths` (reviewed 2026-09-11) made `query prefabs`/`query effects`/
`query scenes`/`stats` read `ProjectConfig.prefab_catalog`/`.asset_catalog` via
`utils::resolve_catalog_paths(project_dir)` → `silent_parse`, instead of the old hardcoded
`"prefabs/prefabs.ron"`/`"assets.ron"` literals. Three failure modes fall straight out of
`silent_parse`'s contract, all probe-verified:

1. **Wrong-type target parses as an EMPTY catalog, exit 0.** Neither `AssetCatalog` nor
   `PrefabCatalog` carries `#[serde(deny_unknown_fields)]`, and `silent_parse` never calls
   `.validate()`. `asset_catalog: "scenes/main.scene.ron"` deserializes fine (every field but
   `schema_version` is `#[serde(default)]`) → `query effects` prints `(0 effects)` **exit 0**,
   `stats` prints `Effects: 0`, while the real `assets.ron` at the convention path is now
   invisible. `validate` on the identical project exits 1 (`Unsupported AssetCatalog
   schema_version 2 (expected 1)`) because it *does* wire those two `validate()`s. Net: the
   change made `query`/`stats` *less* correct than before for a misconfigured field, and the
   failure mode is absence of output.

2. **`Some("")` is not treated as unset.** `unwrap_or_else` only fires on `None`, so an emptied
   field yields path `""`; `project_dir.join("")` is the project dir itself, `exists()` is true
   (the `try_parse guards exists(), not is_file()` class), `read_to_string` on a directory errors
   → `None`. `query` then prints `Error:  not found or could not be parsed in <dir>` — no
   filename, double space, exit 2. `stats` silently zeroes. `validate` reports it, but only for
   **one** of the two fields: `parse_configured_path`'s dedup is `results.iter().any(|r| r.rel_path
   == path)` and both empties are `""`, so the second is swallowed and the listing shows a
   blank-named file row.

3. **Absolute and `../` catalog paths are followed, not rejected** — by `query`/`stats` *and* by
   `validate`. `Path::join` discards the project dir for an absolute RHS, so a catalog outside the
   project loads clean, exit 0, in both. The `absolute_asset_catalog_path_is_rejected_exits_1`
   test covers asset *references inside* a catalog (`check_asset_catalog_path`), NOT the
   `asset_catalog`/`prefab_catalog` fields themselves; `load_scene_path_traversal_rejected`
   likewise covers scene paths only.

**Multi-`.project.ron` (`find_project_ron` first-match) is NOT a new divergence.** `query`/`stats`
call the same `utils::find_project_ron` `validate` does, in the same process, so they always agree
on which file wins; on NTFS `read_dir` is name-ordered and stable. Probe with two configs pointing
at different catalogs: both commands used the alphabetically-first one and never mentioned the
other project file or its catalog at all. `assets/projects/integration_tests/`'s three configs all
set `asset_catalog: "assets.ron"` + `prefab_catalog: "prefabs/prefabs.ron"`, i.e. identical to the
old literals, so its `query`/`stats` output is byte-identical before and after — and nothing in the
repo runs `query`/`stats` against it anyway (`validate_projects.rs` omits it).

**How to apply:** when asked whether `query`/`stats` "already handle" a catalog-path case, the
answer is only ever "they resolve the path" — no `validate()`, no `is_file()`, no case check, no
empty-string guard, no traversal guard. Any assertion that a `query`/`stats` exit 0 means a project
is well-formed is wrong.

**Update (`rules_to_state_machine_consolidation`, 2026-09):** the logic-half gap this note used to
flag here is fixed, and `rules.ron` no longer exists as a concept at all (so the old "22 dead
`logic/rules.ron` rules on `3rd_person_game_demo`" example can no longer happen — there is nothing
left to be dead). `query.rs` and `stats.rs` now both resolve `state_machine_path` via
`utils::resolve_catalog_paths` instead of a hardcoded `"logic/state_machine.ron"`/`"logic/rules.ron"`
literal, and `resolve_logic_files`/`resolve_catalog_paths` explicitly document that
`state_machine_path` has **no** convention-path fallback (an unset `state_machine_path` means no
logic loads at all — see `validate.rs` around line 751 and `utils.rs`'s `resolve_catalog_paths`).
The three silent modes above (wrong-type-target, `Some("")`, absolute/`..` paths) are about the
*catalog* fields (`asset_catalog`/`prefab_catalog`) and are unaffected by this update — those are
still real and still unfixed.

Related: [[cli-validate-never-calls-schema-validate]], [[try-parse-exists-not-is-file]],
[[validate-hardcoded-source-file-literals]],
[[icon-sheet-empty-string-sentinel]]
