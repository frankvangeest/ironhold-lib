# Feature: Over-the-shoulder follow + lock-on target camera

_Status: Draft — plan-review 2026-10-02: needs more design work (see "Plan-review" section at the end)_
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
      example, "movement is not changed" callout, split-screen note), `crates/ironhold_core/src/capabilities/CLAUDE.md` (lock-on layer, ordering vs
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

## Plan-review (2026-10-02)

Run by `/plan-review` while Frank was away: system-architect and ux-gamedesigner-reviewer, in parallel, read-only, claims checked against the code. **Combined verdict: needs more design work** — the blocking items below must be folded into this plan (and Frank's answers to the open questions recorded) before it can be marked Ready. The reports are reproduced verbatim (headings demoted one level).

### System-architect review

## System-architect plan review: `planning/features/lock_on_camera_mode.md`

Reviewed against `integration` @ `34803b1` (no `crates/` commits since the plan's `d56a554`, so every
citation still applies).

### Claims checked against the code

| Plan claim | Result |
|---|---|
| F1: Follow offsets are world-space (`camera.rs:543,551`) | **Confirmed.** `desired_pos = target.translation + follow.offset`, `look_target = target.translation + follow.look_at_offset`. The target's rotation is never read, so the backlog's "OTS needs zero engine code" premise is wrong. |
| F2: +Z is behind | **Confirmed.** `player.rs:568` uses `transform.forward()` (-Z) for movement, and `Turn` pre-multiplies a Y rotation (`player.rs:593-595`). The existing demo Follow preset `offset: (0.0, 3.0, 6.0)` (`camera_modes/prefabs/prefabs.ron:63`) already puts the camera behind at +Z. |
| F3: Orbit formula (`camera.rs:286-292`) | Confirmed. `look_at_offset` is world-space (`:287`). |
| F4: targeting chain (`targeting.rs:58-65`), auto-clear (`:373-392`) | Confirmed. No ordering edge to the camera chain, so adding `.after(target_auto_clear_system)` creates no cycle (same as `target_hud_update_system`, `lib.rs:355`). |
| F5/F6: `resolve_orbit_config_for_multiplayer` `:1318-1333`, `apply_camera_mode` `:1423-1529`, `SetCameraMode` `action_executor.rs:1000-1128` | Confirmed. |
| F7: exhaustive matches | Mostly right. **`validate.rs:428` is `camera_mode_nested_split_party_problem`, not a "range check"** (see N4). `query.rs` and `stats.rs` contain no camera code at all. |
| F8: shake is additive on `Or<(Orbit, Party)>` (`camera.rs:1117-1134`) | Confirmed. Lock-on is an absolute pose writer, so Decision 5's "shake does not accumulate" holds. |
| Two `FollowState` construction sites (`entity_spawner.rs:1460,1597`), no `FollowCameraDef` literals | Confirmed. Grep finds neither outside those sites, and none in tests. |
| Determinism: no camera system is a gameplay input | Holds for this feature. `player.rs`, `input.rs` and the sim never read `ActiveCameraMode`. The one existing camera-to-sim write is `camera_orbit_system`'s `character_rotate` (`camera.rs:~279-283`, `rotate_y` on the player's `Transform` in `Update`). It predates this plan, and lock-on adds nothing like it. |

### Verdict
**Needs more design work.** The schema shape is sound (`offset_space` on Follow; `LockOn` as a registry variant running as Orbit plus a `LockOnFraming` component). So are the crate boundaries, WASM/perf and determinism reasoning. The three blocking items below are all in the v2 pose and input design, and each would make an acceptance criterion or playtest step fail as written. v1 (OTS) is ready on its own.

### Blocking

**B1. Switching targets while locked makes the camera jump. The plan says it should not.**
In Approach v2 steps 3-4, only `orbit.yaw` and `weight` are smoothed. `focus` and `radius` are computed from the *current* target's raw position each frame. When the target changes from A to B by Tab, Shift+Tab or a click (`apply_player_target`, `targeting.rs:333`), `weight` stays at 1.0. In that same frame the focus moves by `focus_bias·(aim_B − aim_A)` and the radius by `distance_zoom·(sep_B − sep_A)`. For example, a dummy 5 m away versus one 20 m away is a 9 m radius change, cut only by the `max_radius` clamp. This breaks AC 3 ("no frame-to-frame translation step exceeds the smoothing-implied bound") and playtest 2 ("smooth re-aim").
**Correction:** add `framed_aim: Vec3` (and/or `framed_sep: f32`) to `LockOnState`. Move it toward the live aim point at `1 − exp(−rate·dt)`, reusing `yaw_smoothing` or adding `retarget_smoothing` with a default. Snap it only on engage from `weight == 0`. Use it in place of `last_target_pos` (release then naturally holds the last framed point). Add a test: two targets at different distances, retarget while engaged, assert the maximum per-frame translation stays below a bound.

**B2. The pose geometry is ambiguous, and one reading puts the camera at or in front of the player.**
Step 4 says "pose = the existing Orbit formula with that radius/focus", which is `camera = focus + rot·Z·radius` with `look_at(focus)`. That anchors the camera to the *shifted* focus. The camera's horizontal setback from the player is then `radius·cos(pitch) − shift`. `shift` grows with separation, but `radius` is clamped at `max_radius`.

Take the plan's own example (r₀ 8, max 14, default pitch 0.5, `focus_bias` 0.4) with the shift read as `0.4·sep`. The setback is 2.3 m at sep 25, 0 at sep 31 (camera directly above the player), and −1.7 m at sep 35, which is the example's `max_distance`. At that point the player is behind the camera and out of frame. The phrase "midpoint-weighted target aim" is not defined, so readers can disagree on whether the shift is `bias·sep` or `bias·sep/2`. AC 3's "target is in the frustum" is also not guaranteed at high authored pitch. With pitch 0.9 and fov 60, a target 35 m away sits about 33° above the view centre, beyond the 30° half-fov.
**Correction:** state the model explicitly:
- `pivot = player + look_at_offset`
- `camera = pivot + rot(yaw, pitch)·Z·radius`, so the camera position is always anchored on the player and the player stays in frame for any separation.
- `rotation = looking_at(lerp(pivot, framed_aim, focus_bias·w_eased))`
- Define `focus_bias` as a fraction of pivot→aim. Drop "midpoint-weighted".
- Qualify the frustum AC: guaranteed at the default pitch range (≤ 0.6 rad), tested at default pitch. Record high-pitch vertical framing as a known v2 limit, or add an optional `locked_max_pitch` later (Open question C).

**B3. With the engine-default mouse bindings, every left-mouse orbit drag drops the lock.**
`click_select_system` clears the player's target on *any* `just_pressed(Left)` that hits no selectable (`targeting.rs:202, 255-259`). The engine default is `orbit_button: "Either"` (`schema/player.rs:257`), and the `camera_modes` hub player authors `"Either"` (`prefabs.ron:44`). So starting a left-mouse orbit drag over empty ground clears `PlayerTarget`, and lock-on releases. Playtest 5 ("mouse orbit while locked: pitch/zoom work") fails on defaults, and AC 2/3 are fragile in the demo the plan extends. This is the same raw-`ButtonInput<MouseButton>` coupling as the `## Bugs` entry "Left mouse button on any UI node also orbits the camera / strafes the character" (`backlog.md:21`). Lock-on turns it from a minor annoyance into a feature-breaking interaction.
**Correction:** the plan must choose one of these, and list it in Tasks and the playtest:
- (a) **Prerequisite fix:** `click_select_system` clears only on a *release* without drag (cursor moved < N px since press). Bundle it with the backlog bug's proposed `UiPointerCaptured` resource so one pointer-gesture state serves orbit, strafe and click-select.
- (b) **Explicit scope-out:** lock-on presets and the demo author `orbit_button: "Right"`, `docs/20` states that left-mouse orbit clears the lock, playtest 5 says "right mouse", and the click-vs-drag issue is logged as its own `## Bugs` item.

Recommendation: (b) for v2, with (a) logged and linked to the pointer-capture bug (Open question A).

### Non-blocking

- **N1. The query signature as written would hit a Bevy B0001 access conflict.** `(&mut Transform, …) With<OrbitCameraMode>` alongside `Query<(&Transform, &PlayerTarget), With<CharacterController>>` and a `fresh_global_transform` target query (`Option<&Transform>`) would panic at startup. Add `Without<CharacterController>` to the camera query, as `camera_orbit_system` does (`camera.rs:195`). Merge the player and target lookups into one read-only `Query<(&GlobalTransform, Option<&Transform>, Option<&ChildOf>, Option<&PlayerTarget>), Without<LockOnFraming>>`.
- **N2. `SelectAimHeight` is not always present.** It is inserted only alongside `ClickSelectable` (`targeting.rs:27-31`). A `targetable`-only prefab lacks it, so the lookup must be `Option` with a fallback to `SELECT_AIM_HEIGHT`, exactly as `debug_selectables_system` does (`targeting.rs:358`). The plan's "make the const `pub(crate)`" covers the fallback.
- **N3. Split-spawn `"default"` restore would silently lose lock-on.** `spawn_split_camera_for_player` hard-codes `AuthoredCameraMode(CameraModeDef::Orbit(cam))` (`entity_spawner.rs:1306`). `resolve_orbit_config_for_multiplayer` returns a bare `CameraConfig`, and `LockOn` currently falls into its `Some(other) => warn!` arm (`:1321`). AC 6 ("default restores it") therefore fails for split players unless:
  - the resolver gains a `LockOn(def) => def.orbit.clone()` arm, returning `(CameraConfig, Option<LockOnCameraDef>)` or the full `CameraModeDef`;
  - the split site stores the real `LockOn` def in `AuthoredCameraMode` and inserts `LockOnFraming`.
  The party/dynamic `first_cam` call sites (`:833`, `:941`) just take `def.orbit`, which is fine. This is cheap, so do it in v2 rather than relying on Open question 2's fallback.
- **N4. CLI task corrections.**
  - Extend `camera_mode_nested_split_party_problem` (`validate.rs:423-446`) to `LockOn`'s `orbit`. `LockOn((orbit: (split: …)))` parses and does nothing, exactly like the Orbit case that check exists for.
  - Add a `LockOn` arm to `camera_mode_vocab_problems` (`:581-587`).
  - `query.rs`/`stats.rs` have no camera awareness, so drop "add `LockOn` to query.rs".
  - The step-6 `query actions` spot-check proves nothing here (no `Action` change). Run the freshly built `validate` on the extended demo instead.
- **N5. `deny_unknown_fields`.** Put it on the new `LockOnCameraDef`. It is a new struct, so there is zero back-compat cost, and it catches `focus_bais` typos. It cannot cover the inner `CameraConfig`. `FollowCameraDef` has no deny, so a typo `offset_spce: Target` silently falls back to `World`. Mention this in the docs row. Adding deny to Follow is a separate schema-tightening item ([[schema-tightening-blast-radius]]).
- **N6. Contradiction about `transition`.** The struct comment says the orbit's own `transition` is "ignored (ours wins)", but `transition()` falls back with `.or(def.orbit.transition)`. Pick one. Recommendation: fall back, and document it.
- **N7. v1 OTS details.**
  - Player yaw is written in `FixedUpdate` at 64 Hz with no transform interpolation. A target-space offset therefore steps visibly at 144 Hz when `smoothing: 0`. Document "keep smoothing > 0".
  - "Full quaternion is safe" overclaims. `ROTATION_LOCKED` stops *physics* rotation, not an authored non-yaw spawn rotation. Extracting yaw only (`Quat::from_rotation_y(rot.to_euler(EulerRot::YXZ).0)`) is cheap and tilt-proof.
- **N8. Fixed-tick interplay.** `gameplay_fixed_tick_pipeline.md` §1 moves tab-targeting to the fixed tick. Once `target_auto_clear_system` leaves `Update`, the `.after(target_auto_clear_system)` edge (and `target_hud_update_system`'s) no longer orders anything; it is naturally satisfied, since FixedMain runs before Update. Note in the plan that the edge is removed or replaced there, and tell the fixed-tick plan it has one more consumer. The determinism reasoning is otherwise correct: render-only output, and std `atan2`/`exp` are fine.
- **N9.** `camera_orbit_system` returns early when the inspector is enabled (`camera.rs:200-205`). `lock_on_framing_system` should mirror that, or it keeps overriding yaw under the inspector.
- **N10.** `apply_camera_mode`: put `remove::<LockOnFraming>()` in the common removal block (`entity_spawner.rs:1439-1448`), not "every arm".
- **N11.** `max_distance` is horizontal (xz) but `target_range` is 3D. A `max_distance < target_range` setting lets Tab select targets that never engage. Consider a `--strict` warning, or document it.
- **N12.** Shift+Tab is a *reversed nearest-first list*, not "previous target" (`targeting.rs:316-317`). The docs and playtest wording should not promise "previous".
- **N13.** Early-out at `weight == 0` with no target must avoid `DerefMut` on `ActiveCameraMode`/`Transform` (read through `&*` first). Otherwise change detection fires every frame. This is cosmetic, since `camera_orbit_system` already marks both every frame.

### Open questions for Frank

- **A (new, from B3). How to handle left-mouse orbit clearing the target.**
  - (a) Click-vs-drag fix in `click_select_system` before v2, bundled with the left-mouse UI pointer-capture bug.
  - (b) v2 lock-on presets use `orbit_button: "Right"`, documented, with click-vs-drag logged as a bug.
  **Recommend (b) now, (a) as the follow-up bug fix.**
- **B (new, from B1).** Reuse `yaw_smoothing` for retarget smoothing, or add a separate `retarget_smoothing` field? **Recommend reusing it.** One dial is easier for designers, and a field can be split out later without breaking anything.
- **C (new, from B2).** Override or clamp pitch while locked so far targets stay in frame? **Recommend not in v2.** Pin the frustum AC to default pitch and log `locked_max_pitch` as a later option.
- **Plan OQ1 (OTS mouse aim).** Agree: no. It is a separate feature.
- **Plan OQ2 (split spawn).** Support it in v2. Per N3 it is a two-site change, so no fallback is needed.
- **Plan OQ3 (gamepad target prev).** Keep forward-cycle only.
- **Plan OQ4 (v3 movement).** Yes. Add it to the Icebox now, depending on `gameplay_fixed_tick_pipeline.md`. Facing must be computed on the tick with `det_math`, as the plan already says.

### Completeness
The plan includes the Planned-at hash, RON examples, a phase table, tests, docs, CLI, demo and the review list. It is missing:
- tests for B1 (retarget continuity) and B3 (left-mouse orbit while locked);
- the split-spawn `AuthoredCameraMode` assertion (N3);
- a `docs/20` note on orbit-button guidance for lock-on presets.

### UX-gamedesigner review

## UX Review: lock_on_camera_mode.md (plan-review, pre-code)

Reviewed: `planning/features/lock_on_camera_mode.md` (planned at `d56a554`), cross-checked against `docs/20_data_formats.md` camera sections (~2327-2585, targeting ~501/587-612, InputMap ~2152-2158, actions ~3846-3847) and `assets/projects/camera_modes/` (prefabs + follow_test scene).

#### Verdict
**Needs more design work.** Most of it is solid. Making OTS an `offset_space` field on `Follow` instead of a new variant is the right call. Auto-engaging from `PlayerTarget` is a clean opt-in, since a designer who wants tab-targeting without a camera lock just keeps `Orbit`. Reusing Orbit underneath means shake, blend and split viewports keep working. Two player-facing gaps need fixing before coding starts. Neither is a code-architecture problem: one is about how a player releases the lock, the other is about framing.

#### Blocking

**B1. Gamepad and keyboard-only players have no way to release lock-on.**
In the plan, the camera is locked whenever the owner's `PlayerTarget` is `Some`. Today a target is cleared only by:
- clicking empty space (mouse; `docs/20` ~1939),
- the `ClearTarget` action, which clears the *primary* player's `CurrentTarget` only (`docs/20` ~3847, ~587-594),
- the target dying, being hidden or despawning,
- walking past `max_distance`, which the plan defaults to `None`, so this never happens.

As a result:
- A gamepad player, or any split-screen player other than player 1, who presses `gamepad_target_next` (North/Y) stays locked until the target dies. There is no input that gets them out.
- Tab-cycling never cycles to "no target".
- Split-screen is where the plan most wants per-player lock-on, and that is exactly where the release path is missing. The plan states "no new input" as a goal, but the game can't be played this way.

**Fix:** add per-player release inputs:
- `InputMap.target_clear` (keyboard, e.g. default `None` or `"Escape"`; check whether the pause menu already owns `Escape`).
- `gamepad_target_clear`, recommended default `"RightThumb"` (R3). R3 is the genre-conventional lock-on toggle. First check that no existing default binding uses RightThumb.

Each one clears that player's own `PlayerTarget`. Add both to the InputMap table, the validate gamepad-collision checks, and the playtest checklist. If Frank won't add an input in this feature, the docs must say plainly that lock-on is mouse-release-only in v2. In that case the split-screen demo must not ship lock-on on a gamepad seat, and `max_distance` must have a finite default (see B2).

**B2. Nothing keeps the player on screen.**
The focus point is `lerp(player, target, focus_bias)`, the radius is clamped to `orbit.max_radius`, and `max_distance` defaults to `None`. Click-select picks any targetable entity within ~70 px of the cursor, which can be a monster 40-60 m away. Example: at 50 m separation with `focus_bias: 0.4`, the focus point sits 20 m from the player while the radius is clamped to 14 m. The camera then frames empty ground between the two and the player is off screen. With `focus_bias` near 1.0 this happens at almost any distance.

The acceptance criteria only require the *target* to be in the frustum, so this failure would pass every test.

**Fix:**
- (a) Change `max_distance` to a finite default, for example `25.0` (stays optional; `None` remains available as an explicit opt-out). This matches the clamped radius budget.
- (b) Bound the focus shift. Either cap the effective bias so the player stays inside the frame, or clamp the focus offset to a fraction of the current radius.
- (c) Document the scale: "0.0 = player, 0.5 = midpoint, 1.0 = target; above ~0.5 the player can leave the frame". The plan's "midpoint-weighted target aim" wording is ambiguous.
- (d) Add an acceptance criterion and a playtest step: "player stays inside the frustum for any target within `max_distance`".

#### Non-blocking

**N1. `offset_space: Target` collides with the targeting vocabulary.**
This same plan introduces lock-on, where "target" means the selected enemy (`PlayerTarget`, `target_next`, `target.changed`, `{target}`). A designer reading `offset_space: Target` on a camera will reasonably think it means "offset relative to my lock-on target".

**Recommend** renaming the variant to `Local` (or `Character` / `Facing`), with the doc line "offset rotates with the followed character's facing". Keep `World` as the default. Keep the enum unquoted, like `ease:` and `orientation:`.

**N2. The `offset` axis meaning is undocumented for `Follow`, and wrong for `World` space.**
The `CameraConfig` table says `offset` is "(right, up, back)" (`docs/20` ~2331). That only holds in character space. For `World` it is world X/Y/Z. The new field makes this difference matter. The OTS recipe needs to say:
- `+X` = right shoulder, `-X` = left shoulder,
- `+Z` = behind (this matches Finding 2; the backlog's `-3.0` was wrong),
- `look_at_offset` rotates too, so `(0.3, 1.5, -4.0)` means "4 m ahead of the character".

**N3. `FollowCameraDef` and `LockOnCameraDef` need real field tables.**
Today the `CameraModeDef` variant table (`docs/20` ~2405-2412) has only a bare "Payload" column. No `Follow` field table exists anywhere: no types, defaults, or required/optional status. Adding `offset_space` to a comma list is not enough. Write proper tables for both structs, covering type, default, units and a one-line effect. Also state that `LockOn`'s FOV comes from `orbit.fov`, whose default is **45**, not 60 like `Follow`/`Fixed`. That is a third FOV default for designers to trip over.

**N4. Make `orbit:` optional.**
The plan makes `orbit` the only required field. `LockOn((focus_bias: 0.5))` would then fail to parse with a struct error. Every other camera payload is fully defaulted. Give `orbit` a default (`CameraConfig::default()`) so the minimal preset is `LockOn(())`.

**N5. The `transition` rule contradicts itself.**
The struct comment says "its own `transition` is ignored (ours wins)", but `transition()` is specified as `def.transition.or(def.orbit.transition)`, a fallback. Pick one rule and document it. I recommend the fallback, plus a `--strict` validate note when both are set. If you choose "ignored", it becomes a silently-unread field, the same problem as `ActionSlotDef.label`, and validate should warn on `orbit: (transition: ...)`.

**N6. The plan's own examples break house style.**
`transition: Some((...))` and `max_distance: Some(35.0)` go against the `implicit_some` convention (`docs/20` ~1922). `ron_lint` rejects `Some(...)` in assets. The docs examples in `docs/20` already write `transition: (duration_secs: ..., ease: ...)` without `Some`. Write `max_distance: 35.0` and `transition: (duration_secs: 0.4, ease: EaseInOut)` everywhere this plan's examples get copied: docs, demo, playtest notes.

**N7. Add `LockOn` to the double-paren gotcha list.**
The "RON syntax gotcha" callout (`docs/20` ~2416-2422) lists the variants that need `X((...))`. Add `LockOn`. Also show the nested form, `LockOn((orbit: (offset: ...), focus_bias: ...))`, where the inner `orbit:` uses *single* parens. Verify the final docs example with `ironhold validate` against a real project; don't write it from the schema.

**N8. The yaw whips at melee range.**
`desired_yaw = atan2(player - target)` swings fast when the player sidesteps or circles a target 1-2 m away, which is the main melee lock-on situation. The 0.05 m cutoff only prevents divide-by-zero-style flips; it does nothing about whip-pan.

**Recommend** fading yaw tracking near the target: scale the effective `yaw_smoothing` down below roughly 2 m of separation. This could be an internal constant first; expose it only if playtest asks. Also add a playtest step: "circle a target at 1.5 m with Q/E strafe; the camera must not whip".

**N9. Document the input and movement consequences, in plain words.**
- **Movement is unchanged (Decision 4).** This is acceptable for v2. Orbit with a mouse-orbited camera already decouples "forward" from screen-up, so lock-on is no worse than today's Orbit. But the name `LockOn` promises a Souls/Zelda strafe-around-target feel, and W walks wherever the character faces. Put a callout at the top of the `LockOn` docs section, not at the bottom: "Camera-only. Your character still moves and turns exactly as in Orbit; the camera just keeps the target in view. Use strafe keys to circle a target."
- **Mouse orbit:** horizontal orbit input does nothing while locked; pitch and zoom still work.
- **Keyboard look:** `look_left`/`look_right` also do nothing while locked. This matters for split-screen players, who rely on keyboard look because split cameras set `orbit_button: "None"`. Add a sentence to the split-screen keyboard-look note (`docs/20` ~2357).
- **Needs verification:** how `character_rotate_button` (default `Some("Right")`) interacts with a locked yaw. If RMB-drag snaps the character to the camera yaw, then while locked it would turn the character to face the target. That would be a nice free "face target" for mouse users, or a surprise. Verify it, then document whichever behaviour you get.

**N10. `max_distance` release leaves the target ring on.**
When the player walks past `max_distance`, the framing releases but the target stays selected. The player sees a ring but the camera is no longer locked. This is defensible, but write it down in the field table: "the target stays selected (ring and HUD remain); only the camera framing lets go".

**N11. Silent-failure cases validate should catch.** These follow the warn-vs-silent principle:
- A `LockOn` preset or `camera_mode` in a scene with no `targetable: true` entities. `--strict` warning: the camera will behave as plain Orbit forever.
- `split:`/`party:` nested inside `LockOn`'s `orbit:`. Extend the existing hard error for `Orbit` (`docs/20` ~2576).
- `LockOn` authored on a party-mode player, which is silently ignored because a shared camera has no single owner. Add a validate warning and a doc line.
- If Open Q2 falls back to "warn at spawn" for split scenes, make it an `ironhold validate` error too, not only a runtime `warn!`.

**N12. Name `distance_zoom` for what it does.**
It sits next to `zoom_speed` (scroll-wheel speed) inside `orbit:` and reads like the same family of setting. It actually means "extra metres of camera distance per metre between you and the target". Recommend `separation_zoom` (or `zoom_per_metre`), with the units in the table. `focus_bias`, `yaw_smoothing`, `engage_secs` and `release_secs` are well named. `_secs` matches `duration_secs`, `fade_secs` and `delay_secs`. `*_smoothing` as an exponential rate matches `Follow.smoothing`/`rotation_smoothing`.

**N13. Numeric defaults look reasonable, with the B2 caveat.**
- `yaw_smoothing: 6.0` has a half-life of about 0.12 s, responsive without snapping.
- `engage_secs: 0.35` and `release_secs: 0.5` follow the documented "keep blends ≤0.4 s for gameplay" guidance (`docs/20` ~2559). A slightly longer release is good feel.
- `distance_zoom: 0.6` saturates against `max_radius: 14` at roughly 8 m separation with the example `offset (0,5,8)`. After that, framing relies entirely on `focus_bias`, which is why B2 matters.
- `focus_bias: 0.4` is fine once it is bounded.

**N14. Demo and project impact.**
- **Fine:** `offset_space` defaults to `World`, so every existing `Follow` stays byte-identical. Only `camera_modes/player_follow_demo` uses `Follow`. Existing `camera_modes:` registries (`entity_logic_demo` main, `local_coop_demo` room11) are unaffected.
- **Showcase scene:** put it in `camera_modes`, as planned. Add a new `lock_on_test` scene with its own hub portal, matching the existing per-mode `*_test` scene pattern, plus an `ots_test` scene. That's better than overloading the hub.
- **WASM trap:** none of the `camera_modes` player prefabs authors `target_next`. They inherit `"Tab"`, which browsers intercept in WASM (`docs/20` ~2152). The new lock-on player prefab must set `target_next: "KeyT"`. The playtest checklist's "Tab onto a dummy" must say KeyT for the web build.
- **Hint text:** in-world hint labels must use ASCII `-` (the engine font has no em-dash glyph) and stay under the font-width budget (~45 chars per line at 700 px, 22 px font).
- **`3rd_person_game_demo`:** do not switch its default camera to `LockOn`. Its MMO-style click and KeyT targeting would make every selection swing the camera. If you want it there, add an opt-in `"lock_on"` registry preset toggled by a key rule, documented as single-player-only, because the rule events are primary-player-only.
- **Split-screen:** a `local_coop_demo` room is the natural place for the split-screen proof, following the room11 precedent. If you add one, follow the room conventions: portal chain, exits listed, 82-char ceiling.
- Coordinate with `dynamic_camera_mode_switching.md` v2 as the plan says.

**N15. Docs tasks the plan misses.**
- `docs/STATUS.md` camera feature list.
- The target-indicator section (`docs/20` ~501): add one line saying that a `LockOn` camera reads the same selection.
- The `InputMap` table (`target_next` / `gamepad_target_next` rows): mention that these engage lock-on when the camera is a `LockOn` mode. Also add the B1 clear inputs there.
- The "Worked examples" paragraph under the registry section (`docs/20` ~2581): point to the new demo scene(s).
- The plan lists `crates/ironhold_core/src/CLAUDE.md` as a docs task. That's fine for developers, but designers can't open it. Every designer-relevant line (camera-only movement, yaw override, split behaviour) must land in `docs/20`.

**N16. Playtest checklist additions.**
- Use KeyT, not Tab, in the web build.
- Click-select a target 40+ m away: the player stays on screen (B2).
- Gamepad seat: lock on with Y, then release with the new clear button (B1).
- Circle a target at melee range with strafe keys (N8).
- Split player: try `look_left`/`look_right` while locked (expect overridden), and RMB-drag while locked (N9).
- OTS: confirm `+X` puts the camera over the *right* shoulder, and tank-turn quickly with A/D to judge the swing arc at `smoothing: 10`.
- Round-trip `SetCameraMode` to `"default"` with a `transition:` on both ends. Known asymmetry: the transition is read from the target mode.

#### Open questions for Frank (with recommendations)

1. **OTS aiming (mouse yaw drives the character):** Recommend **no** for this feature, as planned. Log it as its own backlog item ("shooter OTS: FirstPerson-style yaw with a third-person pose, plus shoulder swap"). `offset_space` alone covers the action-adventure OTS that most designers will reach for first.
2. **Split-screen `LockOn` at spawn:** Recommend **supporting it at spawn**. Authoring `camera_mode: LockOn(...)` on a split player prefab is the obvious path a designer will try, and `SetCameraMode(owner_player)` alone needs a rule per scene. If it proves invasive, the fallback must be an `ironhold validate` **error** plus a docs note, not only a runtime `warn!`. A designer looking at a working Orbit camera won't open the console.
3. **Gamepad target switching:** Recommend **forward-cycle only for now** (no `gamepad_target_prev`, no stick-flick, since right-stick-X is Turn). But add the **release** button (B1); that is the real gap. Previous-target on gamepad can wait for playtest feedback.
4. **v3 movement (face-target / strafe-around-target):** Recommend **queueing it in Icebox now**, linked from the docs callout ("planned: lock-on movement"). The name `LockOn` sets a feel expectation that v2 deliberately doesn't meet. A visible backlog item makes the camera-only scope read as staged, not unfinished.
5. **(new) Naming:** rename `offset_space: Target` to `Local`/`Character` (N1) and `distance_zoom` to `separation_zoom` (N12). Recommend doing both before any RON ships, while renaming costs nothing.
