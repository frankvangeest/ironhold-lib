# Feature: `Action` enum `#[serde(deny_unknown_fields)]`

_Status: In Progress_
_Planned at: `cbe2f2a` (2026-09-03)_

## What

Reject unknown/mistyped fields on any `Action` variant at RON parse time instead of silently
dropping them. Today, authoring `PlayAnimationOn(target: "{self}", clip: "death", start_at_fracton: 1.0)`
(typo: `fracton`) parses cleanly — serde silently discards the unrecognized field — and the
animation plays from the start instead of seeking, with zero diagnostic from the engine,
`ironhold_cli validate`, `ron_lint`, or anywhere else. This closes that gap the same way
`PrefabComponents` already was (`22e7749`, May 2026): a single `#[serde(deny_unknown_fields)]` on
the `Action` enum's derive.

## Why

Ranked in the top 2 by 3 of 5 stakeholders in `planning/stakeholder_priority_list.md` (2026-09-03)
— system-architect (schema-as-API-surface integrity), alignment-reviewer (systemic silent
authoring-path divergence), ux-gamedesigner-reviewer (worst-case non-programmer debugging
experience: no error text, no stack trace, just RON that silently does nothing). `Action` is the
single highest-traffic authoring surface in the whole data-driven pipeline — every rule, state
machine transition, behavior file, and dialogue choice authors a `Vec<Action>` — so a typo here can
originate from any of those four authoring surfaces and be invisible in all of them alike.

## Approach

- Add `#[serde(deny_unknown_fields)]` to `Action`'s existing `#[derive(Debug, Clone, Deserialize, PartialEq)]`
  in `crates/ironhold_core/src/schema/actions.rs`.
- Confirmed by inspection: no variant uses `#[serde(flatten)]` (which is incompatible with
  `deny_unknown_fields`), so this is a single-attribute change with no per-variant rework needed.
  `deny_unknown_fields` is a no-op for tuple/newtype variants (`LoadScene(String)`,
  `SetVariable(String, String)`, etc.) — it only takes effect on the struct-shaped variants
  (`Spawn { .. }`, `PlayAnimationOn { .. }`, `CameraShake { .. }`, etc.), which is exactly where a
  stray field can currently hide.
- **Compatibility sweep before landing** (per the backlog item's own note — this is the actual
  risk, not the code change): every shipped project's `rules.ron`/`state_machine.ron`/
  `behaviors/*.behavior.ron`/`dialogues/*.dialogue.ron` must still parse cleanly. Run the full
  `ironhold_core` test suite (which parses every example project's RON via `ron_validation.rs` and
  project-specific tests) plus `ironhold_cli validate` against every `assets/projects/*` directory
  before considering this done. Any real hit found here is itself a pre-existing authoring bug this
  feature is designed to surface — fix the RON, not the attribute.
- No schema version bump — this is a stricter parse, not a new field; existing correct RON is
  unaffected.

## Tasks
- [x] Add `#[serde(deny_unknown_fields)]` to `Action`
- [ ] Full `ironhold_core` test suite green (parses every example project's action-bearing RON)
- [ ] `cargo check -p ironhold_cli` green (mandatory schema-change gate)
- [ ] Sweep every `assets/projects/*` with `cargo run -p ironhold_cli -- validate` (not the cached binary — schema just changed)
- [ ] Fix any real stray-field RON hit found by the sweep (expected: none, but this is the point of the change)
- [ ] Docs: note in `docs/20_data_formats.md`'s `Action` section that unknown fields are now a hard parse error
- [ ] `crates/ironhold_core/src/CLAUDE.md`: same note if it documents Action's parsing behavior
- [ ] WASM dev build + play-test checklist

## Open questions
- None — this is a well-scoped, low-risk hardening change with clear precedent (`PrefabComponents`).

## Acceptance criteria
- Given a `.ron` file authoring an `Action` variant with a field name not defined on that variant,
  when the project is parsed (engine load, `ironhold_cli validate`, or a unit test), then parsing
  fails with a clear serde field-name error instead of silently succeeding.
- Given every currently-shipped `assets/projects/*` RON file, when parsed under the new attribute,
  then parsing still succeeds unchanged (no false positives against real, correct RON).
