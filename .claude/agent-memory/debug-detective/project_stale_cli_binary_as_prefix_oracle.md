---
name: stale-cli-binary-as-prefix-oracle
description: The gitignored tools/bin/ironhold.exe can be run against a new CLI test fixture to get the PRE-fix validate output with zero cargo invocation — the only way to falsify a validate.rs review when a background cargo run forbids building
metadata:
  type: project
---

`tools/bin/ironhold.exe` (gitignored, built from whatever `ironhold_cli` source was current when
it was last cached) can be invoked directly against any fixture/project directory. Because it is
stale relative to an in-flight `validate.rs` change, it is a **pre-fix oracle**: run it on the new
fixture and you see exactly what validate emitted *before* the diff, with no `cargo` process and
no touch of the shared `CARGO_TARGET_DIR`.

**Why:** this repo's review workflow routinely runs concurrently with a background
`cargo test -p ironhold_core --test '*'`, and concurrent cargo against the shared target dir has
corrupted builds here — so "just run the validator" is normally off the table during a review. The
cached binary sidesteps it entirely. It also settles two questions a static read cannot:
(a) does the new fixture RON actually parse (look for `scenes/x.scene.ron   OK`), and
(b) which errors the fixture *already* produced before the change.

**How to apply:** during any `crates/ironhold_cli/src/commands/validate.rs` review, run
`./tools/bin/ironhold.exe validate <abs path to the new fixture dir>` from the primary checkout
(the binary lives there, not in feature worktrees) and diff its output against what the new code
should add. Two recurring findings this exposes:

- **A new fixture often trips a pre-existing unrelated check**, so `assert_eq!(code, 1)` and any
  substring the old check already prints prove nothing about the new one. Real case
  (2026-09-04, `feature/duplicate_gamepad_index_join_prefabs`): a `kind: Primitive` prefab placed
  in `join_prefab_keys` already fired `unsupported_join_prefab` containing the literal
  `join_prefab_keys[1]` — two of the new test's four assertions were satisfied pre-fix. Fixtures
  aimed at one check should be built to trip only that check.
- `flycam_player_tag_conflict/prefabs/prefabs.ron` is the precedent that a `kind: Actor` fixture
  needs **no** `assets.ron` — `asset_catalog` is `None`, so model-reference checks are skipped
  entirely. Use Actor, not Primitive, whenever the fixture must survive the join/GLB-only guards.

Related: [[action-ron-typos-are-silent]] (the same stale binary, there a *hazard* rather than a
tool — it silently accepted fields the current schema rejects), [[validate-reference-checks-token-blind]],
[[validate-hardcoded-source-file-literals]].
