---
name: camera-config-party-split-nesting
description: party:/split: are nested INSIDE components.camera; flycam is TAG-driven not field-driven; the real docs/20 camera surface spans ~1814-2557 (not the 2023-2350 camera_modes.md still claims)
metadata:
  type: project
---

`components.camera` (`CameraConfig`, ~15 flat fields) is the biggest single RON component block a
designer hand-writes, and the two local-co-op switches `party:` (`PartyZoomDef`) and `split:`
(`SplitScreenDef`, itself nesting `dynamic:` = `DynamicSplitDef` and the flat `own_viewport_only`)
live **inside** it, read from the **first** `"player"`-tagged scene entity only.

**Flycam is selected by TAG, not by a field.** `tags: ["flycam"]` spawns the fly camera; the
`components.flycam:` (`FlyCamDef`) block is entirely optional tuning. Verified 2026-08-07: of the
3 shipped flycam projects, only `foliage_demo` authors a `flycam:` block — `custom_materials` and
`terrain_demo` are tag-only with engine defaults. Any camera-refactor backward-compat rule phrased
as "detect the old `camera:`/`flycam:` **fields**" therefore breaks 2 of 3 projects; detection must
key on the **tags** (`"flycam"` / `"player"`), with the config blocks optional.

**Why this matters for any camera refactor:** `planning/features/camera_modes.md` proposes replacing
`camera: (...)` with `camera_mode: Orbit(...)`. Blocker 4 (resolved 2026-08-01) moves `split:`
(carrying `own_viewport_only` inside it) to a **sibling** field of `camera_mode:` under
`components:`; `party:` becomes a `Party` *variant*. The pre-existing party/split mutual-exclusivity
(both set → warn, `split` wins) is explicitly preserved; `split.dynamic`'s internally-managed merged
camera uses the `Party` variant under the hood, which is not a designer-authored contradiction.

**Docs surface to update (verified 2026-08-07 — the plan's "~lines 2023-2350" is stale):**
`### Special tag: "flycam"` 1814 · `### Special tag: "player"` 1886 · `#### How a controller gets
assigned to a player` 1927 · `CameraConfig` field table 2067 · `### Shared party camera` 2125 ·
`### Split-screen camera` 2192 · `#### Per-viewport target ring visibility (own_viewport_only)` 2282
· `### Dynamic split-screen` 2364 · `### Grid split-screen` 2451 · `### Local co-op hot join` 2558.
Also stale-on-ship outside docs/20: `docs/STATUS.md` lines ~52/54 ("Data-configured via
`player.camera`", "spawned via `"flycam"` tag"), `docs/10_architecture.md:13`, `docs/00_overview.md:52`.

**Migration-example constraint (local_coop_demo, verified 2026-08-07):** 10 rooms, 16 `camera:`
blocks. Room-exclusive player prefab pairs (safe to migrate one in isolation): room3
(`player_p1_split`/`_p2_split`), **room4** (`player_p1_split_h`/`_p2_split_h` — simplest, best
candidate), room5 (`_dynamic`), room7 (`_primitive`, but that's a do-not-retrofit regression
baseline). Shared pairs (migrating one changes 2 rooms): `player_p1`/`_p2` = main+room2 (party),
`player_p*_grid` = room6+room8, `player_p1_split_ring` = room9+room10.
`3rd_person_game_demo` has exactly 3 plain `camera:` blocks (`player_warrior`, `player_male`,
`player_female`), no co-op — but 2 of them spawn via `Action::Spawn` character-select, so migrating
it also exercises the runtime-spawn camera path, not just the scene-load one.

**Agreed targeting recommendation (plan-review 2026-08-01):** per-camera actions in a multi-camera
scene should take an optional `owner_player: u32` (the established per-player field name — see
[[player-index-owner-player-wiring]]), **not** a viewport/slot index. Omitted = applies to *all*
active cameras; `owner_player` on a party-mode scene or an unjoined hot-join slot should warn, not
silently no-op (see [[warn-vs-silent-fallback-principle]]).

**How to apply:** when reviewing camera schema changes, check that (a) `party`/`split`'s authoring
location is stated explicitly, (b) the "first player entity wins" rule survives the move, (c) the
full doc surface above is on the update list (not just the `CameraConfig` table), (d) hot-join
(`join_prefab_keys`, see [[hot-join-input-prefab-coupling]]) has a defined camera-mode answer, and
(e) any new co-op-shaped syntax gets at least one shipped example in `local_coop_demo` — otherwise
the new form exists only in docs (see [[local-coop-demo-room-conventions]],
[[primitive-player-fields]]).
