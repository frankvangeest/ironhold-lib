---
name: camera-modes-v2-pattern
description: camera_modes v2 (SetCameraMode + scene-level camera_modes registry + CameraBlendState) — the AuthoredCameraMode-gated all_cameras query and the apply_camera_mode-vs-spawn-path field-mapping split are where designer-authored fields silently die
metadata:
  type: project
---

Reviewed 2026-08-09 (v2, verdict BLOCKING). Builds on [[camera_modes_v1_pattern]]. New surface:
`GameSceneV2.camera_modes: BTreeMap<String, CameraModeDef>` (scene-level named preset registry,
`#[serde(default)]`) → `LoadedCameraModes` resource (inserted in `scene_loader.rs`'s **Replace
branch only**, beside `LoadedSpawnPoints`); `Action::SetCameraMode { mode, owner_player }`;
`Action::CameraShake` gained `owner_player`; `CameraTransition{duration_secs, ease: EaseKind}` on
every `CameraModeDef` payload; `AuthoredCameraMode(CameraModeDef)` component; `CameraModeOverride`
marker; `CameraBlendState` + `camera_blend_system` (registered LAST in lib.rs's camera `.chain()`).

**THE STRUCTURAL FOOTGUN — v2 has TWO independent "build runtime state from a `CameraModeDef`"
code paths that do NOT map the same fields.** Spawn-time is `spawn_active_camera_for_player`
(entity_spawner.rs ~1408, per-mode `commands.spawn((...))` arms). Switch-time is
`apply_camera_mode` (entity_spawner.rs:1296, per-mode `commands.entity(e).insert((...))` arms).
Its own doc comment admits the duplication and defers unifying it. **Anything a spawn arm applies
to `Transform` rather than to the `ActiveCameraMode` payload is silently dropped on the switch
path**, because `apply_camera_mode` deliberately never touches `Transform` (it assumes the
newly-active mode's per-frame system recomputes it — true for Orbit/Follow/FirstPerson/Flycam,
FALSE for `Fixed`). The concrete `Fixed.position` instance of this is **RESOLVED** (re-verified
2026-09-06): `FixedState` now carries `position`, `apply_camera_mode`'s `Fixed` arm sets it
(entity_spawner.rs:1468), and `fixed_camera_system` (camera.rs:499) writes
`transform.translation = fixed.position` **unconditionally every frame** with a doc comment citing
the three reviews that caught it. The general footgun stands. **Any future
`CameraModeDef` variant/field: check it appears in BOTH functions, and that `*State` carries
everything the per-frame system can't recompute.**

**Second footgun — `SceneStateParams::all_cameras` requires `&AuthoredCameraMode`**
(scene_manager/mod.rs:627), so any camera spawn site that forgets to insert it is invisible to
`SetCameraMode` with **zero diagnostic** (empty `targets` vec → the `for` loop just doesn't run;
there is no "no cameras matched" warn). **RESOLVED — the flycam gap this note originally flagged is
now fixed.** At the time of this review, 5 of 6 sites inserted it
(`spawn_active_camera_for_player`'s 6 arms, `spawn_orbit_camera_for_player` @1179 covering
split/party/hot-join, `spawn_party_orbit_camera` @camera.rs:332 with a synthesized `Party` value);
the `scene_loader.rs` flycam-tag camera spawn did not. It now does (scene_loader.rs ~line 875) —
the insertion carries an inline comment citing this exact finding ("found in camera_modes.md v2's
post-implementation review; every other camera-spawn site already has this"), confirming the fix
and its provenance. `SetCameraMode` is no longer a silent no-op in `tags: ["flycam"]` scenes. Grep
`AuthoredCameraMode` and compare against `Camera3d::default()` spawn sites on any *future* camera
change — the general footgun (a new spawn site forgetting the insert, with zero diagnostic) still
applies to any site not covered above.

**Registry-vs-prefab validation asymmetry (drift to re-check every time):** the runtime warn
(`warn_camera_modes_registry`, scene_loader.rs:1423) and the CLI (`validate.rs` ~1240-1290) agree on
the two shared rules (reserved key `"default"`, `Party(...)` value). CLI adds two it alone can do
(`Fixed.look_at_entity` must be a scene `entities:` id; `SetCameraMode(mode:)` must be `"default"`
or in *some* scene's registry).

**PARTLY CLOSED by `feature/camera_mode_validation` (2026-09-06, commit `3d74da2`):** a shared
`validate_camera_mode_def(&CameraModeDef, context: &str) -> Vec<(String, &'static str)>`
(validate.rs:264) now covers **both** the nested-`split`/`party`-inside-`Orbit(...)` mistake and the
`Fixed` both/neither-`look_at` mistake, wired into **both** the prefab-catalog loop
(`def.components.camera_mode`, validate.rs:1177) and the registry loop (validate.rs:1284). Conditions
verified byte-identical to the runtime warns (`assemble_player_config` entity_spawner.rs:1774;
`spawn_active_camera_for_player`'s `Fixed` arm entity_spawner.rs:1613). **Still uncovered camera-mode
authoring mistakes that DO have a live runtime `warn!` and ARE prefab-local:**
- `components.camera_mode: Party(...)` on a prefab (entity_spawner.rs:1647 + the `Some(other)` arm
  of `resolve_orbit_config_for_multiplayer` @1300). Always wrong on every path; the registry loop
  rejects `Party` ~20 lines above the helper call, so this is an asymmetry *inside the new code*.
- `tags: ["flycam"]` prefab with a non-`Flycam` `camera_mode` (scene_loader.rs:827) — whole mode
  discarded, falls back to `FlyCamDef::default()`. `PrefabDef::is_flycam()` already exists in
  `schema/`, so it's CLI-reachable.
- non-`Orbit` `camera_mode` on a prefab that also authors `components.split`/`party`
  (`resolve_orbit_config_for_multiplayer`, entity_spawner.rs:1304) — authored mode silently replaced
  by legacy `camera:` tuning. Prefab-local subset is checkable; the full condition is scene-scoped.
- Payload string vocabularies with warns: `CameraConfig.orbit_button` (`parse_orbit_button`,
  camera.rs:1085) and `FlyCamDef.look_button` (`parse_flycam_look_button`, flycam.rs:12). Both live
  *inside* a `CameraModeDef` payload, i.e. exactly this helper's remit. (`FlyCamDef`'s
  `forward`/`backward`/... go through `parse_key(..).unwrap_or(KeyCode::KeyW)` with **no warn at
  all** — silently wrong key, and unvalidated by the CLI too.)

**Message-text trap this feature hit — a shared helper's message must not prescribe a remedy that
is only valid at one call site.** The nested-split message ends "...they must be siblings of
`camera_mode`, e.g. `components: (camera_mode: Orbit(...), split: (...))`" — correct for the prefab
call site, **impossible for a `camera_modes:` registry entry**, which has no `components:` block and
where `split`/`party` cannot be authored at all (the only correct remedy there is "delete them").
Likewise the `Fixed`-neither message's "(facing -Z)" is true only on the spawn path; via
`SetCameraMode`/`apply_camera_mode` the camera keeps its *current* rotation.

**`Fixed` both-`look_at` is a documented-contract-vs-implementation split** — `FixedCameraDef`'s doc
(schema/camera.rs:176) says "Exactly one ... should be set", but `fixed_camera_system`
(camera.rs:500-504) resolves `look_at_entity` then `.or(fixed.look_at)`, and `FixedState`'s own doc
says "Takes priority over `look_at` **when both resolve**" — i.e. `look_at` is a working fallback
for a despawned/not-yet-spawned target. The CLI now hard-errors (exit 1) on that shape.

**`CameraShake` vs `SetCameraMode` `owner_player` are NOT identical despite docs claiming so**
(20_data_formats.md:3516 says "follows identically"). `SetCameraMode` explicitly rejects a shared
Party camera (`targets.0.len() > 1` → warn+no-op); `CameraShake` has no such check and happily
shakes the shared camera for everyone. Also `owner_player` resolves via
`resolve_player_entity_by_index` (action_executor.rs:1652), where `n == 0` matches a player with
`PlayerIndex(0)` **or no `PlayerIndex` at all** (mirrors `targeting::is_primary_player`).

**`camera_modes:` on an overlay scene is silently discarded** — `warn_camera_modes_registry` runs
before the overlay/Replace branch (so its two warns still fire), but `LoadedCameraModes` is only
inserted in Replace. No "ignored on overlay load" diagnostic exists.

**Correct by construction (do not regress):** no new system touches `ActionQueue`;
`camera_blend_system` mutates only `Transform`/`Projection` and self-removes at `remaining <= 0`;
`fixed`/`follow`/`first_person` systems mutate only `Transform` (+ the target's yaw for FirstPerson);
all systems ARE registered this time (the v1 unregistered-system bug did not recur); `EaseKind` is a
real unquoted enum with `#[serde(default)]`; `query.rs` got its `Action::SetCameraMode` arm; both
demos (`entity_logic_demo` main + `local_coop_demo` room11) wire scene_key_bindings → rule →
action entirely in RON; CLI has a fixture project per new validate rule.

**Stale docs found (pre-existing v1 debt the v2 doc pass didn't catch):** `CameraConfig.fov` is
documented as default `60.0` in both `schema/player.rs`'s doc comment (~line 155) and
`docs/20_data_formats.md:2087`, but `default_fov()` is **45.0** (deliberately, to preserve
pre-v1 framing). Both also still say FOV interpolation is "v2 scope" after it shipped.
`CameraConfig.transition`/`FlyCamDef.transition` are new authorable fields absent from their
docs field tables.
