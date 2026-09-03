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
FALSE for `Fixed`, whose `fixed_camera_system` only calls `transform.look_at()` and never sets
translation). Concretely at review: `FixedCameraDef.position` is read only at
entity_spawner.rs:1514 (spawn) and nowhere in `apply_camera_mode`, and `FixedState` has no
`position` field — so `SetCameraMode` onto any `Fixed` registry preset leaves the camera where it
was and merely rotates it. That is the flagship v2 use case (cutscene camera), it's what both
shipped demos and every doc/plan fence author, and no test asserted translation. **Any future
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
(`warn_camera_modes_registry`, scene_loader.rs:1324) and the CLI (`validate.rs` ~652-724) agree on
the two shared rules (reserved key `"default"`, `Party(...)` value). CLI adds two it alone can do
(`Fixed.look_at_entity` must be a scene `entities:` id; `SetCameraMode(mode:)` must be `"default"`
or in *some* scene's registry). But **several v1 prefab-level checks were never extended to
registry values** even though the plan's task list marks it `[x]`: the nested-`split`/`party`-
inside-`Orbit(...)` warn (entity_spawner.rs:1650) and the `Fixed` both/neither-`look_at` warn
(entity_spawner.rs:1495) are prefab-only. Registry entries authoring those parse fine and are inert.

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
