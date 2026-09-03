---
name: validate-coverage-gaps
description: ironhold_cli validate checks key-lookup fields asymmetrically — sibling fields with identical names/semantics are often unchecked; docs never say which half is covered
metadata:
  type: project
---

`ironhold_cli validate` cross-file checks are added one field at a time, so **sibling fields with
the same name and the same failure mode are routinely left unchecked** — and the docs note added
alongside each new check never says the check is scoped to only one of them.

Known asymmetries (verified against current `validate.rs`):

| Checked | Unchecked sibling |
|---|---|
| `MerchantDef.currency_stat` in `stats.ron` | `ItemDef.currency_stat` in `items.ron` (e.g. `gold_coin`) |
| `MerchantDef.stock[].item_key` in `items.ron` | `InventoryContainerDef.initial_items[].item_key` (chest_01/chest_02 in 3rd_person_game_demo) |

**CLOSED:** `Action::ToggleOverlay(String)` is now checked in the same match arm as `LoadScene` /
`LoadSceneOverlay` / `PreloadScene` (`validate.rs` ~242: `Action::LoadScene(path) |
Action::LoadSceneOverlay(path) | Action::PreloadScene(path) | Action::ToggleOverlay(path) => {...}`)
— no longer an unchecked sibling. Do not re-flag it.

Also: the merchant checks **silently skip entirely** when `ProjectConfig.items_path` is unset or
`stats/stats.ron` is absent. Copying `merchant_vendor` into a new project without setting
`items_path` therefore produces an empty shop with zero validate output — the single most likely
beginner failure for that feature, and it is the one case the check cannot see.

House style for messages is good and worth matching: `prefab {:?}: <field> {:?} not found in
<file>` (mirrors the older `prefab {:?}: behavior {:?} not found on disk`). `{:?}` quoting makes
values greppable straight back into the RON. `error_type` reuse: `missing_file` for on-disk paths,
`missing_reference` for catalog key lookups.

**Why:** designers read the checks list as "if validate passes, my keys are good." Partial coverage
of a field family is worse than none, because it teaches false confidence.

**How to apply:** whenever a new `validate` check lands, enumerate every *other* field in the
schema with the same lookup target and either cover it or say so explicitly in the docs note.
Always check `docs/60_contributing.md` ▸ "Checks performed" was updated — see [[docs-lag-actions]].
