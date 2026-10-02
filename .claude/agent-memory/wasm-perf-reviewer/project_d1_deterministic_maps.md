---
name: project-d1-deterministic-maps
description: D1 HashMap->BTreeMap/IndexMap swap on gameplay maps (CooldownMap, PendingIntentActions, LoadedStats, key/gamepad bindings) — per-frame cost neutral-or-better, ~zero size
metadata:
  type: project
---

D1 (feature/d1_deterministic_gameplay_maps, reviewed 2026-10-02) swapped iterated gameplay maps to ordered types.
- LoadedStats = IndexMap<String, LiveStat> — SAME monomorph as pre-existing StatMap, so ~0 bytes of wasm; iter_mut over a dense Vec is faster than HashMap bucket scan. indexmap was already a dep.
- CooldownMap/PendingIntentActions BTreeMap: cooldown_tick early-returns on empty; flush uses mem::take (BTreeMap::new is alloc-free). Lost: HashMap::drain capacity reuse — irrelevant, BTree allocs nodes per insert anyway, press-gated.
- action_bar_input_system buffers events in Vec::new() + stable sort; idle frame = zero alloc (len<=20 stable sort is insertion sort, no scratch buffer).
- Pre-existing nit (not D1): InputMap::parse_key allocates a String for single lowercase-letter names ("q") — called per binding per frame in global_input_system. Pre-resolving KeyCode at bind-load time removes it.

**Why:** determinism for lockstep/replay; future reviewers shouldn't re-flag BTreeMap/IndexMap here as a perf regression.
**How to apply:** treat ordered-map swaps on small (<50) gameplay maps as perf-neutral; only flag if map is large and lookup-heavy per frame.
