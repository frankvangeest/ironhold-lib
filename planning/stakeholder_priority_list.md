# Stakeholder Priority List

**Snapshot as of `ce77bdd` (2026-10-01).** Second edition. This is a point-in-time snapshot, not a
living/re-generated document — priorities will drift as the codebase changes, so treat this as a
read of that moment, not an ongoing source of truth. Re-run manually later if a fresh read is
wanted. The first edition (`db1ede0`, 2026-09-03) is preserved in git history
(`git show 8a8176c:planning/stakeholder_priority_list.md`).

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
warranted, promoted or acted on independently of this snapshot.

## What changed since the first edition

The September 2026 batch shipped a large share of the first list: `Action`/FSM/dialogue
`deny_unknown_fields`, the `rules.ron` → `state_machine.ron` consolidation, a fixed physics timestep,
about ten `ironhold_cli validate` hardening items, item-gated interactables, per-player action bars,
gamepad action-bar slots and (30 Sep) mouse-click action-bar slots. Two new areas opened up: the
**Ocean Simulation Demo** section (eight queued items, 30 Sep) and the first **UI-pointer capture**
bug (found by the mouse-click playtest). Several items below are the same ones as last time — those
are the ones nobody has had room for yet.

---

## System-Architect — stability, maintainability, future-proofing

*Lens: what would most hurt this codebase's trajectory if ignored for another 6 months.*

**Previous top-5, status:**

| # | Item | Status |
|---|---|---|
| 1 | Rapier cross-platform float divergence | **Changed.** Cause was already corrected (2026-09-15: `enhanced-determinism` was on all along); the real blocker, the variable timestep, shipped as `deterministic_fixed_timestep` v1 (`6f720de`, merge `c3ec4fe`). v2 (the cross-platform determinism harness) is still Queued — the remaining work is measurement |
| 2 | `Action` `deny_unknown_fields` | **Shipped** (merge `33842ff`; `features/done/action_deny_unknown_fields.md`) |
| 3 | Scene-singleton camera/input config on `PrefabDef` | **Still open** (Icebox; `camera_modes` map exists, but per-player `camera_mode`/`camera`/`flycam`/`split`/`party` are still `PrefabDef` fields) |
| 4 | Test-suite trust | **Changed.** The flaky test is gone (targeting race fixed, merge `80f5ab1`; local-coop flake closed as non-reproducing). Structural half still open: "no warning was logged" infra, ambiguity detection, schedule-graph assertions, no `SystemSet`s |
| 5 | `spawn_scene_v2` at the 16-param ceiling | **Still open** (still exactly 16 params; file now ~3,570 lines) |

**New top-5:**

### 1. Build the determinism harness before the ocean/buoyancy physics batch lands
**Source:** `planning/backlog.md` ▸ Beta 0.5 ▸ Cross-platform determinism harness; `planning/features/deterministic_fixed_timestep.md` (v2 Queued)

Fixed-timestep v1 removed the cause we knew about, but nobody has measured whether native and WASM
actually agree tick by tick. Eight ocean-demo items were queued on 2026-09-30, including buoyancy
forces, a wind field and a boat controller — the largest batch of physics-adjacent code since the
last snapshot, and it would land before anything can detect a divergence. If the harness comes
first it's a regression gate; if it comes after, it's an archaeology dig.

### 2. A declared schedule-ordering contract instead of bare `.before(fn)` edges
**Source:** `planning/backlog.md` ▸ Queued ▸ Engine/Runtime ("Schedule-graph assertions + first named `SystemSet`", "Bevy ambiguity-detection hardening"); `planning/claude_suggestions.md` ▸ Targeting

The flaky test was fixed, but nothing would catch the next ordering race: there are no `SystemSet`s,
no ambiguity detection, and the canary test for this was removed. WASM is single-threaded, so an
ordering race can behave differently on web than natively. One `PipelineSet::PreInterpreter` plus
ambiguity detection in debug/test builds would turn ordering from a convention into a guarantee.

### 3. One input-ownership primitive: UI pointer capture vs. world input
**Source:** `planning/backlog.md` ▸ Bugs ("Left mouse button on any UI node also orbits the camera / strafes the character")

Clickable action bars made a long-standing gap obvious: camera orbit and strafe read raw mouse state
with no UI guard, and only `click_select_system` checks for a pressed UI node. Each new clickable
widget currently gets an ad-hoc guard, or none. The coming work (draggable windows, AoE placement, a
slider panel) will all compete for the pointer; a single `UiPointerCaptured` resource read by every
world-input system should exist before that work starts.

### 4. Loader failures must be visible, not infinite loading screens
**Source:** `planning/backlog.md` ▸ Bugs (stale `rules_path:` hangs loading forever) and ▸ Queued (no retry for a failed `.scene.ron` load); `.claude/agent-memory/system-architect/schema_tightening_blast_radius.md`

Tightening the schema was the right call, but each time it happens a field that used to be silently
ignored now fails the whole file, and the runtime responds by hanging. Players see a blank loading
screen, not the CLI's clear error. Every loader needs a `Failed` arm that latches and shows the
error on screen.

### 5. Break up the scene_manager monolith before the 16-param wall breaks the build
**Source:** `.claude/agent-memory/system-architect/fragile_modules.md`; `planning/backlog.md` ▸ Queued ▸ UI ("Extract a shared panel-chrome helper")

`spawn_scene_v2` is still at exactly 16 params and the scene_manager files add up to about 9,100
lines. The three panel-spawning copies (inventory, shop, container) are a cheap first extraction;
waiting until a feature needs a 17th param means doing it under deadline pressure.

---

## Alignment-Reviewer — RON designer-reachability, no hardcoded behavior

*Lens: what a designer currently can't do through RON alone, or where behavior silently diverges by authoring path.*

**Previous top-5, status:**

| # | Item | Status |
|---|---|---|
| 1 | RON-authorable collider friction / physics materials | **Still open** (hardcoded at `entity_spawner.rs:328`/`:1140`, `scene_loader.rs:466`/`:686`; the wall-friction fix added another engine rule rather than a schema field) |
| 2 | `Action` `deny_unknown_fields` | **Shipped** (`3677859`, 2026-09-04) — went further than asked: all FSM and dialogue containers, plus `LoadState::Failed` arms now log errors |
| 3 | Behavior `entry_actions` never get `{target}` | **Changed — closed, premise was wrong** (substitution was already applied since `4e692db`; `rewrite_self`/`rewrite_target` are now exhaustive matches, so a missing arm is a compile error) |
| 4 | CLI `collect_actions` ignores dialogue `do_actions` | **Shipped** (`63b31e6`, 2026-09-04; `query`/`stats` still ignore dialogue actions) |
| 5 | Magic `tags` strings drive spawn semantics | **Still open** (drops out of the new top-5 only because worse silent divergences surfaced) |

**New top-5:**

### 1. `action_needs_target`'s hand-maintained allowlist silently turns `{target}` into `""`
**Source:** `planning/backlog.md` ▸ Bugs (split out 2026-09-14, flagged by all 3 reviewers on `targeting_race_fix`)

`capabilities/action_bar.rs` still lists only 11 variants while `rewrite_target` substitutes many
more. With no target set, `SetVariable`, `OpenContainer`, `AddItem`/`RemoveItem`/`TransferItem`,
`ResetToSpawn`, `EmitEventAfterDelay` and `Spawn.id`/`spawn_point` all fire against an empty id,
with no `no_target` event and no warning. Since `rewrite_target` became exhaustive this is the last
hand-synced copy of that list. The fix: derive "needs a target" from `rewrite_target` itself.

### 2. Real pause: designers cannot pause the world through RON
**Source:** `planning/backlog.md` ▸ Queued ▸ Engine/Runtime; `planning/features/real_pause.md`

Every shipped "pause" is an overlay drawn over a game that keeps running — NPC AI, physics, timers,
interacts. No action exists that a designer could author to stop it, and the docs currently teach
the overlay pattern as the standard way to pause. A missing engine primitive, not an authoring
mistake: no amount of RON can work around it.

### 3. Player-tagged prefabs silently ignore `inventory:`/`interactable:`/`dialogue:`/`behavior:`/`trigger_zone:`
**Source:** `planning/claude_suggestions.md` (debug-detective, `4df567f`, 2026-09-11)

`spawn_player_entity_core` never calls `attach_prefab_features`, so the same capability block works
on any NPC or prop and does nothing on a player prefab — with no warning and no CLI check. Behavior
differs depending on which kind of prefab the designer wrote the block on.

### 4. Dialogue `do_actions` get `{self}` but never `{target}`
**Source:** `planning/claude_suggestions.md` (system-architect, 2026-09-28); agent-memory `dialogue_system_pattern.md`

`dialogue.rs` has its own `substitute_self_in_action`, a third copy of the substitution logic that
skips `action_substitution.rs`. The same `{target}` token resolves in `state_machine.ron` and
behavior files but stays a literal string inside a dialogue choice — and it is the next place a new
`Action` variant's substitution arm will be forgotten.

### 5. RON-authorable collider friction / physics materials (carried over)
**Source:** `planning/backlog.md` ▸ Queued ▸ Engine/Runtime

Slippery ice or sticky mud still cannot be expressed without a Rust change. Every new physics bug in
this area keeps adding engine constants instead of exposing a field.

---

## Game-World-Designer — missing/unstable features, world-building & player experience

*Lens: what kind of game world or moment-to-moment experience is currently impossible or fragile to build.*

**Previous top-5, status:**

| # | Item | Status |
|---|---|---|
| 1 | Quest system (v1 + v2) | **Still open** (both bullets unchecked; the `Collect` objective still waits on Loot v1) |
| 2 | Item-gated interactable | **Shipped** (`features/done/item_gated_interactable.md`; wired into Greywatch's seal door — `requires_item: "old_key"`, `entity.interact_blocked:seal_door`) |
| 3 | Sound zones | **Still open** (Audio channels also still Queued) |
| 4 | Day/night cycle | **Still open, context changed** — the new Ocean Demo item "Runtime environment control actions + light parameters" adds `SetSunDirection/Intensity/Color`, `SetAmbientLight`, `SetFog` and must coordinate with `TimeOfDay`; the ocean work may land lighting setters first |
| 5 | Loot system v1 | **Still open, partly superseded** — monster corpse loot (v2 separate-corpse design, `Spawn.at_entity`, `ec3cb5e`) and the inventory `max_stack` fix shipped; there are still no rolled loot tables |

**New top-5:**

### 1. Quest system — core loop (v1) + presentation layer (v2)
**Source:** `planning/backlog.md` ▸ Queued ▸ Gameplay & Environment

Still the single largest gap between a diorama and a world that remembers what you did. The Seal
Door now gates properly on possession, but nothing ties "Maren asked you for the key" to "you opened
the door" to "come back for your reward" — each promise is still a loose bundle of `GameVariables`.
Every designed world needs a visible throughline, and this is the only item that provides one.

### 2. Loot system — roll + auto-loot (v1)
**Source:** `planning/backlog.md` ▸ Queued ▸ Gameplay & Environment

Corpses can now be looted, but every drop is hand-authored per prefab, so danger has no variance —
a second zombie is never more interesting than the first. Rolled tables are what make scavenging
off the critical path worth the risk, and Loot v1 is also still the dependency blocking Quest's
`Collect` objective, so it unlocks item 1 as well.

### 3. Sound zones
**Source:** `planning/backlog.md` ▸ Queued ▸ Gameplay & Environment

Greywatch's design depends on a temperature gradient — the village feels safe, the wilds feel tense
— but today that gradient exists only visually. Carried over unchanged: cheap, needs nothing beyond
a fade envelope, and the Ocean demo now lists it as a soft dependency for wind and wave ambience.

### 4. Day/night cycle
**Source:** `planning/backlog.md` ▸ Queued ▸ Gameplay & Environment; overlaps the Ocean "Runtime environment control actions" item

A world fixed at one sun angle can't show that time passes or that danger rises after dark; the
`time.dusk` hooks are what make this a design tool rather than a shader trick. Plan it together with
the Ocean demo's light setters now, before two systems fight over the sun.

### 5. Save / load game state
**Source:** `planning/backlog.md` ▸ Icebox ▸ Engine / Runtime; `planning/features/save_load_game_state.md`

As soon as quests and loot exist, a world that forgets everything on a page reload breaks the
promise those features make — most sharply on WASM, where a closed tab is the normal way a session
ends. Sits in the Icebox today; recommended for promotion to Queued directly behind Quest v1 so the
first real questline isn't throwaway.

---

## UX-Gamedesigner-Reviewer — designer-authoring experience

*Lens: what silently goes wrong, is hard to discover, or wastes a non-programmer designer's iteration time.*

**Previous top-5, status:**

| # | Item | Status |
|---|---|---|
| 1 | RON typos silently no-op | **Shipped** (`3677859`; failed loads of logic, `.behavior.ron` and `.dialogue.ron` now log an `error!` with the file path). The `rules.ron` removal also shipped (CLI is now `query logic`) |
| 2 | "validate passed" ≠ "will work" | **Changed — mostly closed** (~10 CLI-validate Done entries in September). Still open: leftover `logic/rules.ron` never flagged, no ContainerPanel-slots vs. `max_slots` check, no CLI guard for the `{target}` gate |
| 3 | Missing demo projects | **Still open, zero movement** (`prefab_demo`, `ui_demo`, `audio_demo`, `scene_transitions_demo`, `parkour_demo` — none exists) |
| 4 | RON parse footguns | **Still open** (only the camera_mode double-paren note exists; no general RON-syntax primer, no quoted-vs-bare rule) |
| 5 | Tofu boxes for non-ASCII text | **Still open (lint only)** — the `--strict` `non_ascii_char_in_text` lint shipped, but all 20 shipped tofu strings are still there (16 in `particles_demo`, 4 in `effect_mayhem_demo`) and the font is still ASCII-only |

**New top-5:**

### 1. The pause menu doesn't pause anything, and the docs teach it as the way to do it
**Source:** `planning/backlog.md` ▸ Queued ▸ Engine/Runtime ("Real pause"); `planning/features/real_pause.md`; agent-memory `project_pause_is_cosmetic.md`

Every designer copies the pause example in `docs/30` (~L224-256). Behind the menu, monsters keep
attacking, loot still works and timers keep firing; `primitive_world`'s game-over screen can even
be played behind. The engine gives no warning, and the bug shows up first for players, not for
whoever built the pause menu.

### 2. Loading failures hang the game forever with nothing on screen
**Source:** `planning/backlog.md` ▸ Bugs (stale `rules_path` hang) + Queued (scene-load retry/timeout)

One leftover field in `.project.ron`, or a failed fetch of the scene file, leaves a loading screen
that never finishes; the only error is in the browser console. For someone testing only through the
web build that looks like a crash with no cause. An on-screen error message would turn "it's
broken" into a fix that takes a minute.

### 3. Demo projects for the core authoring patterns (carried over)
**Source:** `planning/backlog.md` ▸ Queued ▸ Designer Experience

Two cycles have passed with no movement. Prefabs, UI and audio are still only taught by accident
inside demos about other systems — still the cheapest way to cut a designer's time to their first
working project.

### 4. An action-bar `{target}` becomes an empty string with no warning
**Source:** `planning/backlog.md` ▸ Bugs (`action_needs_target` allowlist drift)

`SetVariable`, `OpenContainer`, `AddItem` and five other actions skip the "no target" check; with
no target selected they quietly write to or open an empty id. The September targeting fix made this
happen every time instead of occasionally. The designer sees nothing happen and gets no event to
react to. *(Same defect as Alignment-Reviewer #1, seen from the designer's side.)*

### 5. `motion:` is used in shipped projects but appears nowhere in the reference docs
**Source:** `planning/backlog.md` ▸ Queued ▸ Engine/Runtime ("docs/20 has no entry for MotionDef")

Five shipped projects use `motion:` (`particles_demo`, `entity_logic_demo`, `stats_demo`,
`primitive_world`, `effect_mayhem_demo`), yet `docs/20_data_formats.md` has no entry for it — and
its timing changed to the fixed 64Hz physics tick, which affects how motion looks on fast
displays. Designers can only learn it by copying a demo.

---

## WASM-Perf-Reviewer — browser runtime performance & binary size

*Lens: frame-time impact, allocation/GC pressure, binary-size trajectory, first-load/first-frame stalls.*

**Binary size check-in:** `pkg/ironhold_web_bg.wasm` (release build, 2026-09-29) = 32,090,055 bytes
(~30.6 MiB), ~64 MB below the 95 MB warn line and ~69 MB below GitHub's 100 MB hard limit, and
already under the 50 MB "optimal" target. Size is not a factor in any item below.

**Previous top-5, status:** none shipped; September went to CLI hardening, the logic consolidation,
the fixed timestep and action-bar features.

| # | Item | Status |
|---|---|---|
| 1 | WASM terrain first-frame stall | **Still open** (`terrain.rs:73` `AsyncComputeTaskPool`, `:108` `block_on(poll_once)`; no `cfg(wasm32)` progressive path) |
| 2 | Per-frame collection allocations | **Changed — partly gone.** `message_interpreter_system` was deleted by the rules consolidation (its Vec with it); `stats.rs:19` stat-key Vec and `player.rs:453` input HashMap remain. The backlog wording still names the deleted system |
| 3 | Scene transition material cache | **Still open** (`scene_loader.rs:174-185` clears and rebuilds every catalog material on each `LoadScene`) |
| 4 | Frozen animation clips evaluated forever | **Still open, now logged** (promoted to backlog 2026-09-14; `animation.rs:366-367` only pauses, never drops the `AnimationGraphHandle`) |
| 5 | `format!` before the change-detection guard | **Still open** (`stat_display.rs:162/227/438`, `camera.rs:977`) |

**New top-5:**

### 1. WASM terrain generation first-frame stall
**Source:** `planning/backlog.md` ▸ Performance

On WASM the async compute pool falls back to running on the main thread, so a large heightmap
freezes the first frame for 100–500 ms. Every session that opens a terrain project hits it; nothing
else in the repo costs as much at the moment a player forms a first impression, and there is still
no fallback that builds the terrain gradually.

### 2. Extend pipeline warmup to Text2d / UI / Sprite
**Source:** `planning/backlog.md` ▸ Performance ("Extend pipeline warmup to Text2d and UI pipelines"); `planning/claude_suggestions.md` (`pipeline_warmup_system` / `Sprite`)

`pipeline_warmup_system` still only queries `With<Mesh3d>` (`lib.rs:449`). On WebGPU pipelines
compile synchronously on first draw, so the first text, UI or sprite render can stall 100–300 ms.
More likely in practice than last time: `3rd_person_game_demo` players now carry `Textured` `Sprite`
bars, so the risk applies to a shipped project, not a hypothetical one.

### 3. Paused/frozen animation clips are fully evaluated forever
**Source:** `planning/backlog.md` ▸ Performance (promoted 2026-09-14)

`freeze: true` only pauses the clip; bevy_animation 0.18 still samples curves and writes every bone
`Transform` each frame — about 0.2–0.5 ms/frame for 6 corpses over their 300 s lifetime, and
`par_iter_mut` is serial on WASM. The cheapest fix on this list (drop `AnimationGraphHandle` once
the pose is frozen), and the cost grows with every new corpse or prop-freeze pattern.

### 4. Scene transition material cache
**Source:** `planning/backlog.md` ▸ Performance

Every `LoadScene` throws away and rebuilds every material in the catalog — an estimated 50–200 ms
hitch each time a player goes through a portal, growing with catalog size, which is the wrong
direction for a data-driven engine meant to grow.

### 5. Remaining always-on per-frame allocations (stat key Vec + `format!` before guard)
**Source:** `planning/backlog.md` ▸ Performance (two entries: per-frame collections; `format!` before change-detection guard)

`stat_modifier_system` clones every stat key into a new Vec for every entity every frame, and the
stat-display and target-HUD update systems allocate a `format!` even when the guard then skips the
write (×4 in split-screen). Each is small, together they are a constant allocator cost in every
scene, and a `Local` buffer or computing the string after the guard removes them. While doing this,
update the stale `message_interpreter_system` reference in the backlog entry.

---

## Cross-stakeholder signal

Three items were independently placed in the top 5 by **two** stakeholders each, and one more is
the root of a new bug — the strongest consensus in this snapshot:

- **Real pause** — Alignment-Reviewer #2 (a missing engine primitive no RON can work around) and
  UX-Gamedesigner-Reviewer #1 (the docs teach the non-pausing overlay as the standard pattern). Already
  a logged `planning/backlog.md` ▸ Queued ▸ Engine/Runtime item with `planning/features/real_pause.md`.
- **Loader failures must be visible, not infinite loading screens** — System-Architect #4 and
  UX-Gamedesigner-Reviewer #2. Logged as a Bug (stale `rules_path` hang) plus a Queued retry/timeout item.
- **`{target}` silently becomes `""` for actions outside `action_needs_target`'s allowlist** —
  Alignment-Reviewer #1 and UX-Gamedesigner-Reviewer #4, the same defect seen from the engine side and
  from the designer side. Logged as a Bug; the suggested fix is deriving the check from `rewrite_target`.
- **UI pointer capture** — System-Architect #3, rooted in the new Bug logged by the mouse-click
  playtest; likely to be needed before draggable windows, AoE placement and the Ocean demo's slider panel.

The first edition's consensus item (`Action` `deny_unknown_fields`) shipped on 2026-09-04.

## Also worth flagging

- **Planning hygiene:** `planning/backlog.md` `## Active` still lists the `rules.ron` consolidation
  bullet even though its Done entry exists (System-Architect).
- **Sequencing:** the Ocean Simulation Demo batch touches physics (buoyancy), lighting (Runtime
  environment control actions, overlapping Day/night) and pointer input (slider panel) — the
  determinism harness, the Day/night plan and a `UiPointerCaptured` primitive are all best settled
  before it starts.
