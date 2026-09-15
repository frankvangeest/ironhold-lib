---
name: determinism-networking
description: Corrected state of the "Rapier cross-platform float divergence blocks multiplayer" concern — enhanced-determinism is already ON; the real blockers are variable timestep + no snapshot/rollback layer, not Rapier floats
metadata:
  type: project
---

**2026-09-15 investigation corrected the long-standing framing.** The claim "Rapier3D is not
cross-platform deterministic, this is the hard blocker" (my own earlier memory, and
`planning/stakeholder_priority_list.md` item #1) is **wrong as stated**. Verified against vendored
crate sources:

- `crates/ironhold_core/Cargo.toml:16` already enables `enhanced-determinism`.
- `bevy_rapier3d-0.33.0` forwards it: `enhanced-determinism = ["rapier3d/enhanced-determinism"]`.
- `rapier3d-0.31.0` expands it to `["simba/libm_force", "parry3d/enhanced-determinism"]` — routes
  every transcendental through pure-Rust `libm` instead of platform intrinsics.
- It also **disables** rapier's FTZ/DAZ MXCSR poke (`rapier3d-0.31.0/src/utils.rs:433-462` — the
  x86 `_mm_setcsr` branch is `cfg(not(feature = "enhanced-determinism"))`), removing the single
  clearest native-vs-WASM divergence (WASM has no flush-to-zero mode).
- No `parallel`, no `simd-stable`/`simd-nightly` anywhere in the tree (rapier `compile_error!`s if
  SIMD + enhanced-determinism are combined, so this can't silently regress).
- Rapier's own docs claim bit-level cross-platform determinism incl. WASM under exactly this config.

**Why:** the concern was recorded from general "physics engines aren't cross-platform deterministic"
folklore, never checked against this repo's actual feature flags.

**How to apply:**
- The real, verified blockers are ordinary engineering, not a physics-engine limitation:
  1. **Variable timestep.** `capabilities/physics.rs:12` uses `RapierPhysicsPlugin::default()` →
     `TimestepMode::Variable { max_dt: 1/60 }` in `PostUpdate`, stepping on real frame delta. Two
     machines at different framerates diverge on frame 1. Fix already exists in the dep:
     `.in_fixed_schedule()` + `TimestepMode::Fixed`. This also collapses the "two physics clocks"
     complexity that `capabilities/player.rs`'s whole `jump_air_grace`/`jump_liftoff_y`
     belt-and-braces design exists to work around — a maintainability win independent of netcode.
  2. **No snapshot/restore layer.** Nothing can serialize or rewind world state. `serde-serialize`
     is on for rapier but unused.
  3. **A small set of std transcendentals in our own sim code** (rapier's libm forcing covers
     rapier/parry via `simba/libm_force`, NOT glam): `player.rs:289` `.acos()` for the
     slope-walkability branch, and `player.rs:579` `transform.rotate_y()` → glam `sin_cos`.
     **"Only two call sites" is too narrow** (found 2026-09-15 plan review): `capabilities/motion.rs`
     is a third sim-affecting site — `motion_system` is registered in **`Update`** (`lib.rs:332`),
     reads wall-clock `Time::elapsed_secs()/delta_secs()`, and writes `Transform` via
     `Quat::from_rotation_{x,y,z}` (glam `sin_cos`) + `.sin()` bob. Motion props carry colliders/
     sensors, so this feeds the physics world on a variable clock. Camera/particle/decal/flycam/
     foliage transcendentals are presentation-only and don't matter. `npc.rs` is transcendental-free
     (only `distance`/`normalize` → `sqrt`, IEEE-exact).
- **`FIXED_TICK_RATE: f32 = 64.0`** (`capabilities/player.rs:13`) — Bevy's default `Time<Fixed>`
  rate, hardcoded into `jump_air_grace_ticks()`/`coyote_ticks()`. Any `TimestepMode::Fixed { dt }`
  MUST be `1.0/64.0` (or change both together); `1.0/60.0` would run physics 6.7% fast vs wall
  clock and desync the jump-tick math.
- **The integration tests already run `TimestepMode::Fixed { dt: 1.0/64.0, substeps: 1 }`**
  (`player_slope_jump_tests.rs:101`, `prop_ground_veto_tests.rs:171`, `wall_friction_tests.rs:124`)
  while production runs `Variable`. Consequence: the whole jump/slope/coyote suite validates a
  timestep mode the shipped game never uses — so switching production to `Fixed` cannot be
  regression-caught by those tests (they stay green either way); only playtest can. Also means
  Rapier *does* run in integration tests despite `physics.rs`'s `#[cfg(not(test))]` (cfg(test) is
  false when the lib is compiled as an integration test's dependency — see `action_tests.rs:223`).
- `TimestepMode` is a **Resource** in bevy_rapier3d 0.33 (`plugin/configuration.rs:14`), not a
  field of `RapierConfiguration` (which is a Component). The plugin `init_resource`s it during
  `build()` and warns if it isn't `Fixed` when the schedule is `FixedUpdate` — so insert the
  resource *before* `add_plugins(RapierPhysicsPlugin…)` to avoid a spurious startup warning.
- Already deterministic and worth not re-litigating: `SpawnRegistry.entities` is a `BTreeMap`
  (`scene_manager/mod.rs:366`), ActionQueue is FIFO, **zero `rand` usage in `ironhold_core`**,
  gameplay `dt` comes from `Time<Fixed>`.
- **Key architectural fact that changes the recommendation:** there is no free-body rigid-body
  gameplay. `RigidBody::Dynamic` appears only on player/NPC capsules (all `ROTATION_LOCKED`);
  everything else is `Fixed`. Gameplay overwrites `linvel.x/z` every tick, so Rapier is effectively
  a *query* engine (shape casts, raycasts, sensors) + a Y-axis/contact solver — not an emergent
  simulation. Physics state that would need syncing is therefore small and mostly re-derivable.
- Recommended sequencing (see the 2026-09-15 report): fixed timestep first as a standalone
  maintainability feature, then a cross-platform divergence *harness* (native vs WASM tick-hash
  comparison) to convert the assumption into a measurement, then decide netcode model. Do not
  commit to lockstep/rollback before the harness produces data.
- `planning/features/networking_multiplayer.md` (3 forms, Beta 0.6/0.8/0.9) already exists and is
  sound; its pre-implementation checklist gates on Beta 0.5 (deterministic tick + replay), which is
  the right gate. `static_scene_mode.md` is still Queued, and no `RunMode` enum exists —
  `start_app(project_path, scene_override)` (`lib.rs:755`) still has no run-mode parameter.
