---
name: gpu-physics-wgrapier
description: Status of Dimforge wgrapier/wgmath GPU physics and why CPU height-array is correct for terrain ground queries
metadata:
  type: project
---

Researched 2026-06-15: Dimforge's GPU physics ecosystem (wgmath.rs) vs. ironhold terrain ground-query needs.

**Facts (as of research date — verify before acting later):**
- `wgrapier3d`/`wgrapier2d` are published on crates.io but stuck at **v0.2.0, last release Nov 2024**, no docs.rs, <2k downloads. Repo: github.com/dimforge/wgmath. Self-labeled "GPU rigid-body physics (WIP)".
- Dimforge's 2026 roadmap says they will **rewrite** the WGSL wgrapier experiment using rust-gpu, sharing code with rapier. So current crate is a throwaway prototype.
- `wgsparkl` = GPU MPM (sand/fluid/elastic), also WIP, no crates.io release.
- **No Bevy integration** for wgmath/wgrapier. `bevy_rapier` wraps CPU Rapier only. A bridge would be hand-written (wgpu compute pipeline + render-graph scheduling + async readback).
- wgrapier exposes broad-phase + Soft-TGS solver for **dynamic bodies** (demos ~93k bodies). It does NOT expose heightfield collision / raycast / point-projection (those are parry/wgparry territory).

**Decision/guidance:**
- For terrain ground-check ("player XZ → surface Y"), the CPU height-array is the **correct primary design**, not a fallback. It's a 4-texel bilinear lookup: nanoseconds, deterministic, WASM-safe.
- GPU physics needs a GPU→CPU readback to feed the player controller (sync, CPU-side) — defeats the "no round-trip" benefit and is async on WebGPU.
- Share ONE heightmap (via LoadedAssetCatalog) between GPU vertex displacement and CPU height source so they can't desync.

**Why:** Frank asked whether wgrapier could do GPU terrain physics and avoid keeping a CPU height array. It can't — wrong problem (simulator vs. point query) and immature.

**How to apply:** If terrain/physics comes up again, recommend CPU height-array. Only revisit wgrapier when: Dimforge ships the rust-gpu rewrite with docs+releases, an official Bevy bridge exists, AND there's a dense dynamic-body workload (particle/debris collision) — that's wgrapier's real strength, not terrain. Determinism concern: GPU float results not guaranteed stable across vendors (see docs/40_determinism_and_networking.md). Binary-size concern: WebGPU compute adds surface, repo already ~90.7MB/100MB. See [[fragile_modules]].
