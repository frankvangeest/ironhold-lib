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
(`d50045d`), project `ui_scale` and the responsive web canvas (`b5c1800`, `f2194a3`), `ui_demo`,
and a health/respawn loop on the room-3 click targets. Determinism groundwork also landed (D1
`9e167ca`, D2 `7637605`).

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

### 2. Invalid data must fail loudly and safely: no hangs, no per-frame panics
**Source:** `planning/backlog.md` ▸ Bugs (stale `rules_path:` hang, `:41`) and ▸ Queued (no scene-fetch retry, `:64`); `.claude/agent-memory/system-architect/runtime_validate_is_advisory.md`, `schema_tightening_blast_radius.md`

Merges last edition's #4 with a new finding. `ProjectConfig::validate()` at load time only logs
`error!` and the runtime keeps using the invalid config: the responsive_ui review (2026-10-11)
caught an `f32::clamp(min, max)` that would have panicked every frame on `min_scale > max_scale`,
which on WASM means a dead tab. On loading, a parse failure still yields a blank loading screen
forever. Two rules are needed: every loader gets a `Failed` arm that stops the load and shows the
error on screen, and every per-frame consumer of a validated field must survive the values
`validate()` rejects.

### 3. One input-ownership primitive, now including touch
**Source:** `planning/backlog.md` ▸ Queued ("Left mouse button on any UI node…", `:24`) and the queued `mobile_ui_demo`; `.claude/agent-memory/system-architect/panel_input_blocking.md`

Clickable UI grew sharply (flex `Group`, `SetUiVisible`, `ui_scale`, the mobile track), and touch
will be a third pointer source beside mouse and gamepad. `camera_orbit_system` and
`input_translator_system` still read raw `ButtonInput<MouseButton>`, and `Display::None` panels can
leave `panels_open > 0`, locking input with nothing visible. One `UiPointerCaptured` resource,
written once per frame from the UI focus path and read by every world-input system, should exist
before touch lands — otherwise each new source gets its own ad-hoc guard.

### 4. Cheapest determinism measurement in place before the ocean batch; make presentation/simulation a real contract
**Source:** `planning/backlog.md` ▸ D5 (`:151`), Cross-platform determinism harness (`:148`); `crates/ironhold_core/src/schema/actions.rs:449` (`Action::is_presentation_only`); `.claude/agent-memory/system-architect/ui_display_none_and_ui_id_facts.md`

The full cross-platform harness is expensive; D5 (two `App`s, same scripted input, assert identical
`ActionQueue`/`GameEvent` sequences) is S–M and would gate every later physics-adjacent merge,
including the eight queued ocean items. Separately, `is_presentation_only()` shipped with
`SetUiVisible` as its only `true` case and nothing reads it yet; its doc comment names audio and
camera actions the code doesn't match. That classification will decide what a future lockstep or
replay layer may skip, so it needs an explicit rule and a validate/test guard before more actions
are classified by habit.

### 5. Break up the scene_manager monolith before the 16-param wall breaks the build
**Source:** `.claude/agent-memory/system-architect/fragile_modules.md`, `inventory_panel_triplication.md`; `planning/backlog.md` ▸ Queued ▸ UI (panel-chrome helper, `:275`)

Same item, numbers went the wrong way: `scene_loader.rs` at 3,686 lines, and the UI work added a
`UiNodeId` insert to every `spawn_ui_element_node` arm. `spawn_scene_v2` is still at 16 params, so
the next feature needing one more resource hits the limit. The three inventory/shop/container panel
copies are still the cheapest first extraction; UI spawning is the obvious second.

**Notable new things since last edition:**
- The core CLAUDE.md split (`e5c5239`, merge `37de6f4`) lost no rules but added a new risk: some
  must-rules now live only in `docs/dev` pages, or are placed by subject rather than in the folder
  of the implementing file, so they don't load when that file is edited
  (`core_claude_md_split_review.md`).
- Core hardcodes the host-page canvas selector `#ironhold-canvas`; bevy_winit panics if an embedder
  page lacks it. Not logged in the backlog yet (`web_canvas_host_contract.md`).

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

**New top-5:**

### 1. The two substitution sites still kept in sync by hand both end in `_` wildcards
**Source:** `planning/backlog.md` ▸ Bugs (`action_needs_target` allowlist); `planning/claude_suggestions.md` (system-architect, 2026-09-28); agent-memory `ui_set_visible_pattern.md`, `dialogue_system_pattern.md`

Of four substitution sites, `rewrite_self` and `rewrite_target` are exhaustive matches (a missing
arm is a compile error); `action_needs_target` (`action_bar.rs:429`) and `substitute_self_in_action`
(`dialogue.rs:320`) are not. With no target set, `SetVariable`, `OpenContainer`,
`AddItem`/`RemoveItem`/`TransferItem`, `ResetToSpawn`, `EmitEventAfterDelay` and `Spawn.id`/
`spawn_point` fire against `""` with no `no_target` event or warning, and `{target}` stays a literal
string in dialogue. The `ui_set_visible` plan asked for explicit pass-through arms in both; they
were not added. Fix both at once: derive "needs a target" from `rewrite_target`, route dialogue
through `action_substitution.rs`.

### 2. Real pause: no RON action stops the world
**Source:** `planning/backlog.md` ▸ Queued ▸ Engine/Runtime; `planning/features/real_pause.md`; `backlog.md:21`

Every shipped "pause" is an overlay: NPC AI, physics, `tick_delayed_events_system` (`lib.rs:785`,
no pause gate) and interact/loot keep running. In `primitive_world`, health reaching 0 under the
overlay never reaches `game_over` because the transition fires only `from: "playing"`. The docs now
say to "handle these in `global_on`" — a workaround for the missing primitive, not a replacement,
and a designer cannot write the missing piece in RON.

### 3. Capability blocks on player-tagged prefabs are silently dropped
**Source:** `planning/claude_suggestions.md` (debug-detective, `4df567f`, 2026-09-11)

`inventory:`, `interactable:`, `dialogue:`, `behavior:`, `trigger_zone:` work on any NPC or prop and
do nothing on a player prefab, with no runtime warning and no `validate` check. The player path now
shares `assemble_player_config`/`spawn_player_entity_core`, so wiring `attach_prefab_features` in —
or adding a warning plus a CLI error — is a single-site change.

### 4. `validate` skips every action in ActionBar slots
**Source:** `planning/backlog.md` ▸ Queued ▸ UI ("`collect_actions` does not scan `ActionBar` slot `do_actions`", 2026-10-11)

`collect_actions` (`ironhold_cli/src/commands/validate.rs:807-834`) walks the state machine,
behaviors and dialogue choices only. Slot `do_actions` are where designers write combat actions, so
scene-path existence, `item_key`, `spawn_point`, `SetCameraMode` and `SetUiVisible` id checks all
skip them, and `discover_extra_scenes` never parses an overlay opened only from a slot (a false
`set_ui_visible_unknown_id` the designer cannot suppress). `SetUiVisible` already worked around this
with its own slot scan — now the second hand-written action walker.

### 5. UI visibility: overlays can't start hidden, the ActionBar can't be hidden at all
**Source:** `planning/backlog.md` ▸ Queued ▸ UI ("Overlay nodes cannot start hidden", "An `ActionBar` cannot be hidden at runtime", 2026-10-11/12)

The documented start-hidden recipe is `SetUiVisible` on `scene.ready:<scene>`, but overlays never
fire it (`scene_loader.rs:1332` writes `SceneEvent::Ready` only when `!is_overlay`), so pause/menu
overlays can't start with a node hidden and nothing says why. `validate` correctly rejects
`SetUiVisible` on an `ActionBar` (its slots would still fire), leaving cutscenes, menus and
portrait layouts with no way to hide it. Responsive UI phase 3 (orientation swaps via
`SetUiVisible` pairs) depends on both gaps closing.

**Notable new things since last edition:**
- `SetUiVisible` + `UiNodeId` (`d50045d`) follows the pipeline correctly (guarded `Display` write,
  no capability pushing to `ActionQueue`); the empty-id footgun is fixed (`action_executor.rs:812`).
- The flex `Group` and `ProjectConfig.ui_scale` (`b5c1800`) are RON-authorable, and every
  `scene.ui` scan goes through `walk_ui_nodes`, so nested nodes get every validate check.
- After D1 (`9e167ca`), declaration order in `stats.ron` and the string order of bindings/slot keys
  decide same-frame outcomes; designers need to know that.

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

**New top-5:**

### 1. Quest system: core loop (v1) + presentation layer (v2)
**Source:** `planning/backlog.md` ▸ Queued ▸ Gameplay & Environment (L109-110); `planning/features/quest_system.md`

Still the biggest gap between a diorama and a world that remembers what you did: Greywatch's Maren →
Old Key → Seal Door → reward chain is a loose bundle of `GameVariables`. Since the plan was drafted,
item gating (`requires_item`) and dialogue conditions shipped and should be first-class objective
and branch sources, and in co-op `KillCount` needs to know who landed the hit (the queued "Attacker
identity on hit events", L68). The plan needs a refresh pass before coding, not just a pickup.

### 2. Scene lighting that actually takes effect: fix ambient, add fog, one shared `EnvState`
**Source:** self-described, from `planning/features/ocean_simulation_demo.md` ~L348-351 (the plan-review recommended logging it under Bugs; not yet done). Overlaps Day/night (L104) and the ocean "Runtime environment control actions" item (L125).

`scene_loader.rs` ~L2998-3007 spawns `AmbientLight` as an entity; in Bevy 0.18 that is a per-camera
component, and scene-wide ambient is the `GlobalAmbientLight` resource. Checked against code only,
not in a browser, but authored ambient likely does nothing — 37 scenes set `lighting: ambient:`,
including all four `3rd_person_game_demo` scenes. Fog doesn't exist in the schema at all. These are
the two cheapest mood tools there are; fix ambient and add `EnvState` and fog before day/night so
the sun has a single owner from day one.

### 3. Real pause
**Source:** `planning/backlog.md` ▸ Queued ▸ Engine/Runtime (L67); `planning/features/real_pause.md`; Bugs "Dying while paused never reaches game over" (L21)

The overlay freezes nothing; a player pausing to read a dialogue choice can be killed behind the
menu, and in `primitive_world` that death never reaches `game_over`. A safe pause is a basic trust
contract, and every richer world (quests, longer dialogue, inventory screens) makes players pause
more. The plan exists since 2026-09-15 and is scoped.

### 4. Loot system: roll + auto-loot (v1)
**Source:** `planning/backlog.md` ▸ Queued ▸ Gameplay & Environment (L112); `planning/features/loot_system.md`

Corpses are lootable but every drop is hand-authored per prefab, so the second zombie is never more
interesting than the first. All stated dependencies (Inventory, `Spawn.at_entity`) have shipped, and
Loot v1 also unblocks Quest's `Collect` objective.

### 5. Location-aware, reliable audio: sound zones + the WASM autoplay unlock
**Source:** `planning/backlog.md` ▸ Queued ▸ Gameplay & Environment (Sound zones L106, Audio channels L105); Bugs "WASM AudioContext autoplay-unlock is unreliable" (L30)

Greywatch's safe-village vs. tense-wilds contrast exists only visually. Sound zones need little
beyond a fade envelope, and the ocean demo lists them as a soft dependency. Paired with the autoplay
bug because on the web build — the only public build — sound can silently fail even after a real
click (repro in `entity_logic_demo`); zones built on that would be undependable. Unlock first.

*Dropped this time:* Day/night is folded into #2 so the sun has one owner; Save/load is deferred —
it matters once Quest and Loot exist, and should be promoted to Queued then.

**Notable new things since last edition:**
- `SetUiVisible` (`d50045d`) makes progressive/diegetic HUD reveal authorable (note the `ActionBar`
  itself is still excluded).
- Room 3 click targets now have a health/death/respawn loop (`d9f9ae0`) — the first RON-only,
  end-to-end "threat lifecycle" outside `3rd_person_game_demo`.
- Responsive UI phases 1-2 (`b5c1800` + `f2194a3`) put mobile/portrait players in scope, raising the
  stakes on world-space label legibility (open bugs L33-35).

---

## UX-Gamedesigner-Reviewer — designer-authoring experience

*Lens: what silently goes wrong, is hard to discover, or wastes a non-programmer designer's iteration time.*

**Previous top-5, status:** none shipped. October's designer-facing work went to UI layout
(`Group`, `ui_scale`, `SetUiVisible`).

| # | Item | Status |
|---|---|---|
| 1 | The pause menu doesn't pause anything | **Still open, zero movement.** `real_pause.md` still Queued (~L67); the `docs/30` pause example (~L223-257) still has no warning. Led to a new Bug: dying while paused never reaches game over (~L21, `primitive_world/logic/state_machine.ron:130`) |
| 2 | Loading failures hang forever | **Still open.** `project_loader.rs` `Failed` arms (L153-213) cover only catalogs; nothing handles a failed `.project.ron` or reads `UntypedAssetLoadFailedEvent`. `play.html` shows a message (`b5c1800`) only when the engine itself can't start. Bug (~L41) and retry item (~L64) open |
| 3 | Demo projects for core authoring patterns | **Changed: partly moved.** Minimal `ui_demo` shipped (`1167eb9`, Hide/Show stations from `d50045d`). `prefab_demo`, `audio_demo`, `scene_transitions_demo`, `parkour_demo` still unbuilt (~L233-243), three editions running |
| 4 | Action-bar `{target}` becomes `""` | **Still open** (`action_bar.rs:429-444`, 11 arms; no CLI check; ~L39) |
| 5 | `motion:` missing from reference docs | **Still open** (14 uses in 5 projects, zero `docs/` entries, ~L66). Tofu is also unchanged: exactly 20 non-ASCII `text:` strings still ship (`particles_demo` 16, `effect_mayhem_demo` 4), flagged again in the 2026-10-10 baseline review (~L260) |

**New top-5:**

### 1. The pause menu doesn't pause anything, and it now causes soft-locks (carried)
**Source:** `planning/backlog.md` ▸ Queued ▸ Engine/Runtime ("Real pause"); ▸ Bugs ("Dying while paused never reaches game over"); `planning/features/real_pause.md`

Two editions with no change, and the cost is now concrete: the `docs/30` pattern combined with
`from: "playing"`-only transitions leaves `primitive_world` stuck in `playing` at 0 health. The only
warning is buried in the corpse-loot section (`docs/30` ~L675-678), not next to the pause example
people copy. Until real pause ships, a two-line note under the example ("the game keeps running;
handle death and timers in `global_on`") would stop the next designer walking into this.

### 2. Action-bar slots get no action-level validate checks
**Source:** `planning/backlog.md` ▸ UI (`collect_actions` skips ActionBar `do_actions`, ~L262) + ▸ Bugs (`action_needs_target` drift, ~L39)

Most combat is authored in action-bar slots — exactly where `ironhold validate` stops looking
(`validate.rs:807`). An overlay opened only from a slot is never parsed (false
`set_ui_visible_unknown_id`), and `{target}` still quietly becomes `""` for 8 action types.
"validate passed" means least for the content a designer iterates on most, and a broken slot just
does nothing with no event to react to.

### 3. Loading failures still hang the game with nothing on screen (carried)
**Source:** `planning/backlog.md` ▸ Bugs (stale `.project.ron` field hang, ~L41) + ▸ Queued (scene-load retry/timeout, ~L64)

One leftover field in `.project.ron`, or a failed scene fetch, leaves a loading screen that never
finishes with the error only in the browser console. October showed the fix is cheap: `play.html`
already got a "cannot start" message (`b5c1800`); extending it to project/scene load failures turns
a mystery hang into a one-minute fix for someone who only tests in the browser.

### 4. `docs/20` is too big to navigate, and it still has holes
**Source:** `planning/backlog.md` ▸ Designer Experience ("Split `docs/20_data_formats.md`", ~L231; "`docs/20` has no entry for `MotionDef`", ~L66)

~354k characters in 24 sections and still growing (`SetUiVisible` alone added a long subsection,
~L3974). A designer can't skim it, so they copy from demos, and that's where gaps hide. `motion:` is
the clearest case: 14 uses in 5 shipped projects, no reference entry, and a behavior change (now on
the 64 Hz physics tick) nobody would read about. Split into per-topic pages and land the `motion:`
entry as part of it.

### 5. No demo projects for prefabs, audio or scene transitions; README table incomplete (carried, narrowed)
**Source:** `planning/backlog.md` ▸ Designer Experience (~L240-243, ~L230 item 4)

`ui_demo` shows the station-per-concept layout works, but prefabs (the first thing a designer
authors), audio and multi-scene flow are still taught by accident inside other demos. The README
project table (`README.md` ~L131) lists `ui_demo` but has no rows for `particles_demo` or
`local_coop_demo`, two of the best copy-from projects — a five-minute fix. `prefab_demo` is the
highest-value next demo.

**Notable new things since last edition:**
- **Docs audience split** (`c188550`): `docs/README.md` is an audience-tagged index, CLI docs moved
  to `docs/15_authoring_tools.md` (16 previously undocumented validate codes added), developer docs
  moved to `docs/dev/` with redirect stubs — the clearest reading order designers have had.
- The `SetUiVisible` docs (`docs/20` ~L3974-3985) are a model for designer reference text: every
  gotcha stated up front, and `validate` catches typo'd ids.
- Responsive web canvas + `ui_scale` and the `audio_volume_percent` variable (`0142eac`) remove two
  whole classes of hand-built workarounds.

---

## WASM-Perf-Reviewer — browser runtime performance & binary size

*Lens: frame-time impact, allocation/GC pressure, binary-size trajectory, first-load/first-frame stalls.*

**Binary size check-in:** `pkg/ironhold_web_bg.wasm` = 32,161,056 bytes (~30.7 MiB), ~63 MB below
the 95 MB warn line and ~68 MB below GitHub's 100 MB hard limit, up 71 KB (+0.2%) from the second
edition. **Caveat:** this is the last committed release build (`ebc338c`, 2026-10-10); it predates
the responsive-UI, `SetUiVisible` and room-3 health merges, so HEAD itself is unmeasured. None adds
a dependency, so expect a few KB. Size is not a factor in any item below.

**Previous top-5, status:** none shipped; all five are still open under `backlog.md` ▸ Performance.

| # | Item | Status |
|---|---|---|
| 1 | WASM terrain first-frame stall | **Still open** (`terrain.rs:73` `AsyncComputeTaskPool::get()`, `:108` `block_on(poll_once)`; no `cfg(wasm32)` progressive path) |
| 2 | Pipeline warmup for Text2d / UI / Sprite | **Still open** (`pipeline_warmup_system` queries only `With<Mesh3d>`, `lib.rs:483`); matters more now that `ui_demo` and nested `Group` scenes add first-frame UI |
| 3 | Frozen animation clips evaluated forever | **Still open** (`animation.rs:367` only `pause()`s; `AnimationGraphHandle` never dropped) |
| 4 | Scene transition material cache | **Still open** (`scene_loader.rs:178-189` clears `mats.built` and rebuilds every catalog material per `LoadScene`) |
| 5 | Remaining per-frame allocations | **Still open** (stat-key Vec clone at `stats.rs:19` and `:118`; `format!` before guard at `stat_display.rs:162/227/438`, `camera.rs:984`); backlog entry still names the deleted `message_interpreter_system` |

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

### 2. WASM terrain generation first-frame stall
**Source:** `planning/backlog.md` ▸ Performance

On WASM the async compute pool runs on the main thread, so `block_on(poll_once)` builds the whole
heightmap mesh in one frame — an estimated 100–500 ms freeze at the moment a player forms a first
impression, contradicting the crate rule "never block the main thread". Fix: a `cfg(wasm32)` path
building a few tiles per frame, or pre-baked GLB terrain.

### 3. Extend pipeline warmup to Text2d / UI / Sprite
**Source:** `planning/backlog.md` ▸ Performance; `planning/features/pipeline_warmup_2d_ui.md`; `planning/claude_suggestions.md` (Sprite warmup)

`pipeline_warmup_system` (`lib.rs:480-503`) only touches `Mesh3d`; on WebGPU each pipeline compiles
synchronously on first draw, so first text/UI/sprite render can stall an estimated 100–300 ms. The
`Textured` `Sprite` stat bars on `3rd_person_game_demo` players are still unwarmed, and October's UI
work (nested `Group`, `ui_demo`, `SetUiVisible` revealing nodes later) adds surfaces that first draw
after the warmup window closes.

### 4. Paused/frozen animation clips are fully evaluated forever
**Source:** `planning/backlog.md` ▸ Performance (promoted 2026-09-14)

`freeze: true` only pauses the clip; bevy_animation 0.18 still samples every curve and writes every
bone `Transform` each frame — an estimated 0.2–0.5 ms/frame for six 42-joint corpses over their
300 s lifetime, with `par_iter_mut` serial on WASM. The smallest diff on the list: drop
`AnimationGraphHandle` once frozen, keeping `ActiveOverride.frozen` durable for the WASM GLTF-respawn
recovery path. Room 3's death/respawn loop (`4288024`) keeps adding corpses.

### 5. Warmup must only strip the `NoFrustumCulling` it added (blocker for the ocean demo)
**Source:** `planning/features/ocean_simulation_demo.md` finding B1; `lib.rs:494-497`; ocean backlog section

When the countdown hits 0, `pipeline_warmup_system` removes `NoFrustumCulling` from every entity
that has it, not only those it inserted. The planned ocean grid needs it permanently (its vertex
shader displaces outside the flat-grid AABB, so wave crests would pop out as the camera pitches).
Fix: a `WarmupNoCull` marker, removing NFC only where present — before ocean v1, together with the
plan's other WASM gates: measure the 192×192 grid on an integrated GPU (~37k vertices × 8 `sincos`),
sort buoyancy by `SpawnId` in a reused `Local<Vec>`, keep the ~336 B all-`Vec4` uniform, and upload
it only on change.

*Demoted but still open:* material cache (was #4) and the per-frame allocation trio (was #5) — neither
got worse and both are smaller than the items above.

**Notable new things since last edition:**
- `ui_scale_system` (`capabilities/ui_scale.rs:75-118`) runs every frame but is a few float ops plus
  an O(1) lookup, with an epsilon-guarded `UiScale` write — web-clean.
- `SetUiVisible` (`d50045d`) runs per action, not per frame, with a guarded `Node.display` write and
  no new pipeline — no idle cost.
- The responsive-canvas pixel cost (#1) is the first new per-frame regression risk in two editions
  and the one thing to address before the next `/release`.

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
  `planning/backlog.md`.
- **Planning hygiene:** the backlog entry for per-frame allocations still names the deleted
  `message_interpreter_system`; `quest_system.md`, `loot_system.md`, `day_night_cycle.md` and
  `save_load_game_state.md` have all been Draft since 2026-06-02 and predate shipped dependencies.
- **Agent memory:** all five reviewers updated their `.claude/agent-memory/` files this round; those
  changes are uncommitted in the primary checkout and should be committed separately per the code
  change workflow.
