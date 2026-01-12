
# Determinism and Networking

## Why we care
Multiplayer (especially rollback/prediction) becomes far simpler if gameplay simulation is deterministic:
Given same initial state and same input stream per tick, results match across platforms.

## Recommended approach
We do NOT require the entire engine to be deterministic from day 1.
We split:
- Deterministic gameplay core (truth)
- Non-deterministic presentation layer (camera smoothing, particles, etc.)

Gameplay core requirements:
- fixed tick rate
- stable iteration order where needed
- deterministic RNG (seeded)
- deterministic input abstraction (InputAction stream)
- snapshot/restore hooks (for rollback and debugging)

Physics note:
Character controllers are easier to keep deterministic than general rigid-body physics.
If full physics is needed later, we must constrain features or use deterministic configuration.

## Networking roadmap options
We want to keep both paths open:
1) Server authoritative replication (easiest first)
2) Rollback netcode (best for latency-sensitive PvP, requires determinism + snapshots)

We start with a deterministic tick + input stream architecture so either approach can be implemented later.
``
