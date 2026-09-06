---
name: try-parse-exists-not-is-file
description: validate.rs's try_parse guards !full.exists() but not !full.is_file(), so feeding it a designer-authored path (empty string, a directory, a non-scene RON) yields a FileResult with a blank/garbage rel_path and a raw OS error — safe only for the hardcoded convention paths every pre-existing caller passes
metadata:
  type: project
---

`crates/ironhold_cli/src/commands/validate.rs::try_parse` (around line 135) is:

```rust
let full = project_dir.join(rel_path);
if !full.exists() { return None; }
parse_file(&full, rel_path, results)
```

Only `exists()` is guarded, never `is_file()`, and `rel_path` is echoed verbatim into the emitted
`FileResult.rel_path` — which is what the CLI prints as the filename column and what `--json`
emits as `files[].path`.

**Why it was safe until 2026-09-06:** every pre-existing caller passes a *hardcoded convention
path* (`"logic/rules.ron"`, `"overrides/model_fixes.ron"`, `load_configured_catalog`'s fallbacks).
None of those can be empty or name a directory. `feature/scene_path_validity` (`a29436d`) was the
first change to feed `try_parse` **arbitrary designer-authored strings** (`ProjectConfig.
initial_scene` and `Action::LoadScene`/`LoadSceneOverlay`/`PreloadScene`/`ToggleOverlay` paths),
which immediately surfaced three degenerate cases, all previously exit 0:

- `initial_scene: ""` -> `project_dir.join("")` is the project dir, which **exists** -> the
  existing `.exists()` cross-file guard also passes -> `read_to_string` on a directory fails ->
  a file row with a **blank filename** and `IO error: ... (os error 3)`, and `"path": ""` in
  `--json`.
- `LoadScene("levels")` (a directory) -> `IO error: Access is denied. (os error 5)`.
- `LoadScene("prefabs/prefabs.ron")` (a real, valid file of the wrong type) -> the *same*
  `prefabs/prefabs.ron` is listed twice, once `OK` (as `PrefabCatalog`) and once `ERROR`
  ("Unexpected field named `prefabs` in `GameSceneV2`"). Nothing in the output names the actual
  mistake, and the bogus failure flips `scenes_parsed_cleanly` false, silently suppressing the
  `--strict` `orphan_rule` check.

**How to apply:** any future change that routes a *designer-authored* path into `try_parse` (or
into `parse_file` directly) needs an `is_file()` guard and a non-empty check first, plus a
purpose-built cross-file error naming the referencing action — `try_parse`'s silent-`None`
contract is documented for "this convention path might not apply," not for "a designer typed
this." Related: [[validate-scene-dedup-is-exact-string]],
[[validate-reference-checks-token-blind]], [[stale-cli-binary-as-prefix-oracle]].
