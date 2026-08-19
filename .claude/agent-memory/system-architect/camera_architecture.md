---
name: camera-architecture
description: Camera component history (OrbitCamera/FlyCamera/PartyOrbitCamera) plus the camera_modes v1 unification that replaced them — ActiveCameraMode/CameraTargets/markers, the Update-schedule chain, and v1's four structural traps
metadata:
  type: project
---

The engine has **three siloed camera components**, `OrbitCamera`, `FlyCamera` (both `capabilities/`) and `PartyOrbitCamera` (local co-op), selected at scene-load time by entity tags / `CameraConfig` block presence (the `"flycam"` tag spawns a `FlyCamera`). They cannot switch mid-session.

**Camera systems run in Update, not FixedUpdate** — deliberate, because camera is render-cadence not physics. The Update `.chain()` (lib.rs:300-311, verified 2026-08-01) is: `animation_resolver_system` → `camera_orbit_system` → `party_camera_follow_system` → `dynamic_split_screen_system` → `split_screen_viewport_system` → `split_viewport_player_label_update_system` → `target_hud_update_system` → **`camera_shake_system`** → `fly_camera_system` → `animation_playback_system`. `camera_orbit_system`/`party_camera_follow_system` each write `cam_transform.translation` from scratch every frame; any system perturbing the camera (e.g. shake) MUST run AFTER **every** base-transform writer in the chain and apply an **additive** offset (`+=`). Shake already sits after both orbit and party (so extending shake to party mode needs no reorder) but BEFORE `fly_camera_system` — which is exactly why shake can never work on a flycam.

Note the core CLAUDE.md rule "physics & camera-follow logic must run in FixedUpdate" refers to the *character-follow* coupling; the actual orbit/flycam camera systems live in Update. Don't flag a camera system in Update as a schedule violation.

**Shake targets OrbitCamera only** (`With<OrbitCamera>` filter on both the executor query and `camera_shake_system`). FlyCamera scenes (terrain_demo, custom_materials) get a logged no-op warning — correct and intentional. There is no shake for flycam; a flycam additive offset would fight `fly_camera_system` which also writes translation each frame.

**The camera_modes feature (planning/features/camera_modes.md) plans to replace BOTH `OrbitCamera` and `FlyCamera` with a single `ActiveCameraMode` component + unified `camera_system` dispatching on mode, plus `Action::SetCameraMode`.** Any new per-camera component (like `CameraShakeState`) adds migration surface to that refactor. When reviewing new camera features, note whether they will need re-homing onto `ActiveCameraMode` — flag the coupling so the camera_modes work accounts for it.

**`OrbitCamera.yaw`/`.pitch` are written by exactly ONE system: `camera_orbit_system`** (verified 2026-07-19). Nothing else reads or mutates them (flycam and `PartyOrbitCamera` have their own separate yaw/pitch). So making them additionally keyboard-writable (per-player keyboard look feature) affects no other consumer — the only frame ordering that matters is shake-after-orbit, already handled.

**Per-player keyboard look SHIPPED as designed (feature/camera-look-controls, 2026-07-19):** `OrbitCamera` gained `look_left_key`/`look_right_key`/`look_up_key`/`look_down_key: Option<KeyCode>` + `look_speed: f32`, all pre-resolved once at spawn from `InputMap.look_*` strings via `InputMap::parse_key` (same precedent as `orbit_lmb`/`orbit_rmb`). `camera_orbit_system` got a `Res<ButtonInput<KeyCode>>` param and a keyboard block that runs UNCONDITIONALLY (independent of the mouse `orbit_active` gate) — the point being split-screen sets `orbit_button:"None"`. Pitch convention is now PINNED in code + test: `look_up` increases pitch toward `max_pitch` (overhead), matching the mouse convention, not "up = sky". `look_speed` is a deliberately separate dial from `orbit_speed` (rad/s hold-rate vs mouse-pixel-delta multiplier) and is forward-designed to also drive gamepad right-stick pitch. `PartyOrbitCamera` was correctly left out (no single per-player owner for a binding).

**Pitch-direction trap:** `CameraConfig.min_pitch` (default 0.1) is documented "looking up", `max_pitch` (default 0.9) "looking down" — the whole authored range is downward-ish angles, there is no true look-at-sky. Higher pitch = camera positioned higher = more top-down (verified from the `Quat::from_axis_angle(X, -pitch)` math). The existing mouse convention: mouse-up (negative screen delta.y) → `pitch += ` → more top-down. Any new "look up"/"look down" binding must pick a convention deliberately: matching the mouse means look_up → pitch increase → overhead, but a player may intuit "look up" as raising the aim toward the horizon = pitch DECREASE. Pin the convention in the feature and assert direction (not just clamp bounds) in tests.

**The camera spawn helpers are at their positional-parameter limit.** `spawn_split_camera_for_player`
(entity_spawner.rs) and `spawn_party_orbit_camera` (camera.rs) are both at 6 positional params after
`per_viewport_target_ring_visibility` added an `own_viewport_only: bool` to each. Both are still
legible (the bool is last, no adjacent bool to transpose with) and this codebase has no
options-struct precedent for spawn helpers — but the *next* per-camera toggle should introduce a
small `SplitCameraOpts`/`PartyCameraOpts` struct rather than a 7th positional param. The
`camera_modes` refactor re-homes both helpers anyway, so fold it in there.

**THE MODE SWITCH IS THE TAG, NOT THE CONFIG BLOCK** (verified 2026-08-07). `PrefabComponents.camera: Option<CameraConfig>` and `.flycam: Option<FlyCamDef>` are both `#[serde(default)]` **tuning** blocks; the thing that actually selects a camera is `tags: ["player"]` / `tags: ["flycam"]` (`scene_loader.rs`'s `TAG_FLYCAM` + the player-config collector). `assemble_player_config` does `components.camera.clone().unwrap_or_else(default_camera_config)`; flycam does `.unwrap_or_default()`. **8 of 10 player-bearing projects have a `tags:["player"]` prefab with NO `camera:` block at all** (quick_scene, primitive_world, entity_logic_demo, stats_demo, particles_demo, effect_mayhem_demo, blank_project, integration_tests — plus local_coop_demo's `player_p2`). Any `camera_modes` backward-compat scheme keyed on "the old `camera:`/`flycam:` field is present" silently leaves all of them cameraless. Detection must be tag-driven.

**Four structural obstacles to the `camera_modes` unification, found in plan-review 2026-08-01 (verify against code before acting — none were fixed at review time):**
1. **`CameraConfig` (schema/player.rs:80) contains `party: Option<PartyZoomDef>` (:115) and `split: Option<SplitScreenDef>` (:122).** So `CameraModeDef::Orbit(CameraConfig)` would carry the very field that selects `Party` mode, and split-screen's *authoring* surface is NOT orthogonal to camera mode even though its *runtime* state (`SplitViewportSlot`/`ActiveSplitScreen`/etc.) genuinely is. Any plan claiming full orthogonality is only half right.
2. **`PartyOrbitCamera` is built from TWO structs** (`spawn_party_orbit_camera(base_camera: &CameraConfig, party: &PartyZoomDef, ...)`), unlike Orbit/Flycam's one-config-each. It also has `targets: Vec<Entity>` (plural), **no `radius` field at all** (derived per-frame from max pairwise separation), a `manual_zoom_offset` runtime accumulator, and deliberately lacks `look_*_key`/`look_speed`/`gamepad_*`/`character_rotate_*`. It is not a "wrap one config as inner payload" shape.
3. **`PartyOrbitCamera` does double duty as a queryable type-level marker.** `dynamic_split_screen_system` (camera.rs:700-701) uses `With<PartyOrbitCamera>`/`Without<PartyOrbitCamera>`, and `local_coop_tests.rs:788` queries it. Bevy filters on component types, not enum variants — `With<CameraModeDef::Party>` is inexpressible. The naive rewrite to `Query<&mut Camera, Without<SplitViewportSlot>>` also matches the persistent overlay `Camera2d` (lib.rs:418, order 1000), making `single_mut()` return `Err(MultipleEntities)` and silently no-op the merge. Keep zero-sized per-mode marker components if the enum lands.
4. **Schema-vs-runtime conflation.** A deserialized `CameraModeDef` cannot hold `Entity` refs, pre-resolved `KeyCode`s, or mutable yaw/pitch/radius — all of which `OrbitCamera`/`PartyOrbitCamera` carry today. The authored def and the resolved runtime state must be two distinct types.

**"Which player owns this camera" is a FIVE-site pattern, all via `OrbitCamera.target`** — `camera_orbit_system`, `split_viewport_player_label_spawn_system` (camera.rs:494), `target_hud_update_system` (camera.rs:622), `dynamic_split_screen_system` (camera.rs:700), and `click_select_system` (targeting.rs:177, via `Option<&OrbitCamera>` — its `None` branch is what routes party/fallback-camera clicks to the primary player). Plus `SceneStateParams::orbit_cameras` (scene_manager/mod.rs:604) for shake. Any camera refactor should extract this into its own small component (e.g. `CameraOwner(Entity)`) rather than burying `target` inside mode variants — it keeps all five as cheap type-level queries and lets `Fixed`/`Flycam` (no owner) and `Party` (many) be representable without a match.

**`OrbitCamera` is now constructed at exactly ONE site** (corrected 2026-08-07): `entity_spawner.rs::spawn_orbit_camera_for_player`. The old second site — the primitive/capsule inline camera block in `scene_loader.rs` — was **removed by `player_model_source_unification` v1**; primitive players now route through `spawn_players_and_camera` like GLB ones. Any new field still breaks `default_camera_config()` (entity_spawner.rs) and the `base_camera_config()`/test literals, since neither `CameraConfig` nor `InputMap` derive `Default`. `spawn_orbit_camera_for_player` has the full `PlayerConfig` in scope, so both `.camera` and `.inputs` (InputMap) are reachable — `OrbitCamera` deliberately mixes both (e.g. `look_*_key`/`gamepad_deadzone` come from `InputMap`, not `CameraConfig`), which is the concrete proof case for `camera_modes`' authored-def-vs-runtime-component split.

**`OrbitCamera.gamepad_index` was DELETED (`f4cca59`, 2026-08-05).** `camera_orbit_system` now resolves the pad live via a disjoint `bound_q: Query<&BoundGamepad>` looked up **through `orbit.target`** — so `camera_orbit_system` itself is now a **sixth** "which player owns this camera" consumer, on top of the five-site list below. `PartyOrbitCamera` still deliberately has no gamepad/look bindings (no single owner), so "no gamepad pitch in `Party` mode" is the correct, intended answer for any unification.

**The scene_loader camera-spawn chain is FOUR branches, not three** (`scene_loader.rs` ~753-848): players → flycam → `spawn_points` non-empty (deliberately spawns **nothing**, FSM will) → default fallback camera.

---

**`camera_modes` v1 LANDED (feature/camera-modes-v1, code-reviewed 2026-08-07).** `OrbitCamera`/`PartyOrbitCamera`/`FlyCamera` are GONE; replaced by `ActiveCameraMode` enum component (`OrbitState`/`PartyState`/`FixedState`/`FollowState`/`FirstPersonState`/`FlycamState` payloads) + `CameraTargets(Vec<Entity>)` + six zero-sized markers (`OrbitCameraMode` … `FlycamCameraMode`). Schema side is the separate `CameraModeDef` in `schema/camera.rs`; `PrefabComponents` gained `camera_mode`/`split`/`party` as siblings; `PlayerConfig` gained the same three. All 4 plan blockers verified honored. **SEVEN camera spawn sites**, all marker-in-sync: `spawn_orbit_camera_from_config`, the 5 arms of `spawn_active_camera_for_player`, `spawn_party_orbit_camera` (camera.rs), the scene_loader flycam block, plus the `Default Camera` fallback (deliberately bare — no `ActiveCameraMode`/`CameraTargets`; every consumer uses `Option<&CameraTargets>` or a marker filter).

**~~THE ONE STRUCTURAL TRAP `camera_mode` INTRODUCED — "the multiplayer branches never read `camera_mode`."~~ FIXED on main** (confirmed 2026-08-09): `resolve_orbit_config_for_multiplayer(pc)` (entity_spawner.rs ~1191) now reads `pc.camera_mode` first — `Some(Orbit(cfg))` → `cfg.clone()`, `Some(other)` → warn + `pc.camera.clone()`, `None` → `pc.camera.clone()`. The split/party paths no longer silently revert a migrated co-op prefab's tuning to engine defaults. Historical context (why the fix exists): migrating a co-op prefab to `camera_mode: Orbit((...))` used to revert `orbit_button`/`zoom_speed`/`character_rotate_button` to engine defaults, reproducing the shared-mouse-delta split-screen bug from [[split-screen-and-shared-mouse]]. Note the fix is Orbit-only — a co-op player authored with any *non*-Orbit `camera_mode` still warns and falls back.

**Also corrected:** all six mode systems (including `follow_camera_system`/`fixed_camera_system`/`first_person_camera_system`) ARE registered in `lib.rs`'s Update `.chain()` as of 2026-08-09 — the v1-review "three written but never registered" note no longer applies.

**`fov` is the other v1 landmine.** `insert_fov` (entity_spawner.rs) inserts `Projection::Perspective{fov}` explicitly; before v1 nothing inserted `Projection` at all, so `Camera3d`'s required-component default was **PI/4 = 45°**. `default_fov()` = 60.0 → every pre-existing project silently widens 45→60 and all 27 screenshot baselines diverge. Party cameras (`spawn_party_orbit_camera`) and the flycam/Default-Camera paths do NOT call `insert_fov`, so they stay 45° — in a dynamic-split scene the FOV pops at every merge/split. `PartyCameraDef` has no `fov` field and `spawn_party_orbit_camera` ignores `base_camera.fov`. Cleanest fix: `fov: Option<f32>`, skip the insert when `None`.

**Three of six mode systems were written but never registered in `lib.rs`** at v1 review time: `follow_camera_system`, `fixed_camera_system`, `first_person_camera_system`. They're `pub fn` in a lib crate so no dead-code warning fires and the suite stays green — `camera_mode: Follow/Fixed/FirstPerson` authored in RON spawns a camera frozen at its spawn transform, silently. Verify registration before trusting any claim that a mode "works".

**`CameraModeDef::Party` is dead schema in v1** — no spawn path honors `PartyCameraDef`; the multiplayer dispatch builds `PartyState` from `CameraConfig` + `PartyZoomDef`, and the single-player arm warns and falls back to Orbit.

**Blocker 1 is only HALF resolved:** `CameraConfig` still carries `split`/`party`, so `CameraModeDef::Orbit(CameraConfig)` re-exposes them inside the payload — but `assemble_player_config` reads them only from `prefab.components.camera` or the new siblings, never from the `camera_mode` payload. `camera_mode: Orbit((split: (...)))` parses and does nothing.

**Marker/enum sync is convention-only** — no single constructor derives the marker from the variant. v2's `SetCameraMode` needs remove-5-markers/insert-1; land a `fn camera_bundle(mode) -> impl Bundle` before then.

**v2 `SetCameraMode` — two facts that make the switch implementable without new plumbing.**
(1) The owning player entity carries the full authored `InputMap` at runtime inside
`CharacterController.inputs` (`capabilities/player.rs`), so an executor switching a camera to
`Orbit`/`FirstPerson`/`Flycam` can recover the pre-resolved key bindings via
`CameraTargets.first()` → `CharacterController.inputs` — no need to stash an `InputMap` on the
camera or in the mode registry. A camera with **zero** `CameraTargets` (`Fixed`/`Flycam`/the bare
`Default Camera`) has no such source and must fall back to engine defaults with a warn.
(2) `resolve_camera_mode(pc)` (entity_spawner.rs:1275) is `camera_mode` override-or-`Orbit(camera)`
fallback only — it is *not* where tag detection happens, so it is safe to reuse verbatim as "the
scene-authored default mode" for any reset-to-default mechanism.

---

**`camera_modes` v2 (feature/camera_modes_v2, reviewed 2026-08-09).** Adds `GameSceneV2.camera_modes: BTreeMap<String, CameraModeDef>` + `LoadedCameraModes` (inserted in scene_loader's Replace branch only, mirroring `LoadedSpawnPoints`), `Action::SetCameraMode{mode, owner_player}`, `owner_player: Option<u32>` on `CameraShake`, `AuthoredCameraMode` (spawn-time, immutable — what `mode: "default"` restores) vs `ActiveCameraMode` (live), `CameraTransition`/`EaseKind`, `CameraBlendState`/`camera_blend_system` (last in the chain), `CameraModeOverride`, and `apply_camera_mode` (entity_spawner.rs, `pub(crate)`) as a deliberately-duplicated switch-time analog of `spawn_active_camera_for_player`'s arms. **Read [[camera-pose-writer-taxonomy]] before touching any of it** — that split is what governs whether a switch or a blend is even coherent for a given mode, and it is the root of most of what v2 review found.

**Two v2 structural facts that outlive any individual bugfix:**
- **`SceneStateParams::all_cameras` requires `&AuthoredCameraMode`**, so any camera spawn site that forgets it is *silently* invisible to `SetCameraMode` (empty target list → the `for` loop body never runs → not even a warn, since every warn arm is upstream of it). There are now **8** camera spawn sites, and the standalone flycam block (`scene_loader.rs` ~829) was the one missed. Prefer `Option<&AuthoredCameraMode>` + warn over the hard requirement.
- **`SceneStateParams::transforms` carries `Without<ActiveCameraMode>`** purely to disjoin from `all_cameras`' `&Transform`. Correct today (only `ResetToSpawn` uses it, which never targets a camera) but enforced only by a doc comment — a future camera-position action routed through it would silently match nothing.

**The engine-managed party camera is reachable by designer-fired actions.** It carries `AuthoredCameraMode(Party(...))` (synthesized in `spawn_party_orbit_camera`), so `SetCameraMode` with `owner_player` omitted targets it along with everything else, while `owner_player: Some(n)` deliberately rejects it. Anything operating over "every camera" must decide explicitly whether the shared party camera is in scope — `apply_camera_mode` rejects `Party` as a *target* mode, which makes the `mode: "default"` restore path on that camera a dead end.

---

**Spectator mode landed (feature/flycam-scene-conflicts, 2026-08-17).** The scene_loader camera-spawn chain is no longer four mutually-exclusive branches: the **flycam block now spawns first and unconditionally** (it has no terrain dependency), then `SuppressPlayerCameras(has_flycam)` is inserted (inside the Replace/`!is_overlay` branch only — correct per [[scene-load-resource-threading]]), then the player/spawn-points/default-camera dispatch runs with the two tail branches gated on `!has_flycam`. `spawn_players_and_camera` gained a `CameraSpawnMode::{Spawn,Suppressed}` param and returns early right after the player-entity loop when Suppressed — which also skips the split+party mutual-exclusion / Grid+dynamic / `own_viewport_only` layer-collision warns and the `ActiveSplitScreen`/`DynamicSplitConfig`/`ActiveSplitSlotCount`/`TargetRingVisibilityMode` inserts. **That early return is only sound because `Action::LoadScene` resets all four of those resources** — treat that reset list as a load-bearing invariant, not housekeeping. `is_split_screen` in scene_loader must stay `!has_flycam && ...` or `WorldLabelRank`/stat-widget entities duplicate for cameras that never spawn.

Two deliberate residual holes (documented in `docs/20_data_formats.md`, logged in `claude_suggestions.md`): dynamic `Action::Spawn`/hot-join player paths call `spawn_active_camera_for_player` unconditionally and never read `SuppressPlayerCameras`; and `SetCameraMode` with `owner_player: None` still targets the flycam itself.

**RON gotcha for every `CameraModeDef` variant: DOUBLE parens** — `Orbit((field: value, ...))`. Single-paren fails with `Expected struct CameraConfig but found "offset"`. `ironhold_cli` has *near*-zero camera awareness (no validation of `camera:` + `camera_mode:` both set, no Party-on-single-player check) — the exceptions are `duplicate_flycam_entity` (scene-scoped) and the two prefab-catalog-scoped flycam checks below.

---

**Flycam tag predicates + the `Action::Spawn` blind spot (feature/flycam-model-warning, 2026-08-19).**
`impl PrefabDef` in `schema/catalog.rs` gained `is_flycam()`/`is_player()`/`flycam_ignored_fields()`
(+ `pub const TAG_FLYCAM`/`TAG_PLAYER`) — **the first behavioral-predicate impl on any schema type**
(every other impl there is `Default`/`From`/`validate()`). This is now the correct home for any
engine-semantics predicate both `runtime/` and `ironhold_cli` need, since the CLI can only reach
`ironhold_core::schema::*`. Contrast `should_insert_nameplate` (`runtime/scene_manager/mod.rs`),
which is CLI-unreachable and therefore duplicated as string literals in `validate.rs`. `is_flycam()`
is fully adopted (2/2 sites); `is_player()` is adopted at 1 of ~10 — the rest still hand-roll
`tags.iter().any(|t| t == "player")` in `validate.rs`, `action_executor.rs`, and `query.rs`.

**`Action::Spawn` has ZERO flycam awareness** (`action_executor.rs` ~140-190, verified 2026-08-19) —
this is the one path where a flycam-tagged prefab's `model:` is NOT dead. Dynamically spawning such
a prefab does a normal `asset_catalog.models.get(prefab_def.model)` lookup: `model: ""` (every
shipped flycam) → warn + no-op; a *resolvable* `model:` → the body spawns as an ordinary prop with
**no camera at all**, and if the prefab is also `"player"`-tagged it gets a full `PlayerConfig`
assembled. So the `flycam_model_never_renders` / `flycam_player_tag_conflict` validate errors' wording
("never appear", "never spawn at all") is true for scene `entities:` placement, not universally —
don't repeat "unconditionally" in docs or messages.
