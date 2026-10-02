---
name: deterministic-container-facts
description: D1 (2026-10-02) container choices for iteration-order determinism + verified std/indexmap facts (WASM std hasher keys come from stack/heap addresses, indexmap 2 deprecates remove)
metadata:
  type: project
---

D1 (`feature/d1_deterministic_gameplay_maps`, reviewed 2026-10-02) converted: `PendingIntentActions`/`CooldownMap` -> BTreeMap (slot-key byte order), `LoadedStats`/`StatCatalog.stats` -> IndexMap (stats.ron declaration order, Frank's decision), Loaded/Project Key/Gamepad bindings -> BTreeMap (implementer chose BTreeMap over the backlog's "resolve-once sorted Vec"; same order, per-frame parse_key cost unchanged). `action_bar_input_system` buffers GameEvents and stable-sorts by slot_key before writing.

Verified facts:
- std `RandomState` on wasm32-unknown-unknown: `sys/random/unsupported.rs` seeds from a stack addr + a fresh Box addr — not random, but depends on allocation history (asset-load timing), plus k0 increments per instance. So "nondeterministic per run" is true on native, merely fragile on WASM; native-vs-WASM divergence is certain.
- Bevy's own `bevy::platform::collections::HashMap` uses FixedHasher (deterministic) — only explicit `std::collections::HashMap` sites are seed-random. Don't over-flag Bevy-hashmap sites as run-to-run random (they are still insertion-history dependent).
- indexmap 2.13 (Cargo.lock): `remove` is deprecated (= swap_remove), so an accidental order-breaking removal warns. Serde Deserialize inserts in source order; duplicate key keeps first position, last value.
- Cooldown `retain` has no order-dependent side effect — converting CooldownMap was hygiene, not a behavior fix.

**How to apply:** D2's source-scan gate should target `std::collections::Hash*` and Bevy hashmaps separately; residual order-dependence after D1 is entity/query order (D4) and cross-system writer order (D3), not containers. Related: [[determinism-networking]], [[rng-and-determinism]].
