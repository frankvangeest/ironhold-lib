# Determinism and Networking

> **Doc type:** Design Notes (vision)
>
> **Status legend:**
> - ✅ **Implemented** — exists in code today
> - 🧪 **Prototype / Partial** — exists but incomplete or unstable
> - 🧭 **Planned** — intended design; not implemented yet

## Status
🧭 Mostly planned (design notes; not implemented yet) — see "Physics determinism" below for the
one sub-area with real ✅ items already in place.

## Why we care
Multiplayer (especially rollback/prediction) becomes far simpler if gameplay simulation is **deterministic**:

> Given the same initial state and the same input stream per tick, results should match across platforms.

Determinism also improves:
- Debugging (replayable runs)
- Testing (golden replays)
- Tooling (record/rewind)

## Scope and philosophy
We do **not** require the entire engine to be deterministic from day 1.

We split the runtime into two conceptual layers:

1. **Deterministic gameplay core (truth)** 🧭
   - Fixed-step simulation
   - Uses only deterministic data types and algorithms
   - Driven by an input stream

2. **Non-deterministic presentation (effects)** 🧭
   - Rendering, animation blending, audio, particles
   - Can be platform-specific or frame-rate dependent
   - Reads from the deterministic state, but does not affect it

This separation allows the project to ship playable builds early while still enabling a clean path to multiplayer.

## What “deterministic” means (practically)
For Ironhold, determinism means:

- **Fixed tick** for gameplay (e.g., 60 Hz) 🧭
- **Ordered, explicit inputs** per tick 🧭
- **No hidden sources of entropy** in the gameplay core 🧭
  - no wall-clock time
  - no nondeterministic iteration ordering
  - no floating-point differences without mitigation

## Common pitfalls (and mitigations)

### RNG in gameplay capabilities 🧭

_(No current violations, verified 2026-09-15: `ironhold_core` has zero `rand` usage today. This
section is a forward-looking guardrail for the first capability that needs randomness, not a
fix for existing code.)_

Any gameplay capability that uses randomness (loot rolls, procedural decisions, AI variance) must use an explicit seeded RNG — never `rand::thread_rng()` or any `from_entropy()` call.

Rules:
- Use `rand_chacha::ChaCha8Rng` — its algorithm is stable across `rand` major versions and is WASM-safe (no OS entropy call, no `getrandom` dependency required)
- Expose it as a named `Resource` (e.g. `LootRng(ChaCha8Rng)`) so multiple systems share and advance the same stream
- Seed from a fixed constant for v1 builds; at Beta 0.5 re-seed from the replay header so replays reproduce identical rolls
- Write the rolling function as `fn roll(&self, rng: &mut impl Rng)` so the seed source can be swapped without touching the logic

`thread_rng()` is banned from `ironhold_core`: it is non-deterministic and panics in WASM without a `getrandom = { features = ["js"] }` feature flag, which pulls in an unnecessary dependency. A seeded `ChaCha8Rng` sidesteps both problems.

### Floating-point differences 🧭
Different CPUs and WASM runtimes can produce tiny float differences that amplify over time.

Mitigations:
- Prefer integer/fixed-point math for core state where feasible
- Quantize/round at boundaries (e.g., store positions in fixed precision)
- Keep floating-point usage in presentation

### Iteration order / hash maps 🧭
Unordered collections can produce different iteration order.

Mitigations:
- Use stable ordering (Vec + sort, BTreeMap)
- Avoid relying on iteration order for gameplay decisions

### Dynamic spawn ids are not reproducible across runs 🧭
`SpawnRegistry.counter` (backing both `Action::Spawn`'s auto-generated id and the `{new_id}` RON
substitution token) is a plain per-scene increment, but its *value* on any given spawn depends on
`ActionQueue` execution order — which mixes `Update`- and `FixedUpdate`-originated events, so it is
not guaranteed to land on the same number across different machines or frame rates. Unique within
one session's scene, not reproducible byte-for-byte across runs.

Mitigations:
- Never use a dynamically-generated spawn id (auto-generated or `{new_id}`-derived) as a save-file
  or network-sync key — key on something author-stable instead (prefab key + a designer-authored
  slot id).

### Physics determinism 🧪
_(Updated 2026-09-15 — system-architect investigation into
`planning/stakeholder_priority_list.md`'s "Rapier cross-platform float divergence" item.)_

✅ Rapier's `enhanced-determinism` feature is already enabled
(`crates/ironhold_core/Cargo.toml:16`) — it unifies transcendental math (`sin`/`cos`/`acos`/etc.)
across platforms via `libm` and disables the native-only MXCSR denormal-flush-to-zero
optimization, both of which are real, verified sources of native-vs-WASM divergence that this
feature closes (it also, as a side effect, makes rapier's joint wake-up iteration order
deterministic, and is mutually exclusive with rapier's SIMD feature — `enhanced-determinism` and
SIMD cannot both be enabled, so this class of regression cannot be introduced silently, but it is
a standing SIMD-throughput cost paid today). Per Rapier's own documentation this gives bit-level
cross-platform determinism under normal use — same rapier version, same feature flags, on
IEEE-754-2008-compliant platforms (conditions this engine already meets) — but **not
independently proven for this engine's specific usage yet**, hence 🧪 not ✅ for the section as a
whole.

What's actually left, in priority order:

- 🧭 **Variable timestep** (the real remaining blocker) — `capabilities/physics.rs` runs Rapier on
  wall-clock `dt`, not a fixed tick. Different frame rates feed the solver different inputs
  regardless of float determinism. See `planning/features/deterministic_fixed_timestep.md` (v1).
- 🧭 **Three un-`libm`'d transcendental call sites** in gameplay code bypass Rapier's libm-forcing
  and could still diverge by a ULP: `player.rs:289`'s `acos` for slope-angle (feeding a
  grounded/airborne branch), `player.rs:579`'s rotation, and `motion.rs:38-55`'s
  `Quat::from_rotation_*`/`.sin()` bob — the last of which is worse-shaped than the first two since
  it runs in `Update` on wall-clock time and can touch entities with colliders. Fix once the
  timestep is fixed. See `planning/features/deterministic_fixed_timestep.md` (v1).
- 🧭 **Unverified in practice** — no harness yet actually compares native vs. WASM-Chrome vs.
  WASM-Firefox state over time. See `planning/features/deterministic_fixed_timestep.md` (v2).

✅ Authoritative gameplay logic is already effectively separate from raw physics: the engine has no
free-body dynamic rigid-body gameplay (dynamic bodies are only rotation-locked player/NPC
capsules) and gameplay overwrites velocity every tick rather than trusting accumulated physics
state — this is the property that makes the rest of this section tractable at all.

Mitigations (remaining):
- Fix the timestep before drawing any conclusion about cross-platform behavior
- Route the three known transcendental call sites above through `libm` directly
- Measure with a real divergence harness before committing a networking model to bit-determinism

## Networking models (planned)

### 1) Lockstep 🧭
- All peers run the same deterministic simulation
- Everyone advances tick N only when they have all inputs for tick N

Pros:
- Simple and bandwidth-light

Cons:
- High latency sensitivity

### 2) Client-side prediction + server reconciliation 🧭
- Client predicts locally using its inputs
- Server is authoritative and sends corrections

Pros:
- Responsive controls

Cons:
- Requires correction smoothing and authoritative state sync

### 3) Rollback netcode 🧭
- Predict missing remote inputs
- When real inputs arrive, rewind to the divergence tick and resimulate

Pros:
- Very responsive, good for action games

Cons:
- Requires deterministic core + rewindable state

## How this ties into Ironhold’s runtime model
This doc depends on the **Messages → Actions → Execution** model described elsewhere.

### Determinism hooks (planned)
- **InputAction** messages become the canonical per-tick input stream 🧭
- The **Action executor** becomes the single place to apply gameplay side effects 🧭
- A **fixed-tick scheduler** runs gameplay actions deterministically 🧭

### Replay / debugging (planned)
- Record: initial snapshot + inputs per tick
- Replay: run simulation from snapshot using recorded inputs
- Verify: hash state per tick to detect divergence

## Implementation snapshot (today)
This is intentionally short and factual.

- 🧪 The runtime has the beginnings of a message/action architecture, but it is not yet designed around a fixed deterministic tick.
- 🧭 No networking layer, rollback, or replay tooling is implemented yet.

## Milestone suggestions
- **Milestone A: Fixed tick gameplay loop** 🧭
  - Establish a deterministic update stage
  - Define canonical input stream format

- **Milestone B: Replay tooling** 🧭
  - Record/replay input streams
  - Tick-level state hashing

- **Milestone C: Networking prototype** 🧭
  - Start with lockstep or authoritative server
  - Add prediction/rollback after determinism + replay are stable

## Non-goals (for now)
- Deterministic rendering/audio
- Cross-platform determinism guarantees before the fixed-tick core exists

