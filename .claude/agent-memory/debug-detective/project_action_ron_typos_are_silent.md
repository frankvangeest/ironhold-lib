---
name: action-ron-typos-are-silent
description: schema::Action has no deny_unknown_fields, so a misspelled struct-variant field in any rules/behavior/state_machine RON is silently dropped by both the engine and ironhold_cli validate
metadata:
  type: project
---

`crates/ironhold_core/src/schema/actions.rs`'s `pub enum Action` is `#[derive(Deserialize)]` with
**no `#[serde(deny_unknown_fields)]`** anywhere on the enum or its variants. Serde therefore
ignores unknown keys inside a struct variant. A typo'd optional field (`start_at_fration: 1.0`,
`freze: true`, `delay_sec:`) parses clean, the field silently takes its `#[serde(default)]`, and
the feature just doesn't happen — no error from the engine and no error from
`ironhold_cli validate`.

**Proof (2026-08-26):** a `tools/bin/ironhold.exe` built Aug 24 — predating
`PlayAnimationOn`'s `start_at_fraction`/`freeze` fields entirely — reported
`logic/rules.ron OK` / `all valid` for `assets/projects/dynamic_animation_control`, whose
`rules.ron` is nothing but seven `PlayAnimationOn(..., start_at_fraction: X, freeze: Y)` calls.
The binary could not possibly know those fields; it dropped them and passed.

Two follow-ons worth remembering:
- Range/value checks added to `validate.rs`'s `cross_file_checks` only fire when the field name
  is spelled correctly. They give false confidence against the more likely authoring mistake.
- `collect_actions` (validate.rs) walks `rules.ron`, `state_machine.ron` and
  `behaviors/*.behavior.ron` only — **dialogue node actions are not collected**, so any
  action-level validation misses dialogue files entirely.
- `deny_unknown_fields` is a serde *container* attribute, not a variant attribute, so it can only
  be applied to the whole `Action` enum — cheap to try, but it would hard-fail any existing RON
  carrying a stray key, so it needs a full `cargo test --test ron_validation --test ron_lint`
  sweep plus a validate pass over every project before being adopted.

**How to apply:** whenever a feature's symptom is "the RON field seems to do nothing", diff the
authored key against the Rust field name character by character *before* investigating the
runtime. And after any schema change, rebuild `tools/bin/ironhold.exe` (root CLAUDE.md step 6) —
a stale cached binary reports OK for fields it has never heard of. See
[[project_serve_py_stale_checkout_trap]] for the same stale-tooling failure shape on the web side.
