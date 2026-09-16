# Determinism and Networking

> **Doc type:** Design Notes (vision)
>
> **Status legend:**
> - ✅ **Implemented** — exists in code today
> - 🧪 **Prototype / Partial** — exists but incomplete or unstable
> - 🧭 **Planned** — intended design; not implemented yet

## Status
🧭 Mostly planned (design notes; not implemented yet) — see "Physics determinism" below for the
sub-area with real ✅ items already in place, most recently the fixed-tick physics schedule
(`planning/features/deterministic_fixed_timestep.md`, v1, 2026-09-15).

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

- **Fixed tick** for gameplay ✅ — 64Hz, not the 60Hz originally sketched here (`FIXED_TICK_RATE`,
  `capabilities/physics.rs`; matches Bevy's own `FixedUpdate` default exactly, see
  `planning/features/deterministic_fixed_timestep.md`). Physics now steps on this same tick
  (`TimestepMode::Fixed`) instead of a variable, wall-clock-driven one.
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

### Floating-point differences 🧪
Different CPUs and WASM runtimes can produce tiny float differences that amplify over time.

_(Updated 2026-09-15, `deterministic_fixed_timestep.md` v1 review.)_ Not just theoretical:
`glam`'s `Quat`/`Vec4`/`Mat3A`/`Affine3A` types have a **per-SIMD-backend implementation** (`sse2`
on native x86_64 in this project, `scalar` on `wasm32-unknown-unknown`, since no `simd128` target
feature is configured) — e.g. `Quat::mul_quat` associates its multiply-adds in a different order
per backend. Routing the underlying transcendental *scalar* calls (`sin`/`cos`/`acos`) through
`libm` (`crates/ironhold_core/src/det_math.rs`) closes one real divergence source, but the
quaternion/affine *composition* that follows is not itself guaranteed bit-identical — see
`det_math.rs`'s "Known residual" doc comment. This is exactly the kind of gap
`deterministic_fixed_timestep.md`'s v2 harness exists to detect with evidence, not assumption.

Mitigations:
- Prefer integer/fixed-point math for core state where feasible
- Quantize/round at boundaries (e.g., store positions in fixed precision)
- Keep floating-point usage in presentation
- Route known per-tick gameplay transcendental call sites through `libm` (`det_math`) rather than
  each platform's own `std`/SIMD backend — closes the scalar-math half of this problem, not the
  glam backend-split half

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
`planning/stakeholder_priority_list.md`'s "Rapier cross-platform float divergence" item, then v1
of `planning/features/deterministic_fixed_timestep.md`.)_

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

✅ **Fixed timestep** — `capabilities/physics.rs` now steps Rapier via `TimestepMode::Fixed` in
`FixedUpdate` (`dt = 1.0 / FIXED_TICK_RATE`, 64Hz) instead of a variable, wall-clock-driven
`PostUpdate` step. Every device now feeds the solver the same `dt` regardless of frame rate.

✅ **Known per-tick transcendental call sites routed through `libm`** — `player.rs`'s slope-angle
`acos` (feeding a grounded/airborne branch) and turn rotation, and `motion.rs`'s
`Quat::from_rotation_*`/bob `sin`, now go through `crates/ironhold_core/src/det_math.rs` instead
of glam's default (`std`) math backend. See the Floating-point-differences section above for the
residual this does *not* close (glam's per-SIMD-backend quaternion/affine composition).

What's actually left, in priority order:

- 🧭 **One-time, spawn-time rotation construction is still `std`-backed** — `Quat::from_euler`,
  `to_radians()`/`.cos()` threshold checks, etc. in `scene_loader.rs`/`entity_spawner.rs`/
  `action_executor.rs` set the *initial* orientation of every authored entity, including static
  colliders (slopes, ramps, walls) — a 1-ULP difference here diverges the simulation from frame 0
  and never converges back, a larger determinism hole than the per-tick sites above. Not yet
  widened to `det_math`; a candidate v1.x/v2 follow-up, not required for v1's own goal (fixing the
  variable timestep).
- 🧭 **Unverified in practice** — no harness yet actually compares native vs. WASM-Chrome vs.
  WASM-Firefox state over time. See `planning/features/deterministic_fixed_timestep.md` (v2) —
  now more valuable than originally scoped, since it's also the only mechanism that would catch
  the glam backend-split residual noted above.

✅ Authoritative gameplay logic is already effectively separate from raw physics: the engine has no
free-body dynamic rigid-body gameplay (dynamic bodies are only rotation-locked player/NPC
capsules) and gameplay overwrites velocity every tick rather than trusting accumulated physics
state — this is the property that makes the rest of this section tractable at all.

Mitigations (remaining):
- Widen `det_math` coverage to spawn-time rotation construction (static collider orientation)
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
- A **fixed-tick scheduler** runs gameplay actions deterministically ✅ — `FixedUpdate` at 64Hz,
  now shared with physics (`planning/features/deterministic_fixed_timestep.md`, v1)

### Replay / debugging (planned)
- Record: initial snapshot + inputs per tick
- Replay: run simulation from snapshot using recorded inputs
- Verify: hash state per tick to detect divergence

## Implementation snapshot (today)
This is intentionally short and factual.

- 🧪 The runtime has a real fixed-tick gameplay+physics schedule (`FixedUpdate`, 64Hz, as of
  `deterministic_fixed_timestep.md` v1) but no canonical input-stream format, snapshot/replay, or
  RNG determinism yet.
- 🧭 No networking layer, rollback, or replay tooling is implemented yet.

## Milestone suggestions
- **Milestone A: Fixed tick gameplay loop** 🧪 — the schedule itself is done (gameplay +
  physics both on `FixedUpdate` @ 64Hz); still missing a canonical input-stream format
  - Establish a deterministic update stage ✅
  - Define canonical input stream format 🧭

- **Milestone B: Replay tooling** 🧭
  - Record/replay input streams
  - Tick-level state hashing — scoped concretely as
    `planning/features/deterministic_fixed_timestep.md` v2

- **Milestone C: Networking prototype** 🧭
  - Start with lockstep or authoritative server
  - Add prediction/rollback after determinism + replay are stable

## Non-goals (for now)
- Deterministic rendering/audio
- Cross-platform determinism guarantees before the fixed-tick core exists
