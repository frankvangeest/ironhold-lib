//! Deterministic transcendental math, routed through the `libm` crate instead of glam's default
//! (`std`) math backend, for the specific gameplay call sites identified in
//! `planning/features/deterministic_fixed_timestep.md` as bypassing `bevy_rapier3d`'s
//! `enhanced-determinism` guarantee. Confirmed against `cargo tree`: this crate's `glam` build
//! does **not** have its own `libm` feature enabled (it resolves `Quat::from_rotation_y` etc.
//! through `std::f32::sin_cos`/`acos` today), so this is a real behavior change, not a no-op.
//! Calling the same explicit `libm` implementation on every platform, rather than each
//! platform's own `std`/intrinsic backend, is what removes this specific divergence source.
//!
//! **Known residual — this closes the transcendental-math divergence, not the whole rotation
//! pipeline.** The raw `libm::{acosf, sincosf, sinf}` calls below genuinely are bit-identical
//! across platforms. What consumes their output is not: `glam`'s `Quat` type has a *different
//! implementation per SIMD backend* (`sse2` on native x86_64, `scalar` on `wasm32-unknown-unknown`
//! in this project, since no `simd128` target feature is configured) — e.g. `Quat::mul_quat`
//! associates its multiply-adds in a different order per backend. So `det_math::quat_from_
//! rotation_y(angle) * existing_rotation` (the actual expression used at every call site here)
//! is *not* guaranteed bit-identical native-vs-WASM, only its left-hand operand is. Likewise,
//! `GlobalTransform` is backed by `Affine3A`/`Mat3A`, which carries the same per-backend split —
//! so the input `player_movement_system`'s ground-cast feeds into Rapier's own bit-deterministic
//! solver is not itself guaranteed bit-identical across platforms. This residual is exactly the
//! kind of thing `planning/features/deterministic_fixed_timestep.md`'s v2 harness exists to
//! measure — closing it (if it turns out to matter in practice) is out of scope for v1.
//!
//! Deliberately narrow: this wraps only the three known gameplay-reachable transcendental call
//! sites (`player.rs`'s slope-angle `acos` and rotation, `motion.rs`'s rotate/bob), not a
//! blanket glam or std math override — see the feature plan's "Why" section for why a targeted
//! fix was chosen over a crate-wide `glam`/`libm`-feature switch (which, per the same
//! investigation, would not have closed the `Quat`/`Affine3A` residual above either — only
//! `glam`'s `scalar-math` feature would, and that drops `Quat`/`Vec4` alignment from 16 to 4,
//! colliding with this crate's WGSL/GPU-upload alignment constraints — see `src/CLAUDE.md`).

use bevy::math::Quat;

/// `libm`-backed replacement for `f32::acos`.
pub fn acos(x: f32) -> f32 {
    libm::acosf(x)
}

/// `libm`-backed replacement for `f32::sin_cos` — returns `(sin(x), cos(x))`.
pub fn sin_cos(x: f32) -> (f32, f32) {
    libm::sincosf(x)
}

/// `libm`-backed replacement for `f32::sin`.
pub fn sin(x: f32) -> f32 {
    libm::sinf(x)
}

/// `libm`-backed replacement for `Quat::from_rotation_x`, which otherwise composes
/// `f32::sin_cos` via glam's default (std) math backend. Matches glam's own construction
/// (`Quat::from_xyzw(s, 0.0, 0.0, c)` where `(s, c) = sin_cos(angle / 2)`) exactly.
pub fn quat_from_rotation_x(angle: f32) -> Quat {
    let (s, c) = sin_cos(angle * 0.5);
    Quat::from_xyzw(s, 0.0, 0.0, c)
}

/// `libm`-backed replacement for `Quat::from_rotation_y` — see `quat_from_rotation_x`.
pub fn quat_from_rotation_y(angle: f32) -> Quat {
    let (s, c) = sin_cos(angle * 0.5);
    Quat::from_xyzw(0.0, s, 0.0, c)
}

/// `libm`-backed replacement for `Quat::from_rotation_z` — see `quat_from_rotation_x`.
pub fn quat_from_rotation_z(angle: f32) -> Quat {
    let (s, c) = sin_cos(angle * 0.5);
    Quat::from_xyzw(0.0, 0.0, s, c)
}
