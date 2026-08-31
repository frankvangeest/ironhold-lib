# ironhold_core — Integration Test Rules

Tests in `ironhold_core/tests/` must:
- Use `setup_test_app()` from `tests/support/mod.rs`, which installs `GamePlugin` (and transitively `PhysicsPlugin`). Do not construct an `App` without `GamePlugin` — missing physics resources cause panics.
- Initialize the `Message` framework (Writer/Reader resources) before running any messaging systems. `setup_test_app()` handles this.

See `tests/support/mod.rs` for the `setup_test_app()` helper. Each test file declares it via `mod support; use support::setup_test_app;`.

**A grounding/slope-detection test built on a solid `Collider::cuboid` proves nothing about real terrain.** This project's actual terrain collider is a zero-thickness `TriMesh` (`capabilities/terrain.rs`'s `ComputedColliderShape::TriMesh(TriMeshFlags::default())`). A shape-cast that starts embedded in the surface (e.g. centered exactly at a resting character's feet) can resolve a correct-by-coincidence surface normal against a thick convex shape like a box, while resolving a near-arbitrary near-horizontal normal against a zero-thickness triangle — because there's no "up" to push out through. This class of bug is invisible to any test suite that only exercises `Collider::cuboid`/`Collider::ball`/etc. ground — it was found twice, independently, in `player_slope_jump_tests.rs`'s post-implementation review, precisely because every test there originally used a solid box slope. When adding a test that exercises `player_movement_system`'s ground-detection shape-cast (or any future normal-angle/contact-based logic), include at least one case against a real `Collider::trimesh(...)` ground, not only a convex primitive — see `player_slope_jump_tests.rs::trimesh_ground_collider` and its `GroundKind::TriMesh` tests for the pattern.

## Test file layout

| File | Domain | Tests |
|---|---|---|
| `fsm_tests.rs` | Global FSM: state transitions, rules matching, `ActionQueue` FIFO ordering | 23 |
| `entity_logic_tests.rs` | Per-entity FSM (`.behavior.ron`), `{self}` substitution, intent event layer | 8 |
| `scene_lifecycle_tests.rs` | Scene load/unload, overlays, model fixes, pipeline warmup, key bindings, animation graph | 18 |
| `spawn_tests.rs` | `Action::Spawn`/`Despawn`, spawn queue rate limiting, preload, composite prefab spawn | 15 |
| `action_tests.rs` | Misc `Action` executor behaviors: variables, delayed events, floating text, target indicator | 13 |
| `npc_tests.rs` | NPC aggro/investigating states, camera shake | 8 |
| `nameplate_tests.rs` | Nameplate anchor spawn, visibility filtering, cleanup, `should_insert_nameplate` tri-state contract, `Player`-vs-NPC gating (`player_enabled`, faction_filter bypass), `ToggleOwnNameplate`/`PlayerNameplatePreference` | 18 |
| `ui_tests.rs` | Button click-to-action wiring, `IconButton` icon/color/shadow sync | 9 |
| `audio_tests.rs` | PlaySound, PlayMusicLoop, StopMusic, SetVolume | 16 |
| `stats_tests.rs` | StatMap, modifiers, resolve_stat | 22 |
| `particle_tests.rs` | SpawnEffect, layers, visual effects (particles + decals + fading lights), quality tiers, budget gating | 27 |
| `ron_validation.rs` | RON schema round-trips | 191 |
| `assets_schema_version_regression.rs` | Schema version regression guard | 1 |
| `ron_lint.rs` | RON style invariants (e.g. no explicit `Some(...)` wrappers) | 1 |
| `ui_panel_blocker.rs` | Headless Bevy UI focus pipeline: panel + overlay backdrop click-blocking (`FocusPolicy::Block`) | 4 |
| `local_coop_tests.rs` | Local co-op: `party_camera_follow_system` midpoint/radius derivation, `player_view_box_clamp_system` position clamp + velocity zero, two-player scene spawn with shared vs. fallback camera, `trigger_zone_system` portal firing for any/both players (Stage 2), gamepad hot-join, per-player action bar slots, gamepad camera look | 135 |
| `gamepad_binding_tests.rs` | `BoundGamepad`/`gamepad_bind_system`: no-`gamepad_index` regression, immediate bind + unrelated-pad-churn stability, late-connecting pad pending-retry, same-`Entity` disconnect/reconnect resume, cross-time double-bind race, same-frame duplicate-seed race | 9 |
| `camera_modes_tests.rs` | camera_modes v2: `Action::SetCameraMode` registry-preset switch + marker swap, `"default"` round-trip restoring the authored mode, unknown-key/Party-in-registry no-ops, `CameraBlendState` insertion matching an authored `transition:`, `owner_player` targeting (single camera / every camera / out-of-range / party-scene no-op), `Action::CameraShake` `owner_player`, `AuthoredCameraMode` spawn-time recording, `camera_blend_system`/`dynamic_split_screen_system` override-suspend unit tests | 14 |
| `player_slope_jump_tests.rs` | Uphill jump lock fix: steps a real Rapier physics world (sloped static collider + player capsule, both `Collider::cuboid` and real-terrain-representative `Collider::trimesh` ground) across many `FixedUpdate` ticks via `run_system_once` — flat-ground/slope repeated-jump regression, walkable-slope-limit gate (unwalkable slopes never grounded, uphill or downhill; walkable slopes' bounded pogo cadence unaffected), positive grounded controls on both geometry families, framerate-independence of the grace window, flat-ground low-jump-height (non-slope instance of the same bug class), double-jump/landing-animation regression guards, coyote-time debounce (first-jump forgiveness window, bounded fall masking, double-jump availability independent of the buffer) | 19 |
| `prop_ground_veto_tests.rs` | Ground-cast sensor-veto fix: a nearby prop's `trigger_zone` sensor (or any `Sensor` collider) could win the ground shape-cast over the real floor, latching the falling animation while standing on flat ground — fixed via `.exclude_sensors()` on the cast's `QueryFilter`. Also covers a related normal-normalization bug (a penetrating hit's EPA normal is not always unit length) and documents, without fixing, the narrower known-remaining limitation of a solid prop pressed directly against the player | 9 |
| `corpse_loot_interact_tests.rs` | `monster_corpse_loot.md` v2's death→corpse-swap and interact→loot vertical slice, loading the REAL `3rd_person_game_demo` prefabs/behavior RON: `Action::Spawn(..., at_entity: "{self}")` placing a corpse at the monster's exact death transform and despawning the original; post-`corpse_new_id_retrofit`, two corpses from repeated deaths of the same monster slot coexisting under distinct `{new_id}`-suffixed ids instead of colliding; real `KeyF` press → `interactable_system` → `OpenContainer`; looted vs. unlooted decay timing (both via the real `SetDespawnTimer` action, not a manually-pushed `Despawn`); the two-corpses-in-range `panels_open` double-open fix; and three regressions found by the mandatory post-implementation review — a stale `SetDespawnTimer` unable to leak onto an unrelated entity reusing its old id (now manufactured directly in the test rather than via the retired guard), a despawned entity's stale `PlayerTarget`/`CurrentTarget` never clearing, and a despawned entity's open container panel never closing (including the natural-decay path specifically, which a first round of tests for the latter two missed by only exercising a manually-pushed `Despawn`) | 11 |

`fsm_tests.rs` through `ui_tests.rs` were split out of a single `integration_tests.rs` (2026-07-02) once it grew to 104 tests / 4258 lines mixing 8 distinct subsystems with no internal organization. See `planning/features/done/split_integration_tests.md`.

## Important: support module placement

`support/mod.rs` lives in a subdirectory (not `support.rs` at the top level). Files directly under `tests/` are compiled as standalone test binaries by Rust; subdirectory files are not. Keeping `setup_test_app()` in `tests/support/mod.rs` prevents the compiler from trying to build it as an independent binary.
