# Stakeholder Priority List

**Snapshot as of `929b7d1` (2026-10-12).** Third edition. This is a point-in-time snapshot, not a
living/re-generated document — priorities will drift as the codebase changes, so treat this as a
read of that moment, not an ongoing source of truth. Re-run manually later if a fresh read is
wanted. Earlier editions are preserved in git history: the first (`db1ede0`, 2026-09-03) via
`git show 8a8176c:planning/stakeholder_priority_list.md`, the second (`ce77bdd`, 2026-10-01) via
`git show 14bfd49:planning/stakeholder_priority_list.md`.

## What this is

Five of the project's specialized review-agent personas were each asked, independently and in
parallel, for (A) the status of their previous top-5 and (B) a fresh, genuine top-5 wishlist from
their own lens, ranked most→least important. Each was grounded in:

- their own `.claude/agent-memory/{agent}/` directory
- `planning/backlog.md` and `planning/claude_suggestions.md`
- spot-checks of the code and docs, so every status claim below was verified rather than assumed
- their own judgment for anything real but not yet logged anywhere ("self-described")

Debug-detective was intentionally excluded again — its memory is mostly a record of already-fixed
bugs, not a forward-looking wishlist.

Every item points to its source (a `backlog.md` item, a `claude_suggestions.md` entry, an
agent-memory file, or "self-described — not yet logged") so it can be chased down and, where
warranted, promoted or acted on independently of this snapshot. Line numbers are as of the
snapshot commit.

## What changed since the second edition

**Almost nothing on the previous lists shipped.** Of the 25 previous items, none moved to Shipped;
the WASM-perf, game-world and UX lists had zero movement. Only 26 of 204 non-merge commits since
`ce77bdd` touched `crates/ironhold_core/src` — the period went to the docs/CLAUDE.md split,
OpenCode tooling, planning, and a burst of UI work: the flex `Group` (`1167eb9`), `SetUiVisible`
(`d50045d`), the responsive web canvas (`710e79a`, review fixes + `play.html` startup message
`b5c1800`) and project `ui_scale` (`f2194a3`), `ui_demo` (`b1d685d`), and a health/respawn loop on
the room-3 click targets (`4288024`). Determinism groundwork also landed (D1 `9e167ca`, D2
`7637605`).

> **Verified (929b7d1/be39d84):** ⚠️ partly — 204 non-merge commits / 26 touching `crates/ironhold_core/src` in `ce77bdd..929b7d1` ✅; none of the 25 previous items is in backlog ▸ Done ▸ October 2026 ✅. Hash fixes: the canvas fit is `710e79a` (`b5c1800` is the phase-1 review fix + `play.html` "cannot start" message, `f2194a3` is `ui_scale`); `ui_demo` was added in `b1d685d`, not `1167eb9`.

The ocean demo batch has still not started (no ocean/buoyancy code exists), so the "settle
prerequisites first" window from the last edition is still open. Two new areas surfaced: the
**responsive-canvas pixel budget** (WASM-Perf #1) and **UI visibility gaps** (overlays can't start
hidden, ActionBar can't be hidden; Alignment #5).

---

## System-Architect — stability, maintainability, future-proofing

*Lens: what would most hurt this codebase's trajectory if ignored for another 6 months.*

**Previous top-5, status:**

| # | Item | Status |
|---|---|---|
| 1 | Determinism harness before the ocean/buoyancy batch | **Still open, still on time.** Harness still Queued (`backlog.md:148`); no ocean/buoyancy code exists. Groundwork shipped: D1 deterministic gameplay maps (`9e167ca`, merge `00c1ab5`), D2 unmarked-`HashMap` source-scan guard (`7637605`, merge `e22ad3a`). D5 (in-process two-`App` smoke test) now queued as a cheaper first slice (`:151`) |
| 2 | Declared schedule-ordering contract | **Changed (planned, not built).** Became D3 (`:149`, `features/gameplay_pipeline_system_sets.md`, Ready since 2026-10-02). Still zero `SystemSet`s and no ambiguity detection in `src/` |
| 3 | UI pointer capture vs. world input | **Still open.** Moved from Bugs to Queued (`:24`); no `UiPointerCaptured` or equivalent in code |
| 4 | Loader failures visible, not infinite loading | **Still open in the engine.** `play.html` now explains module-load failure / early panic / 60 s with no frame (`b5c1800`), but `check_project_loaded` (`project_loader.rs:33-46`) still returns early on a `ProjectConfig` parse failure with no `Failed` arm, and the loading screen keeps drawing frames so the page check never fires. Stale `rules_path:` bug (`:41`) and scene-fetch retry (`:64`) still open |
| 5 | scene_manager monolith / 16-param ceiling | **Still open, and growing.** `spawn_scene_v2` (`scene_loader.rs:41-58`) still exactly 16 params; `scene_loader.rs` 3,570 → 3,686 lines, folder 9,287. Panel-chrome extraction still Queued (`:275`) |

> **Verified (929b7d1/be39d84):** ✅ confirmed — backlog `:148/:149/:151/:24/:41/:64/:275` all match; zero `SystemSet`s and no ambiguity detection in `src/` (grep: only comments); no `UiPointerCaptured`; `project_loader.rs:46` returns early when the config isn't loaded, `:50-52` only logs `validate()` errors, Failed arms `:153-217` cover only sub-assets; `spawn_scene_v2` params `scene_loader.rs:42-57` = 16; 3,570 (`14bfd49`) → 3,686 lines, folder 9,287 lines.

**New top-5:**

### 1. Build D3 (named pipeline `SystemSet`s) now that its plan is Ready
**Source:** `planning/backlog.md` ▸ Beta 0.5 ▸ D3; `planning/features/gameplay_pipeline_system_sets.md`; `.claude/agent-memory/system-architect/schedule_ordering_mechanism.md`

The one item where design is done and only building is left. It has been Ready for 11 days while
the `Update` writer set kept growing (the `SetUiVisible` executor arm, the `ui_scale` system).
Event writers and direct `ActionQueue` pushers (`dialogue_tick_system`, `despawn_timer_system`,
`resolve_pending_behaviors_system`) are ordered only against the interpreter, not each other, so
native (multi-threaded) and WASM (single-threaded) can still disagree on same-frame order. D4, D5,
the schedule-guard follow-ups (`:152`) and "gameplay pipeline onto the fixed tick" (`:153`) all
assume D3's sets exist; every feature merged before D3 adds edges that must be moved later.

> **Verified (929b7d1/be39d84):** ✅ confirmed — `gameplay_pipeline_system_sets.md` status Ready (plan-review 2026-10-02); `dialogue_tick_system` (`dialogue.rs:85`, takes `ResMut<ActionQueue>` `:89`), `despawn_timer_system` (`despawn_timer.rs:46`), `resolve_pending_behaviors_system` (`entity_spawner.rs:564`) exist; backlog `:152/:153` match.

### 2. Invalid data must fail loudly and safely: no hangs, no per-frame panics
**Source:** `planning/backlog.md` ▸ Bugs (stale `rules_path:` hang, `:41`) and ▸ Queued (no scene-fetch retry, `:64`); `.claude/agent-memory/system-architect/runtime_validate_is_advisory.md`, `schema_tightening_blast_radius.md`

Merges last edition's #4 with a new finding. `ProjectConfig::validate()` at load time only logs
`error!` and the runtime keeps using the invalid config: the responsive_ui review (2026-10-11)
caught an `f32::clamp(min, max)` that would have panicked every frame on `min_scale > max_scale`,
which on WASM means a dead tab. On loading, a parse failure still yields a blank loading screen
forever. Two rules are needed: every loader gets a `Failed` arm that stops the load and shows the
error on screen, and every per-frame consumer of a validated field must survive the values
`validate()` rejects.

> **Verified (929b7d1/be39d84):** ✅ confirmed — `project_loader.rs:50-52` logs `Invalid ProjectConfig` and carries on (same log-only pattern for every catalog `validate()`, `:238-323`); the clamp was fixed before merge (`capabilities/ui_scale.rs:62` uses `max().min()`, with a comment saying `f32::clamp` would panic); `:46` returns early forever when the `.project.ron` never parses.

### 3. One input-ownership primitive, now including touch
**Source:** `planning/backlog.md` ▸ Queued ("Left mouse button on any UI node…", `:24`) and the queued `mobile_ui_demo`; `.claude/agent-memory/system-architect/panel_input_blocking.md`

Clickable UI grew sharply (flex `Group`, `SetUiVisible`, `ui_scale`, the mobile track), and touch
will be a third pointer source beside mouse and gamepad. `camera_orbit_system` and
`input_translator_system` still read raw `ButtonInput<MouseButton>`, and `Display::None` panels can
leave `panels_open > 0`, locking input with nothing visible. One `UiPointerCaptured` resource,
written once per frame from the UI focus path and read by every world-input system, should exist
before touch lands — otherwise each new source gets its own ad-hoc guard.

> **Verified (929b7d1/be39d84):** ⚠️ partly — raw `ButtonInput<MouseButton>` reads confirmed in `camera_orbit_system` (`camera.rs:187/191`), `party_camera_follow_system` (`:403/407`) and `input_translator_system` (`input.rs:291/293`). The `Display::None` panel case is reachable only when `validate` is skipped: panels do carry `UiNodeId` (`scene_loader.rs:2213/2492/2677/2807`) and the executor arm doesn't refuse them, but `validate` rejects such a target. Still a real hole, since runtime validation is advisory.

### 4. Cheapest determinism measurement in place before the ocean batch; make presentation/simulation a real contract
**Source:** `planning/backlog.md` ▸ D5 (`:151`), Cross-platform determinism harness (`:148`); `crates/ironhold_core/src/schema/actions.rs:449` (`Action::is_presentation_only`); `.claude/agent-memory/system-architect/ui_display_none_and_ui_id_facts.md`

The full cross-platform harness is expensive; D5 (two `App`s, same scripted input, assert identical
`ActionQueue`/`GameEvent` sequences) is S–M and would gate every later physics-adjacent merge,
including the eight queued ocean items. Separately, `is_presentation_only()` shipped with
`SetUiVisible` as its only `true` case and nothing reads it yet; its doc comment defers the
audio/camera/feedback actions to responsive-UI Phase 3, with no rule for deciding them. That classification will decide what a future lockstep or
replay layer may skip, so it needs an explicit rule and a validate/test guard before more actions
are classified by habit.

> **Verified (929b7d1/be39d84):** ⚠️ partly — `actions.rs:449` exhaustive, only `SetUiVisible => true` (`:451`), no reader outside `actions.rs` (grep) ✅. Corrected: the doc comment (`actions.rs:443-447`) does not contradict the code. It says every other variant stays `false` until Phase 3 classifies the audio/camera/feedback ones. The point about having no rule still stands; the "mismatch" claim was wrong. Ranking unchanged.

### 5. Break up the scene_manager monolith before the 16-param wall breaks the build
**Source:** `.claude/agent-memory/system-architect/fragile_modules.md`, `inventory_panel_triplication.md`; `planning/backlog.md` ▸ Queued ▸ UI (panel-chrome helper, `:275`)

Same item, numbers went the wrong way: `scene_loader.rs` at 3,686 lines, and the UI work added a
`UiNodeId` insert to every `spawn_ui_element_node` arm. `spawn_scene_v2` is still at 16 params, so
the next feature needing one more resource hits the limit. The three inventory/shop/container panel
copies are still the cheapest first extraction; UI spawning is the obvious second.

> **Verified (929b7d1/be39d84):** ✅ confirmed — 3,686 lines; `UiNodeId` inserted in 13 `spawn_ui_element_node` arms (`scene_loader.rs:1847-2807`); 16 params.

**Notable new things since last edition:**
- The core CLAUDE.md split (`e5c5239`, merge `37de6f4`) lost no rules but added a new risk: some
  must-rules now live only in `docs/dev` pages, or are placed by subject rather than in the folder
  of the implementing file, so they don't load when that file is edited
  (`core_claude_md_split_review.md`).
- Core hardcodes the host-page canvas selector `#ironhold-canvas`; bevy_winit panics if an embedder
  page lacks it. Not logged in the backlog yet (`web_canvas_host_contract.md`).

> **Verified (929b7d1/be39d84):** ✅ confirmed — `e5c5239`/`37de6f4` are the split commits; `lib.rs:893` hardcodes `#ironhold-canvas`; bevy_winit 0.18 `winit_windows.rs:293` `panic!("Cannot find element: {selector}.")`; no backlog entry mentions it.

---

## Alignment-Reviewer — RON designer-reachability, no hardcoded behavior

*Lens: what a designer still can't do through RON alone, or where behavior silently changes depending on where the designer wrote it.*

**Previous top-5, status:**

| # | Item | Status |
|---|---|---|
| 1 | `action_needs_target` allowlist turns `{target}` into `""` | **Still open** (`capabilities/action_bar.rs:429-444`: 11 variants, ends `_ => false`; Bugs entry unchanged, `backlog.md:39`) |
| 2 | Real pause | **Still open** (`features/real_pause.md` still Queued, `backlog.md:67`; no `PauseGame`/`TogglePause`/`GamePaused` in `ironhold_core/src`). New logged consequence: "Dying while paused never reaches game over" (`:21`) |
| 3 | Player-tagged prefabs ignore capability blocks | **Still open** (`attach_prefab_features` only called from `entity_spawner.rs:334`, `scene_loader.rs:477`/`:697`; `spawn_player_entity_core` (`entity_spawner.rs:975`) never calls it; no warning in runtime or CLI) |
| 4 | Dialogue `do_actions` get `{self}` but never `{target}` | **Still open** (`dialogue.rs:320-364` only `.replace("{self}", …)`, ends in `other => other`) |
| 5 | RON-authorable collider friction | **Still open** (Queued `:56`; `Friction` hardcoded at `entity_spawner.rs:328`/`:1140`, `scene_loader.rs:470`/`:690`). Drops out only because nobody has asked for it yet |

> **Verified (929b7d1/be39d84):** ✅ confirmed — every file:line above matches (`action_bar.rs:429-444` has 11 arms + `_ => false`; `attach_prefab_features` called only at `entity_spawner.rs:334`, `scene_loader.rs:477/697`; `dialogue.rs:320-363` `{self}`-only, `other => other`; no `PauseGame`/`TogglePause`/`GamePaused` in `src`). Note: the backlog entry at `:39` itself is stale. It still cites `action_bar.rs:380-395`, the deleted `message_interpreter.rs`, and "16 variants" (it is 23 now).

**New top-5:**

### 1. The two substitution sites still kept in sync by hand both end in `_` wildcards
**Source:** `planning/backlog.md` ▸ Bugs (`action_needs_target` allowlist); `planning/claude_suggestions.md` (system-architect, 2026-09-28); agent-memory `ui_set_visible_pattern.md`, `dialogue_system_pattern.md`

Of four substitution sites, `rewrite_self` and `rewrite_target` are exhaustive matches (a missing
arm is a compile error); `action_needs_target` (`action_bar.rs:429`) and `substitute_self_in_action`
(`dialogue.rs:320`) are not. With no target set, `SetVariable`, `IncrementVariable`,
`OpenContainer`, `OpenShop`, `AddItem`/`RemoveItem`/`TransferItem`, `ResetToSpawn`, `SetTarget`,
`StartDialogue`, `ProjectDecal`, `EmitEventAfterDelay` and `Spawn.id`/`spawn_point` (and
`ShowFloatingText.text`) fire against `""` with no `no_target` event or warning, and `{target}` stays a literal
string in dialogue. The `ui_set_visible` plan asked for explicit pass-through arms in both; they
were not added. Fix both at once: derive "needs a target" from `rewrite_target`, route dialogue
through `action_substitution.rs`.

> **Verified (929b7d1/be39d84):** ⚠️ partly — the mechanism is confirmed (`action_bar.rs:257` decides `no_target` via `action_needs_target`, `:266` falls back to `""`, `:279` runs the exhaustive `rewrite_target`), but the list was incomplete. `rewrite_target` (`action_substitution.rs:151+`) substitutes in 23 variants, so 12 variants and 3 fields bypass the allowlist, not ~8. Item text corrected; ranking unchanged (if anything stronger).

### 2. Real pause: no RON action stops the world
**Source:** `planning/backlog.md` ▸ Queued ▸ Engine/Runtime; `planning/features/real_pause.md`; `backlog.md:21`

Every shipped "pause" is an overlay: NPC AI, physics, `tick_delayed_events_system` (`lib.rs:785`,
no pause gate) and interact/loot keep running. In `primitive_world`, health reaching 0 under the
overlay never reaches `game_over` because the transition fires only `from: "playing"`. The docs now
say to "handle these in `global_on`" — a workaround for the missing primitive, not a replacement,
and a designer cannot write the missing piece in RON.

> **Verified (929b7d1/be39d84):** ✅ confirmed — `tick_delayed_events_system` (`lib.rs:785-800`) subtracts raw `Time` delta with no state/pause check (registered `lib.rs:357`, comment says "no pause gate"); `primitive_world/logic/state_machine.ron:130` is `( from: "playing", on: "stat.player_health.depleted", to: "game_over" )`, and `toggle_pause` moves `playing → paused` (`:125`); backlog `:21`/`:67` match.

### 3. Capability blocks on player-tagged prefabs are silently dropped
**Source:** `planning/claude_suggestions.md` (debug-detective, `4df567f`, 2026-09-11)

`inventory:`, `interactable:`, `dialogue:`, `behavior:`, `trigger_zone:` work on any NPC or prop and
do nothing on a player prefab, with no runtime warning and no `validate` check. The player path now
shares `assemble_player_config`/`spawn_player_entity_core`, so wiring `attach_prefab_features` in —
or adding a warning plus a CLI error — is a single-site change.

> **Verified (929b7d1/be39d84):** ✅ confirmed — `claude_suggestions.md:389` (debug-detective, `4df567f`, 2026-09-11); `spawn_player_entity_core` (`entity_spawner.rs:975`) and `assemble_player_config` (`:1722`) never call `attach_prefab_features` (only callers `entity_spawner.rs:334`, `scene_loader.rs:477/697`).

### 4. `validate` skips every action in ActionBar slots
**Source:** `planning/backlog.md` ▸ Queued ▸ UI ("`collect_actions` does not scan `ActionBar` slot `do_actions`", 2026-10-11)

`collect_actions` (`ironhold_cli/src/commands/validate.rs:807-834`) walks the state machine,
behaviors and dialogue choices only. Slot `do_actions` are where designers write combat actions, so
scene-path existence, `item_key`, `spawn_point`, `SetCameraMode` and `SetUiVisible` id checks all
skip them, and `discover_extra_scenes` never parses an overlay opened only from a slot (a false
`set_ui_visible_unknown_id` the designer cannot suppress). `SetUiVisible` already worked around this
with its own slot scan — now the second hand-written action walker.

> **Verified (929b7d1/be39d84):** ✅ confirmed — `collect_actions` (`validate.rs:807-833`) walks state machine, behaviors and dialogue choices only; the `SetUiVisible` slot workaround is at `validate.rs:867-886`, `set_ui_visible_unknown_id` at `:957`, `discover_extra_scenes` at `:687`; backlog `:262` matches.

### 5. UI visibility: overlays can't start hidden, the ActionBar can't be hidden at all
**Source:** `planning/backlog.md` ▸ Queued ▸ UI ("Overlay nodes cannot start hidden", "An `ActionBar` cannot be hidden at runtime", 2026-10-11/12)

The documented start-hidden recipe is `SetUiVisible` on `scene.ready:<scene>`, but overlays never
fire it (`scene_loader.rs:1332` writes `SceneEvent::Ready` only when `!is_overlay`), so pause/menu
overlays can't start with a node hidden by that recipe (`docs/20` ~L3980 does say so). `validate` correctly rejects
`SetUiVisible` on an `ActionBar` (its slots would still fire), leaving cutscenes, menus and
portrait layouts with no way to hide it. Responsive UI phase 3 (orientation swaps via
`SetUiVisible` pairs) depends on both gaps closing.

> **Verified (929b7d1/be39d84):** ⚠️ partly — `scene_loader.rs:1332-1333` is confirmed, and backlog `:261`/`:263` match. Two corrections:
> - The docs do explain it: `docs/20` L3980 says "overlays do not fire `scene.ready`, so an overlay node cannot be started hidden yet". So "nothing says why" was wrong.
> - There is probably already a workaround. `spawn_scene_v2` writes `SceneEvent::Loaded` (`:87`) in the same run as the base scene's `Ready` (`:1333`), for overlays too. It runs `.before(fsm_interpreter_system)` (`lib.rs:233`), so the UI commands are flushed before the interpreter reads either event. That means `SetUiVisible` bound to `scene.loaded:<overlay>` should already see the overlay's nodes. This is unverified (no test was run), and `docs/20` L3892 / `docs/30` L88 describe `scene.loaded` as "entities not yet spawned", which looks inaccurate for interpreter consumers.
>
> If one test confirms it, the overlay half drops to a docs fix and this item's weight moves to the `ActionBar` half. That may lower its rank.

**Notable new things since last edition:**
- `SetUiVisible` + `UiNodeId` (`d50045d`) follows the pipeline correctly (guarded `Display` write,
  no capability pushing to `ActionQueue`); the empty-id footgun is fixed (`action_executor.rs:812`).
- The flex `Group` and `ProjectConfig.ui_scale` (`f2194a3`) are RON-authorable, and every
  `scene.ui` scan goes through `walk_ui_nodes`, so nested nodes get every validate check.
- After D1 (`9e167ca`), declaration order in `stats.ron` and the string order of bindings/slot keys
  decide same-frame outcomes; designers need to know that.

> **Verified (929b7d1/be39d84):** ⚠️ partly — `action_executor.rs:812` empty-id guard ✅. `ui_scale` was cited as `b5c1800` but is `f2194a3` (fixed above).

---

## Game-World-Designer — missing/unstable features, world-building & player experience

*Lens: what kind of game world or moment-to-moment experience is currently impossible or fragile to build.*

**Previous top-5, status:** nothing on the list moved; the October Done section is all UI,
infrastructure and determinism work.

| # | Item | Status |
|---|---|---|
| 1 | Quest system (v1 + v2) | **Still open** (`backlog.md` L109-110 unchecked; `features/quest_system.md` still Draft, planned at `e9a421e`, 2026-06-02 — older than item gating, dialogue conditions and the attacker-identity plan) |
| 2 | Loot system v1 | **Still open** (L112; `features/loot_system.md` Draft since 2026-06-02); still blocks Quest's `Collect` objective |
| 3 | Sound zones | **Still open** (L106); Audio channels (L105) also open. The one audio change, `audio_volume_percent` (`0142eac`), is UI only |
| 4 | Day/night cycle | **Still open, context changed again.** The ocean plan-review (`features/ocean_simulation_demo.md` ~L368-370) now requires day/night to be a producer into a shared `EnvState` with `env_apply_system` as the only light writer; the day/night plan doesn't have that amendment yet |
| 5 | Save / load game state | **Still open** (Icebox, L303; Draft since 2026-06-02), not promoted |

> **Verified (929b7d1/be39d84):** ✅ confirmed — backlog L105/106/109/110/112/303 unchecked; `quest_system.md`/`loot_system.md` Draft, planned at `e9a421e` (2026-06-02); `ocean_simulation_demo.md` L368-370 has the `EnvState`/`env_apply_system` contract, and `day_night_cycle.md:184` still writes `AmbientLight` "via ECS queries" (no amendment); `0142eac` is the audio-volume readout.

**New top-5:**

### 1. Quest system: core loop (v1) + presentation layer (v2)
**Source:** `planning/backlog.md` ▸ Queued ▸ Gameplay & Environment (L109-110); `planning/features/quest_system.md`

Still the biggest gap between a diorama and a world that remembers what you did: Greywatch's Maren →
Old Key → Seal Door → reward chain is a loose bundle of `GameVariables`. Since the plan was drafted,
item gating (`requires_item`) and dialogue conditions shipped and should be first-class objective
and branch sources, and in co-op `KillCount` needs to know who landed the hit (the queued "Attacker
identity on hit events", L68). The plan needs a refresh pass before coding, not just a pickup.

> **Verified (929b7d1/be39d84):** ✅ confirmed — L68 is the attacker-identity item; the plan is still Draft at `e9a421e`.

### 2. Scene lighting that actually takes effect: fix ambient, add fog, one shared `EnvState`
**Source:** self-described, from `planning/features/ocean_simulation_demo.md` ~L348-351 (the plan-review recommended logging it under Bugs; not yet done). Overlaps Day/night (L104) and the ocean "Runtime environment control actions" item (L125).

`scene_loader.rs` L2998-3007 spawns `AmbientLight` as an entity; in Bevy 0.18 that is a per-camera
component, and scene-wide ambient is the `GlobalAmbientLight` resource. Checked against the Bevy
0.18 source, not in a browser: authored ambient does nothing, and each such scene load also
spawns a stray render-graph-less `Camera`. 37 scenes set `lighting: ambient:`,
including four of the five `3rd_person_game_demo` scenes (not `pause`). Fog doesn't exist in the schema at all. These are
the two cheapest mood tools there are; fix ambient and add `EnvState` and fog before day/night so
the sun has a single owner from day one.

> **Verified (929b7d1/be39d84):** ✅ confirmed, and stronger than stated. `Cargo.lock` pins `bevy_light`/`bevy_pbr` 0.18.0. Read from the registry source:
> - `bevy_light-0.18.0/src/ambient_light.rs:8-12`: `AmbientLight` is documented as "added to a camera to override `GlobalAmbientLight`" and has `#[require(Camera)]`.
> - `bevy_pbr-0.18.0/src/render/light.rs:705-723`: `prepare_lights` reads the per-view override only via `Option<&AmbientLight>` on `With<Camera3d>` entities. At `:1153-1158` it falls back to `Res<GlobalAmbientLight>` (white, 80 cd/m², `ambient_light.rs:80-87`).
> - ironhold never writes `GlobalAmbientLight` (grep).
>
> So `lighting.ambient`/`ambient_brightness` (documented as working in `docs/20` L286-287) is a no-op today. The spawned entity becomes a bare `Camera`, which also triggers bevy_render's "has a `Camera` component, but it doesn't have a render graph" warn (`bevy_render-0.18.0/src/camera.rs:88-92`) on every such load. No ironhold camera query picks it up (all filter `With<Camera3d>` or a marker). Count 37 ✅ (`grep -l` of `ambient:` across `*.scene.ron`). Correction: "all four 3rd_person scenes" → four of five. Not yet its own `## Bugs` entry (mentioned only inside the ocean item at backlog L125). This is source-confirmed, no longer "likely", which strengthens the item's rank.

### 3. Real pause
**Source:** `planning/backlog.md` ▸ Queued ▸ Engine/Runtime (L67); `planning/features/real_pause.md`; Bugs "Dying while paused never reaches game over" (L21)

The overlay freezes nothing; a player pausing to read a dialogue choice can be killed behind the
menu, and in `primitive_world` that death never reaches `game_over`. A safe pause is a basic trust
contract, and every richer world (quests, longer dialogue, inventory screens) makes players pause
more. The plan exists since 2026-09-15 and is scoped.

> **Verified (929b7d1/be39d84):** ✅ confirmed — `real_pause.md` Queued, planned at `2a034d6` (2026-09-15); mechanism as in Alignment #2.

### 4. Loot system: roll + auto-loot (v1)
**Source:** `planning/backlog.md` ▸ Queued ▸ Gameplay & Environment (L112); `planning/features/loot_system.md`

Corpses are lootable but every drop is hand-authored per prefab, so the second zombie is never more
interesting than the first. All stated dependencies (Inventory, `Spawn.at_entity`) have shipped, and
Loot v1 also unblocks Quest's `Collect` objective.

> **Verified (929b7d1/be39d84):** ✅ confirmed — L112 unchecked, `loot_system.md` Draft; `Spawn.at_entity` exists (`action_substitution.rs`, executor). The "all dependencies shipped" claim was not re-audited beyond that.

### 5. Location-aware, reliable audio: sound zones + the WASM autoplay unlock
**Source:** `planning/backlog.md` ▸ Queued ▸ Gameplay & Environment (Sound zones L106, Audio channels L105); Bugs "WASM AudioContext autoplay-unlock is unreliable" (L30)

Greywatch's safe-village vs. tense-wilds contrast exists only visually. Sound zones need little
beyond a fade envelope, and the ocean demo lists them as a soft dependency. Paired with the autoplay
bug because on the web build — the only public build — sound can silently fail even after a real
click (repro in `entity_logic_demo`); zones built on that would be undependable. Unlock first.

> **Verified (929b7d1/be39d84):** ✅ confirmed — backlog L30 (autoplay bug, names `entity_logic_demo`), L105/L106 unchecked.

*Dropped this time:* Day/night is folded into #2 so the sun has one owner; Save/load is deferred —
it matters once Quest and Loot exist, and should be promoted to Queued then.

**Notable new things since last edition:**
- `SetUiVisible` (`d50045d`) makes progressive/diegetic HUD reveal authorable (note the `ActionBar`
  itself is still excluded).
- Room 3 click targets now have a health/death/respawn loop (`4288024`) — the first RON-only,
  end-to-end "threat lifecycle" outside `3rd_person_game_demo`.
- Responsive UI phases 1-2 (`710e79a` + `b5c1800` + `f2194a3`) put mobile/portrait players in scope, raising the
  stakes on world-space label legibility (open bugs L33-35).

> **Verified (929b7d1/be39d84):** ⚠️ partly — the room-3 loop is `4288024` (`d9f9ae0` is the follow-up label-stacking fix); the canvas fit is `710e79a`. L33-35 are the label-legibility bugs ✅.

---

## UX-Gamedesigner-Reviewer — designer-authoring experience

*Lens: what silently goes wrong, is hard to discover, or wastes a non-programmer designer's iteration time.*

**Previous top-5, status:** none shipped. October's designer-facing work went to UI layout
(`Group`, `ui_scale`, `SetUiVisible`).

| # | Item | Status |
|---|---|---|
| 1 | The pause menu doesn't pause anything | **Still open, zero movement.** `real_pause.md` still Queued (~L67); the `docs/30` pause example (~L223-257) still has no warning. Led to a new Bug: dying while paused never reaches game over (~L21, `primitive_world/logic/state_machine.ron:130`) |
| 2 | Loading failures hang forever | **Still open.** `project_loader.rs` `Failed` arms (L153-213) cover only catalogs; nothing handles a failed `.project.ron` or reads `UntypedAssetLoadFailedEvent`. `play.html` shows a message (`b5c1800`) only when the engine itself can't start. Bug (~L41) and retry item (~L64) open |
| 3 | Demo projects for core authoring patterns | **Changed: partly moved.** Minimal `ui_demo` shipped (`b1d685d`, Hide/Show stations from `d50045d`). `prefab_demo`, `audio_demo`, `scene_transitions_demo`, `parkour_demo` still unbuilt (~L233-243), three editions running |
| 4 | Action-bar `{target}` becomes `""` | **Still open** (`action_bar.rs:429-444`, 11 arms; no CLI check; ~L39) |
| 5 | `motion:` missing from reference docs | **Still open** (14 uses in 5 projects, zero `docs/` entries, ~L66). Tofu is also unchanged: exactly 20 non-ASCII `text:` strings still ship (`particles_demo` 16, `effect_mayhem_demo` 4), flagged again in the 2026-10-10 baseline review (~L260) |

> **Verified (929b7d1/be39d84):** ⚠️ partly — `ui_demo` was added in `b1d685d`, not `1167eb9` (that is the `walk_ui_nodes` refactor), and backlog L240 (`ui_demo` project) is still unchecked although the project exists. Confirmed: `docs/30` pause example L223-257 has no warning (only L675-678 mentions the missing pause gate); no `UntypedAssetLoadFailedEvent` reader in `src`; `motion:` = 14 uses in 5 projects (primitive_world 5, stats_demo 4, entity_logic_demo 2, particles_demo 2, effect_mayhem_demo 1), zero hits in `docs/`; exactly 20 non-ASCII `text:` strings (particles_demo 16, effect_mayhem_demo 4).

**New top-5:**

### 1. The pause menu doesn't pause anything, and it now causes soft-locks (carried)
**Source:** `planning/backlog.md` ▸ Queued ▸ Engine/Runtime ("Real pause"); ▸ Bugs ("Dying while paused never reaches game over"); `planning/features/real_pause.md`

Two editions with no change, and the cost is now concrete: the `docs/30` pattern combined with
`from: "playing"`-only transitions leaves `primitive_world` stuck in `playing` at 0 health. The only
warning is buried in the corpse-loot section (`docs/30` ~L675-678), not next to the pause example
people copy. Until real pause ships, a two-line note under the example ("the game keeps running;
handle death and timers in `global_on`") would stop the next designer walking into this.

> **Verified (929b7d1/be39d84):** ✅ confirmed — see Alignment #2; `docs/30` L675-678 is the only no-pause-gate note.

### 2. Action-bar slots get no action-level validate checks
**Source:** `planning/backlog.md` ▸ UI (`collect_actions` skips ActionBar `do_actions`, ~L262) + ▸ Bugs (`action_needs_target` drift, ~L39)

Most combat is authored in action-bar slots — exactly where `ironhold validate` stops looking
(`validate.rs:807`). An overlay opened only from a slot is never parsed (false
`set_ui_visible_unknown_id`), and `{target}` still quietly becomes `""` for 12 action types (plus 3 fields).
"validate passed" means least for the content a designer iterates on most, and a broken slot just
does nothing with no event to react to.

> **Verified (929b7d1/be39d84):** ⚠️ partly — the mechanism is confirmed (`validate.rs:807`, backlog L262/L39). The count was 8; it is 12 variants + 3 fields (see Alignment #1). Corrected.

### 3. Loading failures still hang the game with nothing on screen (carried)
**Source:** `planning/backlog.md` ▸ Bugs (stale `.project.ron` field hang, ~L41) + ▸ Queued (scene-load retry/timeout, ~L64)

One leftover field in `.project.ron`, or a failed scene fetch, leaves a loading screen that never
finishes with the error only in the browser console. October showed the fix is cheap: `play.html`
already got a "cannot start" message (`b5c1800`); extending it to project/scene load failures turns
a mystery hang into a one-minute fix for someone who only tests in the browser.

> **Verified (929b7d1/be39d84):** ✅ confirmed — backlog L41/L64; `play.html` (`b5c1800`) has `ironholdStartupFailed` for crash/`unreachable` and a 60 s no-start timeout (L328-343) only; `project_loader.rs:46` early-returns when the config never loads.

### 4. `docs/20` is too big to navigate, and it still has holes
**Source:** `planning/backlog.md` ▸ Designer Experience ("Split `docs/20_data_formats.md`", ~L231; "`docs/20` has no entry for `MotionDef`", ~L66)

~361k characters in 25 top-level sections and still growing (`SetUiVisible` alone added a long subsection,
~L3974). A designer can't skim it, so they copy from demos, and that's where gaps hide. `motion:` is
the clearest case: 14 uses in 5 shipped projects, no reference entry, and a behavior change (now on
the 64 Hz physics tick) nobody would read about. Split into per-topic pages and land the `motion:`
entry as part of it.

> **Verified (929b7d1/be39d84):** ⚠️ partly — the size was stale. `docs/20` at `929b7d1` is 360,691 characters with 25 `## ` headings; "354k" is the backlog L231 figure from 2026-10-07. `#### SetUiVisible` is at L3974 ✅, and the `motion:` gap is confirmed. Corrected; ranking unchanged.

### 5. No demo projects for prefabs, audio or scene transitions; README table incomplete (carried, narrowed)
**Source:** `planning/backlog.md` ▸ Designer Experience (~L240-243, ~L230 item 4)

`ui_demo` shows the station-per-concept layout works, but prefabs (the first thing a designer
authors), audio and multi-scene flow are still taught by accident inside other demos. The README
project table (`README.md` ~L131) lists `ui_demo` but has no rows for `particles_demo` or
`local_coop_demo`, two of the best copy-from projects — a five-minute fix. `prefab_demo` is the
highest-value next demo.

> **Verified (929b7d1/be39d84):** ✅ confirmed — `README.md:131` is the `ui_demo` row; no `particles_demo`/`local_coop_demo` rows (grep); backlog L241-243 unchecked.

**Notable new things since last edition:**
- **Docs audience split** (`c188550`): `docs/README.md` is an audience-tagged index, CLI docs moved
  to `docs/15_authoring_tools.md` (16 previously undocumented validate codes added), developer docs
  moved to `docs/dev/` with redirect stubs — the clearest reading order designers have had.
- The `SetUiVisible` docs (`docs/20` ~L3974-3985) are a model for designer reference text: every
  gotcha stated up front, and `validate` catches typo'd ids.
- Responsive web canvas + `ui_scale` and the `audio_volume_percent` variable (`0142eac`) remove two
  whole classes of hand-built workarounds.

> **Verified (929b7d1/be39d84):** ✅ confirmed — `c188550` adds docs/15 rows for 16 previously undocumented codes (commit message); `docs/20` L3974-3985 is the `SetUiVisible` gotcha list.

---

## WASM-Perf-Reviewer — browser runtime performance & binary size

*Lens: frame-time impact, allocation/GC pressure, binary-size trajectory, first-load/first-frame stalls.*

**Binary size check-in:** `pkg/ironhold_web_bg.wasm` = 32,161,056 bytes (~30.7 MiB), ~63 MB below
the 95 MB warn line and ~68 MB below GitHub's 100 MB hard limit, up 71 KB (+0.2%) from the second
edition. **Caveat:** this is the last committed release build (the `pkg/` commit immediately
following `a987fd8`, 2026-10-10); it predates
the responsive-UI, `SetUiVisible` and room-3 health merges, so HEAD itself is unmeasured. None adds
a dependency, so expect a few KB. Size is not a factor in any item below.

> **Verified (929b7d1/be39d84):** ✅ confirmed — blob size 32,161,056 bytes vs 32,090,055 in the 2026-09-29 build (+71,001 B); `4288024`, `710e79a`, `f2194a3` and `d50045d` are not ancestors of that build; no `Cargo.toml`/`Cargo.lock` changes since it. Hygiene fix: the original cited the `pkg/` commit's hash, which `planning/CLAUDE.md` forbids, so it is now described in words.

**Previous top-5, status:** none shipped; all five are still open under `backlog.md` ▸ Performance.

| # | Item | Status |
|---|---|---|
| 1 | WASM terrain first-frame stall | **Still open** (`terrain.rs:73` `AsyncComputeTaskPool::get()`, `:108` `block_on(poll_once)`; no `cfg(wasm32)` progressive path) |
| 2 | Pipeline warmup for Text2d / UI / Sprite | **Still open** (`pipeline_warmup_system` queries only `With<Mesh3d>`, `lib.rs:482`); matters more now that `ui_demo` and nested `Group` scenes add first-frame UI |
| 3 | Frozen animation clips evaluated forever | **Still open** (`animation.rs:367` only `pause()`s; `AnimationGraphHandle` never dropped) |
| 4 | Scene transition material cache | **Still open** (`scene_loader.rs:178-189` clears `mats.built` and rebuilds every catalog material per `LoadScene`) |
| 5 | Remaining per-frame allocations | **Still open** (stat-key Vec clone at `stats.rs:19` and `:118`; `format!` before guard at `stat_display.rs:162/227/438`, `camera.rs:984`); backlog entry still names the deleted `message_interpreter_system` |

> **Verified (929b7d1/be39d84):** ✅ confirmed — `terrain.rs:73`/`:108` and no `cfg(target_arch)` in terrain.rs; `animation.rs:367` `pause()`; `scene_loader.rs:177-188` clear + rebuild; `stats.rs:19`/`:118` key-Vec clones; `stat_display.rs:162/227/438` and `camera.rs:984` `format!`. Drift fixed: warmup query is `lib.rs:482`.

**New top-5:**

### 1. Cap the fitted canvas's total physical pixels (responsive UI Phase 4 "render cap")
**Source:** `planning/claude_suggestions.md` ("Phase 4 render budget should cap total physical pixels, not just mobile DPR", `d758771`); `planning/features/responsive_ui.md` Phase 4; agent memory `project_responsive_canvas.md`

Phase 1 sets `fit_canvas_to_parent: true` (`lib.rs:894`), so the backing store is now CSS viewport ×
devicePixelRatio instead of a fixed 1280×720 logical box. Phones got cheaper (DPR 3: 8.3 MP → ~2.8
MP), but large desktop monitors became the worst case: 4K at DPR 1.5 is ~8.0 MP vs. 2.07 MP before
(~3.9× fill-rate every frame), and with no `Msaa` override Bevy's default 4× MSAA puts render
targets at roughly 350 MB. The only per-frame regression on this list, hits every project, and not
live yet (the `pkg/` build predates it) — it ships with the next release. Clamp the backing size to
a pixel budget of ~2–3.7 MP, not a mobile-only DPR clamp.

> **Verified (929b7d1/be39d84):** ✅ confirmed.
> - Code: `lib.rs:893-894` sets `canvas` + `fit_canvas_to_parent: true`, introduced in `710e79a`; before that the window was Bevy's default 1280×720 logical.
> - MSAA: no `Msaa` anywhere in `crates/`. bevy_render 0.18 registers `Msaa` as a required component of every `Camera` (`bevy_render-0.18.0/src/camera.rs:56`), defaulting to `Sample4` (`view/mod.rs:169-174`).
> - The pixel arithmetic checks out (1280×720×3² = 8.3 MP; 2560×(1440−44)×1.5² ≈ 8.0 MP; 1280×720×1.5² = 2.07 MP).
> - The ~350 MB render-target figure is an estimate and was not measured. No `Hdr` is used in core, so 8 MP × 4 samples × (colour + depth) lands nearer ~260-300 MB. Same order of magnitude, ranking unchanged.

### 2. WASM terrain generation first-frame stall
**Source:** `planning/backlog.md` ▸ Performance

On WASM the async compute pool runs on the main thread, so `block_on(poll_once)` builds the whole
heightmap mesh in one frame — an estimated 100–500 ms freeze at the moment a player forms a first
impression, contradicting the crate rule "never block the main thread". Fix: a `cfg(wasm32)` path
building a few tiles per frame, or pre-baked GLB terrain.

> **Verified (929b7d1/be39d84):** ✅ code confirmed (`terrain.rs:73/108`). The 100–500 ms figure is an estimate and was not measured.

### 3. Extend pipeline warmup to Text2d / UI / Sprite
**Source:** `planning/backlog.md` ▸ Performance; `planning/features/pipeline_warmup_2d_ui.md`; `planning/claude_suggestions.md` (Sprite warmup)

`pipeline_warmup_system` (`lib.rs:479-502`) only touches `Mesh3d`; on WebGPU each pipeline compiles
synchronously on first draw, so first text/UI/sprite render can stall an estimated 100–300 ms. The
`Textured` `Sprite` stat bars on `3rd_person_game_demo` players are still unwarmed, and October's UI
work (nested `Group`, `ui_demo`, `SetUiVisible` revealing nodes later) adds surfaces that first draw
after the warmup window closes.

> **Verified (929b7d1/be39d84):** ✅ confirmed — `3rd_person_game_demo/prefabs/prefabs.ron:84` `style: Textured(`, drawn as `Sprite` children (`stat_display.rs:301-313`); `pipeline_warmup_2d_ui.md` still Draft; `claude_suggestions.md:74` is the Sprite entry. Line drift fixed (`lib.rs:479-502`). Stall figures are estimates.

### 4. Paused/frozen animation clips are fully evaluated forever
**Source:** `planning/backlog.md` ▸ Performance (promoted 2026-09-14)

`freeze: true` only pauses the clip; bevy_animation 0.18 still samples every curve and writes every
bone `Transform` each frame — an estimated 0.2–0.5 ms/frame for six 42-joint corpses over their
lifetime (up to 600 s unlooted, 20 s after looting), with `par_iter_mut` serial on WASM. The smallest diff on the list: drop
`AnimationGraphHandle` once frozen, keeping `ActiveOverride.frozen` durable for the WASM GLTF-respawn
recovery path.

> **Verified (929b7d1/be39d84):** ⚠️ partly — `animation.rs:367` only `pause()`s ✅, and `freeze: true` is set in the three `corpse_policy_*.ron`. Two corrections:
> - Corpse lifetime: `lootable_corpse.behavior.ron:42` gives 600 s unlooted and `:61` gives 20 s after looting, not 300 s.
> - Room 3: the cited loop creates no corpses or frozen clips. `local_coop_demo/behaviors/click_target.behavior.ron` hides and re-shows the same primitive entity via `SetEntityVisible` + `EmitEventAfterDelay`. That sentence has been removed.
>
> The per-frame cost is an estimate. Ranking unchanged (the demoted-to-#4 position already reflects modest cost).

### 5. Warmup must only strip the `NoFrustumCulling` it added (blocker for the ocean demo)
**Source:** `planning/features/ocean_simulation_demo.md` finding B1; `lib.rs:494-497`; ocean backlog section

When the countdown hits 0, `pipeline_warmup_system` removes `NoFrustumCulling` from every entity
that has it, not only those it inserted. The planned ocean grid needs it permanently (its vertex
shader displaces outside the flat-grid AABB, so wave crests would pop out as the camera pitches).
Fix: a `WarmupNoCull` marker, removing NFC only where present — before ocean v1, together with the
plan's other WASM gates: measure the 192×192 grid on an integrated GPU (~37k vertices × 8 `sincos`),
sort buoyancy by `SpawnId` in a reused `Local<Vec>`, keep the ~336 B all-`Vec4` uniform, and upload
it only on change.

> **Verified (929b7d1/be39d84):** ✅ confirmed, and it is live today, not only an ocean blocker. When the countdown reaches 0, `lib.rs:494-497` removes NFC from every `With<NoFrustumCulling>` entity, and nothing marks which ones warmup inserted.
>
> Particle pool groups already depend on permanent NFC: `particle_renderer.rs:364/392` spawn every pool mesh with `NoFrustumCulling` (`Transform::default()` with world-space vertices). A pool group first created inside the 4-frame window (for example by the documented scene-load `SpawnEffect` warmup recipe) loses NFC at warmup end and is then culled against a stale AABB. The ocean plan's N17 (`ocean_simulation_demo.md` L394) flagged this as "may be affected". It follows from the code but has not been reproduced in a browser.
>
> Arguably this deserves a higher rank than #5, and its own `## Bugs` entry independent of ocean.

*Demoted but still open:* material cache (was #4) and the per-frame allocation trio (was #5) — neither
got worse and both are smaller than the items above.

**Notable new things since last edition:**
- `ui_scale_system` (`capabilities/ui_scale.rs:75-118`) runs every frame but is a few float ops plus
  an O(1) lookup, with an epsilon-guarded `UiScale` write — web-clean.
- `SetUiVisible` (`d50045d`) runs per action, not per frame, with a guarded `Node.display` write and
  no new pipeline — no idle cost.
- The responsive-canvas pixel cost (#1) is the first new per-frame regression risk in two editions
  and the one thing to address before the next `/release`.

> **Verified (929b7d1/be39d84):** ✅ confirmed — `ui_scale_system` at `capabilities/ui_scale.rs:75`; `SetUiVisible` arm guarded (`action_executor.rs:812-820`, `if node.display != display`).

---

## Cross-stakeholder signal

- **Real pause — three stakeholders** (Alignment #2, Game-World-Designer #3, UX #1). A missing
  engine primitive no RON can work around, a trust contract for players, and a docs pattern that
  teaches the non-pausing overlay — plus a logged soft-lock bug ("Dying while paused never reaches
  game over"). The strongest consensus in this snapshot, and it has now been on the list for two
  editions. `planning/backlog.md` ▸ Queued ▸ Engine/Runtime; `planning/features/real_pause.md`.
- **Action-bar slots & `{target}`** (Alignment #1/#4, UX #2) — `action_needs_target` turning
  `{target}` into `""`, dialogue never resolving `{target}`, and `validate` not scanning slot
  `do_actions` are one family: three hand-maintained walkers/lists that disagree with the exhaustive
  `rewrite_target`/`rewrite_self`. Fixing the family once beats three separate patches.
- **Loader failures must be visible** (System-Architect #2, UX #3) — Bug (stale `rules_path` hang) plus
  Queued retry item; `play.html`'s new "cannot start" message (`b5c1800`) shows the on-screen
  approach works and just needs extending to project/scene load failures.
- **UI visibility gaps** (Alignment #5, game-world "progressive HUD" note) and **UI pointer capture**
  (System-Architect #3, mobile/touch) both gate Responsive UI phase 3 and the mobile track.
- **Shared environment ownership** — Game-World-Designer #2 (`EnvState`/ambient fix/fog) and WASM-Perf
  #5 (warmup NFC fix) both come from the ocean plan-review and both are cheap to settle before ocean
  v1 starts.

The second edition's Rapier-determinism consensus has become D1/D2 shipped plus D3/D5 queued; of the
second edition's consensus items, only the determinism groundwork moved.

> **Verified (929b7d1/be39d84):** ✅ confirmed — each bullet restates items verified above. The `{target}` family is three walkers that disagree with the exhaustive pair; `rewrite_self` (`action_substitution.rs:13`) and `rewrite_target` (`:151`) have no `_` arm.

## Also worth flagging

- **Process:** with only 26 of 204 non-merge commits touching engine source since the last snapshot,
  essentially all 25 previous items stayed open. Items that recur across editions (real pause,
  loader failures, `{target}` drift, demo projects, monolith split, terrain stall) are the ones
  nobody has had room for — consider reserving a slot per batch for one carried item.
- **Sequencing:** before the Ocean Simulation Demo batch starts — D3 `SystemSet`s and D5 (or the full
  determinism harness), the `EnvState`/ambient fix, the warmup-NFC fix, a `UiPointerCaptured`
  primitive, and the canvas pixel cap (so ocean's GPU cost is measured against a bounded baseline).
- **Unlogged:** the `#ironhold-canvas` host-page contract (System-Architect) and the ambient-light bug
  (Game-World-Designer, recommended for `## Bugs` by the ocean plan-review) are not yet in
  `planning/backlog.md` as their own entries (the ambient bug appears only inside the ocean item's
  plan-review note at L125). Neither is the warmup-strips-all-NFC behaviour (WASM-Perf #5), which
  already affects particle pool meshes.
- **Planning hygiene:** the backlog entry for per-frame allocations still names the deleted
  `message_interpreter_system`; the `action_needs_target` Bug (L39) still cites
  `action_bar.rs:380-395`, the deleted `message_interpreter.rs` and "16 variants"; the `ui_demo`
  bullet (L240) is unchecked though the project shipped (`b1d685d`); `quest_system.md`, `loot_system.md`, `day_night_cycle.md` and
  `save_load_game_state.md` have all been Draft since 2026-06-02 and predate shipped dependencies.
- **Agent memory:** all five reviewers updated their `.claude/agent-memory/` files this round; those
  changes were committed separately in `be39d84`.

> **Verified (929b7d1/be39d84):** ⚠️ partly — the agent-memory note was stale (committed in `be39d84`, tree clean), and the "Unlogged" and hygiene lists were extended with the findings above. Draft status/dates of the four plans are confirmed (`planned at` 2026-06-02 in each).

## Verification summary

Debug-detective pass at `be39d84` (snapshot `929b7d1`). It used static reading of the repo, git, and the pinned Bevy 0.18.0 registry sources only: no cargo build/test, no browser.

- **Annotations:** 39 in total: 27 ✅ confirmed, 12 ⚠️ partly (claim right in substance but a count, hash or detail was wrong or stale), 0 ❌ wholly wrong. Every status table plus every top-5 item, notables block and "Also worth flagging" carries one.
- **Most consequential corrections:**
  1. **Game-World #2 (ambient) is confirmed from Bevy source, no longer "likely".** `AmbientLight` is a per-camera override with `#[require(Camera)]`, and `prepare_lights` reads it only from `Camera3d` entities. ironhold never writes `GlobalAmbientLight`, so `lighting.ambient` is a no-op in all 37 scenes, and each load spawns a stray render-graph-less `Camera` (warn). This strengthens the item.
  2. **WASM-Perf #5 (warmup NFC strip) is live today.** Particle pool meshes (`particle_renderer.rs:364/392`) already rely on permanent `NoFrustumCulling` and lose it if created inside the warmup window. It is not only an ocean prerequisite, which argues for a higher rank and its own Bug (inferred from code, not browser-reproduced).
  3. **Alignment #5 (overlay start-hidden) may already have a workaround.** `scene.loaded:<overlay>` is written in the same `spawn_scene_v2` run as `Ready` and is applied before the interpreter, so `SetUiVisible` on it should work (untested). Also, the docs do say why `scene.ready` fails for overlays. If one test confirms this, the overlay half becomes a docs fix and the item may drop.
  4. **The `{target}` → `""` gap is wider than stated** (Alignment #1, UX #2): 12 variants + 3 fields bypass `action_needs_target`, not ~8.
  5. **WASM-Perf #4:** corpses live 600 s / 20 s, not 300 s, and room 3's loop creates no corpses (the sentence was removed).
  6. **Hash fixes:** `ui_demo` = `b1d685d` (not `1167eb9`); canvas fit = `710e79a`; `ui_scale` = `f2194a3` (not `b5c1800`); room-3 loop = `4288024` (not `d9f9ae0`). The forbidden `pkg/` hash `ebc338c` was replaced with a description.
  7. **Smaller fixes:** docs/20 is ~361k chars / 25 sections; four of five 3rd_person scenes set ambient; `is_presentation_only`'s doc comment doesn't contradict the code; warmup line drift (`lib.rs:479-502`, `:482`).
- **No reordering was done.** Rank implications are stated in the annotations (GW #2 and WASM #5 up, Alignment #5 possibly down).
- **Not verified:** frame-time and stall estimates (terrain 100-500 ms, warmup 100-300 ms, frozen clips 0.2-0.5 ms) and the ~350 MB MSAA figure, which need a browser profile; the `scene.loaded` overlay workaround, which needs one test; the stray ambient `Camera` warn, which needs a run; Loot's full dependency list.
