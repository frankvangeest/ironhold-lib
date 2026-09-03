# Feature: CLI validate hardening (batch: scene-path check + merchant cross-validation)

_Status: Done_
_Planned at: `06022a8` (2026-08-30)_
_Reviewed by alignment-reviewer, system-architect, debug-detective, ux-gamedesigner-reviewer
(2026-08-30) — all four independently converged on the same two real gaps (missing
`ToggleOverlay` coverage, silent missing-`items_path` case); findings folded in below. See
`planning/claude_suggestions.md` ▸ CLI / Tooling for the deferred follow-ups._

**Real bug found by the new check during verification**: `quick_scene`'s `logic/rules.ron` had a
rule `ui.button_pressed:start_menu → LoadScene("scenes/start_menu.scene.ron")` — that scene file
never existed, and no button in the project ever fires `ui.button_pressed:start_menu` either (100%
dead RON, likely copy-paste leftover from another project's start-menu pattern). Removed the rule.
This is exactly the class of bug item 1 exists to catch — not a false positive.

**Review findings folded in (all four reviewers independently found #1 and #2):**
1. **`Action::ToggleOverlay(String)` is a fourth scene-path variant and was missed** — it's
   resolved by the identical runtime path as `LoadScene`/`LoadSceneOverlay`/`PreloadScene`, and is
   actually used with real paths in `primitive_world/logic/state_machine.ron`. Added to the same
   match arm; extended the `bad_scene_path` fixture with a `ToggleOverlay` case to prove it.
2. **A configured-but-missing `items_path` was silently swallowed** — `try_parse` returns `None`
   for a non-existent file with no diagnostic at all, correct for a *convention* path (no file =
   feature not used) but wrong for a *configured* one (the designer explicitly asserted a file
   should be there, and the runtime unconditionally tries to load it). Now pushes a `FileResult`
   naming the configured path. New fixture `bad_items_path` + test.
3. **`ProjectConfig.initial_scene` was still unchecked anywhere in the CLI** (3/4 reviewers) — the
   highest-consequence scene path of all (a typo boots to a blank screen), same resolver, same
   class of bug this whole item exists to close. Added the same exists-check. New fixture
   `bad_initial_scene` + test.
4. **Docs gaps, both fixed:** `docs/60_contributing.md`'s "Checks performed" list (the canonical
   designer-facing answer to "what does validate catch?") never mentioned either new check —
   added. The new `docs/20_data_formats.md` note pointed at a `LoadSceneOverlay` table row that
   didn't exist — added the three missing overlay-action rows (`LoadSceneOverlay`, `UnloadOverlay`,
   `ToggleOverlay`) instead of just rewording around the gap.
5. **Stale, adjacent doc claim fixed while touching the same paragraph:** `docs/20_data_formats.md`
   said merchant buy/sell were "display-only in v1; planned for v1.1" — verified against
   `action_executor.rs`'s actual `Action::BuyItem` handler: buying is fully implemented (deducts
   `currency_stat`, adds the item, emits `item.bought:{item_key}`); only selling (no `SellItem`
   action exists) is genuinely unimplemented. Corrected both the inline `currency_stat` field note
   and the "v1 scope note" callout.
6. **Message clarity (debug-detective):** `currency_stat` defaults to `"gold"` when omitted
   entirely, so a merchant with no stats.ron `"gold"` entry gets an error message quoting a value
   the designer may never have typed. The error now notes when the missing value is the schema
   default rather than implying a typo. Also applied the "paths are relative to the project
   folder" clarification (ux-gamedesigner-reviewer) to the scene-path message.
7. **Deliberately not done, logged instead:** moving the merchant-check loop to interleave with
   the existing per-prefab loop at `validate.rs:717` (system-architect, code-organization only,
   not correctness); sorting `cross_errors` for deterministic output (debug-detective, pre-existing
   across the whole file, not introduced here); the half-dozen adjacent unvalidated item-key sites,
   the hardcoded-vs-configurable catalog-path asymmetry, `collect_actions` skipping dialogue files,
   the Windows NTFS case-insensitivity gap, and `quick_scene`'s remaining dead `pause_button`/
   `start_game`/`test_actions` RON (deferred specifically to the not-yet-built "UI trigger
   reachability check" item, which would catch all three systematically) — all logged to
   `planning/claude_suggestions.md` ▸ CLI / Tooling rather than expanding this branch's scope.

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
project consuming the WASM build (`planning/backlog.md` ▸ Bugs, `ea72d72`); item 2 has been Queued
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
- [x] Add the `LoadScene`/`LoadSceneOverlay`/`PreloadScene`/`ToggleOverlay` match arm.
- [x] Add the `ProjectConfig.initial_scene` exists-check.
- [x] Load `items.ron` (via `ProjectConfig.items_path`) into `do_validate`, thread through
      `cross_file_checks` — reporting (not silently skipping) a configured-but-missing path.
- [x] Add the merchant `currency_stat`/`item_key` cross-checks.
- [x] Fixture + test for each check (`crates/ironhold_cli/tests/fixtures/`,
      `validate_cross_file.rs`) — 7 new tests total (`missing_scene_path_in_load_scene_exits_1`
      covering both `LoadScene` and `ToggleOverlay`, `missing_initial_scene_exits_1`,
      `missing_items_path_target_exits_1`, `missing_merchant_currency_stat_exits_1`,
      `missing_merchant_item_key_exits_1`).
- [x] Verify against every real example project — all 13 validate clean, including
      `3rd_person_game_demo`'s real merchant/items.ron/stats.ron and `primitive_world`'s real
      `ToggleOverlay` usage. Found and fixed one real pre-existing bug along the way
      (`quick_scene`'s dead `start_menu` rule, see above).
- [x] Docs: `docs/20_data_formats.md` (`LoadScene`/`LoadSceneOverlay`/`UnloadOverlay`/
      `ToggleOverlay` rows + `MerchantDef` fields + corrected stale buy/sell scope note),
      `docs/30_runtime_events_and_logic.md` (matching action bullets), `docs/60_contributing.md`
      ("Checks performed" list — the actual canonical designer-facing summary, missed in the
      first pass).

## Open questions
- None — both checks mirror existing, already-established patterns in the same file.

## Acceptance criteria
- A rule/behavior/state-machine action referencing a `LoadScene`/`LoadSceneOverlay`/`PreloadScene`/
  `ToggleOverlay` path that doesn't exist on disk causes `validate` to exit 1 with a `missing_file`
  error naming the path.
- A project's `initial_scene` that doesn't exist on disk causes `validate` to exit 1.
- A configured `items_path` that doesn't resolve to a real file causes `validate` to exit 1 naming
  that path (not silently skipped).
- A merchant prefab's `currency_stat` not defined in `stats.ron` causes `validate` to exit 1.
- A merchant prefab's `stock[].item_key` not defined in `items.ron` causes `validate` to exit 1.
- Every shipped example project still validates clean (zero new false positives), including
  `3rd_person_game_demo`'s real merchant and `primitive_world`'s real `ToggleOverlay` usage.
- A project with no `items_path` (no item catalog at all) sees no new errors from the merchant
  check, even if it happens to author a `merchant:` block (should be treated as pre-existing/
  unrelated to this check — no `items.ron` to check against).
