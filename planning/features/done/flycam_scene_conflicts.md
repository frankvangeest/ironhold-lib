# Feature: Flycam scene conflicts — duplicate flycam tags + player/flycam priority

_Status: Done_
_Planned at: `607234a` (2026-08-17)_
_Revised after plan review (system-architect + ux-gamedesigner-reviewer), 2026-08-17 — see "Plan review changes" below._
_Shipped 2026-08-19 — playtest confirmed by Frank, no console errors. See `planning/backlog.md`'s matching entry for the post-implementation-review summary._

## What
Fixes two related silent-drop bugs in `scene_loader.rs`'s flycam-entity handling (see
`planning/backlog.md`'s "Flycam scenes silently drop entities in two related cases"):

1. **Duplicate flycam tags.** A scene authoring two `tags: ["flycam"]` entities silently keeps
   only the last one in `entities:` order (`flycam_start` unconditionally overwritten at
   `scene_loader.rs:241`) — no warning that the first was discarded.
2. **Player + flycam combo.** A scene combining a `tags: ["player"]` entity with a
   `tags: ["flycam"]` entity spawns the player's own camera and drops the flycam entirely — the
   dispatch at `scene_loader.rs:767-879` is a plain `if !player_configs.is_empty() { .. } else if
   let Some(flycam) = .. ` chain, so the player branch always wins when both are present.

Decided fix for (2) (Frank, 2026-08-17): **flycam takes priority** — a "spectator mode" use case.
When both are present, the player entity/entities still spawn normally (movement, stats, targeting,
etc. all keep working), but they get **no camera of their own**; the flycam becomes the scene's
sole active camera instead.

## Why
Both are real, silent authoring mistakes with no diagnostic today, found via debug-detective's
investigation of a related bug ("A flycam-tagged prefab's own `model:` never renders"). A
designer building a spectator/debug-fly-around view over live gameplay (player entities doing
their normal thing, camera free-flying independently) currently can't do this at all — the player
branch always wins — with zero indication why the flycam appears to do nothing.

## Plan review changes
Reviewed pre-implementation by `system-architect` and `ux-gamedesigner-reviewer`. Both returned
"refine" verdicts. Changes folded into the Approach below:

- **`SuppressPlayerCameras` resource, not a `PendingPlayerConfig` field.** The original plan's
  `has_flycam: bool` field on `PendingPlayerConfig` is dropped. Instead, a new scene-level
  resource is inserted at scene load and reset on every `Action::LoadScene` (alongside the four
  existing camera resources at `action_executor.rs:58-61`) — the terrain-deferred path
  (`spawn_player_when_terrain_ready`) just reads it as a normal `Res<...>` system param, no struct
  change needed.
- **`spawn_players_and_camera` takes a 2-variant enum, not an 11th positional `bool`.** The
  function already has 10 parameters including an `Option<&mut PrimitivePlayerCtx>` — a bare
  `bool` at the call site is unreadable. `CameraSpawnMode::Spawn` / `CameraSpawnMode::Suppressed`.
- **`is_split_screen` (`scene_loader.rs:755`) must gate on `flycam_start.is_none()`.** It's
  currently computed from `player_configs` alone; with 2+ players + `split` + a flycam present, it
  would stay `true` while no split cameras actually exist, duplicating `WorldLabelRank`/
  stat-widget entities that can never display (real gap found in review, not in the original
  bug report).
- **`info!` → `warn!` for the suppression case, with prescribed remedy wording.** Precedent in
  this file (`camera.split`+`camera.party` both set) shows a *supported* outcome with a defined
  winner still gets a `warn!`, not `info!` — because the console is the designer's only signal and
  `info!` lines are easy to miss among routine scene-load logging. Message names the effect and
  the fix: "...flycam takes priority; the player spawns and plays normally but gets no camera of
  its own. This is supported (spectator mode) — remove the flycam entity if you wanted the
  player's own camera."
- **New warning: flycam + `split`/`party` combo.** Not identified in the original plan. Because
  camera suppression is total (not just "downgrade to one camera"), a co-op scene that authors
  `camera.split`/`camera.party` on its first player AND also has a stray flycam entity would
  silently lose split-screen entirely with only the generic suppression warning as a clue. Add a
  second, specific `warn!` for this combination.
- **Duplicate-tag warning gets a CLI validate counterpart.** Every other duplicate-authoring
  `warn!` in `scene_loader.rs` (gamepad index, `player_index`, etc.) pairs with an
  `ironhold_cli validate` check; flycam has zero CLI coverage today. Add a matching validate
  check for duplicate `tags: ["flycam"]` entities in one scene.
- **Docs get a named playtest scene and explicit disambiguation**, not "whichever project is
  simplest." Scope: `assets/projects/camera_modes/scenes/flycam_spectator_test.scene.ron` (new),
  since `camera_modes` already hosts `flycam_test.scene.ron` and is the project designers look at
  to compare camera modes side by side. Docs must also disambiguate this feature from the
  already-supported `camera_mode: Flycam(...)` set directly on a *player* prefab
  (`spawn_camera_for_mode`, `entity_spawner.rs:1431`) — a designer could otherwise reasonably
  confuse the two authoring paths.
- **Two edge cases documented as known limitations, not fixed in this change** (kept out of scope
  to keep this fix bounded to the scene-load bugs actually reported; logged to
  `planning/claude_suggestions.md` as follow-ups instead):
  - `Action::Spawn`ing a `tags: ["player"]` prefab at runtime (`entity_spawner.rs:396`, dynamic
    join / character-select path) always spawns that player's own full-window Orbit camera via
    `spawn_player_entity`, regardless of `SuppressPlayerCameras` — a scene that starts in
    spectator mode and then dynamically spawns a player mid-session would get a competing camera.
    Not part of the reported bug (both reported cases are scene-load-time only).
  - `Action::SetCameraMode` with `owner_player` omitted targets *all* cameras carrying
    `AuthoredCameraMode`, including the flycam itself (`action_executor.rs:915-918`) — this can
    convert the flycam into a targetless `Orbit` camera with no built-in recovery path except
    `"default"`. Pre-existing ambiguity, unrelated to this fix's scope.

## Approach

### Duplicate flycam tags (simple)
Add a scene-load `warn!` in the `is_flycam` branch (`scene_loader.rs`, immediately before the
unconditional overwrite at line 241) when `flycam_start` is already `Some` — name both entity ids,
state that only the last one spawns, and prescribe the fix (remove one of the two `tags:
["flycam"]` entities). Mirrors the existing duplicate-authoring `warn!` style in this file (name
offenders → consequence → remedy). No runtime behavior change — last-tag-wins stays as-is, now
diagnosed.

Add a matching `ironhold_cli validate` check (new, `crates/ironhold_cli/src/commands/validate.rs`)
using the same per-scene entity/prefab-tag lookup pattern already used for other prefab-tag checks
there: for each scene, count entities whose resolved prefab has `tags` containing `"flycam"`; if
more than one, report naming both.

### Player + flycam priority (the real change)
Add `pub(crate) enum CameraSpawnMode { Spawn, Suppressed }` and a `pub(crate) struct
SuppressPlayerCameras(pub bool)` resource (`runtime/scene_manager/mod.rs`, alongside
`ActiveSplitScreen` etc.; registered via `.init_resource::<SuppressPlayerCameras>()` in `lib.rs`
next to the other camera resources; reset to `SuppressPlayerCameras(false)` in
`action_executor.rs`'s `Action::LoadScene` handler alongside the four existing resource resets at
lines 58-61).

`spawn_players_and_camera` (`entity_spawner.rs`) gains a `camera_spawn: CameraSpawnMode` parameter.
Player entities always spawn (the existing per-player loop, and the `player_index: 0` duplicate
warning, both unconditional). Immediately after that loop, before touching any camera resource or
spawning any camera: if `camera_spawn` is `Suppressed`, `return`. This skips every camera-related
resource insert (`ActiveSplitScreen`, `DynamicSplitConfig`, `ActiveSplitSlotCount`,
`TargetRingVisibilityMode`) and every camera spawn call (single-player Orbit, split, party) —
leaving those resources exactly as `Action::LoadScene` just reset them, the same state a
flycam-only zero-player scene already produces.

Both call sites of `spawn_players_and_camera` determine the flag from `flycam_start`/the resource:
1. **Immediate path** (`scene_loader.rs`, non-terrain): `if flycam_start.is_some() {
   CameraSpawnMode::Suppressed } else { CameraSpawnMode::Spawn }`.
2. **Terrain-deferred path** (`spawn_player_when_terrain_ready`): add
   `suppress: Res<crate::runtime::scene_manager::SuppressPlayerCameras>` as a system param, read
   `suppress.0`. No `PendingPlayerConfig` change needed.

**`is_split_screen` (`scene_loader.rs:755`) gate added:**
```rust
let is_split_screen = flycam_start.is_none()
    && player_configs.len() >= 2
    && player_configs.first().is_some_and(|p| p.split.is_some());
```

**Flycam spawning moves out of the exclusive `if/else if` entirely** — spawns unconditionally and
immediately whenever `flycam_start.is_some()`, regardless of `player_configs`/`scene.terrain`
(flycam is a plain `Transform` + `Camera3d` spawn, no terrain dependency). Restructured shape in
`scene_loader.rs`:

```rust
if let Some((fc_transform, mode)) = flycam_start {
    // spawn flycam camera immediately — unconditional, moved above the player branch
    // (existing spawn code, unchanged)

    if !player_configs.is_empty() {
        warn!(
            "Scene has both a `tags: [\"player\"]` entity and a `tags: [\"flycam\"]` entity — \
             flycam takes priority; the player spawns and plays normally but gets no camera of \
             its own. This is supported (spectator mode) — remove the flycam entity if you \
             wanted the player's own camera."
        );
        if player_configs.first().is_some_and(|p| p.split.is_some() || p.party.is_some()) {
            warn!(
                "... first player's `camera.split`/`camera.party` config is ignored entirely \
                 because a flycam-tagged entity is also present — split-screen is fully \
                 suppressed, not downgraded to one camera. Remove the flycam entity to restore \
                 split-screen."
            );
        }
    }
}

commands.insert_resource(crate::runtime::scene_manager::SuppressPlayerCameras(flycam_start.is_some()));

if !player_configs.is_empty() {
    // unchanged terrain/non-terrain branches; non-terrain call site passes
    // camera_spawn: if flycam_start.is_some() { Suppressed } else { Spawn }
} else if flycam_start.is_none() && !scene.spawn_points.is_empty() {
    // ...unchanged
} else if flycam_start.is_none() {
    // default camera — unchanged, now also gated on no flycam already spawned above
}
```

### What happens to camera-consuming systems when a player has no camera?
Not a new problem class — same shape as two already-supported cases: `Party`-mode (one shared
camera, no per-player ownership) and a flycam-only scene with zero players (already shipped).
Verified during plan review by reading every camera-target lookup: all are `let Some(..) else {
continue }` (`camera.rs:215, 531, 589, 808, 957`), `click_select_system` falls back to the primary
player (`targeting.rs:210-213`), and `Action::CameraShake` already warns-and-no-ops on a flycam
scene since flycam carries neither `OrbitCameraMode` nor `PartyCameraMode`. No panics found in
static review; **verify empirically in the playtest step anyway** (target an entity, try
`CameraShake`, confirm no panics) rather than relying solely on static review.

## Tasks
- [ ] `scene_loader.rs`: warn on duplicate `tags: ["flycam"]` entities (name both, state
      consequence, prescribe remedy).
- [ ] `ironhold_cli`: add a matching `validate` check for duplicate `tags: ["flycam"]` entities
      per scene.
- [ ] `runtime/scene_manager/mod.rs`: add `CameraSpawnMode` enum and `SuppressPlayerCameras`
      resource; register via `.init_resource::<SuppressPlayerCameras>()` in `lib.rs`.
- [ ] `action_executor.rs`: reset `SuppressPlayerCameras(false)` in `Action::LoadScene` alongside
      the existing four camera-resource resets.
- [ ] `entity_spawner.rs`: add `camera_spawn: CameraSpawnMode` param to `spawn_players_and_camera`;
      `return` immediately after the player-entity loop (before any camera resource/spawn) when
      `Suppressed`.
- [ ] `scene_loader.rs`: restructure the player/flycam/spawn-points/default-camera dispatch per
      the Approach section above; add the `is_split_screen` gate; add the two `warn!`s
      (suppression, split/party-ignored); insert `SuppressPlayerCameras` resource.
- [ ] `spawn_player_when_terrain_ready`: add `Res<SuppressPlayerCameras>` param, pass through as
      `camera_spawn`.
- [ ] Tests: duplicate-flycam-tags warning fires (and only the last spawns, unchanged); CLI
      validate reports duplicate flycam tags; a player+flycam scene spawns the player entity with
      no per-player camera AND spawns exactly one flycam camera; the same for a player+flycam+
      terrain scene once terrain is ready; a flycam+split/party scene warns and suppresses split
      resources too; a flycam-only scene (no players) is unaffected (regression); a player-only
      scene is unaffected (regression).
- [ ] Docs: `docs/20_data_formats.md`'s flycam section (~line 1880-1950) — add a "Spectator mode:
      `\"player\"` + `\"flycam\"` in one scene" subsection with a RON snippet, a "what still
      works / what doesn't" list (player HUD/nameplates/stats/movement/AI keep working; the
      player gets no camera; the flycam entity's own `model:` still doesn't render; `CameraShake`
      still no-ops), disambiguation from `camera_mode: Flycam(...)` authored directly on a player
      prefab, and the two known-limitations bullets from "Plan review changes" above. Link to
      `flycam_spectator_test.scene.ron`.
- [ ] Playtest aid: `assets/projects/camera_modes/scenes/flycam_spectator_test.scene.ron` (new) —
      one player entity + one flycam entity in the same scene. Register per CLAUDE.md's "Adding a
      new asset project" steps if it needs its own screenshot baseline / index.html entry (likely
      not, since it's an additional scene in an existing registered project — verify).
- [ ] `planning/claude_suggestions.md`: log the two known-limitation edge cases (dynamic
      `Action::Spawn` after flycam suppression; `SetCameraMode` with omitted `owner_player`
      targeting the flycam) as follow-up candidates.

## Open questions
- **Does any project actually want this combo today**, or is this purely a latent capability being
  unlocked? No existing example project combines the two — the playtest aid task above adds a new
  scene, not a fix to an existing broken one.
- **Terrain-deferred flycam+terrain+player**: the flycam camera spawns immediately (doesn't wait
  for terrain), but the player entity is still terrain-deferred as normal — is a brief window
  where the flycam is active but the player hasn't spawned yet acceptable? (Almost certainly yes —
  terrain generation is typically sub-second — but call out during playtest if it reads as broken.)

## Acceptance criteria
- Given a scene with two `tags: ["flycam"]` entities, scene load logs a warning naming both, and
  `ironhold_cli validate` also reports it; only the last one's transform is used (unchanged
  behavior, now diagnosed on both paths).
- Given a scene with both a `tags: ["player"]` entity and a `tags: ["flycam"]` entity, the flycam
  becomes the active camera, a `warn!` names the suppression and its remedy, the player entity
  spawns and its normal systems (movement, stats, targeting) keep working, and the player does NOT
  get its own camera. If the player's `camera.split`/`camera.party` is also set, a second `warn!`
  names that it's fully ignored.
- Given the same combo in a scene with `terrain: Some(...)`, the flycam spawns immediately and the
  player spawns (camera-less) once terrain is ready, with the same end state as the non-terrain
  case.
- Given a flycam-only scene (no players) or a player-only scene (no flycam), behavior is
  byte-identical to today (regression tests), including `is_split_screen`'s value for a 2+-player
  split scene with no flycam present.
