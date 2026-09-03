---
name: camera-config-party-split-nesting
description: party:/split: now live as sibling fields of components.camera (the migration this memory predicted has shipped); flycam is TAG-driven not field-driven; doc-surface list and local_coop_demo migration constraints below still apply
metadata:
  type: project
---

**UPDATED — the predicted fix shipped.** `party:`/`split:` are no longer nested inside
`components.camera` (`CameraConfig`). `PrefabComponents` (`schema/player.rs`) now carries them as
sibling fields — `pub split: Option<SplitScreenDef>` / `pub party: Option<PartyZoomDef>` — resolved
from the new sibling location first, falling back to the legacy nested `camera.split`/`camera.party`
for backward compat (both still only meaningful on the *first* `"player"`-tagged scene entity). See
[[camera-mode-reachability-matrix]] for confirmation that this sibling resolution actually reaches
the split/party/dynamic camera spawn paths (it does, as of the v1 post-implementation fix).

**Flycam is selected by TAG, not by a field.** `tags: ["flycam"]` spawns the fly camera; the
`components.flycam:` (`FlyCamDef`) block is entirely optional tuning. Any camera-refactor
backward-compat rule phrased as "detect the old `camera:`/`flycam:` **fields**" breaks projects that
are tag-only with engine defaults (verified historically true of `custom_materials`/`terrain_demo`);
detection must key on **tags** (`"flycam"` / `"player"`), with the config blocks optional.

**Docs surface to update** whenever this area changes (verify current line numbers before citing —
these drift): `### Special tag: "flycam"`, `### Special tag: "player"`, `#### How a controller gets
assigned to a player`, `CameraConfig` field table, `### Shared party camera`, `### Split-screen
camera`, `#### Per-viewport target ring visibility (own_viewport_only)`, `### Dynamic split-screen`,
`### Grid split-screen`, `### Local co-op hot join` — all in `docs/20_data_formats.md`. Also check
`docs/STATUS.md`, `docs/10_architecture.md`, `docs/00_overview.md` for stale camera-surface summaries.

**Migration-example constraint (local_coop_demo):** rooms have room-exclusive player prefab pairs
(safe to migrate one in isolation) vs. shared pairs (migrating one changes 2 rooms, e.g.
`player_p1`/`_p2` = main+room2 party, `player_p*_grid` = room6+room8, `player_p1_split_ring` =
room9+room10). Check which shape a new example exercises before treating it as proof the sibling
fields work everywhere — `local_coop_demo` room11 (v2) is the confirmed working `camera_mode:
Orbit((...))` + sibling `split:` example on a 2-player scene.

**Targeting recommendation:** per-camera actions in a multi-camera scene take an optional
`owner_player: u32` (the established per-player field name — see
[[player-index-owner-player-wiring]]), **not** a viewport/slot index. Omitted = applies to *all*
active cameras (except a shared Party camera, which can't round-trip to `"default"`).

**How to apply:** when reviewing camera schema changes, check that (a) `party`/`split`'s authoring
location (sibling field, with legacy nested fallback) is stated explicitly, (b) the "first player
entity wins" rule survives the move, (c) the full doc surface above is on the update list, (d)
hot-join (`join_prefab_keys`, see [[hot-join-input-prefab-coupling]]) has a defined camera-mode
answer, and (e) any new co-op-shaped syntax gets at least one shipped example in `local_coop_demo`.
