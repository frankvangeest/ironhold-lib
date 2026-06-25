---
name: rng-and-determinism
description: RNG in core must be seeded (DeterministicRng/ChaCha8Rng), never thread_rng; getrandom js is only needed by the self-seeding path
metadata:
  type: project
---

Any RNG-using capability in `ironhold_core` should use a seeded `DeterministicRng`
from day one, NOT `rand::thread_rng()`.

**Why:**
- `thread_rng()` self-seeds from OS entropy, which on `wasm32-unknown-unknown`
  routes through `getrandom` and PANICS at runtime in the browser unless
  `getrandom = { features = ["js"] }` is added to `ironhold_web/Cargo.toml`.
  This is a silent trap: works on native, crashes on first roll in WASM.
- A seeded RNG (`seed_from_u64`) pulls no entropy, so it sidesteps the WASM panic
  entirely and `getrandom js` is never needed.
- Beta 0.5 (Deterministic Tick + Replay, see backlog) requires a seeded
  `DeterministicRng` resource anyway — so `thread_rng()` would just be migrated
  away later. Skipping it avoids an executor-signature change + Cargo.toml revert.

**RNG library choice matters for replay determinism:**
- `StdRng` is explicitly NOT reproducible across `rand` major versions — bad for replay.
- Prefer `rand_chacha::ChaCha8Rng::seed_from_u64` (or `rand_pcg`) — stable,
  documented, portable stream. Same cross-platform float-divergence concern as
  Rapier ([[determinism_networking]]): keep threshold comparisons in the integer
  domain where possible; `gen::<f32>()` must round identically native vs WASM.

**How to apply:**
- When advising any feature that rolls/randomizes (loot, spawn jitter, crit
  chance, procedural gen): recommend a shared seeded RNG resource (e.g.
  `runtime/rng.rs`), fixed-constant seed for pre-Beta, re-seed from replay header
  at Beta 0.5. Keep roll fns generic (`rng: &mut impl Rng`) so only the caller
  changes at migration.
- Flag any `thread_rng()`/`from_entropy()` in core as a WASM + determinism risk.
- Loot system (`planning/features/loot_system.md`) was the first case: advised
  seed-from-day-one and NOT adding getrandom js (2026-06-25).
