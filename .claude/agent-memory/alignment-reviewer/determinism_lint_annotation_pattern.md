---
name: determinism-lint-annotation-pattern
description: D2 `// det: lookup-only|order-independent` source-scan guard; how to audit annotation honesty and what catalog BTreeMap conversion does/doesn't change for designers
metadata:
  type: project
---

D2 (2026-10-02) added `crates/ironhold_core/tests/determinism_lint.rs`: fails on any HashMap/HashSet
type/ctor line in ironhold_core/src lacking `// det: lookup-only` or `// det: order-independent`.
Catalog maps (AssetCatalog fields, PrefabCatalog.prefabs, ItemCatalog.items, StatCatalog.modifiers)
became BTreeMap.

**Why it matters for review:** the marker sits on the *declaration* line only; nothing checks later
`.keys()/.iter()` uses. Audit by grepping each annotated name for `.iter|.keys|.values|for .. in`.

**How to apply:**
- Designer-facing: zero RON change. serde HashMap and BTreeMap both silently last-wins on duplicate
  RON map keys; CLI already sorted every catalog listing before D2, so no output change.
- Recurring soft mislabel: "lookup-only" on maps iterated only to build `warn!` text
  (entity_spawner `build_stat_map_from_templates` stat_overrides.keys(), animation.rs
  node_indices.keys() in the missing-clip warn). Log-order only, not gameplay — non-blocking.
- Workspace-root clippy.toml `disallowed-types` cannot see `// det:` comments, yet its reason text
  says annotating silences it; it also covers ironhold_cli. Editor-noise footgun.
- External hash containers never named in source (e.g. bevy `Gltf.named_animations`) are invisible
  to the scan by construction — check them by hand when reviewing iteration changes.
Related: [[same-frame-order-determinism-pattern]], [[fixed-timestep-det-math-pattern]]
