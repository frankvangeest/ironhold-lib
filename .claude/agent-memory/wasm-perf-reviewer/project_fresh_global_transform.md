---
name: fresh-global-transform
description: utils::fresh_global_transform cost profile — ~40 scalar flops/call, translation-only callers do dead Mat3A work, and the &Transform query term kills native parallelism (free on WASM)
metadata:
  type: project
---

`crate::utils::fresh_global_transform(Option<&Transform>, &GlobalTransform, Option<&ChildOf>) -> GlobalTransform`
(`crates/ironhold_core/src/utils.rs`). Added during `deterministic_fixed_timestep` v1 playtest to fix
`Update`-scheduled readers of `GlobalTransform` being one tick stale on double-FixedUpdate-tick frames.
Root entity + `Transform` → `GlobalTransform::from(*t)`; otherwise falls back to the component.

**Per-call cost (per-frame hot path):** `GlobalTransform::from(Transform)` is
`Affine3A::from_scale_rotation_translation` = `Mat3A::from_quat` (~25 scalar flops) + 3 axis×scale
muls (~9 flops) ≈ **40 scalar f32 ops**, no alloc, no sqrt/trig. On wasm32 glam runs **scalar**
(Vec3A SIMD needs `-C target-feature=+simd128`, not set here), so no SIMD win — still nothing at
realistic counts (tens of labels, 1-4 rings). Not a frame-time concern; do not re-flag as one.

**The real inefficiency is dead work, not the op count:** every *tracked-entity* call site immediately
does `.translation()` and discards the 3x3. Confirmed translation-only as of 2026-09-16:
`lib.rs` tracked_q arm, and both `target_indicator.rs` sites (L127, L167 — L167's `gt` is only used
for `let p = gt.translation()`). Only the **camera** call site needs the full affine
(`Camera::world_to_viewport` needs the inverse view matrix) — and it is correctly hoisted into the
`.map()` over the ≤4-camera Vec, so it is ≤4 calls/frame regardless of label count. Inlining + SROA
*probably* DCEs the matrix in the root arm, but the `_ => *global` arm loads 48 bytes from memory and
can force a memory-based phi. A `fresh_translation(...) -> Vec3` sibling helper makes it provably free.
Nit-level; raised 2026-09-16, check whether adopted before re-raising.

**Query-shape consequence (native-only, FREE on single-threaded WASM):** adding `Option<&Transform>`
to a broad lookup query converts it from "reads `&GlobalTransform`, which nothing in `Update` writes"
into "reads `&Transform`, which the whole camera chain + `world_label_screen_pos_system`'s `label_q`
write". That is a **new access conflict**, so those systems can no longer run in parallel on native.
On WASM this costs exactly zero. Two consequences already handled in-tree:
- `world_label_screen_pos_system`'s `.after(camera_blend_system)` is now **load-bearing**, not
  cosmetic — it resolves an ambiguity the query change created. It costs no parallelism, because the
  conflict already forbids concurrency in either direction.
- `camera_q` gained `Without<WorldLabel>` and `target_indicator`'s `global_transforms` gained
  `Without<TrackingTarget>` purely to prove intra-system disjointness against the `&mut Transform`
  queries. `Without<T>` is archetype-match-time only → zero per-frame cost.

**Archetype cost of the extra optional terms:** these queries use random-access `.get(entity)`, so
each call resolves 4 component columns instead of 2 — ~2 extra sparse lookups per label/ring per
frame. Sub-microsecond at any realistic count. Optional terms never narrow archetype matching.

**Binary size:** widens two existing `QueryState` monomorphizations rather than adding systems;
single-digit KB. Noise. See [[project_wasm_size]].

Related: [[project_world_label_screen_pos]], [[project_target_indicator_system]],
[[project_deterministic_fixed_timestep]].
