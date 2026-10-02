# Feature: Over-the-shoulder follow + lock-on target camera

_Status: Draft_
_Planned at: `d56a554` (2026-10-02)_

Backlog items (`## Camera`): **Over-the-shoulder (OTS) camera mode** and **Lock-on target camera mode**. Related, **referenced not absorbed**:
`dynamic_camera_mode_switching.md` (its v2 folds both modes into the demo scene), D3/D4 (`gameplay_pipeline_system_sets.md`, ordering
convention), and the movement-coupling follow-up this plan deliberately defers (Decision 4). Written autonomously while Frank was away; not
plan-reviewed. **Nothing below was compiled or run** — every code claim is a file:line reading, and the numeric defaults are starting points
for the playtest, not measurements.

## Phases

| Phase | Backlog item | Status | Completed |
|---|---|---|---|
| v1 | OTS: `Follow` gains a target-relative `offset_space` (no new variant) | Queued | — |
| v2 | Lock-on: `CameraModeDef::LockOn` framing layer over Orbit, camera-only | Queued | — |
| v3 | (not in this plan) lock-on movement: face-target / strafe-around-target, `FixedUpdate`-side | Icebox | — |

## What
Two designer-facing camera behaviours, authored purely in RON.

**v1 (OTS).** `Follow(...)` accepts `offset_space: Target`, making `offset` and `look_at_offset` rotate with the player's yaw, so the camera
stays behind-and-beside the character as it turns:
```ron
"ots": Follow((offset: (0.8, 1.8, 3.0), look_at_offset: (0.3, 1.5, -4.0), offset_space: Target,
               smoothing: 10.0, fov: 55.0, transition: Some((duration_secs: 0.3, ease: EaseInOut)))),
```
**v2 (lock-on).** A new `LockOn(...)` mode is an ordinary orbit camera until its owning player has a target (`PlayerTarget`); then it swings
behind the player on the player-to-target line, shifts its focus toward the target and pulls back so both stay in frame. Clearing the
target (or the target dying/despawning) eases back to plain orbit with no snap:
```ron
camera_modes: {
    "lock_on": LockOn((
        orbit: (offset: (0.0, 5.0, 8.0), look_at_offset: (0.0, 1.6, 0.0), zoom_speed: 10.0, orbit_speed: 0.5,
                min_radius: 3.0, max_radius: 14.0, fov: 60.0),
        focus_bias: 0.4, distance_zoom: 0.6, yaw_smoothing: 6.0,
        engage_secs: 0.35, release_secs: 0.5, max_distance: Some(35.0),
        transition: Some((duration_secs: 0.4, ease: EaseInOut)),
    )),
},
```
Locking is automatic from the player's own target (Tab / `Shift+Tab` / click / `gamepad_target_next` / `SetTarget`): no new input, no new
event. Movement is **unchanged** in v1/v2 (Decision 4).

## Why
OTS and lock-on are the two third-person framings nearly every action game needs; the engine has Orbit (free) and Follow (fixed) only.
Lock-on is also the first consumer that makes `PlayerTarget` visible in the *camera*, not just a ring/HUD, which the combat demo
(`3rd_person_game_demo`) and the dynamic-switching demo (v2) both want.

## Findings (verified against code at `d56a554`)
1. **The backlog's "OTS may be zero engine code" premise is false.** `follow_camera_system` computes `desired_pos = target.translation +
   follow.offset` and `look_target = target.translation + follow.look_at_offset` (`capabilities/camera.rs:543,551`) — both **world-space**;
   the target's rotation is never read. A shoulder offset therefore only works for a character that never turns. `Orbit`'s `look_at_offset`
   is world-space too (`camera.rs:287`). So OTS needs a small engine change, but **not** a new variant (Decision 1).
2. **Facing convention:** movement uses `transform.forward()` (= -Z) as forward (`player.rs:568`), A/D `Turn` rotates the root
   (`player.rs:593-595`); "behind" is therefore **+Z** in target space. The backlog's example `(0.8, 1.8, -3.0)` assumed +Z forward.
3. **Orbit already decouples camera yaw from character facing:** `offset = Quat(yaw) * Quat(-pitch) * Z * radius`, camera at
   `target_pos + offset`, looking at `target_pos` (`camera.rs:286-292`); `OrbitState` is the only holder of `yaw/pitch/radius`.
4. **Per-player target:** `PlayerTarget(Option<String>)` per player entity; `target_auto_clear_system` clears it on hidden **or**
   unregistered/despawned (`targeting.rs:373-392`); `tab_targeting_system` is per player with its own `target_next` + `gamepad_target_next`
   (`targeting.rs:274-299`). Targeting runs `click_select → tab_targeting → target_auto_clear`, `.chain().before(action_bar_input_system)`
   (`targeting.rs:58-65`). The global `CurrentTarget` only mirrors the primary player — lock-on must read `PlayerTarget`, never `CurrentTarget`.
5. **Camera ownership:** `CameraTargets(Vec<Entity>)`, `.first()` = owner for single-owner modes (`camera.rs:95`); a split-screen scene has one
   real `Orbit` camera per player (`SplitViewportSlot`); only Orbit is supported at **spawn** for split/party players
   (`resolve_orbit_config_for_multiplayer`, `entity_spawner.rs:1318-1333`, warns and falls back for any other mode).
6. **Runtime switch:** `Action::SetCameraMode{mode, owner_player}` resolves a `camera_modes:` preset and calls `apply_camera_mode`
   (`action_executor.rs:1000-1128`, `entity_spawner.rs:1423-1529`), which removes all six mode markers + `CameraShakeState`, inserts the new
   `ActiveCameraMode` + marker + FOV, and `CameraBlendState` blends the *rendered* pose toward whatever the new mode's system writes
   (`camera.rs:1165-1187`, last in the chain, `lib.rs:360`). `AuthoredCameraMode` holds the spawn-time `CameraModeDef` for `"default"` restore.
7. **Exhaustive `CameraModeDef` matches** that a new variant must touch: `transition()`, `radius_range()` (`schema/camera.rs:50,73`),
   `apply_camera_mode` (incl. `unreachable!` arm), `spawn_active_camera_for_player` (`entity_spawner.rs:1552`), `resolve_orbit_config_for_multiplayer`,
   and `ironhold_cli/src/commands/validate.rs` (`:428,:583`, plus `camera_mode_vocab_problems`).
8. **Camera chain** (`lib.rs:336-362`) is one `Update` `.chain()`; `camera_shake_system` adds a one-frame offset to `Transform.translation`
   for `Or<(OrbitCameraMode, PartyCameraMode)>` (`camera.rs:1117-1134`). `target_hud_update_system` is already `.after(target_auto_clear_system)`
   inside that chain (`lib.rs:354`) — the precedent for a camera-side `PlayerTarget` reader.
9. **Stale-transform rule:** an `Update` system reading a physics entity's position must use `utils::fresh_global_transform`
   (`fixed_camera_system` does for `look_at_entity`, `camera.rs:503-512`); `camera_orbit_system`/`follow_camera_system` read local `Transform`.
10. Target aim point: `SelectAimHeight` (default `SELECT_AIM_HEIGHT = 1.0`, private const, `targeting.rs:18,31`) already says where on a target a
    selection "aims" — reusable instead of a second per-prefab field.

## Decisions (with rejected alternatives)
1. **OTS = `offset_space` on `Follow`, not an `OverTheShoulder` variant.** Rejected: a new variant (duplicates Follow's smoothing/fov/transition
   fields and every exhaustive match in Finding 7 for one rotation); a `shoulder_side` toggle (no requirement yet — the backlog itself says to
   add a variant only for a shoulder-swap or camera-yaw-drives-character; neither is asked for). Mouse-aim OTS (mouse yaw rotating the
   character, i.e. FirstPerson's yaw mechanism with a third-person pose) is **out of scope** — Open question 1.
2. **Lock-on = new variant `LockOn(LockOnCameraDef)`, runtime = Orbit + a `LockOnFraming` component.** Rejected: a flag on `Orbit`
   (`CameraConfig` is also the legacy `camera:` struct, built by literals at several sites; lock-on tunables don't belong in it, and a flag
   can't be a distinct `camera_modes:` registry preset); a new `ActiveCameraMode::LockOn` variant (would need its own copy of Orbit's
   mouse/keyboard/gamepad input, or a refactor of the heavily tested `camera_orbit_system`). Chosen: the camera runs as
   `ActiveCameraMode::Orbit` + `OrbitCameraMode` (so Orbit input, `camera_shake_system`, `dynamic_split_screen_system` and split viewports all
   work untouched) plus `LockOnFraming(LockOnState)`, and a new system overwrites the pose after `camera_orbit_system`. `AuthoredCameraMode`
   keeps the real `LockOn` def so `"default"` restore works. `apply_camera_mode` must also `remove::<LockOnFraming>()` on every switch.
3. **Auto-engage from `PlayerTarget`, no explicit toggle.** Engaged ⇔ the owner's `PlayerTarget` is `Some`. Rejected: engage via
   `target.changed`/`target.cleared` → `SetCameraMode` rules (primary player only — Finding 4 — so wrong for split-screen; and a
   rule-per-project burden). A designer who wants "lock only while a key is held" uses two registry presets (Orbit/LockOn) and a rule;
   that already works with `SetCameraMode`.
4. **Camera-only; movement unchanged (v1/v2).** `player_movement_system` is `FixedUpdate` and derives forward from the character's own
   transform; the lock-on yaw is computed in `Update` from render-rate state. Feeding camera yaw back into movement would make
   gameplay depend on `Update`-rate camera state (0/2-tick frames, WASM/native divergence) — directly against the determinism work.
   Consequence to document: while locked, A/D still turns the character (or strafes with the strafe button) and "forward" is wherever the
   character faces, not toward the target. The proper v3 is a gameplay-side feature: a `FixedUpdate` system derives facing/strafe from
   `PlayerTarget` + `SpawnRegistry` positions (pure function of tick state), gated by a new `MovementConfig` flag; the camera never writes
   to the player. Not attempted here.
5. **Pose is a pure function of state, smoothing lives in state not in `Transform`.** Smoothing yaw/weight in `LockOnState` (not
   lerping from last frame's `Transform`) keeps `camera_shake_system`'s additive offset from accumulating and makes the pose independent of
   system order beyond "after `camera_orbit_system`, before shake/blend".
6. **Gamepad:** v1 reuses `gamepad_target_next` (cycles forward) and `Shift+Tab` for reverse on keyboard. Right-stick-X is already player
   `Turn` (`runtime/input.rs:351`), so a right-stick "flick to switch" would collide; deferred (Open question 3).

## Approach

### v1 — `Follow.offset_space`
- `schema/camera.rs`: `enum OffsetSpace { #[default] World, Target }` (unquoted in RON, like `EaseKind`); `FollowCameraDef.offset_space`
  (`#[serde(default)]`). `FollowState.offset_space` (`camera.rs:520`), set at the two `FollowState` construction sites
  (`entity_spawner.rs:1460,1597`; no `FollowCameraDef` struct literals exist outside RON/tests — grep).
- `follow_camera_system`: for `Target`, `let r = target_transform.rotation;` use `r * follow.offset` / `r * follow.look_at_offset`; `World`
  path byte-identical. Player roots are `ROTATION_LOCKED` and only yaw (`entity_spawner.rs` collider setup), so the full quaternion is safe.
  `radius_range()` unchanged (`offset.length()` is rotation-invariant).
- Smoothing note: position lerps toward a moving target-space point, so a fast turn swings the camera on an arc; `rotation_smoothing`
  already damps look-at. No new field.

### v2 — lock-on
Schema (`schema/camera.rs`), all `#[serde(default = ...)]` except `orbit`:
```rust
pub struct LockOnCameraDef {
    pub orbit: CameraConfig,                 // the no-target behaviour; its own `transition` is ignored (ours wins)
    pub focus_bias: f32,                     // 0.0 = look at player, 1.0 = look at target. default 0.4
    pub distance_zoom: f32,                  // extra radius metres per metre of player-target separation. default 0.6
    pub yaw_smoothing: f32,                  // exp rate toward the behind-the-player yaw. default 6.0
    pub engage_secs: f32,                    // weight 0->1. default 0.35
    pub release_secs: f32,                   // weight 1->0 on clear/death/out-of-range. default 0.5
    pub max_distance: Option<f32>,           // beyond this the framing releases (target stays selected). default None
    pub transition: Option<CameraTransition>,
}
```
`CameraModeDef::LockOn(LockOnCameraDef)`: `transition()` → `def.transition.as_ref().or(def.orbit.transition.as_ref())`; `radius_range()` →
orbit's `min_radius..max_radius` (lock-on may add up to `distance_zoom * separation`, clamped to `max_radius`, so the band is the same).
Accepted in the `camera_modes:` registry and as `components.camera_mode` (single-player spawn arm + split/party players, see Tasks);
`Party` stays the only rejected variant.

Runtime (`capabilities/camera.rs`):
- `LockOnFraming(LockOnState)` component: tunables copied from the def + `weight: f32`, `last_target_pos: Option<Vec3>`.
- `lock_on_framing_system`, in the `lib.rs` camera chain **directly after `camera_orbit_system`** (before `party_camera_follow_system`, so
  before shake and `camera_blend_system`), plus `.after(target_auto_clear_system)` exactly like `target_hud_update_system` (`lib.rs:354`), so a
  target cleared this frame is never framed. Query: `(&mut Transform, &mut ActiveCameraMode, &CameraTargets, &mut LockOnFraming),
  With<OrbitCameraMode>`; read-only `Query<(&Transform, &PlayerTarget), With<CharacterController>>` for the owner; target position via
  `SpawnRegistry.entities.get(id)` (keyed lookup only — no iteration) → `fresh_global_transform` (Finding 9) `+ Y * SelectAimHeight`.
- Per camera, once per frame, no cross-camera state (so split-screen cameras are independent and query order is irrelevant):
  1. `engaged = owner.PlayerTarget is Some(id) && target resolves && dist_xz <= max_distance`; else if previously engaged keep
     `last_target_pos` (no snap while releasing).
  2. `weight` moves toward 1/0 at `1/engage_secs` / `1/release_secs` per second (linear, then `EaseKind::EaseInOut.apply` for the pose).
  3. `desired_yaw = atan2(player.x - target.x, player.z - target.z)` (camera sits on the far side of the player from the target; matches the
     Orbit convention `offset = rot(yaw)*Z`, Finding 3); skipped if horizontal separation < 0.05 m. `orbit.yaw` is moved toward it along the
     shortest arc by `1 - exp(-yaw_smoothing * weight * dt)`. Writing `orbit.yaw` (rather than a private yaw) means releasing leaves the
     camera where it is and mouse/keyboard orbit resumes from there — no snap on exit. While engaged, yaw *input* is therefore overridden;
     pitch and zoom input still work.
  4. `focus = lerp(player + look_at_offset, midpoint-weighted target aim, focus_bias * w_eased)`; `radius = clamp(orbit.radius +
     distance_zoom * separation * w_eased, min_radius, max_radius)`; pose = the existing Orbit formula with that radius/focus, written to
     `Transform` (overwriting what `camera_orbit_system` just wrote). Pitch comes from `orbit.pitch`.
- `apply_camera_mode`: new `LockOn` arm = the `Orbit` arm on `def.orbit` (reusing `orbit_state_from_config`) + `LockOnFraming`, FOV = `orbit.fov`;
  every arm gains `remove::<LockOnFraming>()` (Finding 6). `spawn_active_camera_for_player` gets the same arm; `AuthoredCameraMode` stores the
  `LockOn` def. For split/party spawn, `resolve_orbit_config_for_multiplayer` returns the def's `orbit` for `LockOn` and the spawn site inserts
  `LockOnFraming` — verify the two split spawn sites (`spawn_players_and_camera`, `spawn_split_camera_for_player`) while implementing; if that
  proves invasive, ship split support via `SetCameraMode(owner_player: n)` only and warn at spawn (Open question 2).
- Interactions: **Party** — never lock-on (`SetCameraMode` already excludes Party-authored cameras; shared camera has no single owner).
  **Flycam / Fixed / FirstPerson** — switching onto them removes `LockOnFraming`; a registry `LockOn` applied to an owner-less camera
  (`CameraTargets` empty) is inert like Orbit is today. **`CameraBlendState`** — works unchanged: blend runs last and lerps toward the
  lock-on pose. **Shake** — works (marker is Orbit); no accumulation per Decision 5. **View box / dynamic split** — untouched
  (`dynamic_split_screen_system` toggles `is_active` only; lock-on does not gate on `is_active`, matching Orbit so inactive cameras keep
  tracking). **Action bar `{target}`** — unchanged: lock-on only *reads* `PlayerTarget`, never writes it or `CurrentTarget`, and emits no event.
- Perf/WASM: one extra system doing O(cameras) work (≤4), one hash lookup + a few trig calls per engaged camera per frame, no allocation,
  no new dependency, no change-detection churn when `weight == 0` and no target (early-out before touching `Transform`/`orbit`).
- Determinism: render-only output; inputs are `PlayerTarget` (deterministically ordered by the targeting chain), `SpawnRegistry` keyed
  lookup, transforms. Nothing reads another camera or iterates a map. Uses std `atan2/exp` — acceptable because nothing here feeds
  `FixedUpdate`; if v3 ever consumes a facing angle it must use `det_math` and be computed on the fixed tick. Under D3's planned
  `CameraChainSet` it simply joins that set after the targeting stream.

## Tasks
- [ ] Re-confirm Findings 1, 2, 5, 7 against `HEAD` (no code, `git log -- capabilities/camera.rs schema/camera.rs` since `d56a554`).
- [ ] v1 code: `OffsetSpace`, `FollowCameraDef`/`FollowState` field, `follow_camera_system` branch, both `FollowState` sites.
- [ ] v1 tests (`camera_modes_tests.rs`): `World` default unchanged (regression, existing Follow test still passes); `Target` with the player
      rotated 90° puts the camera on the rotated side; `look_at_offset` rotates too; `SetCameraMode` onto an OTS preset blends via `CameraBlendState`.
- [ ] v2 code: `LockOnCameraDef` + variant, `LockOnFraming`, `lock_on_framing_system` + `lib.rs` registration, `apply_camera_mode` arms
      (+ `remove::<LockOnFraming>` everywhere), spawn arm, split-spawn handling, make `SELECT_AIM_HEIGHT` `pub(crate)`.
- [ ] v2 tests (new `lock_on_camera_tests.rs`, test-fixture rule: targets must register in `SpawnRegistry`, see `spawn_targetable_at`):
      no target = identical pose to plain Orbit; target set → camera on the far side of the player and the target inside the frustum;
      target cleared / despawned → weight eases to 0 with no frame-to-frame jump above a threshold; `max_distance` release; two split
      players with different targets get independent poses and the same-frame result is identical across 20 runs (no query-order
      dependence); `SetCameraMode` Orbit → LockOn → `"default"` swaps markers and removes `LockOnFraming`; shake on a locked camera does
      not accumulate; `CurrentTarget` never mutated by the system.
- [ ] Docs: `docs/20_data_formats.md` (`Follow.offset_space` row + OTS recipe with the +Z-is-behind note; `LockOn` payload table, registry
      example, "movement is not changed" callout, split-screen note), `crates/ironhold_core/src/CLAUDE.md` (lock-on layer, ordering vs
      targeting, why camera-only), `docs/30` only if events are mentioned (none added).
- [ ] CLI: `validate.rs` — vocab check for `LockOn` (`orbit_config_vocab_problems(&def.orbit)`), range checks that currently match
      `CameraModeDef::Orbit` (`:428`) extended to `LockOn`'s orbit, error for `focus_bias` outside 0..1 / `distance_zoom < 0` / non-positive
      `engage_secs`/`release_secs`. Check whether `query.rs` lists camera modes (add `LockOn` if it does); then `cargo check -p ironhold_cli` (unconditional, schema change) and the
      step-6 `cargo run -p ironhold_cli -- query ...` spot-check.
- [ ] Demo: extend `assets/projects/camera_modes` (OTS + lock-on presets in a scene with 2 `targetable` dummies and a second pad for
      split-screen) — coordinate with `dynamic_camera_mode_switching.md` v2 rather than duplicating; add `ron_lint`-clean RON before step 4.
- [ ] Review agents in parallel after implementation: alignment-reviewer, system-architect, debug-detective, ux-gamedesigner-reviewer
      (schema/docs/assets), wasm-perf-reviewer (new per-frame system).

## Playtest checklist
1. OTS preset in `camera_modes`: walk and turn with A/D — camera stays behind the right shoulder; crosshair-side space is clear; `offset_space:
   World` preset still behaves as before.
2. Lock-on preset, single player: Tab onto a dummy — camera swings behind you toward the target over ~0.35 s, both visible; Shift+Tab / click a
   different one — smooth re-aim, no flip when passing through the player-target line; click empty space — eases back to orbit, no snap.
3. Kill/despawn the locked target (corpse swap) — fades out over ~0.5 s, no pop.
4. Walk beyond `max_distance` — framing releases, ring stays; walk back — re-engages.
5. Mouse orbit while locked: yaw is overridden, pitch/zoom work; release and orbit freely.
6. Camera shake while locked; SetCameraMode Orbit → LockOn → FirstPerson → Flycam → `"default"` with a transition each; no stuck lock state.
7. Split-screen (2 players, different targets): each viewport frames its own player's target; clearing one doesn't affect the other.
8. Gamepad: `gamepad_target_next` locks, right-stick-Y pitch still works; confirm movement feel is unchanged (A/D turn, forward = facing).
9. Web build (`python test_web.py` + dev playtest): no console errors, frame time unchanged.

## Open questions
1. **OTS aiming:** is "mouse yaw rotates the character, camera shoulder-follows" (a true shooter OTS) wanted? That needs a Follow/OTS variant of
   FirstPerson's character-yaw mechanism and is a different feature from `offset_space: Target`. Plan assumes **no** for now.
2. **Split-screen lock-on at spawn:** acceptable to ship v2 with split support through `SetCameraMode(owner_player)` only if spawn-time
   `LockOn` in `split:` scenes turns out invasive (Finding 5)? Plan defaults to supporting spawn if cheap, else fall back with a warning.
3. **Switching targets while locked on gamepad:** right-stick-X is player turn; add a dedicated `gamepad_target_prev` button (and/or a
   stick-flick mode) now, or keep forward-cycle only? Plan keeps forward-cycle + keyboard Shift+Tab.
4. **v3 appetite:** should face-target / strafe-around-target movement be queued as its own backlog item now (Icebox, depends on the
   fixed-tick pipeline plan), given that without it locked-on movement still feels like free-roam? Plan only records it.

## Acceptance criteria
- Given a `Follow` preset with `offset_space: Target` and `offset: (0.8,1.8,3.0)`, when the player turns 90°, then the camera ends up
  behind-right of the new facing; given `offset_space` omitted, behaviour is byte-identical to today.
- Given a `LockOn` camera and a player with no target, when frames run, then pose and input response equal an `Orbit` camera with the same `orbit:` block.
- Given a target, when the owner's `PlayerTarget` becomes `Some`, then within `engage_secs` the camera sits on the far side of the player from the
  target, the target is in the frustum, and no frame-to-frame translation step exceeds the smoothing-implied bound.
- Given the target is cleared, hidden or despawned, when frames run, then the framing releases over `release_secs` with no snap and the camera
  keeps its final yaw.
- Given two split-screen players with different targets, then each camera frames only its own player's target, identically on every run.
- Given `SetCameraMode` away from a `LockOn` preset, then `LockOnFraming` is gone and no lock state leaks into the next mode; `"default"` restores it.
- `CurrentTarget`, `{target}` substitution, `target.*` events and the player's movement/`Transform` are unchanged by the feature.
- `cargo test -p ironhold_core --test '*'` and `cargo check -p ironhold_cli` pass; `ironhold_cli validate` accepts the demo and rejects the bad-value cases.
