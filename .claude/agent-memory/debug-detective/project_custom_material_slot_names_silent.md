---
name: custom-material-slot-names-silent
description: MaterialKind::Custom reads only texture_0..texture_3 from its textures map; any other slot key is silently dropped with no warn, and validate now blesses its path anyway
metadata:
  type: project
---

`material_factory.rs` resolves a `MaterialKind::Custom` material's textures with four literal
lookups — `custom_def.textures.get("texture_0")` through `"texture_3"`. Any other key in that
`HashMap<String, String>` (`"texture0"`, `"texture_4"`, `"diffuse"`, …) is **never read and never
warned about**. `CustomMaterialDef` is a plain `HashMap`, so serde accepts anything; there is no
`deny_unknown_fields` equivalent for a map key.

**Why:** as of the `asset_catalog_path_check` feature, `ironhold_cli validate` *does* existence- and
case-check every value in that map regardless of slot name (verified: a bad path under slot
`"diffuse"` is reported as `material "c".textures["diffuse"]: ... not found on disk`). That is the
right call for path checking, but it means a typo'd slot with a *valid* path now passes `validate`
completely clean while the texture silently never binds — validate looks authoritative about a
material that will render untextured. The same all-or-nothing shape as the flycam tag drops.

**How to apply:** when a Custom material "ignores my texture" in a playtest, check the slot **key**
spelling first, not the path — the path is the one thing tooling now verifies. A `--strict`
unknown-slot-name warning is the natural gap-filler and does not exist today. Same reasoning applies
to `pack_custom_uniforms`' alphabetical `floats`/`colors` packing convention.

Related: [[flycam-tag-silent-semantics]], [[action-ron-typos-are-silent]],
[[cli-validate-never-calls-schema-validate]]
