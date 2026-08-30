# Feature: CLI validate hardening (batch: scene-path check + merchant cross-validation)

_Status: Ready_
_Planned at: `e5bed27` (2026-08-30)_

## What
Two small, independent `ironhold_cli validate` cross-file checks, batched into one branch since
both are trivial, mechanical, and touch the same file/pattern:

1. `Action::LoadScene`/`LoadSceneOverlay`/`PreloadScene` paths are never checked for existing on
   disk — all three silently fall through `cross_file_checks`'s `_ => {}` catch-all.
2. A `MerchantDef`'s `currency_stat` and each `ShopEntry.item_key` are never cross-checked against
   `stats.ron`/`items.ron` — a typo in either only surfaces as a runtime no-op the first time a
   player opens the shop.

## Why
Both are exactly the class of mistake `validate` already catches for every other reference field
(prefab keys, effect keys, audio keys, modifier keys) — these two are just gaps in that otherwise-
consistent coverage, not new categories of check. Item 1 was reported by an external downstream
project consuming the WASM build (`planning/backlog.md` ▸ Bugs, `5932784`); item 2 has been Queued
since before this session.

## Approach

### 1 — Scene-path existence check
Mirror the existing `behavior_path` exists-on-disk check (same file, `project_dir.join(path).exists()`
pattern). Add a match arm to `cross_file_checks`'s per-action loop, right after the `PreloadGlb` arm:
```rust
Action::LoadScene(path) | Action::LoadSceneOverlay(path) | Action::PreloadScene(path) => {
    if !project_dir.join(path).exists() {
        errors.push(CrossFileError {
            source_file: source.clone(),
            message: format!("scene path {:?} not found on disk", path),
            error_type: "missing_file",
        });
    }
}
```
`project_dir: &Path` is already a parameter of `cross_file_checks` — no signature change needed.

### 2 — Merchant cross-validation
`items.ron` isn't loaded anywhere in `validate.rs` today (unlike `stats.ron`, already loaded via a
hardcoded `"stats/stats.ron"` path). Unlike stats, `items.ron`'s path is **configurable** via
`ProjectConfig.items_path: Option<String>` (`3rd_person_game_demo` uses `"items/items.ron"`) — load
it via that field, `None` meaning no item catalog for this project (skip the check entirely, not an
error, since not every project has items).

In `do_validate`: after `project_config` is parsed, resolve and parse `items_path` into an
`Option<ItemCatalog>`, and thread it into `cross_file_checks` as a new parameter alongside
`stat_catalog`.

New check, walking every prefab in `prefab_catalog` with `components.merchant: Some(m)`:
- `m.currency_stat` not in `stat_catalog.stats` → `CrossFileError` (`error_type:
  "missing_stat_reference"` — reuse if an existing error_type already covers "stat key not
  found", else add one).
- each `m.stock[].item_key` not in `item_catalog.items` → `CrossFileError` (`error_type:
  "missing_reference"`, matching every other key-lookup error in this file).
- Only runs when both the relevant catalog and at least one merchant prefab exist — a project with
  no `items_path` or no merchant prefabs sees no new errors or warnings.

## Tasks
- [ ] Add the `LoadScene`/`LoadSceneOverlay`/`PreloadScene` match arm.
- [ ] Load `items.ron` (via `ProjectConfig.items_path`) into `do_validate`, thread through
      `cross_file_checks`.
- [ ] Add the merchant `currency_stat`/`item_key` cross-checks.
- [ ] Fixture + test for each of the two checks (`crates/ironhold_cli/tests/fixtures/`,
      `validate_cross_file.rs`).
- [ ] Verify against every real example project (`cargo run -p ironhold_cli -- validate
      assets/projects/<name>` for all of them) — confirm zero new false positives, and that
      `3rd_person_game_demo`'s real merchant/items.ron/stats.ron validates clean.
- [ ] Docs: no schema change, but note the two new check types in `crates/ironhold_core/src/CLAUDE.md`
      or wherever `ironhold_cli validate`'s check coverage is otherwise documented, if anywhere.

## Open questions
- None — both checks mirror existing, already-established patterns in the same file.

## Acceptance criteria
- A rule/behavior/state-machine action referencing a `LoadScene`/`LoadSceneOverlay`/`PreloadScene`
  path that doesn't exist on disk causes `validate` to exit 1 with a `missing_file` error naming
  the path.
- A merchant prefab's `currency_stat` not defined in `stats.ron` causes `validate` to exit 1.
- A merchant prefab's `stock[].item_key` not defined in `items.ron` causes `validate` to exit 1.
- Every shipped example project still validates clean (zero new false positives), including
  `3rd_person_game_demo`'s real merchant.
- A project with no `items_path` (no item catalog at all) sees no new errors from the merchant
  check, even if it happens to author a `merchant:` block (should be treated as pre-existing/
  unrelated to this check — no `items.ron` to check against).
