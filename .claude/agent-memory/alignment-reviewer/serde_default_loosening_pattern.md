---
name: serde-default-loosening-pattern
description: Reviewing changes that ADD #[serde(default)] to previously-required schema fields — the typo-detection trade-off, the duplicate-hardcoded-constructor drift risk, and the stale-RON-comment sweep
metadata:
  type: project
---

The mirror image of [[schema-strictness-hardening-pattern]]. When a change makes a schema type
parse *more leniently* (adding `#[serde(default = "...")]` to fields that were required), the
reachability axis is trivially improved — a partial RON block that used to hard-fail now works.
Three things to check instead. Established during the `feature/input_map_defaults` review
(2026-09-10, `InputMap`'s 7 movement/jump fields).

**Why:** a missing-field parse error is also an incidental *typo detector*. Serde ignores unknown
fields unless `deny_unknown_fields` is set, so once every field has a default, `fowrard: "KeyT"`
parses cleanly, is dropped, and the field silently takes its default. Before the loosening, the
same typo produced `missing field 'forward'` and stopped the designer immediately.

**How to apply — three checks on any serde-default-loosening diff:**

1. **Does the struct carry `#[serde(default)]` on all fields but *not* `deny_unknown_fields`?**
   If so, recommend adding it in the same change. Precedent: `FlyCamDef` (`schema/catalog.rs`) —
   the direct sibling key-binding struct — is all-defaults **and** `deny_unknown_fields`, and its
   parent `PrefabComponents` has it too. `InputMap` (`schema/player.rs`) is the outlier: as of
   2026-09-10 all fields default and there is no `deny_unknown_fields`, and its keyboard key-name
   values are neither runtime-warned nor CLI-validated (see [[keybinding-parse-key-vocabulary]]),
   so a typo'd *field name* there has zero diagnostic anywhere.
2. **Is there a hardcoded Rust constructor duplicating the same defaults?** Two sources of truth
   now exist and the compiler will not catch drift. `FlyCamDef` solves this with
   `impl Default for FlyCamDef` calling the same `default_flycam_*()` fns; `InputMap` still has a
   hand-written `default_input_map()` in `runtime/scene_manager/entity_spawner.rs` (its sole
   production construction site, used when `components.inputs` is absent entirely). Delegating it
   to the schema default fns needs them `pub(crate)` — precedent already exists
   (`schema::player::default_fov()` is `pub(crate)` and called from `entity_spawner.rs`).
3. **Sweep shipped RON *comments*, not just docs.** Demo-project RON comments are designer-facing
   documentation and go stale invisibly (no test, no lint, no `cli validate` check reads them).
   Real case: `assets/projects/entity_logic_demo/prefabs/prefabs.ron` carried a multi-line comment
   explicitly telling designers the keyboard fields "must still be spelled out" because they "have
   no serde default" — exactly the sentence the fix falsified. `grep`-check `assets/projects/**`
   for the constraint being removed, in addition to `docs/20_data_formats.md`'s field table (whose
   Default column is often already *aspirationally* correct and needs no edit — but its
   "omit the entire block" phrasing usually does).
