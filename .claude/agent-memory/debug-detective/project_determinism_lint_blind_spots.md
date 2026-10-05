---
name: determinism-lint-blind-spots
description: D2 determinism_lint.rs is a per-line token scan; lists exactly which hash-container usages it cannot see and which markers are honor-only
metadata:
  type: project
---

`crates/ironhold_core/tests/determinism_lint.rs` (D2, reviewed 2026-10-02 at 7637605) flags a line only if it contains `HashMap`/`HashSet` followed by `<`, `::` or `(`, outside a brace-matched `#[cfg(test)] mod` body. Blind spots, verified by reading the scanner:
- `use std::collections::HashMap as M;` / `type T = HashMap<..>; // det: ...` -> every later `M<..>`/`T` use is invisible.
- Hash maps that come from external APIs with no token on the line (e.g. `gltf.named_animations.clone()` + `for .. in &src.named_animations` in animation.rs) are never seen.
- The marker is honor-only and covers the *declaration*, never the iteration sites: `SceneEntityDef.stat_overrides` and `AnimationController.node_indices` are marked lookup-only but are iterated (`.keys()`) into warn! output.
- `#[cfg(test)]` + `mod x;` (out-of-line) would swallow the NEXT braced item; an unbalanced `"{"`/`'{'` literal in a test module extends the skip to EOF (utils.rs has real code after its test module, so this matters there).
- `read_to_string(..).unwrap_or_default()` scans an unreadable file as empty.
- clippy.toml `disallowed-types` cannot see `// det:` comments, so `cargo clippy` warns at every annotated site.

**Why:** a future "guard passed" claim is only as strong as these holes.
**How to apply:** when reviewing a determinism-sensitive diff, grep for `as HashMap`/type aliases and external-API maps by hand; don't trust a `det:` marker without checking the iteration sites of that container. Related: [[nan-from-unclamped-ron-numerics]] (same "validator silently passes" class).
