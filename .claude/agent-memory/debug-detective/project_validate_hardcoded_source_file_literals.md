---
name: validate-hardcoded-source-file-literals
description: validate.rs attributes every catalog-derived error/warning to 14 hardcoded "prefabs/prefabs.ron"/"assets.ron" source_file literals — accurate only while the catalog actually lives at the convention path, which stopped being guaranteed once catalog paths became configurable
metadata:
  type: project
---

`crates/ironhold_cli/src/commands/validate.rs` builds `CrossFileError.source_file` /
`StrictWarning.source_file` from hardcoded string literals, not from the path the catalog was
actually read from: `"prefabs/prefabs.ron"` at ~11 sites and `"assets.ron"` at ~3 (plus the same
literals baked into message text — `"prefab key {:?} not found in prefabs.ron"`,
`"... not found in assets.ron textures"`).

**Why:** this was invisibly correct for years because `do_validate` also *read* those exact
literals. Since `feature/configurable_catalog_paths` the read path comes from
`ProjectConfig.asset_catalog`/`prefab_catalog` via `load_configured_catalog()`, so a project that
relocates either gets errors attributed to a file that does not exist on disk — including in
`--json` output's `"source"` field, which tooling may consume. The file-results list above it
correctly shows the real path, so the two halves of one report disagree.

**How to apply:** any new catalog-derived check should take the resolved path rather than
re-typing the literal. When reviewing a relocation-related test, note that a scene-entity prefab
error is attributed to the *scene* path, so it will NOT catch this — you need an error whose
source is the catalog itself (e.g. `prefab.behavior` pointing at a missing file, or a `--strict`
unused-prefab warning). Related: [[logic-file-on-disk-is-not-loaded]].
