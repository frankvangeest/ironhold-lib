---
name: nan-from-unclamped-ron-numerics
description: sqrt/log of an unclamped authored RON number yields NaN that no `x <= threshold` guard can catch (NaN comparisons are false) and no `as u16` cast reveals (NaN casts to 0) — validators written with `<=` are silent on exactly the input that crashes
metadata:
  type: project
---

Authored RON numerics reach kinematic math unclamped. `resolve_jump_velocity`
(`runtime/scene_manager/scene_loader.rs`) is the reference case: `(2.0 * GRAVITY * h).sqrt()` where
`h` comes straight from `JumpConfig::Fixed { height }` / `RelativeToHeight { percent }`. A negative
authored height gives **NaN**, which then flows into `Velocity.linvel.y` and from there into the
entity transform.

The NaN is invisible to every idiom normally used to guard it:
- `if apex <= reach { warn!(...) }` — **false for NaN**, so the guard never fires. Same for the
  mirrored `ironhold_cli validate` cross-file check. Write `if !(apex > reach)` instead: identical
  for all finite values, and reports NaN.
- `f32::NAN.max(0.0) == 0.0` (`f32::max` ignores NaN), so a `.max(0.0)` clamp silently launders NaN
  into a plausible-looking discriminant.
- `f32::NAN as u16 == 0`, and float→int `as` is saturating (`1e9f32 as u16 == 65535`), so neither a
  NaN nor an absurd magnitude ever wraps — it just quietly becomes 0 or the max.

Failure surface: NaN transforms panic in **debug** builds at `bevy_math::Dir3::new_unchecked`
("The length is NaN", `direction.rs:66`) — but that check is `#[cfg(debug_assertions)]`, so a
**release/WASM** build silently produces NaN directions and positions instead. A browser-reported
"player vanished / camera went wild" with no console panic is consistent with this.

**Why:** found 2026-08-20 reviewing the uphill-jump-lock fix, which added both a scene-load `warn!`
and a matching CLI validate error for an under-powered jump — both using `apex <= reach`, so both
returned "OK" for the negative-height input that hard-panics.

**How to apply:** when reviewing any new validator over authored numerics, check the comparison
direction against NaN, and check whether the upstream math can produce NaN at all (`sqrt`, `ln`,
`acos`, division by an authored value). Prefer negating the healthy predicate (`!(good)`) over
asserting the unhealthy one (`bad`).
