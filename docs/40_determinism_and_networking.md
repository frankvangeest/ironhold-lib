# Determinism and Networking

> **Doc type:** Design Notes (vision)
>
> **Status legend:**
> - ✅ **Implemented** — exists in code today
> - 🧪 **Prototype / Partial** — exists but incomplete or unstable
> - 🧭 **Planned** — intended design; not implemented yet

## Status
🧭 Planned (design notes; not implemented yet)

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

### Physics determinism 🧭
General-purpose physics engines are often not deterministic across platforms.

Mitigations:
- Keep authoritative gameplay logic separate from physics
- Use simple deterministic collision primitives in the gameplay core
- Treat full physics as presentation/approximation unless proven deterministic

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

