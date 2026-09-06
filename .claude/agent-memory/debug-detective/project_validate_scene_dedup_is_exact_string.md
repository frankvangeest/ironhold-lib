---
name: validate-scene-dedup-is-exact-string
description: do_validate's extra-scene discovery dedups by raw authored string, so Scenes/ vs scenes/, ./levels/ vs levels/, and backslash spellings each parse the same file twice and emit every cross-file error twice; .. escapes the project dir entirely
metadata:
  type: project
---

`do_validate` (`crates/ironhold_cli/src/commands/validate.rs`, ~line 2383 onward, added by
`a29436d` on `feature/scene_path_validity`) folds action-referenced / `initial_scene` scene paths
that the `scenes/*.scene.ron` glob missed into the same `scenes` vec every cross-file check walks.
Its two dedup guards — `already_parsed_scenes: HashSet<&str>` and
`extra_scene_paths.contains(&path.as_str())` — compare **raw authored strings**, never
canonicalized paths.

Exact-string duplicates dedup correctly (two `LoadScene`s at the same literal, or `initial_scene`
plus a matching action — both verified clean). What slips through is any spelling that differs as
a string but resolves to the same file on Windows:

- `initial_scene: "Scenes/main.scene.ron"` alongside a globbed `scenes/main.scene.ron` -> both
  parsed, both listed `OK`, the same missing-prefab error printed **twice**, "4 files checked" for
  3 files. (A `path_case_mismatch` error does also fire here, which at least hints at the cause.)
- `"levels/custom.scene.ron"` and `"./levels/custom.scene.ron"` -> duplicated error with **no**
  explanatory diagnostic, because `path_case_mismatch`'s walk deliberately bails out on `.`/`..`/
  empty segments (see its own doc comment, ~line 210).
- `"levels\\custom.scene.ron"` -> duplicated error alongside the backslash diagnostic.
- `--json` gets duplicate `files[].path` entries and byte-identical duplicate `cross_file_errors`.
- `LoadScene("../other_project/scenes/x.scene.ron")` **escapes the project dir**: a scene from a
  different project is parsed and its errors reported under this project's name.

Note `rel()` (`commands/utils.rs`) normalizes `\` -> `/` for globbed paths, so the glob side is
always forward-slashed while the authored side is whatever the designer typed — that asymmetry is
what makes the backslash case a guaranteed miss.

**How to apply:** when reviewing or extending this block, dedup on a canonical form
(`std::fs::canonicalize`, or at minimum lowercase + `\`->`/` + strip `./`) rather than the raw
string, and reject/normalize `..` before joining. Nothing downstream assumes scene ordering,
`rel_path` uniqueness, or a `scenes/` prefix (verified: no `scenes[0]`/`.first()`/`.len()` anywhere,
every consumer is a per-scene loop or a set insert) — so the *only* damage from a double-parse is
duplicated output and an inflated file count, not corrupted check results.

Related: [[try-parse-exists-not-is-file]], [[windows-case-sensitivity-probing]],
[[collect-actions-skips-actionbar-slots]].
