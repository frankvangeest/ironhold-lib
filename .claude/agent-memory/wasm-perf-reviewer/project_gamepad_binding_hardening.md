---
name: project-gamepad-binding-hardening
description: gamepad_bind_system (FixedUpdate) centralizes the old 5x per-frame sorted-gamepad Vec into one system — net WASM win; steady-state cost is 3 tiny allocs/tick; Local diagnostic maps never pruned on despawn
metadata:
  type: project
---

Reviewed 2026-08-01, branch `feature/gamepad-binding-hardening`. Supersedes the per-system
sorted-Vec pattern recorded in [[project-gamepad-controller-input]].

**`resolve_gamepad` is DELETED.** Do not recommend it — the crate-shared helper in
`runtime/input.rs` was removed by this feature. Consumers now read the new
`BoundGamepad(Option<Entity>)` component (`capabilities/player.rs`) and do
`bound.and_then(|b| b.0).and_then(|e| gamepad_query.get(e).ok())` — an O(1) query `get`, no
sort, no alloc.

**Net per-frame WASM win, not a regression.** Before: 5 systems (`input_translator_system`,
`tab_targeting_system`, `interactable_system`, `action_bar_input_system`, `camera_orbit_system`)
each built + sorted their own `Vec<(Entity, &Gamepad)>` every frame (4 in Update, 1 in
FixedUpdate). After: all 5 do zero allocations; one new `gamepad_bind_system` in the
`.chain()`ed FixedUpdate tuple does the sort once. `action_bar_input_system` in particular used
to build the Vec unconditionally at the top even when no slot had a gamepad binding.

**`gamepad_bind_system` steady-state cost (all players bound, no pad churn):** 3 small heap
allocs per FixedUpdate tick — `Vec<Entity>` of pads, `HashSet<Entity>` `connected`,
`HashSet<Entity>` `claimed` — plus a ≤4-element sort and ~6 SipHash ops. Sub-microsecond;
~30 µs/sec at 64 Hz. Zero-gamepad / keyboard-only scenes allocate **nothing** (Bevy `QueryIter`
`size_hint` lower bound == upper bound for unfiltered queries, so `collect()` on an empty query
skips the alloc; `HashSet::with_capacity(0)` is non-allocating in hashbrown). Verified non-issue
at 2-4 players. Path to literally zero allocs if ever wanted: `connected` is redundant (linear
`sorted_gamepads.contains()` at N≤4 beats a HashSet), and `sorted_gamepads`/`claimed` can become
`Local<Vec<Entity>>` reused with `.clear()`.

**Known leak (logged, non-blocking):** the two diagnostic `Local<HashMap<Entity,f32>>` /
`Local<HashSet<Entity>>` in `gamepad_bind_system` are keyed by player `Entity` and only pruned
when that player recovers. A player who despawns (scene change / hot-leave) while in the
"stuck" state leaves a permanent entry. Growth is realistically a handful of ~20-byte entries
per session, but it is unbounded in principle across scene loads.

**Diagnostic timing is browser-correct.** The 3s `GAMEPAD_DIAGNOSTIC_WARN_SECS` threshold
accumulates `time.delta_secs()` inside FixedUpdate (= `Time<Fixed>`, deterministic fixed delta),
not `Instant` — immune to browser timer coarsening. Both `warn!`s are one-shot-guarded by a
`warned.insert()` set, which matters on web: `console.warn` is genuinely expensive per call.

**No new deps, no size impact.** `Cargo.toml`/`Cargo.lock` untouched. Uses `std::collections`,
consistent with the whole of `ironhold_core` (this codebase uses `std::collections` everywhere,
**not** `bevy::platform::collections` — keep new code consistent so no second hashbrown/hasher
monomorphization family is instantiated).
