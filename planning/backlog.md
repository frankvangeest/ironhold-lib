# Backlog

> **How this works**
> - Items progress: `Icebox → Queued → Active → Done`
> - Simple items live here as bullet points. Anything needing design lives in `features/`.
> - This file tracks *what to build next*, not *how* — keep it skimmable.
> - Roadmap and milestone gates: see `docs/50_roadmap_and_milestones.md`
> - Implementation status: see `docs/STATUS.md`

---

## Active

- [x] **Split-screen: remaining single-camera assumption sites — Phase 1 (particle billboard orientation)** — `rebuild_pool_meshes_system`'s `camera_q.single()` had no `is_active` filter, so it fell back to unconditional world-axis billboarding in every split-screen scene (not just an edge case — the widest-reaching of the four sites). Fixed: filters `is_active`, picks the highest-priority active camera via a new shared `camera_priority_key` helper (`capabilities/camera.rs`), which `world_label_screen_pos_system` was also refactored to use. Playtest confirmed by Frank in `local_coop_demo` room3 (new `billboard_test_spark` effect, added for this purpose) — `aa81cbb`. See `planning/features/split_screen_camera_followups.md`.
- [x] **Split-screen: remaining single-camera assumption sites — Phase 2 (`targeting.rs` viewport-aware click-to-select)** — `click_select_system`'s `cameras.iter().find(|c| c.is_active)` picked an arbitrary active camera regardless of cursor position. Fixed: filters to `is_active` cameras whose `logical_viewport_rect()` contains the cursor, then picks via `camera_priority_key` (reused directly from Phase 1). Playtest confirmed by Frank in `local_coop_demo` room3 (new `click_target_test` sphere per viewport) — clicking either sphere correctly selects it, clicking ground clears the target, no console errors — `1c3b910`. See `planning/features/split_screen_camera_followups.md`.
- [x] **Split-screen: remaining single-camera assumption sites — Phase 3 (`nameplate_visibility_system` distance-culling, store-and-read)** — `nameplate_visibility_system`'s `camera_q.single()` had no `is_active` filter, so it silently no-op'd whenever 2+ `Camera3d` entities existed at all. Fixed: `world_label_screen_pos_system` (which already selects one active, viewport-tested camera per `WorldLabel`) now also stashes that camera's distance onto a new `NameplateCameraDistance` component; `nameplate_visibility_system` reads it instead of independently re-selecting, so the two systems can never disagree. Playtest confirmed by Frank in `local_coop_demo` room3 (new nameplate on the existing `click_target_test` props) — both nameplates show correctly, no console errors — `d00c5f7`. See `planning/features/split_screen_camera_followups.md`.
- [x] **Split-screen: remaining single-camera assumption sites — Phase 4 (`WorldLabelRank` extended to stat labels / world stat bars)** — `stat_label`/`Ascii`-style `world_stat_bar` spawn loops (scene-load and `Action::Spawn`/wave-spawn paths) had no rank duplication, so a widget simultaneously visible in 2+ active split viewports only showed in one. Fixed: same `WorldLabelRank` pattern as the `world_labels:`/`label:` fix, but gated on the scene actually being split-screen (these widgets rewrite every frame regardless of `Visibility`, unlike static label text) — ordinary scenes are unaffected. `Pixel`-style bars, damage popups, and nameplate anchors remain single-instance (documented limitation). Playtest confirmed by Frank in `local_coop_demo` room3 (new `stat_widget_test` prop, scene-placed + dynamically spawned) — both viewports show both spheres' label + bar correctly, no console errors — `ee3fdf8`. **Final phase — feature moved to `planning/features/done/split_screen_camera_followups.md`.**
- [x] **Per-player targeting for split-screen (Phase 1: selection & display)** — see `planning/features/per_player_split_screen_targeting.md`. `CurrentTarget` was one shared global resource; player 1 and player 2 fought over the same target in split-screen. Fixed: new `PlayerTarget` component added alongside `CurrentTarget` (not replacing it), so `{target}` substitution and the action bar's cost gate stay unchanged — only click-select/Tab-cycle/the target indicator ring/a new opt-in `target_hud:` per-viewport readout became per-player. Playtest confirmed by Frank in `local_coop_demo` room3 (distinct `target_next` keys per player, `target_indicator:`/`target_hud:` blocks, a `target_display`-bound Label proving the legacy var blanks with 2+ players) — no console errors — `e677921`. Phase 2 (per-player action-bar execution) stays deferred/not yet started.

---

## Bugs

- [x] **Portal room-name labels render static and mis-positioned in every split-screen room** — `world_label_screen_pos_system` (`lib.rs:508`) required exactly one `Camera3d` (`camera_q.single()`); every split-screen scene has 2+ (room5's dynamic split always has 3 — two split cameras plus the party camera, regardless of which is active; room6's grid split has 4). The `.single()` call failed every frame, so the system returned early and the label's `Transform` never updated past its default spawn value. Fixed: `world_label_screen_pos_system` now queries every active `Camera3d` and picks the one whose own viewport rect contains the projected point (deterministic order). A second playtest round found the same portal simultaneously visible in 2+ active split viewports (e.g. player 1 approaching the portal where player 2 stands) only showed the label in one viewport — fixed via a new `WorldLabelRank` component so both the scene-level `world_labels:` and per-entity `label:` spawn paths duplicate one sibling per possible active-camera rank. See `planning/features/done/world_label_split_screen_positioning.md`.
- [x] **`npc_revive` stop-action sentinel leaks into clip pipeline** — `PlayAnimationOn(clip: "npc_revive")` fired by enemy behavior on `alive` entry reached the raw-clip-name branch of `animation_resolver.rs` on fresh spawns (no active override → stop-action check was skipped), setting `controller.current = "npc_revive"` and triggering two fallback paths per enemy per spawn. Fixed: sentinels are now intercepted before the raw-clip branch and always dropped.
- [ ] **Stale `EmitEventAfterDelay` fires after entity state exit** — `enemy_spider` 'dead' state schedules `spider.hide:{self}` at 15s; if the spider respawns before 15s elapses the pending delay fires and hides the now-alive spider. Root cause: delay system has no cancel/guard on state transition. Reproduce: kill a spider in `3rd_person_game_demo` and wait for respawn within 15s.
- [ ] **uphill jump lock** — when jumping against an uphill slope, the player can land in a state where `jump` never re-triggers: the character controller reports ground contact but the slope normal keeps the jump cooldown active. Suspected cause: Rapier's ground-contact normal threshold in the character controller or the jump cooldown not resetting when sliding contact ends. Reproduce: 3rd_person_game_demo, run toward any hill and spam jump while ascending.
- [x] **composite prefab child positions and physics wrong for nested Actor/Prop** — Root cause: Rapier reads `GlobalTransform` before `TransformPropagate` runs; a Bevy-parented child entity's `GlobalTransform` at that moment equals its local offset, not the world position — Rapier locked the `Fixed` body there permanently. Fix: `spawn_primitive_children` computes `world_child_tf = parent_world_tf.mul_transform(child_tf)` and spawns nested Actor/Prop entities as root-level entities (no `add_child`) at the composed world position; root entities satisfy `GlobalTransform == Transform` from frame 1. `TriggerZone` sensor child entities had `Visibility::default()` removed to eliminate visibility propagation overhead (was causing frame stutter).
- [ ] **Frame/audio stutter worsens on camera movement (WASM release)** — periodic stall visible in `primitive_world` release build; intensifies when moving the camera. Suspected cause: WebGPU synchronous pipeline compilation stalls when new mesh+material combinations enter the frustum for the first time. `pipeline_warmup_system` (4-frame `NoFrustumCulling` pass) covers scene-loaded entities but may not cover all variants or dynamically entering geometry. Reproduce: `primitive_world`, walk around for 10–20 seconds and observe frame hitches.
- [x] **Overlay Backdrop lacks `FocusPolicy::Block` — click-through only incidentally prevented** — The "Overlay Backdrop" full-screen node (`scene_loader.rs` ~line 1200, spawned by `LoadSceneOverlay`) had `Node` + `GlobalZIndex(100)` + `OverlayEntity` but no `Interaction`/`FocusPolicy::Block`; `Node`'s `FocusPolicy` defaults to `Pass`, so `ui_focus_system` did not actually stop at the backdrop — it only "worked" because tested overlays (pause/options) visually cover the whole base scene. Same root cause as the panel click-through bug fixed in `ec84bcf`. Fixed: added `bevy::ui::FocusPolicy::Block` + `Interaction::default()` to the backdrop's component tuple, matching the panel-root pattern. Regression tests in `tests/ui_panel_blocker.rs`.
- [x] **Character-select player nameplate ignores `show_nameplates`** — `action_executor.rs:163` (`Action::Spawn` for a player-tagged prefab, the character-select flow) gated `nameplate_display_name` with only `prefab.nameplate != Some(false)` — it never checked `scene.show_nameplates`. Fixed: the player now has its own independent `show_player_nameplate` control instead of inheriting `show_nameplates` at all — see `planning/features/done/player_nameplate_visibility.md` (v1).

---

## Queued

### Engine / Runtime

- [ ] **Static scene mode (`?static=1`)** — freeze all time-driven systems (animations, NPC AI, motion, particles) immediately after `SceneEvent::Ready` so browser screenshot baselines are pixel-identical across runs. Mechanism: parse `?static=1` URL param in the WASM runner → `StaticMode(bool)` resource → pause `Time<Virtual>` + seek all `AnimationPlayer`s to t=0 on scene ready. Requires `start_app` signature change (all three crates) and a one-line change to `test_web.py`. See `planning/features/static_scene_mode.md`.
- [ ] **Promote magic `tags` to typed prefab fields** — add `collectable: bool`, `player: bool`, and `flycam: bool` as `#[serde(default)]` fields on `PrefabDef`; `tags` remains for free-form designer labels but control-flow semantics move to typed fields; consistent with the `PrefabKind` enum casing work that cleaned up `kind`. Additive, no migration required.
- [ ] **Page visibility / focus-loss handling** — freeze delta time, pause audio, and drop render to zero when the browser tab loses focus; resume cleanly on tab restore without physics or audio desync; wire Bevy's `WindowFocused` / `ApplicationLifetime` events behind a `pause_on_focus_loss: bool` field on `ProjectConfig` (default `true`); opt-out lets streaming / spectator scenes keep running. Sourced from Phaser's focus-loss model.
- [ ] **Optional `physics` Cargo feature** — gate Rapier3D behind a `physics` feature on `ironhold_core` so projects that don't use colliders skip the ~15 MB of Rapier symbols in the WASM binary; `ColliderDef` in RON becomes a validated-but-no-op field when the feature is absent; `PhysicsPlugin` conditionally compiled; `ironhold_web` enables `physics` by default but a future stripped build could omit it. Sourced from Phaser's Arcade vs Matter modular physics model.

### Camera
- [ ] **Per-player keyboard camera pivot for split-screen** — `camera_orbit_system`'s yaw/pitch are mouse-only (`orbit.yaw`/`orbit.pitch` change only via `MouseMotion` gated by `orbit_lmb`/`orbit_rmb`), and split-screen scenes deliberately set `orbit_button: "None"` per player (one shared mouse can't drive 2+ simultaneously active `OrbitCamera`s independently — see `crates/ironhold_core/src/CLAUDE.md` ▸ Local co-op). Character movement doesn't rotate the camera either (`camera_orbit_system` never reads player facing). Net effect: once in split-screen, no player can look around at all — camera angle is frozen at whatever yaw/pitch it spawned with. Add a keyboard-bound turn-left/turn-right (and optionally pitch) input per player's own control scheme (reusing the same per-scheme-key pattern Stage 6 used for the 4th player's Numpad scheme), wired to that player's own `OrbitCamera.yaw`/`.pitch` only — never the other players'. _Surfaced 2026-07-11 during the split-screen particle-billboard-orientation playtest, when manual camera orbiting turned out to be impossible to use for visually confirming the fix in a 2-viewport scene._
- [ ] **Camera mode unification (v1)** — unify `OrbitCamera` and `FlyCamera` under a single `ActiveCameraMode` resource; backward-compat mapping for existing `camera:`/`flycam:` prefab fields; no new designer-facing surface, but de-risks `CameraShake` re-homing and the persistent-camera/black-frame issue. Local co-op (2026-07-04) added a third sibling, `PartyOrbitCamera`, with its own duplicated tuning fields/mouse-orbit block; Stage 3 (2026-07-05) uses real `OrbitCamera`s for split-screen but adds `SplitViewportSlot`/`ActiveSplitScreen` as separate, camera-mode-agnostic state (deliberately NOT coupled into `OrbitCamera`, so it survives this refactor without untangling) — this unification should still account for split-screen's viewport-assignment concern as a fourth mode-adjacent thing, not just Orbit/Fly/Party. See `planning/claude_suggestions.md` ▸ Camera. See `planning/features/camera_modes.md`
- [ ] **Camera modes — new modes + switching (v2)** — `Follow`, `FirstPerson`, `Fixed` modes in RON; `SetCameraMode` action with optional eased transitions; FOV interpolation. _Dep: camera mode unification (v1)._ See `planning/features/camera_modes.md`
- [ ] **Per-viewport-only target ring visibility** — today (per `per_player_split_screen_targeting.md` Phase 1) every player's target ring renders in every split viewport, tinted by `PLAYER_LABEL_COLORS` so whose ring is whose stays visually clear; some designers may instead want a player's ring visible **only** in their own viewport. Needs each split `OrbitCamera` assigned a distinct `RenderLayers` (in `spawn_players_and_camera`) and each player's `TrackingTarget` ring entity tagged with only its owner's layer — `RenderLayers` is unused anywhere in this engine's gameplay cameras today (only `inspector.rs`, for inspector/game UI isolation), so this is new plumbing, not a tweak to an existing mechanism. Expected perf impact: neutral-to-slightly-positive — no new shader/pipeline, Bevy's existing per-camera visibility check already does the work, and it can only reduce draw calls (each viewport draws fewer rings, not more); the cost is implementation surface, not runtime. _Surfaced 2026-07-15, a follow-up question during the per-player targeting playtest review — not implemented, no plan file yet._

### Local Co-op Split-Screen Demo
New example project (`local_coop_demo`): two local players (keyboard + optional gamepad) move
through portal-linked scenes, each showcasing a different screen-sharing configuration. Local
co-op on one machine — unrelated to and does not depend on the Beta 0.6 LAN networking milestone.
Staged incrementally; each stage ships and is playtested before the next starts.
- [x] **Stage 1 — foundation: 2-player schema, shared framing camera, view-box clamp** — see `planning/features/done/local_coop_foundation.md` — `da81799`
- [x] **Stage 2 — portal/teleport action** — moves both players to the next scene when either enters the portal; needed zero new engine code (existing `TriggerZone`/`LoadScene` mechanics already generalize to N players) — `8181ccd`
- [x] **Stage 3 — vertical split-screen scene** — two real per-player `OrbitCamera`s, each
      constrained to its half of the window via `Camera.viewport`; fixed a camera-order-ambiguity
      console warning found during playtest — `b59a3e7`
- [x] **Stage 4 — horizontal split-screen scene** — reused Stage 3's mechanism almost entirely (a
      new `SplitOrientation::Horizontal` enum variant + one new match arm; `entity_spawner.rs`,
      `ActiveSplitScreen`, `SplitViewportSlot` needed zero changes) — `b5844c7`
- [x] **Stage 5 — dynamic split-screen scene** — viewport boundary follows player positions,
      reusing `PartyOrbitCamera` (merged) + Stage 3/4's per-player cameras (split) via
      `Camera.is_active` toggling rather than runtime spawn/despawn; hysteresis + orientation-lock
      prevent flicker/mid-split axis flips — `02d7ccb`. All 5 stages shipped — feature doc moved to
      `planning/features/done/local_coop_foundation.md`.
- [x] **P1/P2 nameplate & HUD distinction** — split into two independent halves during planning
      (2026-07-07): the HUD-corner-label half shipped as "split-screen player HUD labels" — a
      colored "P1"-"P4" corner label per player's split-screen viewport, derived from `PlayerIndex`
      and a fixed engine palette (not the RON `material` tint); wasm-perf follow-up guarded an
      unconditional `Node` write causing per-frame UI relayout — `af6727f` / `b034a53`. See
      `planning/features/done/local_coop_player_hud_labels.md`. The *nameplate* half (floating 3D
      "Player N" tags) turned out to be blocked on a separate, deeper bug —
      `nameplate_visibility_system`'s `camera_q.single()` no-ops entirely once 2+ real cameras exist
      (every split-screen scene, Stage 3+) — not folded into this item; now tracked as its own
      bullet under `### Camera` ▸ "Split-screen: remaining single-camera assumption sites".
- [x] **Stage 6 — 4-way split-screen scene (N-way generic)** — generalizes the split system from
      2-way (`Vertical`/`Horizontal`) to a new `Grid` orientation driven by player count (static
      only, no dynamic merge); removes the `.take(2)` cap in `spawn_players_and_camera`; adds
      Numpad key support for a 4th keyboard scheme so 4 players (WASD / Arrows / IJKL / Numpad)
      share one keyboard, each visually distinguished by a solid-color material tint (blue/pink/
      dark green/red). Playtest surfaced two real bugs (material tint never reached players —
      `PlayerConfig` didn't carry the field at all; UI needed reworking to per-quadrant control
      hints) and one added polish item (floating "Room N" labels above every portal, all 6 scenes)
      — all fixed. Screenshot baseline for `room6` deferred (build-time constraints this session).
      `2a5e425`. See `planning/features/done/local_coop_4way_split.md`.
- Diagonal split-screen scoped out at design time — Bevy's `Camera.viewport` is rectangle-only; a true diagonal cut needs a stencil/shader mask, untested on this engine's WASM/WebGL2 target.
- Dep (soft): promotes "Gamepad / controller input" (Icebox) from icebox to in-scope, sized down to exactly this demo's needs.

### Gameplay & Environment

- [ ] **Status effect icons — HUD bar (v1)** — `StatusEffectBar` UI node in scene RON; shows active player buffs/debuffs as a strip of icons; icons are asset catalog texture keys on modifier templates; updates via change detection on `ActiveModifiers`. See `planning/features/status_effect_icons.md`
- [ ] **Status effect icons — world-space strip (v2)** — icon strip above entities (not just the player); shares `collect_visible_modifiers` logic; separate spawn/despawn path per entity. _Dep: HUD bar (v1)._ See `planning/features/status_effect_icons.md`
- [ ] **Layered icon UI node** — new `LayeredIcon` UI node type; each layer declares a texture key, tint color (r,g,b,a), and opacity; layers are alpha-composited in declaration order (bottom → top); v1 alpha-stack only — additive blend mode deferred to a future `blend:` field per layer; feeds action bar slot icons and status effect icon strips directly.
- [ ] **AoE ground targeting** — `TargetingMode: GroundAoE(radius)` on skill action bar slots; pressing the slot enters a placement mode showing a circle decal under the cursor; confirming fires the slot's `do_actions` with `{aoe_position}` substitution; cancelling (right-click / Escape) exits without firing. See `planning/features/aoe_ground_targeting.md` _Hard dep: Skill action bar._
- [ ] **Gamepad-routed action-bar slots** — action bar slots are keyboard-only (`Res<ButtonInput<KeyCode>>`), with no path through a player's `InputMap` the way movement/`target_next` have. Surfaced during Phase 2 plan review of per-player targeting (2026-07-15, both system-architect and ux-gamedesigner-reviewer independently flagged it): in the realistic "one keyboard player + one gamepad player" local co-op configuration, the gamepad player's action bar renders fully but can never fire. Without this, per-player action-bar execution is only usable when both players share one keyboard with disjoint hotkeys.
- [ ] **Spawn wave / encounter system** — `WaveDef` in RON: an ordered sequence of spawn steps each with a prefab key, count, delay, and optional position list; fires on an event (`StartWave("wave_01")`), emits `wave.complete:{id}` when all spawned entities are dead; supports looping waves and inter-wave delays. Designer-friendly alternative to scripting individual `Spawn` + `EmitEventAfterDelay` chains. See `planning/features/spawn_wave_encounter.md`
- [ ] **Day/night cycle** — `DayNightCycleDef` in scene RON: cycle duration, sun color/intensity keyframes at dawn/noon/dusk/midnight; `TimeOfDay` resource drives directional light + ambient each frame; `SetTimeOfDay(hour)` and `SetDaySpeed(multiplier)` actions; emits `time.dawn` / `time.noon` / `time.dusk` / `time.midnight` events designers can hook; WASM compatible (pure CPU, no post-process). See `planning/features/day_night_cycle.md`
- [ ] **Audio channels (volume buses)** — `channels: HashMap<String, f32>` on `AudioConfig`; each audio entry in `assets.ron` declares a `channel` key; `SetChannelVolume(channel, f32)` action scales that category within the master ceiling; enables independent music/sfx/ambient balance without touching source files. _Depends on mute toggle + master volume._
- [ ] **Item-gated interactable** — `requires_item: "key_id"` field on `PrefabDef.interactable`; fires `entity.interact_blocked:{id}` when the player lacks the item, `entity.interacted:{id}` when they have it; enables key-locked doors and quest-gated triggers without a GameVariable workaround. _Dep: Inventory._
- [ ] **Conditional dialogue choices** — `condition_var`/`condition_value` fields on a `DialogueChoiceDef`; hides the choice when the named `GameVariable` doesn't match; enables quest-gated reward branches and post-event dialogue without duplicating nodes. _Dep: Dialogue system._
- [ ] **Sound zones** — ambient audio driven by player location; a new `kind: SoundZone` trigger zone variant with `audio_key`, `volume`, and `fade_distance` fields; entering the zone fades in the audio, leaving fades it out; defined entirely in scene RON using the existing trigger zone + `PlayMusicLoop`/`StopMusic` actions, no new systems needed beyond the fade envelope.
- [ ] **World-space icon stat bar** — row of per-cell sprites (hearts, shields, or any catalog icon) above entities, `WorldIconBarDef` schema field on `PrefabDef`; full cells show filled icon, empty cells show depleted icon; requires sprite-sheet or paired asset catalog entries; design needed (asset reference format, partial-cell handling)
- [ ] **Stat radar labels** — render stat-key labels at each axis tip of `StatRadar`; blocked by UI text on `UiMaterial` nodes; low priority
- [ ] **Equipment system (v1)** — string-key slot system (`EquipmentSlotsDef` on `PrefabDef`); `equippable`+`slot`+`stat_bonuses` on `ItemDef`; `EquipmentMap` component + `PlayerEquipment` resource; `Equip`/`Unequip`/`UnequipAll` actions; stat delta snapshot for reversal on unequip; two-handed exclusion. See `planning/features/equipment_system.md` _Deps: Inventory (hard); Stat templates (soft)._
- [ ] **Quest system — core loop (v1)** — `quests/quests.ron` catalog; `QuestLog` resource (persists across scenes); `AcceptQuest`/`CompleteQuest`/`FailQuest` actions; objectives: `KillCount`, `Collect`, `ReachLocation`, `TalkTo`, `Custom`; `auto_complete` flag; reward types: `GiveItem`, `GiveStat`, `UnlockQuest`, `RunActions`. Fully testable via events alone. See `planning/features/quest_system.md` _Deps: Inventory, Dialogue (soft); Stat templates (shipped)._
- [ ] **Quest system — presentation layer (v2)** — `QuestTracker` UI node; quest-giver `!`/`?` nameplate indicator; `DialogueCondition::QuestState` patch for dialogue branching on quest state. _Deps: Quest core (v1), Nameplate system._ See `planning/features/quest_system.md`
- [ ] **Monster drop pickups** — enemies spawn interactable drops (health potion, coins) at their death position via a new `at_entity` field on `Action::Spawn`; behavior files handle collect and auto-expire logic entirely in RON; exercises the full dynamic-spawn pipeline with a meaningful reward loop. Lands the `at_entity` primitive that Loot v1 reuses. See `planning/features/monster_drop_pickups.md`
- [ ] **Loot system — roll + auto-loot (v1)** — `loot/loot_tables.ron` catalog; `RollEach`/`RollOne` strategies; `loot_table` on `PrefabDef`; `RollLootTable(entity)` action with `auto_loot: true` delivering directly to inventory; designer-wired via behavior file; closes Quest `Collect` objective dep. See `planning/features/loot_system.md` _Deps: Inventory (hard); `at_entity` Spawn field (soft — reuse from Monster drop pickups)._
- [ ] **Loot system — physical loot bags (v2)** — `LootBag` entity with pickup UI; `PickupLoot`/`ClearLootBag` actions; `ItemQuality` for icon border tinting; nested tables. _Dep: Loot v1._ See `planning/features/loot_system.md`
- [ ] Timeline / sequencer — scripted cutscene playback from a RON timeline asset

### Particle System v2

- [ ] **Bloom / post-processing in scene RON** — WASM-BLOCKED: Bevy's `Bloom` requires `#[require(Hdr)]`; HDR breaks the WASM build. Parked until performant HDR/post-process support is available in Bevy's WebGPU backend. Do not implement a native-only workaround — that splits the runtime model. See `planning/features/particle_bloom.md`
- [ ] **Shared effect library** — `assets/shared/effects/` with reusable effects and per-project overrides. See `planning/features/particle_shared_library.md`

### Rendering & Assets

- [ ] **LOD — pre-baked mesh swap** — distance-based LOD switching using offline-generated LOD GLB files; `lod_levels: [(distance, model)]` on `PrefabDef` declares swap thresholds; a system watches camera distance and swaps `SceneRoot` handle; LOD meshes generated offline (Blender / `meshopt`) and referenced in `assets.ron`; no runtime compute required — fully WASM-compatible. See `planning/features/lod_prebaked_mesh_swap.md`
- [ ] **Channel-packed ORM texture shader** — new WGSL shader variant (`custom_texture_triplanar_pbr_packed.wgsl`) that reads a single packed ORM texture (R=occlusion, G=roughness, B=metallic) instead of three separate textures; matches the default export format from Substance Painter; frees two sampler slots and halves texture bandwidth for PBR properties; no schema changes — designers swap the shader key and point one texture at `texture_1` instead of three.
- [ ] **Deferred rendering** — replace clustered forward with Bevy's deferred pipeline to remove the `MAX_FADING_LIGHTS = 16` cap and efficiently handle large numbers of dynamic lights (torches, particle lights, explosions); transparent/additive materials (particles, decals) stay on the forward path automatically. Investigation complete — WASM builds clean, GL degrades gracefully; one remaining step: manual Chrome WebGPU console check. See `planning/features/deferred_rendering.md`
- [ ] **Toon / cel shading (3-tone, 4-tone, 5-tone)** — WGSL-only `CustomMaterial` shaders for stylized discrete light bands; 3- and 4-tone fit current uniform budget; 5-tone uses a ramp texture; design: `planning/features/toon_shading.md`
- [ ] **LOD — runtime generation + caching** — WASM-BLOCKED: generating simplified meshes at runtime requires offthread compute (web workers + `SharedArrayBuffer`); Bevy's WASM build does not support this today. Also covers IndexedDB caching of generated LODs and Bevy meshlets (GPU-driven micro-mesh rendering — not WASM-stable). Parked until Bevy's WASM offthread compute support matures.
- [ ] Decal system — project a texture onto geometry without modifying meshes
- [ ] Animated texture support in `CustomMaterial` (frame index via time uniform)
- [ ] Water / reflective plane primitive with animated normal map — WASM-BLOCKED: reflection passes require multi-pass rendering or screen-space sampling; not performantly supported in Bevy's WebGPU backend yet. Parked until WASM support matures.
- [ ] Post-process pass authoring — expose WGSL post-process shader slot per scene — WASM-BLOCKED: same root cause as Bloom and water; HDR and multi-pass post-process break or perform poorly on WebGPU. Parked until Bevy's WebGPU backend matures.

### Beta 0.5 — Deterministic Tick + Replay
- [ ] Fixed-tick schedule for gameplay systems (separate from render tick)
- [ ] Deterministic RNG resource (seeded, replaces any `rand` usage in gameplay)
- [ ] `InputAction` stream capture to file (native)
- [ ] Replay playback from captured stream
- [ ] Snapshot/restore stub for core gameplay state
- [ ] Determinism constraints doc

### Beta 0.6 — Multiplayer Form 1: LAN Co-op
See `planning/features/networking_multiplayer.md`. Gate: Beta 0.5 (deterministic tick) must ship first.
- [ ] Library spike (Bevy Lightyear vs alternatives — see feature file pre-checks)
- [ ] `HostGame` / `JoinGame` / `DisconnectGame` actions
- [ ] Input replication: client sends `InputActionMessage` per tick; host authorises
- [ ] State replication: `Transform`, `AnimationState`, `StatMap`, `GameVariables`
- [ ] `multiplayer.*` pipeline events
- [ ] LAN co-op demo scene
- [ ] Network protocol doc + integration tests

### Beta 0.7a — Loading Screen
- [ ] Loading screen overlay during `LoadingScene` / `LoadingProject` states
- [ ] `scene.loading_progress:{0-100}` milestone events from loader and terrain task
- [ ] `loading_scene` field in project config for custom splash scenes
- [ ] Docs + tests
- [ ] Design: `planning/features/loading_screen.md`

### Beta 0.7b — Scene Preloading
- [ ] `preload_poll_system`: watch `PreloadedScenes` handles, emit `scene.preloaded:{name}`
- [ ] `LoadScene` fast-path when handle is already loaded in `PreloadedScenes`
- [ ] Docs + tests
- [ ] Design: `planning/features/scene_preloading.md`

### Beta 0.8 — Multiplayer Form 2: Internet Player-Hosted
See `planning/features/networking_multiplayer.md`. Gate: Beta 0.6 (LAN) must ship first.
- [ ] Relay/signaling service decision and deployment (Matchbox recommended — see feature file)
- [ ] WASM-compatible transport confirmed in browser
- [ ] Lobby system: `CreateLobby` / `JoinLobby` / `ListLobbies` actions
- [ ] `LobbyList` UI scene node
- [ ] Internet co-op demo (accessible from WASM build)
- [ ] Relay setup docs

### Beta 0.9+ — Multiplayer Form 3: Dedicated Server
See `planning/features/networking_multiplayer.md`. Gate: Beta 0.8 (internet listen server) must ship first. Requires a separate detailed feature file before coding.
- [ ] `ironhold_server` crate (headless Bevy, no render/window)
- [ ] `start_server()` entry point in `ironhold_core`
- [ ] Client-mode thin renderer (no local simulation)
- [ ] Server admin actions (kick, change_scene)
- [ ] Dedicated server demo + deployment guide

### Terrain
- [ ] Terrain snap — `snap_to_terrain: true` on entity def makes Y an offset above terrain surface; design: `planning/features/terrain_snap.md`
- [ ] Terrain chunked streaming — generate and load only chunks within a player radius; unload distant chunks; requires chunk-aware terrain capability rewrite
- [ ] **Improved terrain rendering — Phases 1+2** — UV elimination + U16 indices (~25 % vertex memory); mesh chunking (per-chunk culling + incremental async generation, eliminates WASM first-frame stall); unblocks terrain snap and chunked streaming. See `planning/features/improved_terrain_rendering.md`
- [ ] **Improved terrain rendering — Phases 3+4** _(gated on Phase 0 WebGPU PoC)_ — GPU-derived XZ positions; compressed normals; CPU height-array shared between GPU and Rapier. Start only after Phase 0 investigation confirms feasibility. See `planning/features/improved_terrain_rendering.md`

### Performance
- [ ] **Off-thread texture decode in WASM** — use the browser's `ImageBitmap` API to decode textures off the main WASM thread, eliminating main-thread stalls during asset loads; requires a `wasm32`-specific code path in the asset loading pipeline or a Bevy plugin that wraps `createImageBitmap`; investigate whether Bevy's current WASM asset pipeline already defers texture decode before implementing. Sourced from Phaser's web-optimised asset loader model.
- [ ] **Extend pipeline warmup to Text2d and UI pipelines** — spawn hidden warmup entities for `Text2d` and UI `Node` at scene load to pre-compile the 2D/UI GPU pipelines, eliminating WASM frame spikes on first text/UI render; design: `planning/features/pipeline_warmup_2d_ui.md`
- [ ] **Discrete LOD steps for depth-scaled label font sizes** — snap `base_font_size * scale` to a small fixed set (e.g. 100 %, 75 %, 50 %, 25 %) instead of rounding to every integer; bounds atlas slot count to ~4 per label regardless of depth range, at the cost of a slight stepping artefact on slow zoom. Integer rounding + 0.5-threshold guard already fix the per-frame atlas upload problem; this is a further atlas-memory micro-optimisation.
- [ ] **Scene transition material cache** — `scene_loader` rebuilds all materials in the asset catalog on every `LoadScene`, including materials already built for the previous scene; cache built handles per-project in a persistent resource and only rebuild when the catalog key set changes; estimated 50–200 ms saving per transition on large projects (`scene_loader.rs` material rebuild block)
- [ ] **WASM terrain generation first-frame stall** — `AsyncComputeTaskPool` degrades to `block_on` on the main thread in WASM, causing 100–500 ms jank on first frame for large heightmaps; fix by splitting mesh generation across multiple frames (progressive tile-by-tile build) or pre-baking terrain meshes as GLB assets; requires `#[cfg(target_arch = "wasm32")]` code path (`terrain.rs` poll system)
- [ ] **Per-frame collection allocations in hot systems** — `stat_modifier_system` (key clone Vec), `message_interpreter_system` (event Vec rebuilt each frame), and `player_movement_system` (input HashMap) each allocate new collections every frame; refactor to reuse pre-allocated collections via `.clear()` on a `Local` resource or avoid the intermediate collection entirely; WASM GC pressure is higher than native so the gain is larger there

### Profiling & Diagnostics
- [ ] Diagnostics HUD — F3 overlay: FPS, frame time, entity count, draw calls, triangles, CPU/RAM (native); design: `planning/features/diagnostics_hud.md`
- [ ] Tracy integration — `--features trace_tracy` on native runner; per-system CPU timeline; design: `planning/features/tracy_integration.md`

### Designer Experience
- [ ] **Foliage demo — visual tuning** — refine cluster placement, leaf scale, and per-prefab parameters in `foliage_demo` to match the reference image quality. Blocked on live RON reload or live editor — parameter tuning without a tight feedback loop is impractical. Pick up after either ships.
- [ ] **Extend `entity_logic_demo`** — add one clearly labeled station per behavior concept: multi-state FSM behavior file with `EmitEventAfterDelay` loop (goblin-guard pattern), a timed-door sequence (`EmitEventAfterDelay` chain), side-by-side trigger zone enter vs. interactable [F] comparison, and a `global_on` example showing project-wide vs. entity-local events; modeled on the station-per-concept layout of `particles_demo`
- [ ] **`ui_demo` project** — standalone demo project for every UI capability: buttons, data-bound labels with `SetVariable`/`IncrementVariable`, overlays, pause-menu pattern, and all stat display widget types; gives designers a single project to copy patterns from
- [ ] **`audio_demo` project** — standalone demo project focused on audio authoring: `PlaySound`, `PlayMusicLoop`, `SetVolume`, stop/loop patterns, and how audio is triggered from RON rules; no existing project makes audio its primary focus
- [ ] **`scene_transitions_demo` project** — standalone demo project with 3–4 scenes wired through a state machine, demonstrating portal navigation, scene overlays, preloading, and multi-scene project config; distinct from `particles_demo` where portal navigation is a side feature
- [ ] **Schema version v2→v3 migration guide** — add a "Migrating from v2 to v3" section in `docs/20_data_formats.md` covering: rename `rules_path` → `state_machine_path`, bump `schema_version` to `3`, convert `rules.ron` to the FSM format, and the warning to expect if both files coexist
- [ ] **Magic-string event/action validator** — `tools/ron_validator/` CLI that cross-checks event names used in `rules.ron` / `state_machine.ron` against the set emitted by capabilities and reports unknown event keys before runtime; eliminates silent no-ops from typos in event names
- [ ] **CLI `--strict` merchant cross-validation** — cross-check merchant `currency_stat` against stat defs and `item_key` against `items.ron` at validate time; catches typos before any runtime run, mirroring existing scene→prefab checks.
- [ ] **CLI `validate` cross-checks UI trigger reachability** — derive each scene `Button`/`IconButton`/`global_key_bindings` entry's `ui.button_pressed:{trigger}` event and confirm a rule/transition/binding actually handles it; catches "button animates but nothing happens" typos at author time instead of at runtime. See `planning/features/ui_trigger_reachability_check.md`.

### UI
- [ ] UI element types beyond `Button`: `Label`, `Image`, `ProgressBar`, `Panel`
- [ ] UI layout — stack/flex layout or anchor-based positioning replacing raw pixel coords
- [ ] Font + theme config per project
- [ ] Drop shadow support for UI text
- [ ] Drop shadow support for world entity text labels
- [ ] **Draggable UI windows** — players drag inventory, shop, and container panels by their title bars; pure positional `Node.left/top` mutation (no ActionQueue involvement); position resets to RON-authored origin on scene change. See `planning/features/draggable_windows.md`
- [ ] **Cursor grab icon for draggable windows** — grab-hand cursor on title-bar hover, grabbing-fist while dragging; cosmetic companion to draggable windows, pipeline-free. _Dep: draggable UI windows._ See `planning/features/cursor_grab_icon.md`
- [ ] **Hover tooltips for inventory and action bar** — floating card on inventory-slot and action-bar-slot hover; shows item name/description/stats or skill title/effect/cooldown footer; single shared overlay node repositioned on hover change, edge-clamped to viewport; pipeline-free; supersedes the never-rendered `ActionSlotDef.label` field. See `planning/features/tooltip_system.md`
- [ ] **Three-channel icon masking** — `icon_colors: [(r,g,b,a); 3]` on `ActionSlotDef` and `ItemDef`; `IconMaskMaterial` WGSL UiMaterial maps each R/G/B channel to an independent designer-specified color from one channel-packed sprite cell; existing `icon_color` tint path unchanged. See `planning/features/icon_three_channel_mask.md`

### Tools
- [ ] `tools/ron_formatter/` — auto-format `.ron` files (indentation, trailing commas)
- [ ] Live reload server — watch `assets/` and push scene reload to running native build via IPC
- [ ] GLB batch inspector — produce a markdown table of node names, animations, and materials for a whole folder
- [ ] **Icon sheet builder** — `tools/icon_sheet/build.py` stitches a folder of individual icon PNGs into a power-of-2 RGBA atlas + sidecar manifest mapping filename-stem to `icon_index`; no engine changes; eliminates hand-packing and magic cell counting for designers. See `planning/features/icon_sheet_builder.md`
- [ ] **Live project editor** — `crates/ironhold_editor`; axum server on port 3001 serving React frontend + WASM game preview + REST API; `schemars`-derived JSON Schema → RJSF forms; RON ↔ JSON bridge with validation gate; WebSocket-triggered iframe reload on save; v1 edit-only, v2 create/delete. See `planning/features/live_project_editor.md`
- [ ] **Playwright/`test_web.py` can't get a WebGPU device in this sandboxed dev environment** — every project fails headless with "Unable to find a GPU!"; headed mode finds an adapter but device creation fails on a missing `dxil.dll`/`dxcompiler.dll` in Playwright's bundled Chromium (confirmed environment-specific — a separately-installed real browser on the same machine works fine). Workaround found (`--use-webgpu-adapter=d3d11` launch flag) gets the app running with zero console errors, but screenshot capture (`page.screenshot()`) still times out once WebGPU is actively rendering, and visual 3D rendering correctness with the workaround wasn't confirmed before parking. See `planning/investigations/headless_webgpu_testing.md`.

---

## Icebox

### Physics
- [ ] **wgrapier — GPU physics watch item** — `wgrapier3d` v0.2.0 (Dimforge, Nov 2024) is a WebGPU compute-shader dynamic-body simulator; not usable for terrain height queries (requires async GPU→CPU readback); no Bevy integration; Dimforge has announced a full rewrite on `rust-gpu`. Revisit when the rust-gpu rewrite ships with real releases, an official Bevy bridge exists, and a dense dynamic-body workload justifies it.

### Engine / Runtime
- [ ] **Bevy 0.19 upgrade** — gated on `bevy_rapier3d`, `bevy_framepace`, and `bevy_common_assets` all shipping 0.19-compatible releases (none confirmed as of 2026-06-23). Est. ~4–7 person-days once the dep tree resolves. See `planning/investigations/bevy_019_upgrade.md`.
- [ ] **Camera/input configuration → scene layer** — `orbit_camera`, `flycam`, and `player` blocks on `PrefabDef` are scene singletons consumed as such by the runtime; architecturally they belong on the scene (or project config); migration path: introduce optional scene-level camera/input fields first, deprecate-but-keep prefab fields, then clean up in a `PREFAB_CATALOG_SCHEMA_VERSION` bump; lower urgency once per-instance overrides ship since that reduces the pressure on prefab forks. From scene/prefab boundary analysis.
- [ ] **Scene layer compositing** — `layer: Overlay | Base` field on `GameSceneV2`; overlay scenes render on top of the base scene without unloading it, enabling persistent pause menus, HUD layers, and cutscene overlays authored entirely in RON; renderer approach (two active Bevy worlds vs. `RenderLayer` masking) needs design investigation before coding. Sourced from Phaser's layered-scene architecture.
- [ ] Capability registry — declare events, actions, and validation rules per capability; replaces ad-hoc wiring
- [ ] Schema migrations — versioned upgrade paths with diagnostics on load failure
- [ ] **Gamepad / controller input** — wire Bevy's built-in gamepad input through the existing `InputAction` system and RON key bindings; map stick axes to movement/camera and face buttons to `InputAction` variants; designers declare gamepad bindings in the same input config block as keyboard; needed for web builds targeting controller users
- [ ] **Local co-op hot join/leave (min 1, max 4 players)** — dynamically spawn/despawn players at runtime and recompute the split-screen layout live, instead of the player count being fixed at scene load. Needs a per-scheme "press to join" trigger and a decision on what happens to a leaving player's state. Not yet drafted — write a `planning/features/` doc before starting. Discussed 2026-07-06 while scoping the 4-way split scene; deliberately scoped out of that feature to avoid bundling a runtime-lifecycle change into a viewport-math change. _Dep: Stage 6 — 4-way split-screen scene (N-way `Grid` split) landing first, since this builds on top of a split system that already tolerates a variable player count._
- [ ] **Save / load game state** — `SaveGame` / `LoadGame` actions; serialize `GameVariables`, per-entity `StatMap`, and active modifier state to a JSON/RON file (native) or `localStorage` (WASM); `AutoSave` trigger on configurable events; scene transitions preserve state across loads. See `planning/features/save_load_game_state.md`
- [ ] **Input remapping** — let players rebind keyboard and gamepad actions at runtime via a settings UI; bindings persisted to a per-player config file (native) or `localStorage` (WASM); designer declares remappable actions and default bindings in project RON; depends on gamepad input feature for full coverage
- [ ] **Per-instance stat overrides on `Action::Spawn` (v2)** — extend `stat_overrides` support to dynamic `Action::Spawn` so runtime-spawned entities can also start with non-default stats; requires threading the override map through `QueuedSpawn` and `drain_spawn_queue_system`; depends on per-instance overrides v1 (scene-placed) shipping first.
- [ ] **`ChildOf` hierarchy migration** — migrate from `Children`/`Parent` (Bevy pre-0.16 API) to the `ChildOf` relationship component (Bevy 0.16+); the animation system queries `&Children` to walk GLB hierarchies and all spawners use `with_children()` — these need updating to the forward-looking API before a future Bevy upgrade removes the compat shim
- [ ] **Required components on project-defined components** — adopt `#[require(...)]` (Bevy 0.15+) on project-defined marker components (e.g. `TriggerZone`, `FadingLight`, `LevelEntity`) so that inserting the primary component automatically inserts its mandatory companions; reduces manual bundle construction in spawners and makes component contracts explicit at the type level
- [ ] **Consistent `assets.ron` entry shapes** — `models` entries use `(path: "...")`, `textures` are bare strings, `audio` uses `(path: "...", volume: ...)`; unifying the shapes reduces copy-paste errors and parse confusion; requires schema version bump
- [ ] `Condition` expressions in rules (`score >= 10`, `variable == "value"`) — currently only event matching
- [ ] Hot-reload for `.scene.ron` and `rules.ron` in native debug builds

### Groups & Membership
- [ ] **Group system — Tier 1 (factions, teams, parties)** — generic RON-defined `GroupDef` (kind, max_members, default_stance); `LoadedGroups` global resource mapping group-id → member set + `GroupMembership` component on entities; `AddToGroup` / `RemoveFromGroup` / `DisbandGroup` actions; `group.joined:{id}:{entity}` / `group.left:{id}:{entity}` / `group.full:{id}` events into the existing pipeline; faction stance rules (Hostile/Neutral/Friendly) for AI targeting; useful standalone in single-player for factions, arena teams, and NPC parties. Tier 2 (guild, chat, raid hierarchy) deferred to Beta 0.6 networking milestone. See `planning/features/group_system_tier1.md`

### Gameplay Capabilities
- [ ] **Equipment system — v2 visual mesh attachment** — `model_attachment`/`AttachmentDef` for equipping visible mesh props (weapons, helmets) to named bone sockets; requires GLB skeleton socket authoring and runtime bone-query APIs. _Dep: Equipment system v1._
- [ ] **Grid system** — square, hexagonal (flat-top / pointy-top), and triangular cell layouts; `grid: GridDef` on scene RON; `(col, row)` addressing for all types (axial for hex); `PlaceOnGrid` / `StartGridMove` / `SetCellPassable` / `FindPath` actions; A* with node cap; `GridPosition` component; `grid.cell_entered` / `grid.move_complete` / `grid.path_blocked` events; Gizmos debug overlay; WASM-compatible. See `planning/features/grid_system.md`

---

## Done (reference)

### July 2026
- [x] **Action bar custom hotkeys** — bind any `parse_key`-recognised key name (`"KeyQ"`, `"KeyE"`, `"F2"`) to action bar slots in RON; removes the hardcoded `DIGIT_KEYS` table; new `key_hint` field overrides the on-screen corner glyph (distinct from the pre-existing `label` field, kept separate after a UX-caught naming collision); fully backward-compatible with existing `"1"`–`"9"` layouts, including a real migration bug fix (`parse_key("i")` was case-sensitive and would have silently killed the existing inventory slot). Unparseable/duplicate hotkeys get both a runtime `warn!` and an `ironhold_cli validate` error. Hard dependency for per-player action-bar execution (Phase 2, `planning/features/per_player_split_screen_targeting.md`), shipped first per that dependency. See `planning/features/done/action_bar_custom_hotkeys.md`. All 5 reviews clean; playtest confirmed by Frank (including a mid-playtest content tweak — the demo's new "Taunt" slot now deals damage and triggers monster aggro) — `8df3cfc`
- [x] **Dynamically-spawned stat labels/bars inherit scene `label_depth_scale`** — see `planning/features/done/depth_scale_dynamic_spawn.md`. `drain_dynamic_stat_ui_system` now calls `resolve_label_depth_scale` against the scene's `label_depth_scale` block instead of hardcoding `depth_scale: None`, so wave-spawned enemies depth-scale identically to scene-placed ones. First feature run through the new gitops branching workflow (feature branch/worktree → parallel code review → integration merge); plan review caught and corrected a wrong premise before coding started (no per-prefab override field exists — the fix purely propagates the scene-level setting). All 5 code reviews clean; 3 new integration tests; dev playtest confirmed via screenshot comparison — `b08e447`
- [x] **Player nameplate visibility — v2: `ToggleOwnNameplate` runtime action** — see `planning/features/done/player_nameplate_visibility.md`. Lets a player flip their own nameplate visibility at runtime (mirrors `ToggleMute`'s pattern exactly, including two distinct `nameplate.own_shown`/`nameplate.own_hidden` events); an explicit per-prefab override always wins; the preference resets per scene load (documented, not a bug). Caught and fixed a real `cargo check -p ironhold_cli` compile error (missing match arm) and a real test regression in a v1 test. All reviews clean; full test suite green — `a66fce7`
- [x] **Player nameplate visibility — v1: `Player` marker + `show_player_nameplate`** — see `planning/features/done/player_nameplate_visibility.md`. Adds a real `Player`/`PlayerOwnership` marker (multiplayer forward-compat hook), an orthogonal `show_player_nameplate: bool` scene field (default false), makes `faction_filter` bypass the player entirely, and closes the character-select nameplate bug. Also fixed a previously-undiscovered 6th nameplate-gating site found during implementation. All reviews (architect, game-designer, alignment, wasm-perf, ux) clean; full test suite + dev/release WASM builds play-tested and confirmed — `96e09c9`
- [x] **Extract nameplate spawn-condition predicate to `should_insert_nameplate()` helper** — the `nameplate != Some(false) && (show || nameplate == Some(true))` guard was copy-pasted across 5 sites (`scene_loader.rs` ×4, `entity_spawner.rs:375`) with the `show` input differing per path (`scene.show_nameplates` vs `nameplate_config.enabled`); extracted to `fn should_insert_nameplate(nameplate: Option<bool>, show: bool) -> bool` beside `tag_spawned_entity` in `scene_manager/mod.rs`. Alignment-reviewed (ALIGNED); tri-state contract locked in with a dedicated unit test — `48889f1`
- [x] **Extract `set_panel_open()` helper in action_executor** — `LoadedInventoryUi::set_panel_open(bool)` replaces 7 inline saturating add/sub sites — `53643ca`
- [x] **Inventory / shop / container click-blocking backdrop** — per-rect `FocusPolicy::Block` on panel roots — `ec84bcf`
- [x] **Audio icon toggle button** — see `planning/features/done/audio_icon_button.md`. Replaced the main HUD's "Toggle Mute" text button + "Audio: {state}" label with a single top-right `IconButton`; grew into a general-purpose reusable node with icon/active/hover/click tint colors and an optional drop-shadow, all RON-authorable — `d76a235`
- [x] **Split `integration_tests.rs` into domain files** — see `planning/features/done/split_integration_tests.md`. 104-test/4258-line file split into 8 domain files (`fsm_tests.rs`, `entity_logic_tests.rs`, `scene_lifecycle_tests.rs`, `spawn_tests.rs`, `action_tests.rs`, `npc_tests.rs`, `nameplate_tests.rs`, `ui_tests.rs`); no production code changed, all 104 tests still pass — `3570198`

### June 2026
- [x] **Overlay modal backdrop (click-blocking)** — transparent full-screen `GlobalZIndex(100)` node auto-spawned by `LoadSceneOverlay`; overlay content at `GlobalZIndex(101)`; blocks base-scene button clicks through overlays. See `planning/features/done/overlay_modal_backdrop.md`.
- [x] **Unify prefab feature application across all spawn paths** — moved `attach_prefab_features` to `entity_spawner.rs`; `spawn_prefab_instance` calls it at its tail; all three spawn paths (GLB Actor/Prop, composite Primitive, single-mesh Primitive) route through one function. See `planning/features/done/unify_prefab_feature_application.md`.
- [x] **Consolidate conditional prefab-feature application (sibling divergence)** — introduced `attach_prefab_features` in `scene_loader.rs`; both Primitive branches call it instead of duplicating 6 feature blocks each.
- [x] **Nameplate system** — floating name + health bar above entities, scene-wide opt-in (`show_nameplates: true`) with per-prefab override; distance and faction filtering. See `planning/features/done/nameplate_system.md`
- [x] **Intent event layer** — `intent.slot.{n}:{entity}` emitted before committing; routes through interpreter so designers can cancel/redirect ability slots from RON rules. See `planning/features/done/intent_event_layer.md`
- [x] **Icon washed-out fix** — sRGB established as universal RON color convention; `Color::linear_rgba` fixed in shop icon spawn and decal base color storage; 14 doc labels corrected. See `planning/features/done/icon_washed_out_fix.md`
- [x] **Inventory & item system** — `items/items.ron` catalog; `PlayerInventory` resource; `InventoryPanel`/`ShopPanel`/`ContainerPanel` UI nodes; `OpenShop`/`BuyItem`/`OpenContainer`/`TakeAllFromContainer` actions; `MerchantDef`; currency via stat system. See `planning/features/done/inventory_item_system.md`
- [x] **Dialogue system** — `.dialogue.ron` assets; `DialoguePanel` UI node; `StartDialogue`/`EndDialogue`/`AdvanceDialogue` actions; branching choices; `{self}`/`{target}` substitution; auto-wired to `entity.interacted`. See `planning/features/done/dialogue_system.md`
- [x] **NPC aggro-on-hit + Investigating state** — `NpcHitQueue` relay; `Investigating` FSM state walks to attacker's last-known position; `investigate_timeout_secs` per prefab; `npc.investigating`/`npc.investigation_failed` events. See `planning/features/done/npc_aggro_on_hit.md`
- [x] **Camera shake** — `Action::CameraShake { duration_secs, intensity }`; procedural position shake on orbit camera; wired to player-hit events in `3rd_person_game_demo`. See `planning/features/done/camera_shake.md`
- [x] **Spider NPC behavior polish** — death animation holds last frame (no `duration`, `stop_action: "npc_revive"`); respawn clears death pose via `PlayAnimationOn(clip: "npc_revive")` in "alive" entry_actions; spider stops instantly on death (direct `StatMap` health check in `npc_behavior_system` eliminates 2-frame relay delay); patrol walk/idle/waypoint-pause working; attack override fires only on player reach; corpse hide/respawn timing designer-configurable via `EmitEventAfterDelay` in behavior RON.
- [x] **Per-prefab `select_aim_height` for click targeting** — `select_aim_height: f32` (default 1.0) on `PrefabDef`; `SelectAimHeight(f32)` component; fixes snake/spider click hitboxes floating 0.6–1 m above the visible body; orc/zombie unaffected (no field set → default 1.0)
- [x] **Target indicator color by category and per-prefab override** — layered color resolution: prefab `indicator_color` (direct RGBA) > `indicator_category` key in scene `named_colors` > scene `color` fallback; material memo keyed by resolved colour bits — `d945ea9`
- [x] **Embed capability shaders & fix hardcoded shared asset paths** — `stat_radar`, `foliage` (×2), `flame_material`, `pool_flame` shaders embedded via `include_str!()`; foliage fabricated texture fallback removed; CLI validate now cross-checks foliage `leaf_texture` keys; fixture RON kind-field format fixed — `planned at b1ca9b6`
- [x] **Creature collider sizing — snake & spider** — `collider_height`/`collider_radius` tuned on `enemy_snake` (0.8/0.3) and `enemy_spider` (1.2/0.4); eliminates oversized humanoid capsule blocking player approach.
- [x] **NPC dead-state fix + `ResetToSpawn` action** — visibility guard in `npc_behavior_system` stops ghost hitboxes; new `ResetToSpawn(entity)` action teleports NPC to origin on respawn; wired in all three enemy behavior files.
- [x] **Hitbox debug toggle** — `debug_target_hitboxes` GameVariable draws 0.5 m yellow gizmo spheres at `ClickSelectable` aim points; toggled via two HUD buttons in `3rd_person_game_demo`.
- [x] **Multi-source animations (animation packs + shared-rig mesh variants)** — `animation_sources: [catalog_key, ...]` on `AnimationPolicy.ron`; animation graph merges clips from all listed GLBs plus the model GLB; enables splitting clips across domain packs (locomotion, magic, gun) and sharing one pack across mesh variants with identical bone names; backwards-compatible.
- [x] **Character select + runtime player spawn via `Action::Spawn`** — WoW-style character selection screen in `3rd_person_game_demo`; player prefabs spawned at runtime via `Action::Spawn` with camera + controller assembled from prefab RON; `ActiveTonemapping` threads scene tonemapping to the orbit camera; FSM states carry character choice across the scene load boundary; display-only `preview_*` prefabs prevent stray cameras.
- [x] **Character select idle animations + model consolidation** — all three preview characters play idle animations on the select screen; model orientation fixed via `model_fixes.ron` (180° Y); zombie GLB scene root renamed to match animation pack root for `animation_sources` retargeting; mesh-only duplicate GLB subfolders removed (character_female, zombie); catalog keys unified (`character_male_mesh` → `character_male` etc.); docs updated: minimum-1-animation GLB requirement and 180° Y rotation convention.
- [x] **3rd-person orbit camera — sky shows tan ground on steep pan** — reduced `max_pitch` from 1.5 → 0.9 rad; removed redundant fallback camera spawn when `spawn_points` present; docs updated.
- [x] **GLB Splitter tool** — `tools/glb_splitter/split.py`; splits a monolithic GLB into a mesh-only file (buffer-compacted) and named animation-group files; `--mesh-only`, `--one-per-clip`, `--by-prefix`, `--group` modes; preview tool + commit hook skip animation-only GLBs — `96326d7`
- [x] **100+ shared GLB models + AVIF previews** — props, characters, creatures; GLB preview tool fixes (fallback materials, mesh-only bounds, proportional clip planes, pixel-count blank detection) — `7486104`
- [x] **Snake and spider GLB enemies** — `kind: Actor` NPC support; `enemy_snake` + `enemy_spider` with AI, hit effects, patrol waypoints in `3rd_person_game_demo` — `df8c94b`
- [x] **NpcDef collider sizing + stat_overrides in spawn** — `collider_radius`/`collider_height` on `NpcDef`; `spawn_prefab_instance` validates and applies stat overrides; NPC integration test — `1112fc4`
- [x] **Dynamic spawn missing components** — `motion`, `stat_label`, `world_stat_bar` now attach on dynamic `Action::Spawn` entities via `DynamicStatUiQueue` — `8005606`
- [x] **Mute audio toggle + master volume** — `ToggleMute`/`SetVolume` actions; `AudioConfig` on `ProjectConfig`; `SyncAudioState` for label init — `5df25da`
- [x] **Per-instance stat overrides on SceneEntityDef** — `stat_overrides: HashMap<String, f32>` on placed scene entities; unknown keys warn at load time — `4416545`
- [x] **Mouse click-to-select + Tab targeting** — screen-space proximity selection; Tab/Shift-Tab nearest-first cycling; `{target}` substitution; `SetTarget`/`ClearTarget` actions; `target.*` events — `34bc77d`
- [x] **Consolidate entity-spawn component insertion** — `tag_spawned_entity` helper owns all spawn metadata; all 7 spawn sites route through it — `0afd6a0`
- [x] **Skill action bar (1–9)** — `ActionBar` UI node; cooldown overlay; `CooldownMap`; `ShowFloatingText` action; demo in `primitive_world` — `187463d`
- [x] **stats_demo + blank_project starter** — standalone stats showcase; minimal copy-and-rename starter project — `fb30a6d`
- [x] **Foliage square shadows fix** — `NotShadowCaster` on cluster entities; `cast_shadows` RON opt-in — `c4b1ef8`
- [x] **Stylized foliage** — `kind: Foliage` prefab type; billboard leaf cards; `FoliageMaterial` WGSL; `height_bias`/`seed` for crown shape — `a1e7ffd`
- [x] **PrefabCatalog schema v2** — `PrefabKind` enum; `PrimitiveShapeKind` typed enum; `PREFAB_CATALOG_SCHEMA_VERSION` → 2 — `fba367e`

### May 2026
- [x] **Particle mesh buffer recreation** — in-place `Mesh::attribute_mut` replaces per-frame Vec rebuild; eliminates per-frame allocations in pool renderer — `e03c945`
- [x] **Animation player entity lookup cache** — `last_player_entity` fast-path in `animation_playback_system`; O(1) common case — `c72c57b`
- [x] **Flipbook / sprite sheet animation** — UV sub-rect baked per-frame in CPU pool renderer; `explosion_4x4.png` sheet — `9d76d59`
- [x] **Quality tiers + particle budget** — `SetParticleQuality` action; priority (`Player`/`Npc`/`Ambient`); live-count cap; portal navigation; Arcane Observatory demo — `1bc97d9`
- [x] **Ground decals / AoE projections** — `ProjectDecal` action; AoE circles, impact splats, cast indicators — `786800f`
- [x] **Extended particle behaviours** — rotation over lifetime, non-uniform scale, Ring/Sphere/Line/Arc emitters, velocity curves — `d14d1b3`
- [x] **Dynamic effect lights** — `light` block on `EffectDef` spawns a temporary fading `PointLight` — `7a87684`
- [x] **Ironhold CLI enhancements** — `watch`, `stats`, `query actions/events`, `validate --strict` orphan detection; `--json` throughout — `a67c450`
- [x] **Ironhold CLI** — `validate`, `inspect glb/texture/audio`, `query prefabs/effects/scenes/rules`; cross-file checks; exit codes — `61bbc26`
- [x] **Staggered entity spawning + web spawn hang fix** — `PendingEntitySpawns` drains 2/frame; `PreloadPrefab` on `scene.ready` eliminates the quick_scene hang — `b812b26`
- [x] **Stat display — per-entity stat routing** — `resolve_stat` helper; dotted keys route to `StatMap`, plain to global `LoadedStats`; `StatLabelMarker` floating labels — `2015a1e`
- [x] **World-space pixel stat bar** — unified `WorldStatBar` with Ascii and Pixel styles — `98ca5d0`
- [x] **Game stats Phase 2: buffs and modifiers** — named modifier templates; additive/multiplicative/override kinds; stacking rules; `ApplyModifier`/`RemoveModifier` actions — `84b5d15`
- [x] **Stat display — StatRadar** — `StatRadar` UI node (3–12 axes); WGSL polar-coordinate shader via `UiMaterial`; polygon grid — `763ac72`
- [x] **Stat display — StatBar + StatSpread** — `StatBar`/`StatSpread` UI node types; colour bands; change-detection update — `57e1628`
- [x] **Game stats Phase 2a: stat templates** — `stat_templates` on `PrefabDef`; `StatMap` component; dot-routing `ModifyStat`/`SetStat`; threshold/regen — `5292961`
- [x] **Animation T-pose on landing** — `animation_playback_system` detects `AnimationPlayer` entity change and resets `graph_initialized`; investigation resolved — `9ed1917`
- [x] **PrefabComponents deny_unknown_fields** — `#[serde(deny_unknown_fields)]` on `PrefabComponents`; clear field-name error on typos — `7144420`
- [x] **Terrain path consolidation** — `TerrainConfigV2` is now single struct (schema + runtime `Component`); `TerrainConfig` removed; `scale.z` bug fixed — `ece80c1`
- [x] **`implicit_some` RON extension** — `ImplicitRonPlugin` enables `implicit_some` globally; 671 `Some()` wrappers removed — `4590d77`
- [x] **Nested prefabs — mesh support** — `spawn_primitive_children` dispatches on `kind`; GLB props nestable in composite prefabs — `d8d0ed5`
- [x] **Nested prefabs** — `children: [key, ...]` references; cycle detection; `village` demo in `primitive_world` — `6625775`
- [x] **Particle effect spawning v1** — campfire, torch, explosions, UV distort/scroll; pool renderer; multi-layer `EffectDef` — `27f097d`
- [x] **Silent failure diagnostics** — warn when `rules.ron` is skipped by `state_machine_path`; warn on `Spawn` position+spawn_point conflict; debug log when no rule matches an event — `762dfc1`
- [x] **Game stats Phase 1: core stat model** — `StatDef` (base/min/max/regen/thresholds); `LoadedStats` resource; `ModifyStat`/`SetStat` actions; threshold events — `270ff7e`

### April 2026
- [x] **Beta 0.4 — Entity Logic FSM** — per-entity `.behavior.ron`; `{self}` substitution; `TriggerZone`; `Interactable`; `SetVariable`/`IncrementVariable`; data-bound UI labels; `GameVariables` resource; `entity_logic_demo` project — `2235add`
- [x] **Beta 0.3 — Global Logic FSM v1** — `state_machine.ron`; state transitions; FSM-driven scene loading — `dca4fd0`

### January 2026
- [x] **Beta 0.2 — Event/Action Bus refactor** — `Action` enum moved to schema layer; rule-based bindings in RON; `message_interpreter_system` data-driven — `9ef2547`
- [x] **Beta 0.1 — Baseline Runtime** — declarative RON schemas for project config and scenes; `schema_version` enforcement; native + WASM builds stable — `434fb9a`
