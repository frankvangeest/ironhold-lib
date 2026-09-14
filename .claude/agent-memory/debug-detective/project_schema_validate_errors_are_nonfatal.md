---
name: project-schema-validate-errors-are-nonfatal
description: project_loader logs schema validate() failures with error! and then loads the invalid catalog anyway, so "validate rejects X" never means X is unreachable at runtime
metadata:
  type: project
---

`project_loader.rs` calls `catalog.validate()` for StatCatalog and ItemCatalog inside
`if let Err(e) = ... { error!(...) }` and then unconditionally does
`LoadedItemCatalog(Some(catalog.clone()))`. The error is advisory only — the malformed catalog is
installed into the world regardless.

So `ItemCatalog::validate`'s `max_stack == 0` rejection does **not** make `max_stack: 0`
unreachable: it reaches `add_to_slots`, where `count.min(0) == 0` writes an
`ItemStack { count: 0 }` into every empty slot (bounded by `max_slots`, so it terminates — but
fills the container with zero-count phantom stacks). The CLI validator makes this worse: it wires
only 2 of 6 schema `validate()`s and ItemCatalog is not one of them
([[project-cli-validate-never-calls-schema-validate]]), so nothing blocks it at design time either.

**Why:** surfaced reviewing `feature/inventory_max_stack_fix` (2026-09-12) — the fix threads the
real catalog into the prefab-spawn `add_to_slots`, which newly exposes the spawn path to whatever
the catalog actually contains, including values `validate()` "rejects".

**How to apply:** never reason "schema `validate()` forbids it, so the runtime can't see it."
Before dismissing an out-of-range authored value, check whether its loader treats the validate
failure as fatal — in `project_loader` it never does. Assume every authored numeric can arrive at
the consuming code unclamped (same lesson as [[project-nan-from-unclamped-ron-numerics]]).
