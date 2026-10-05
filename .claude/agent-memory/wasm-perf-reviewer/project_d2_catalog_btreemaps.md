---
name: d2-catalog-btreemaps
description: D2 determinism guard swapped schema catalogs + LoadedModifiers + particle buckets to BTreeMap; perf-neutral on WASM, don't re-flag
metadata:
  type: project
---

D2 (feature/d2_determinism_guard, reviewed 2026-10-02) made AssetCatalog/PrefabCatalog/ItemCatalog/StatCatalog.modifiers/LoadedModifiers BTreeMap<String,_>, particle_renderer `buckets` BTreeMap<GroupKey,_> (GroupKey derives Ord: variant idx then texture_path memcmp), animation clip_names BTreeSet. Rest of the diff is `// det:` comments (zero cost); `tests/determinism_lint.rs` is the gate.

Verdict: perf-neutral. Reasons worth remembering:
- Catalogs are ≤~190 entries (primitive_world largest); lookups are spawn/load/action-time only. ~7-8 short memcmps ≈ one SipHash-1-3 on wasm32.
- compute_effective (per stat per frame) only calls modifier_defs.get when active_modifiers non-empty; modifier counts ≤~4 → single-leaf linear scan, beats SipHash.
- particle buckets: ≤11 groups = one leaf node = 1 alloc/frame (HashMap did 1-3 growth allocs); per-particle group_key() String clone is PRE-EXISTING and dominates.
- Catalog clones (LoadedModifiers, LoadedItemCatalog) are project-load only.
- Size: est. +10-50 KB on 30.6 MB release (see [[project-wasm-size]]).

**How to apply:** don't re-flag these swaps; the real nit is the per-alive-particle String clone in Particle::group_key (could key buckets by &str / interned id).
