---
name: project-ground-cast-loop
description: player_movement_system's FixedUpdate ground shape-cast is now a bounded retry loop (MAX_GROUND_CAST_CANDIDATES=4); typical case still exactly 1 cast and zero allocations
metadata:
  type: project
---

`player_movement_system` (capabilities/player.rs, `FixedUpdate`, chained) does the engine's only
per-tick physics **shape** cast — a downward `Collider::ball(collider_radius)` sweep for ground
detection, once per `CharacterController` entity per tick. As of the `prop-ground-veto` fix it is a
bounded retry loop rather than a single call.

**Cost shape (measured by inspection of bevy_rapier3d 0.33 / rapier3d 0.31):**
- `ReadRapierContext::cast_shape` → `with_query_pipeline` → `BroadPhase::as_query_pipeline` is
  **O(1)** — it only wraps refs to the already-maintained BVH. There is NO per-call pipeline
  rebuild. So a retry iteration costs one extra BVH descent + narrow-phase on the few swept-AABB
  candidates, plus two `HashMap<Entity, Handle>` lookups. Retrying is genuinely cheap.
- The expensive candidate in this engine is the terrain `TriMesh` (zero-thickness, see the long
  comment block in `player.rs`); each retry re-tests it from scratch, since rapier exposes no
  "next hit" iterator.
- `enhanced-determinism` is enabled on `bevy_rapier3d`, so parry runs with SIMD/reassociation
  disabled — every cast here is somewhat slower than a stock rapier build. Factor that into any
  cast-count estimate.

**Retry loop is gated on geometry, not always-on:** it re-casts only when the nearest hit's
`witness1.y > feet_pos.y + collider_radius * 0.5` (a side contact, not underfoot). At the default
`max_walkable_slope_deg: 45.0`, a slope contact sits at `feet + 0.29*r` — comfortably inside the
`0.5*r` tolerance — so **walking on open ground or any walkable slope still costs exactly 1 cast**,
identical to before. Extra casts only near solid props/walls pressed against the player.

**Allocation:** `excluded_this_tick` is a `Vec<Entity>` built with `Vec::new()`, which does not
allocate. Common (unvetoed) path = **zero allocations**. Vetoed path = exactly one 32-byte dlmalloc
alloc (`Entity` is 8 B → `RawVec` MIN_NON_ZERO_CAP 4) with no realloc, since the loop pushes at most
4. This is WASM linear memory, never the JS heap — no GC involvement. A `Local<Vec<Entity>>` buffer
would save at most one small malloc in a rare branch and add per-player/per-tick clear-correctness
risk: **not worth it**. Do not flag this as "per-frame collection allocation" — it is not in the
same class as the `format!`/`collect()` patterns flagged elsewhere.

**`QueryFilter::predicate` first use in the engine.** `player.rs` (and
`tests/prop_ground_veto_tests.rs`) are the only `.predicate(` call sites. It is `Option<&dyn Fn>`,
stack-only, no boxing, no new parry monomorphization — only bevy_rapier's `new_scoped` predicate
arm becomes live. Sub-KB size impact. Note it is currently attached **unconditionally**, including
on the first (always-executed) iteration when the exclusion list is empty, so every ground cast now
pays a `&dyn Fn` indirect call + `entity_from_collider` user_data decode per broad-phase candidate.
Negligible (~a handful of candidates per cast) but a one-line `if !excluded.is_empty()` guard would
remove it from the 100% path.

**Amplification factor to remember for any FixedUpdate physics work:** Bevy's `Time<Fixed>` catches
up after a browser hitch (default `max_delta` 0.25 s → up to ~16 ticks in one rendered frame at the
64 Hz tick rate). Any per-tick cost is therefore multiplied by ticks-per-frame, not by 1, exactly
when the frame is already slow. Bounded-but-multiplied is the right mental model here; see
[[player-spawn-unification]] for the related `TimestepMode::Variable` vs FixedUpdate mismatch.

**How to apply:** treat added work in `player_movement_system` as `players x ticks-per-frame`, cap
4 players (`MAX_SPLIT_PLAYERS`). Purely a Rapier scene query — no renderer, WebGL2/WebGPU, uniform,
or wgpu-feature surface at all.
