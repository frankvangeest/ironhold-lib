---
name: determinism-networking
description: Relationship between static-scene mode, simulation determinism, and future networking; Rapier cross-platform float divergence is the hard blocker
metadata:
  type: project
---

Static scene mode (`?static=1` → run-mode resource → pause `Time<Virtual>` on `SceneEvent::Ready` + seek AnimationPlayers to t=0) is largely ORTHOGONAL to networking determinism. Static mode = output stability (freeze the clock for pixel-stable screenshots). Networking determinism = simulation reproducibility (same inputs + start state → identical state every tick). Different problems.

**Why:** Frank asked (2026-06-14) whether static mode lays groundwork for the determinism networking will need. Honest answer: only one reusable seam — taking explicit control of `Time<Virtual>` from a top-level resource is the same chokepoint a fixed-timestep network loop wants.

**How to apply:**
- Networking determinism actually requires: fixed timestep (FixedUpdate, constant dt), deterministic float math, deterministic iteration order (HashMap/HashSet iteration is the risk; our ActionQueue is already FIFO so it's fine), seeded RNG threaded through state, input as sole nondeterminism source. Static mode addresses NONE of these — it stops the sim rather than reproducing it.
- **Hard blocker: Rapier3D is not cross-platform deterministic.** Native vs WASM diverge. This gates any lockstep networking and deserves its own `planning/investigations/` file before any networking commitment.
- Determinism is a prerequisite for *lockstep* networking only — NOT for state-replication / server-authoritative models, which tolerate nondeterminism and are the realistic first target given Rapier.
- When advising on static mode scope: recommend minimal-but-clean — route clock control through a single owned `SimClock`/time-control abstraction (not inline `Time<Virtual>` pokes), and model run mode as an enum (`Live | Static | future Replay/Lockstep`) not a bare `StaticMode(bool)`, since the `start_app` signature was just changed across all three crates and we want to avoid a second break. Defer fixed-timestep and RNG threading.
