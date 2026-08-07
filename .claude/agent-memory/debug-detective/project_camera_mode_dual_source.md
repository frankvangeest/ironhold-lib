---
name: camera-mode-dual-source
description: Multi-player camera dispatch reads PlayerConfig.camera (legacy) not camera_mode, so authoring camera_mode on a split/party player silently resets tuning to engine defaults
metadata:
  type: project
---

`spawn_players_and_camera`'s 2+-player branches (split, party, dynamic-split, and the
no-party/no-split fallback) build their cameras from `PlayerConfig.camera` / `first.camera`
— the *legacy* `components.camera:` block — never from `camera_mode`. Only the
single-player branch (`entities.len() < 2`) and `spawn_player_entity` go through
`spawn_active_camera_for_player`/`resolve_camera_mode`.

Because `assemble_player_config` fills `camera` with `default_camera_config()` when
`components.camera:` is absent, a designer who migrates a co-op player prefab from
`camera:` to `camera_mode: Orbit((...))` does not merely "lose the mode" — the whole
tuning block silently reverts to engine defaults (`orbit_button: "Either"`,
`character_rotate_button: Some("Right")`, `zoom_speed: 10.0`, offset `(0,5,10)`).

**Why:** the enum/marker refactor unified the *runtime* component but left the
authored-side dispatch split across two independent fields that are both always
already-defaulted, so there is no "was it authored?" signal to warn on.

**How to apply:** whenever reviewing camera/local-coop changes, check whether a code path
reads `player_config.camera` or `resolve_camera_mode(player_config)` — the two diverge for
every 2+-player scene. Related: [[project_renderlayers_reserved_scheme]].
