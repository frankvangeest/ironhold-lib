# Section map: splitting `crates/ironhold_core/src/CLAUDE.md`

_Mapped at: `b24078b` (2026-10-07). Source file: 1,752 lines / 160,039 chars (~40k est. tokens), auto-loaded
whenever any file under `crates/ironhold_core/src/` is touched. This file is the **map only** — nothing has been
moved. The plan that uses it is `planning/features/core_claude_md_split.md`._

**How it was made.** The file was cut into five ~32k-char slices on paragraph boundaries and each slice was
classified block-by-block by a read-only agent against the same scheme (type, scope, destination, safety-critical,
cross-refs, notes). Line ranges were spot-checked against the file (~45 block starts); two or three agent
boundaries are off by 1-2 lines, which does not matter for planning but **must be re-derived with a script at
move time**. Char counts are the agents' (measured with `wc` for slices 3-5, estimated for a few `~` rows) and
overlap slightly; treat totals as +/-10%. Nothing here was verified against the compiler or tests.

**Destination labels** (resolved 2026-10-07: topic content lives in tool-neutral `docs/dev/<topic>.md`; Claude Code gets a thin `.claude/rules/<topic>.md` path-scoped stub and every topic a pointer line in its directory file, because OpenCode has no path-scoped rules; see the plan):
`PARENT` = stays in `crates/ironhold_core/src/CLAUDE.md` · `CAP` = `capabilities/CLAUDE.md` · `SM` =
`runtime/scene_manager/CLAUDE.md` (there is no content that belongs only in `runtime/`; every interpreter,
executor and loader file lives in `runtime/scene_manager/`) · `SCH` = `schema/CLAUDE.md` · `TOPIC:<name>` =
on-demand reference (path-scoped rule or skill) · `DOC:` = a `docs/` page · `CUT` = derivable or history.
**Safety** = `Y` means a must/never rule whose violation breaks determinism, physics, WASM or data: it has to load
for *every* file that could violate it.

## 1. Heading/content mismatches (why a mechanical split by heading is impossible)

| `##` section (lines) | What is actually in it |
|---|---|
| "Entity FSM (per-entity behavior)" 113-705 (52k) | {self}/{new_id} substitution (127-189), **per-player targeting / action bars / gamepad slots / mouse click** (190-351, ~70% of the first slice), target indicator + split-screen ring visibility + target HUD (352-462), corpse/loot/despawn executor notes (464-620), animation pipeline (622-684), dialogue (686-702). Only 111-126 is really entity FSM. |
| "Physics & movement must use FixedUpdate" 731-1022 (24k) | 731-777 is the real FixedUpdate block (+ `max_frame_delta_secs`); **779-1022 (~20.5k) is one topic: the `player.rs` jump / ground-detection saga**. |
| "Audio" 1053-1066 | `### Audio file authoring` (1060) starts with `audio_volume_var_system` (a runtime system note I added on 2026-10-06 under the wrong heading); the real authoring text is 1063-1066. |
| "Despawning: prefer try_despawn()" 1084-1260 + 1261-1645 (48k) | 1084-1120 is the despawn rule; **1121-1200 player-construction sites, 1224-1260 collider/friction, and 1261-1645 (~34k) local co-op / split-screen / gamepad / view-box / viewport-aware labels** sit under this heading. |
| "Dynamic spawning" 1646-1752 (14k) | spawn queue (real), then nameplate/depth-scale/validation material, GLB preloading, particle warmup. |

## 2. Rows

Columns: lines | chars | title | type | dest | safety | notes (cross-refs, risks, stale pointers).
Types: RULE, GOTCHA, RAT = rationale/incident history, REF = reference, DER = derivable.

### Slice 1 — lines 1-351 (32,442 chars)

| lines | chars | title | type | dest | safety | notes |
|---|---|---|---|---|---|---|
| 1-34 | 2,476 | Message→Interpreter→Action→Executor pipeline; sensors never push actions | RULE | PARENT | Y | "single most important architectural rule"; event-name table (23-27) partly derivable, keep |
| 35-53 | 1,277 | Adding new actions + `deny_unknown_fields` consequences | RULE/GOTCHA | SCH | Y | no `serde(flatten)`, rename = hard break, bump `schema_version`; steps 1-3 derivable; step 2 points at the executor (add pointer to SM) |
| 54-69 | 655 | RON struct vs tuple variant syntax | GOTCHA (mostly DER) | SCH (2 lines) or CUT | N | the compiler error reveals it |
| 70-80 | 1,466 | Conditions on rules: only `LogicState`; `EnterState` removed | RAT+RULE | SM | Y-ish | forced state change skips entry/exit actions; line 72 says "see below" but text is inline; do not strand "do not add a general condition system" |
| 81-91 | 836 | No hardcoded assets; sRGB colours only | RULE | PARENT | Y | heading "Before coding" is misleading; line 86 duplicates root CLAUDE.md (~80 chars CUT); heading "Color field convention" is cited by the plan for flex Group |
| 92-107, 109-110 | 2,045 | Composite/nested prefab spawning via `spawn_primitive_children` | REF+RULE | SM | Y | all child spawning goes through it; depth cap 8; `ChildSpawnCtx` field list (~700 chars) derivable |
| 108 | 747 | `scene.ui` must be walked via `walk_ui_nodes` | RULE | SCH (+ pointer from SM) | Y | added 2026-10-05 and bolted onto the composite-prefab list |
| 111-126 | 992 | Entity FSM overview, interpreter chain order, never bypass pipeline | REF+RULE | SM | Y (line 125, 243 chars: no `Commands` access) | chain list derivable from the plugin schedule |
| 127-143 | 1,519 | Supported `{self}` targets (15 examples) | DER | CUT | N | derivable from `action_substitution.rs` (`rewrite_self`); **hook `action_docs_reminder.py:15` tells Claude to add new variants to this list**; line 188 depends on it |
| 144-187 | 3,732 | `{new_id}` semantics, collisions, resolved at executor | GOTCHA+RAT | SM (trim ~40%) | Y-ish | 170-175 (~500 chars) is `dialogue.rs` → CAP stub |
| 188-189 | 486 | `{target}` substitution via `CurrentTarget` | REF/GOTCHA | SM (merge with `{new_id}`) | N | "same as `{self}` above" must be rewritten |
| 190-205 | 1,393 | Per-player targeting Phase 1: `PlayerTarget` vs `CurrentTarget` | REF/RAT | TOPIC:action-bar-input-routing | N | keep a one-line "primary player = `PlayerIndex(0)` or none" definition in CAP (used across 190-351); cites "Player-construction sites" (later in file) |
| 206-246 | 3,828 | Per-player action bars Phase 2: `owner_player`, `owns_slot`, cost pools, duplicate-key detectors | REF+RAT | TOPIC:action-bar-input-routing | N | keep a one-line gotcha in CAP: the state-machine rule override resolves against the primary player |
| 247-255 | 738 | `action_bar_input_system` requires `PlayerTarget` on every `CharacterController` | GOTCHA | CAP | Y-ish | silent no-op drops a whole bar; "four-site inventory above" is dangling (and stale: now five sites) |
| 256-283 | 2,483 | Gamepad-routed action-bar slots (`gamepad_key`, `BoundGamepad`) | REF/RAT | TOPIC:action-bar-input-routing | N | key fact: `gamepad_key` fires only from the owning player's `BoundGamepad` |
| 284 | 2,342 | Mouse-click action-bar slots (`Option<Ref<Interaction>>`, `InspectorEnabled`) | GOTCHA+RAT | TOPIC + 2 rules in CAP | Y-ish | "do not replace with a `ClickedSlots` resource" is load-bearing; single 2.3k paragraph; test recipe could go to tests/CLAUDE.md |
| 286-314 | 2,433 | Targeting chain ordering `.before(action_bar_input_system)`; transitive edge to the interpreter | RULE+RAT | CAP + pointer in SM | Y | **not covered by a test — this text is the only guard**; consider a PARENT bullet "any system touching `PlayerTarget`/`CurrentTarget` must be ordered explicitly" |
| 315-322 | 622 | `Targetable` test fixtures must register in `SpawnRegistry` | GOTCHA | tests/CLAUDE.md (already there: `tests/CLAUDE.md:11` has the same rule → CUT here after verifying) | N | duplicate |
| 323-351 | 2,372 | `target.*` events, screen-space selection, `select_aim_height`, `target_*` vars blank at 2+ players | REF+GOTCHA | CAP (trim ~800 derivable) | N | keep: screen-space not raycast, vars blank at 2+ players, `target.changed*` primary-only |

### Slice 2 — lines 352-728 (32,263 chars)

| lines | chars | title | type | dest | safety | notes |
|---|---|---|---|---|---|---|
| 352-382 | 2,421 | Target indicator ring, per-player, colour precedence | REF+RAT | TOPIC:split-screen-cameras-and-widgets (keep the "no dedup, deliberate" decision, 1-2 lines, in CAP) | N | rest derivable |
| 383-395 | 1,191 | Per-viewport ring visibility: RenderLayers intro | REF | TOPIC:split-screen-ring-visibility | N | "sole owners of this arithmetic" → one-line CAP rule; layer 31 reservation touches `inspector.rs` |
| 396-405 | 811 | Orbit camera / ring layer assignment | REF | same topic | N | "keyed on `player_index`, not loop order" is a GOTCHA |
| 406-413 | 717 | Party camera must carry `all_ring_layers()` | GOTCHA | CAP | N (renders zero rings) | invariant found in plan review |
| 414-423 | 877 | `TargetRingVisibilityMode` lifecycle | REF+GOTCHA | same topic | N | "no mid-scene toggle without re-tagging" → one line in SM |
| 424-431 | 751 | Warnings: `player_index` collision, non-hot-join Spawn | REF | same topic | N | derivable from the `warn!` sites |
| 433-437 | 413 | `pipeline_warmup_system` doesn't touch RenderLayers | GOTCHA | CAP | N | touches `lib.rs` (src root) |
| 439-444 | 589 | Light visibility intersects light RenderLayers (shadowless lit mesh) | GOTCHA | CAP (or PARENT if RenderLayers used outside capabilities) | N | |
| 446-450 | 370 | "Reader-facing entry points" for the ring feature | DER | CUT | N | grep gives the same |
| 452-462 | 1,035 | Per-viewport target HUD readout | REF | CAP (keep the `.after(split_screen_viewport_system)` ordering line) | N | rest derivable |
| 464-466 | 326 | `behavior` works on composite primitive prefabs | GOTCHA | SM | N | drop the empty bold lead at 464 ("New capabilities for entity logic:" groups 464-620 under a misleading label) |
| 468-471 | 397 | `TriggerZone` description | DER | CUT | N | covered by docs/20 |
| 472-476 | 1,229 | `Interactable` fires for every in-range interactable | GOTCHA | CAP | N | 587-596 depends on it |
| 478-488 | ~1,150 | Lootable corpse design summary | RAT+REF | TOPIC:lootable-corpse (also in docs/30) | N | mostly history |
| 489-505 | ~1,315 | `Action::Spawn.at_entity` copies full transform; skip with warning, never origin | GOTCHA/RULE | SM | Y-ish | same-frame Despawn race; mirrors `SpawnEffect.entity` |
| 507-522 | 1,508 | Corpse ids unique via `{new_id}` (retrofit history) | RAT | CUT (one current-state line into TOPIC:lootable-corpse) | N | |
| 524-543 | 1,914 | Prefer `SetDespawnTimer` over `EmitEventAfterDelay`+`Despawn` | RULE+RAT | CAP (542-543 rule + one-line why) | N | cut the incident narrative |
| 545-552 | 685 | `target_auto_clear_system` clears on despawn | GOTCHA | CAP | N | content is targeting, heading is the corpse group |
| 554-561 | 651 | `Action::Despawn` closes an open container panel | GOTCHA | SM | N | linked to 587-596 |
| 563-573 | 1,066 | A dying entity can't catch its own respawn timer; global rule needed | GOTCHA | TOPIC:lootable-corpse | N | cites the "Despawning" notes |
| 575-585 | 1,029 | Respawn rules must be in `global_on`, not state-scoped `on:` | RULE+RAT | TOPIC:lootable-corpse (+ one runtime/ line: `tick_delayed_events_system` is pause-ungated) | Y (permanent lost respawn) | continues 563-573; keep contiguous |
| 587-596 | 949 | `OpenContainer` must not double-count `panels_open` | GOTCHA | SM | N | depends on 472-476 |
| 598-606 | 805 | No `trigger_zone` on a Dynamic rigid-body prefab | GOTCHA/RULE | SM | Y (physics mass) | add a pointer from the CAP trigger code; a backlog item is cited |
| 608-620 | 1,150 | Superseded same-entity corpse design (cautionary) | RAT | CUT | N | the surviving reason lives in the 524-543 rule |
| 622-637 | 1,012 | Animation resolver/playback pipeline, field-ownership table | REF+GOTCHA | CAP | N | compact table |
| 638-646 | 711 | `pending_seek` purpose | GOTCHA | CAP | N | |
| 647-657 | 851 | Paused clip must be resumed before next play | GOTCHA | CAP | N | load-bearing |
| 658-669 | 889 | `set_seek_time` not `seek_to`; duration via the live graph | RULE+RAT | CAP | N | cites a `tag_spawned_entity` doc comment |
| 670-675 | 542 | `ActiveOverride.seek_fraction`/`frozen` must be durable | GOTCHA | CAP | Y (WASM-only regression) | |
| 677-684 | 644 | Spawn-already-posed needs a minimal `AnimationPolicy` | GOTCHA | CAP or TOPIC:lootable-corpse | N | |
| 686-702 | 1,390 | Dialogue system | REF (mostly DER) | CAP (trim to: ordering `.after(button_system).after(interactable_system).before(fsm_interpreter_system)`, `{self}` substitution, `portrait` not implemented) | N | |
| 706-709 | 538 | WebGPU 16-byte alignment for GPU structs | RULE | PARENT | Y (web panic) | heading likely cited by name |
| 711-720 | ~1,050 | WGSL authoring rules for designer-facing shaders | RULE+GOTCHA | CAP (+ "always test in a web build" one-liner in PARENT) | Y (WebGPU validation) | check `runtime/` material creation before finalising |
| 722-728 | ~1,260 | Engine-internal shaders embedded via `include_str!`/`uuid_handle!`; no fabricated asset paths | RULE | PARENT | Y (breaks projects without `assets/shared/`) | spans runtime, schema, capabilities |

### Slice 3 — lines 729-1062 (32,289 chars)

| lines | chars | title | type | dest | safety | notes |
|---|---|---|---|---|---|---|
| 729-730 | 70 | Pointer to `docs/25_custom_shaders.md` | REF | PARENT (stays with the shader section) | N | |
| 731-733 | 200 | Physics/movement/camera-follow must run in `FixedUpdate` | RULE | PARENT | Y | **wording is stale: the camera chain runs in `Update`** (`claude_suggestions.md` ~L531, `opencode_compatibility.md` F2) — fix while moving |
| 734-749 | 1,449 | Rapier steps in `FixedUpdate` at 64Hz; multi-tick frames; `mark_dirty_trees` | RAT+GOTCHA | PARENT (condensed) | Y | "two separate mitigations" intro leads into 750-764 — move 734-764 together |
| 750-758 | 816 | Update systems must not read `&GlobalTransform`; use `utils::fresh_global_transform` | RULE | PARENT | Y | cross-cutting; phrase on the helper name so grep finds it |
| 759-764 | 572 | "Confirmed fine, deliberately unconverted" list | RAT | CAP | N | splits an argument from its rule; keep a pointer |
| 765-768 | 219 | Faster-than-64Hz displays step motion (accepted) | RAT | CAP (or fold into the PARENT paragraph) | N | |
| 769-777 | 803 | `ProjectConfig.max_frame_delta_secs` caps catch-up ticks | REF | SCH (one-line pointer) or CUT | N | duplicates `docs/20#max_frame_delta_secs` |
| 779-814 | 3,022 | Jump reset can't rely on a ground-check edge; slope walkability gate | RAT/INCIDENT | TOPIC:player-ground-detection | N | heading is under the FixedUpdate section; tests named at 779/841 |
| 815-855 | 3,363 | `jumps_used` reset: grace ticks, velocity, liftoff height; dual-clock history | REF+RAT | TOPIC:player-ground-detection | N | `jump_liftoff_y` teleport caveat (850-854) is a real GOTCHA; stale "used to" prose trimmable |
| 856-882 | 2,323 | Reset must read `raw_grounded`, never coyote-buffered `is_grounded` | RULE+RAT | CAP (rule + 2-line incident; full story in TOPIC) | Y | "a debounce that smooths feel must never leak into correctness timing" |
| 883-911 | 2,388 | Coyote time: debounced grounding | REF | TOPIC:player-ground-detection | N | **`animation_resolver.rs:30` cites the "Coyote time" section** |
| 912-928 | 1,484 | `can_jump`: coyote buffer unlocks only the first jump | RULE+RAT | CAP (condensed) | Y | keep adjacent to 856-882 |
| 929-949 | 1,868 | Ground shape-cast must `.exclude_sensors()` and `normalize_or_zero()` | GOTCHA+RAT | CAP (condensed) | Y | generic for any physics query in capabilities/ (`npc.rs` LOS precedent) |
| 950-1004 | 4,635 | `ground_cast` re-query loop, "underfoot", `is_walkable_contact`, known gap | REF+RAT | TOPIC:player-ground-detection | N | monotone-loop rationale (980-993) is not derivable; link the backlog bug on compound colliders |
| 1005-1022 | 1,420 | `jump_air_grace_ticks` NaN clamp; accepted pogo consequences | REF+GOTCHA | TOPIC:player-ground-detection | N | accepted-tradeoff list is not derivable |
| 1023-1026 | 3,780 | Deterministic iteration order: BTreeMap/IndexMap; `determinism_lint` markers | RULE | PARENT | Y | **`tests/determinism_lint.rs:8` and `:201` (a failure message) quote the heading "Deterministic iteration order on gameplay paths"; `docs/40:102` too — keep the heading text**; D1 site inventory could be a list |
| 1027-1029 | 175 | Terrain generation is async | RULE | CAP | Y (WASM main thread) | |
| 1030-1032 | 359 | Inspector isolation (`cfg_attr(feature = "inspector")`) | RULE | PARENT | Y (inspector must not ship) | applies to components everywhere |
| 1033-1040 | 957 | Native frame cap, unfocused throttle, pipeline warmup | REF+GOTCHA | PARENT (lib.rs note) | N | 300-2000 ms WASM stall rationale not derivable |
| 1041-1052 | 773 | Change-detection discipline for render-affecting components | RULE | PARENT | Y (WASM frame time) | **`camera.rs:878` and `nameplate.rs:49` cite "Change-detection discipline"** |
| 1053-1059 | 728 | Audio preloading: `preload_audio_system`, `LoadedAudioHandles` | REF | SM | N | handles must stay alive |
| 1060-1062 | 885 | `audio_volume_var_system` mirrors `AudioState` to a `GameVariable` | REF+GOTCHA | SM | N | **sits under "Audio file authoring"** — a runtime system note; test-harness gotcha (non-default fraction) is real |

### Slice 4 — lines 1063-1462 (34,117 chars)

| lines | chars | title | type | dest | safety | notes |
|---|---|---|---|---|---|---|
| 1063-1066 | 605 | SFX: no leading silence, WAV only | GOTCHA | DOC:asset-authoring (or `docs/60`) | N | |
| 1067-1083 | 1,214 | `tag_spawned_entity` is the single spawn-metadata source | RULE | SM | Y (data) | "four-site inventory below" is stale: five sites |
| 1084-1110 | 2,330 | Prefer `try_despawn()` when an entity may already be gone | RULE+RAT | PARENT | N | cross-cuts capabilities and scene_manager; incident trimmable to ~600 chars |
| 1111-1120 | 859 | Child `LevelEntity`/`OverlayEntity` self-tagging is deliberate | RULE (don't "clean up") | PARENT | Y (cross-scene leaks) | continues the try_despawn argument |
| 1121-1200 | 6,479 | Player-construction sites inventory (1-5) + post-dispatch rule | REF (+RULE) | TOPIC:player-spawn-sites; 2-3 line RULE in SM | N | stale "line 560"; heading sits under "Despawning"; widely cited as "Player-construction sites" / "four player-construction sites" |
| 1201-1212 | 962 | Player `stat_label`/`world_stat_bar` routing; 16-param ceiling | GOTCHA | SM | N | `SystemParam` 16-param limit on `spawn_scene_v2` is the reusable gotcha |
| 1213-1223 | 953 | `PrefabDef.material` not auto-applied on the player path | GOTCHA | SM | N | restate "new rendering `PrefabDef` field must be checked against the player path" |
| 1224-1260 | 3,163 | Collider divergence; Friction 0.15 idle-only after the wall-friction fix | GOTCHA+RAT | CAP (collapse to current state) | Y (physics) | `wall_friction_tests.rs` |
| 1261-1330 | 6,139 | Local co-op: Party, split, Grid, dynamic split, fallback | REF | TOPIC:split-screen-cameras-and-widgets | N (lib.rs ordering part Y) | "neither set: warn" (1326-1329) short enough for CAP |
| 1331-1358 | 2,389 | Split prefabs need `zoom_speed:0`, `orbit_button:"None"`, `character_rotate_button:None` | GOTCHA | CAP | N | three-field list is load-bearing; playtest narrative trimmable |
| 1359-1373 | 1,358 | Keyboard camera look | REF+GOTCHA | same TOPIC | N | pitch-direction pin → one CAP line |
| 1374-1382 | 724 | `CameraShake` query must exclude Fixed/FirstPerson/Flycam | GOTCHA | CAP | N | straddles `camera.rs` and `scene_manager/mod.rs` |
| 1383-1405 | 1,959 | `SetCameraMode`; Authored vs Active camera mode; `camera_blend_system` must be last in the chain | GOTCHA | CAP | Y (`lib.rs` ordering) | ordering lives in `lib.rs` → needs a PARENT pointer (see risk 3) |
| 1406-1462 | 4,983 | `WorldLabelRank` viewport-aware labels; per-rank widget duplication | REF+GOTCHA | same TOPIC | N | continues into 1463-1528 (one story) |

### Slice 5 — lines 1463-1752 (29,918 chars)

| lines | chars | title | type | dest | safety | notes |
|---|---|---|---|---|---|---|
| 1463-1474 | 1,080 | `particle_renderer` billboard viewport-aware (Phase 1) | RAT+GOTCHA | CAP (use `camera_priority_key`, filter `is_active`; accepted limit of one billboard camera) | N | rows 1463-1486 + 1488-1509 are one story with 1406-1462 |
| 1476-1486 | 1,035 | Targeting `click_select` viewport-aware (Phase 2) | RAT+GOTCHA | CAP (dead-quadrant click does nothing) | N | |
| 1488-1509 | 2,067 | Nameplate distance culling via stored camera distance (Phase 3) | GOTCHA+RAT | CAP | N | rule: read the stored distance, never re-select a camera; refers to `WorldLabelRank` (Phase 4, earlier) |
| 1511-1528 | 1,705 | Split-screen player HUD labels (P{n}) | REF | CAP (keep the ordering rule + "relies on full-window Camera2d" warning) | N (ordering is a GOTCHA) | "see above" for `PlayerIndex` will dangle |
| 1530-1574 | 4,180 | Gamepad routing: `BoundGamepad`, `claimed` invariant | GOTCHA+RAT | TOPIC:gamepad-routing + ~800-char rule stub | Y (stub: never bind to an already-claimed entity; ascending-`PlayerIndex` order; consumers take `Option<&BoundGamepad>`) | spans `capabilities/player.rs`, `runtime/input.rs`, the consumers — cross-cutting |
| 1576-1587 | 1,092 | Gamepad button/axis RON mapping, parity gap | REF (partly DER) | TOPIC:gamepad-routing | N | keep "no gamepad camera-yaw is deliberate" |
| 1589-1604 | 1,404 | Duplicate `gamepad_index`/`player_index` validation scope | RAT+GOTCHA | SCH or TOPIC (+ 2-line CLI note) | N | stops someone "fixing" the `entities:`-only scope |
| 1606-1636 | 2,884 | Gamepad-triggered hot join: `PendingJoinGamepad` | REF+GOTCHA | TOPIC:gamepad-routing | Y partial (reset to `None` each run; one pad per frame; `.take()` in the executor) | needs "Player-construction sites" reachable; "troubleshooting note above" is unlocated/dangling |
| 1638-1644 | 629 | View-box clamp | REF+GOTCHA | CAP | N | "zero `linvel` on the clamped axis" prevents jitter |
| 1646-1655 | 940 | Spawn queue: `SPAWNS_PER_FRAME = 2` | RAT+RULE | SM (or PARENT) | Y (WASM) | Spawn must go through the queue; clear on `LoadScene`; heading "Dynamic spawning" may be cited |
| 1657-1667 | 3,978 | Component parity: dynamic vs scene-placed (nameplate gating) | REF+RULE | SM (nameplate details to CAP) | RULE: "do not re-inline the predicate at a new call site" | callers span `scene_loader.rs`, `entity_spawner.rs`, `action_executor.rs` → place where all load |
| 1668-1693 | 3,868 | Label depth scale + validation coverage | REF+RAT | SM (single-resolver rule) + SCH (`radius_range`); CLI walkthrough ~2.5k derivable | N | starts as a bullet of the previous list |
| 1695-1711 | 1,576 | CLI vs runtime camera-set asymmetry for label depth scale | RAT | CUT (one sentence in SM; already in `claude_suggestions.md`) | N | someone may "fix" the asymmetry |
| 1713-1729 | 1,452 | GLB preloading `PreloadPrefab`/`PreloadGlb` | REF | DOC (WASM load-stall page) + 3-line GOTCHA in SM | Y (WASM; handles must stay alive; cleared on `LoadScene`) | RON snippet derivable from the example project |
| 1731-1752 | 2,028 | Particle pipeline warmup + `ParticleBudget` footgun | GOTCHA+REF | CAP (budget footgun) / same DOC page | N | check `docs/20` before cutting (designers need it) |

## 3. Totals by destination (agents' numbers, +/-10%; before condensing)

| Destination | ~chars | Notes |
|---|---|---|
| PARENT | 16,700 | pipeline, no-hardcoded-assets/sRGB, determinism (3.8k), FixedUpdate + `GlobalTransform` rules, WebGPU alignment + engine-shader embedding, inspector isolation, change-detection, try_despawn + self-tagging, frame-pacing note |
| `schema/` | 3,500 | adding actions + `deny_unknown_fields`, `walk_ui_nodes`, `radius_range`, validation-scope notes |
| `runtime/scene_manager/` | ~25,000 -> ~18k condensed | conditions, composite spawning, FSM chain rule, `{new_id}`/`{target}`, `at_entity`, container/trigger_zone gotchas, tag_spawned_entity, component parity, spawn queue, label depth resolver, audio |
| `capabilities/` | ~45,000 raw -> ~25k condensed | action-bar/targeting gotchas, targeting ordering, ring/HUD/camera rules, animation pipeline, ground-detection **hard rules (~5.7k)**, friction/collider, view-box, WGSL, terrain, dialogue stub — **still large: see plan risk R4** |
| TOPIC references (on-demand) | ~70,000 | ground detection 14.8k · split-screen/camera/widgets ~25k · action-bar input ~10k · gamepad routing ~9.5k · player-spawn-sites 6.5k · lootable corpse ~4k |
| DOC pages | ~3,500 | WASM first-use stalls (GLB preload + particle warmup), asset authoring |
| CUT | ~11-12k (+ 30-40% condensation of moved incident prose) | `{self}` targets list, `ChildSpawnCtx` fields, `TriggerZone` description, retrofit/superseded corpse history, CLI-vs-runtime asymmetry walkthrough, derivable validation walkthrough, duplicates of root CLAUDE.md and `tests/CLAUDE.md` |

**Estimated cost of an edit afterwards** (parent ~16.7k + directory file, chars/4): `schema/` ~5k tokens ·
`runtime/scene_manager/` ~9k · `capabilities/` ~10k (+3-6k when a path-scoped topic rule also matches), versus ~40k
today for every one of them.

## 4. Citation inventory (repo files pointing at this file)

167 citation lines in 106 files: 112 planning (mostly `planning/features/done/`; historical), 37 agent memory
(historical), 8 CLAUDE.md/commands/agents/hooks, 7 code comments, 2 docs, 1 other. **Live ones that must be
handled in the same commit as the move:**

| File | Cites | Needed |
|---|---|---|
| `crates/ironhold_core/tests/determinism_lint.rs:8,:201,:43` | heading "Deterministic iteration order on gameplay paths" (`:201` is printed in a failing test's message) | keep the heading text in PARENT |
| `docs/40_determinism_and_networking.md:102` | same heading | keep |
| `capabilities/camera.rs:878`, `capabilities/nameplate.rs:49` | "Change-detection discipline" | keep heading in PARENT |
| `capabilities/animation_resolver.rs:30` | "Coyote time" section | update the comment to the new location |
| `capabilities/targeting.rs:101` | "player-construction" section | update |
| `crates/ironhold_cli/src/commands/validate.rs:1906`, `:2336` | "hot join"; a `{self}`-substitution note | update |
| `docs/20_data_formats.md:3292` | "underlying" controller-icon notes | check |
| `.claude/hooks/action_docs_reminder.py:15` | "add it to the {self} targets list in `crates/ironhold_core/src/CLAUDE.md`" | **rewrite — that list is being cut** |
| `.claude/hooks/capability_registration_reminder.py:18` | "`CLAUDE.md` capability notes" | point at `capabilities/CLAUDE.md` |
| `AGENTS.md:7`, `.claude/commands/ship-feature.md:32`, `crates/ironhold_core/tests/CLAUDE.md:11`, root `CLAUDE.md` (2 lines) | the file by path | re-check wording |
| `planning/claude_suggestions.md` ~L531 | FixedUpdate camera wording | resolved by the split (fix the rule text) |

Historical citations (done plans, agent memory, `opencode_compatibility.md`) are left as records; the parent gets a
**moved-sections index** (old heading -> new location) so they stay resolvable.

## 5. Stale or dangling items found while mapping (fix at move time)
- "four-site inventory" (1067, 1121, 247-255) — it is **five** sites; "set at line 560" (1180) is stale
- "see above/below" pointers that cross the new boundaries: "Player-construction sites" (cited from 190-205 and 247-255, defined at 1121), "Gamepad-triggered hot join below" (256-283 -> 1606), "the split-screen section below" (323-351), "Phase 1-4"/`WorldLabelRank`, `PlayerIndex` "(see above)" (1511-1528), "Monotonic per-entity id generation", "the troubleshooting note above" (1606-1636; not located), "the OpenContainer double-count bug below" (554-561 -> 587-596)
- `FixedUpdate` camera-follow wording (731-733) contradicts `lib.rs` (camera chain runs in `Update`)
- the `audio_volume_var_system` note under "Audio file authoring" (1060)
- `opencode_compatibility.md` says the file is "~154 KB"
