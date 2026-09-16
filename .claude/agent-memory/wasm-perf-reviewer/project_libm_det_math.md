---
name: project-libm-det-math
description: Why routing gameplay transcendentals through the libm crate (det_math.rs) is a perf/size no-op on the web build specifically — libm is already compiled into the tree, and wasm32 std transcendentals are already software libm
metadata:
  type: project
---

`crates/ironhold_core/src/det_math.rs` (added on `feature/deterministic_fixed_timestep`, reviewed
2026-09-15) wraps `libm::{acosf, sincosf, sinf}` plus hand-rolled `quat_from_rotation_{x,y,z}`,
replacing `f32::acos`, `Quat::from_rotation_*`, and `f32::sin` at three gameplay call sites
(`player.rs` slope `acos` + turn rotation, `motion.rs` rotate/bob).

**`libm` was already a compiled, linked crate — adding it as a direct dep adds no new compilation
unit.** Verified in `Cargo.lock`: the diff is a single `+ "libm",` line under `ironhold_core`'s dep
list; `libm 0.2.15`'s own `[[package]]` block already existed, pulled in by `glam 0.30.10`
(`nostd-libm`), `bevy_math 0.18.0`, `simba 0.9.1` (`libm_force`, rapier's `enhanced-determinism`
path), `num-traits`, `naga`, `core_maths`. No feature-unification change either: `simba` already
declares `libm = "0.2"` with default features, and libm's `default = ["arch"]`.

**On wasm32 specifically, std's transcendentals are already software libm.**
`f32::sin`/`cos`/`acos` lower to `sinf`/`cosf`/`acosf` libcalls that `compiler_builtins` satisfies
with its bundled copy of the same libm source on `wasm32-unknown-unknown` — there is no hardware
transcendental on wasm to lose. So the per-call delta on the *web* path is ~zero; `sincosf` (one
shared argument reduction) is if anything marginally cheaper than glam's two separate `sin`+`cos`
calls. The real speed delta from this change is on native (MSVC/glibc CRT), not web. Do not flag
`det_math` as a web hot-path cost.

**Order-of-magnitude:** the wrapped sites total roughly 30-120 libm calls per 64 Hz tick at
realistic entity counts (Motion prefabs are single-digit per project: `primitive_world` 5,
`stats_demo` 4, `entity_logic_demo`/`particles_demo` 2). At ~20-60 ns/call that is single-digit
microseconds per second of wall clock — unmeasurable.

**Size:** a few KB at most (`acosf`/`sincosf` symbols possibly not previously reachable; the
compiler-builtins copy and the crate copy do not unify since one is `extern "C"` and one is a Rust
path). Irrelevant against the measured 30.6 MB release blob — see [[project-wasm-size]].

**Note glam does NOT already use libm here.** Both `glam feature "std"` and `glam feature
"nostd-libm"` are activated in the tree, but glam's `nostd-libm` only takes effect when `std` is
off — so `Quat::from_rotation_y` really does go through std, and the `det_math` wrapper is not
redundant at the source level.

Companion to [[project-deterministic-fixed-timestep]] (the scheduling half of the same change).
