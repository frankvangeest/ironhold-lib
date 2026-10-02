---
name: same-frame-order-determinism
description: D1 HashMap->BTreeMap/IndexMap conversions make RON file order / key-name order designer-visible; what to check when a review touches same-frame event emission order
metadata:
  type: project
---

D1 (feature/d1_deterministic_gameplay_maps, reviewed 2026-10-02) turned map iteration order into
designer-facing semantics:
- `StatCatalog.stats`/`LoadedStats` = IndexMap -> stats.ron declaration order drives same-frame
  threshold/modifier-expiry event order (indexmap `serde` feature; ron visits map entries in file order).
- Key/gamepad bindings = BTreeMap -> key-NAME (byte-lexicographic) order, not file order.
- Action bar events buffered + stable-sorted by slot key -> lexicographic, so "10" sorts before "2".
- Per-entity `StatMap` order = prefab `stat_templates` Vec order (was already deterministic, undocumented).
- FSM reads UiEvents, then GameEvents, then SceneEvents per frame; each event processed in turn and a
  transition mutates state before the next event is matched (so "first transition wins" is approximate).
- Sort is on the slot key/binding string AS AUTHORED, and parse_key accepts aliases ("1"/"Digit1",
  "q"/"Q"/"KeyQ") — so "Digit1" sorts after "2"; a numeric "10" slot is not a key name at all (click-only).
- Schema fields (`global_key_bindings` etc.) stay `HashMap`; only runtime resources became BTreeMap
  (project_loader collects; scene_loader merges) — CLI only uses contains_key/keys/values, unaffected.
- `LoadedModifiers` stays a HashMap — safe only because it is lookup-only, never iterated for emission.

**Why:** shipped projects at review time had thresholds on only one stat (player_health) and all
`stat.modifier.expired:*` bindings did the same SetVariable, so reordering was behavior-neutral — a
future project with thresholds on 2+ stats is the first one where file order changes outcomes.
**How to apply:** when reviewing any new emitter that loops a map/query, ask which order designers
see and whether docs/20 states it; flag lexicographic-vs-numeric surprises; ask for a RON-parse
order test, not just a hand-built-map test. Related: [[system-ordering-determinism-fixes]].
