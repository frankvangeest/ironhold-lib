# Claude Suggestions

> Raw development-time observations. Frank reviews and promotes good ones to `backlog.md`.
> Format: **title** _(observed at `<hash>` <YYYY-MM-DD>)_ — what + why, both one sentence.

---

## Correctness

- ~~**Panel Open/Close action arms share identical blocker logic — extract a helper**~~ _(observed at `ba01c5e` 2026-07-01; fixed `53643ca` 2026-07-01)_ — `LoadedInventoryUi::set_panel_open(bool)` now backs all 7 call sites.

- ~~**Overlay Backdrop (z=100) lacks `FocusPolicy::Block` — may only block clicks incidentally**~~ _(observed at `ba01c5e` 2026-06-30; fixed 2026-07-02, architect-reviewed, logged as a Bug not a feature — self-contained one-node fix)_ — Added `FocusPolicy::Block` + `Interaction::default()` to the "Overlay Backdrop" node, matching the panel-root pattern from `ec84bcf`. Regression tests in `tests/ui_panel_blocker.rs`.

## Performance


- ~~**Extend pipeline warmup to cover Text2d and UI pipelines**~~ _(promoted to backlog `e02d9e1` 2026-05-05; see `planning/features/pipeline_warmup_2d_ui.md`)_

- **Particle texture atlas for ≤4 total draw calls** _(observed at `0221d9e` 2026-05-19)_ — The pool renderer gives O(distinct textures) draw calls; the campfire's 6 Kenney flame sprites still cost 6 draw calls — packing them into one atlas PNG at startup (or as a prebuilt asset) and remapping UVs would hit the ≤4 target. Concrete basis: campfire_body uses sprites: [flame_01..04] = 4 groups, campfire_core [flame_05..06] = 2 groups.


## Particles / Visual

- **Glow halo layer on fire effects (SDR-safe "fake bloom")** _(observed at `a16bd98` 2026-05-23)_ — Add a third layer to `campfire_fire` (and similar fire effects): a large (size ~1.5), near-transparent (alpha ~0.06 start, 0.0 end), additive-blend, solid-orange quad with zero spread and slow drift upward; this approximates the soft halo bloom would add without touching HDR or post-processing. Concrete basis: investigated while implementing and reverting Bloom — the existing multi-layer EffectDef system supports this entirely in RON with no code changes.

## Testing

- ~~**Further split `integration_tests.rs` as it grows**~~ _(observed at `c07c1e0` 2026-05-27; promoted to backlog and fixed 2026-07-02, see `planning/features/done/split_integration_tests.md`)_ — split into 8 domain files (`fsm_tests.rs`, `entity_logic_tests.rs`, `scene_lifecycle_tests.rs`, `spawn_tests.rs`, `action_tests.rs`, `npc_tests.rs`, `nameplate_tests.rs`, `ui_tests.rs`) once the file reached 104 tests / 4258 lines.


## Audio

- ~~**CLI `validate` should cross-check scene button triggers against FSM rules**~~ _(observed at `517afe7` 2026-06-28; promoted to backlog and feature plan `3570198` 2026-07-02, see `planning/features/ui_trigger_reachability_check.md`)_ — `scene_loader.rs:1384` strips the `ui.` prefix from button `action` strings silently; a mis-prefixed button (e.g. `action: "toggle_mute"` instead of `action: "ui.toggle_mute"`) produces a different trigger that matches no FSM rule, giving the "button animates but nothing happens" symptom with no error at startup. A validator pass comparing each scene button's derived `ui.button_pressed:{trigger}` against all events handled in `rules.ron`/`state_machine.ron` would catch this at author time; fits the existing `query events` CLI surface.

- **Collapse dual `GlobalVolume` write in audio actions** _(observed at `43c5a84` 2026-06-10; architect-reviewed 2026-07-02 — leave as-is, no feature/backlog promotion)_ — `action_executor_system` writes `GlobalVolume` directly after mutating `AudioState`, but mutating `AudioState` also trips `is_changed()`, so `audio_state_system` writes it again the following frame; benign today (idempotent, one cheap `f32` write per user-triggered change, not per-frame) — two sources of truth in name only, not a real cost problem. **The naive fix is wrong**: stripping the executor's direct write and relying solely on `audio_state_system` (which runs `.before(message_interpreter_system)`, i.e. earlier in the frame than the executor) would defer the `GlobalVolume` write to frame N+1, while the already-playing-sink update loop (`bg_music_query`) stays in the executor and still applies in frame N — splitting one logical volume change across two frames (new sinks lag one frame behind live sinks), which is strictly worse than today's idempotent double-write. A correct single-writer collapse would also need to move the sink-update loop into `audio_state_system`. Not worth the churn unless `GlobalVolume` writes actually become expensive (unlikely — it's an in-memory float).

## Nameplate System

- ~~**Extract nameplate spawn-condition predicate to a single helper**~~ _(observed at `fcf8209` 2026-06-25; architect-reviewed 2026-07-02, promoted to backlog)_ — The `prefab.nameplate != Some(false) && (show || prefab.nameplate == Some(true))` guard is copy-pasted across 5 sites (not 5 in `scene_loader.rs` alone — 4 there plus `entity_spawner.rs:375`, whose `show` input is `nameplate_config.enabled` rather than `scene.show_nameplates`); extracting it to `fn should_insert_nameplate(nameplate: Option<bool>, show: bool) -> bool` prevents the sites from drifting, identical to what `tag_spawned_entity` was built to kill. Review also found a 6th, *different* (truncated) predicate at `action_executor.rs:163` — logged separately as a bug in `backlog.md` since it's a behavioral question (does the character-select player nameplate honor `show_nameplates`?), not just duplication.

- **Cache nameplate bar `Mesh`/`ColorMaterial` handles to avoid per-entity GPU allocation** _(observed at `fcf8209` 2026-06-25; architect-reviewed 2026-07-03 — leave as-is, no feature/backlog promotion)_ — `nameplate_setup_system` calls `meshes.add(Rectangle::new(bar_w, bar_h))` and `color_materials.add(ColorMaterial::from(...))` per entity per bar; since bar dimensions and colors are scene-global (`NameplateOptionsDef`), handles could be memoised in a `Local<HashMap>` keyed on `(ordered_float(bar_w), ordered_float(bar_h), [u32;4])`, matching the pattern in `target_indicator.rs`. **Premises confirmed accurate** (still holds after the v1/v2 nameplate rework — no per-entity override of width/height/color exists; the cache key would be correct), **but rejected on cost grounds**: `nameplate_setup_system` fires once per `Added<NameplateTag>` (spawn-bounded), not per-frame — a scene load produces only dozens of one-time `Assets<T>` insertions, and `Mesh2d`/`ColorMaterial` handles don't each trigger a new render pipeline compile, so there's no per-handle GPU state-churn analogous to real WASM footguns like pipeline-warmup stalls. `target_indicator.rs`'s memo exists because it re-resolves on every target switch (repeated, unbounded over a session) — nameplates resolve once per entity lifetime, a different cost profile the precedent doesn't transfer to. Revisit only if a future mode spawns hundreds+ nameplated entities per scene (e.g. a horde wave).

- **Two-writer Visibility contract on nameplate anchors** _(observed at `fcf8209` 2026-06-25)_ — Both `nameplate_visibility_system` and `world_label_screen_pos_system` write the anchor's `Visibility`; correctness relies on explicit `.after()` ordering and a force-hide-only policy in the nameplate system. If ordering ever changes, nameplates within distance could remain hidden. Hardening option: a `NameplatePolicyHidden` marker that `world_label_screen_pos_system` reads as a veto before setting `Visible`.

## Physics / Composite Prefabs

- **`trigger_zone` + `colliders` on the same entity — sensor ball is overwritten by compound** _(observed at `9f61177` 2026-06-27)_ — In `entity_spawner.rs::spawn_prefab_instance`, `trigger_zone` inserts `Collider::ball + Sensor` first (line 88), then `colliders` inserts `Collider::compound` which overwrites the single `Collider` slot (line 144); the intended 2.5 m ball sensor is silently replaced by the compound physical shapes (which are `Sensor`-marked but wrong size/shape). Fix: spawn the trigger zone sensor on a separate child entity (`TriggerZone + TriggerZoneId(name) + Collider::ball + Sensor`) so the two colliders coexist.

## Scene Loading

- ~~**Consolidate the 5 entity-spawn sites behind one "attach standard components" helper**~~ _(observed at `728c997` 2026-06-08; promoted to backlog `34bc77d` 2026-06-08 → Queued ▸ Engine / Runtime)_

- ~~**Insert `PrefabKey` on dynamic `Action::Spawn` spawns**~~ _(observed at `728c997` 2026-06-08; promoted to backlog `34bc77d` 2026-06-08 → Queued ▸ Engine / Runtime)_

- ~~**Consolidate conditional prefab-feature application (the sibling divergence)**~~ _(observed at `cef818a` 2026-06-08; promoted to backlog `cdee26b` 2026-06-08 → Queued ▸ Engine / Runtime)_

- ~~**`attach_prefab_features` and `spawn_prefab_instance` are two parallel feature lists**~~ _(observed at `6440b29` 2026-06-28; promoted to backlog 2026-06-28 → Queued ▸ Engine / Runtime; see `planning/features/unify_prefab_feature_application.md`)_ — After introducing `attach_prefab_features` for the primitive branches, a new `PrefabDef` capability field still requires wiring in **both** `attach_prefab_features` (scene_loader.rs) and `spawn_prefab_instance` (entity_spawner.rs) or GLB-vs-Primitive will diverge. Full unification would require refactoring the GLB path to also call `attach_prefab_features`, but that conflicts with the `ec` borrow pattern; a lighter fix is a code comment checklist at both function definitions.

- ~~**Push `stat_overrides` into `spawn_prefab_instance` so the StatMap is built once**~~ _(promoted to backlog `df8c94b` 2026-06-14 → Active ▸ Engine / Runtime)_

- ~~**Add optional `collider_radius`/`collider_height` to `NpcDef` for GLB actor sizing**~~ _(promoted to backlog `df8c94b` 2026-06-14 → Active ▸ Engine / Runtime)_

- ~~**Add integration test asserting GLB Actor `components.npc` attaches `NpcAgent` + `LocomotionState`**~~ _(promoted to backlog `df8c94b` 2026-06-14 → Active ▸ Engine / Runtime)_

- ~~**Dynamic `Action::Spawn` entities miss `motion`, `stat_label`, and `world_stat_bar`**~~ _(promoted to backlog `df8c94b` 2026-06-14 → Queued ▸ Engine / Runtime; see `planning/features/dynamic_spawn_components.md`)_

- ~~**Tune `collider_radius`/`collider_height` on `enemy_snake` and `enemy_spider` prefabs**~~ _(promoted to backlog `0f86e07` 2026-06-17 → Queued ▸ Gameplay & Environment; see `planning/features/collider_tuning_creatures.md`)_

- ~~**Per-prefab `depth_scale: Some(true)` override silently ignored on dynamic spawns**~~ _(promoted to backlog `0f86e07` 2026-06-17 → Queued ▸ Engine / Runtime; see `planning/features/depth_scale_dynamic_spawn.md`)_

- ~~**Extract `assemble_player_config` shared helper to prevent scene-loader/executor drift**~~ _(dropped 2026-06-23 — covered by the Queued "Consolidate conditional prefab-feature application" item)_

- **Tune `SELECT_PIXEL_RADIUS` for tighter click-to-select feel** _(observed at `16eccff` 2026-06-17)_ — The current 70 px screen-space selection radius feels loose at close range (can click 1–2 m to the side of an enemy and still select); dropping to 40–50 px would require more deliberate targeting without being too tight for casual play. Concrete basis: play-test observation with hitbox debug overlay showing the mismatch between the visible sphere and the selectable area.

- **Add skybox or procedural atmosphere to remove "all ground" background** _(observed at `d9d0232` 2026-06-16)_ — Without a skybox the ClearColor (dark grey) fills any area not covered by geometry; the large 100×100 sand ground plane covers the entire screen at max camera pitch, making the whole background sandy. A Bevy `EnvironmentMapLight` cube map or a procedural sky dome (gradient blue) would fill the background correctly at any pitch angle and dramatically improve scene readability at steep camera angles.

- **Add collider-tuning doc example + discovery hint for NPC capsules** _(observed at `be229b7` 2026-06-17)_ — `docs/20_data_formats.md` NpcDef table has `collider_radius`/`collider_height` but no worked example in the doc prefab; the `enemy_snake`/`enemy_spider` prefabs are now the only live examples of non-humanoid capsule sizing. Adding a `rat` example to the doc table and a one-liner ("start from the model's visual height, or run `ironhold inspect glb` to read bounds") near line 1288 would make capsule tuning self-discoverable for designers authoring new creatures.

- ~~**`spawn_prefab_instance` capability checklist — new prefab fields silently skip GLB actors**~~ _(dropped 2026-06-23 — fully covered by the Queued "Consolidate conditional prefab-feature application" item; that fix eliminates this whole class of bug)_

- **Extract `InventoryParams` from `SceneStateParams` to split the god-param** _(observed at `c7de3f3` 2026-06-20)_ — `SceneStateParams` in `mod.rs` now has ~25 heterogeneous fields across audio, targeting, dialogue, NPC, camera, stats, and inventory; extracting the 6 inventory-specific fields (`player_inventory`, `loaded_item_catalog`, `inventory_ui`, `container_inventories`, `inventory_panel_q`, `shop_panel_q`) into an `InventoryParams` `SystemParam` bundle would make the inventory surface independently auditable. No correctness issue — `SystemParam` derive nests tuples without hitting Bevy's 16-param ceiling.

- ~~**CLI `--strict` cross-validate merchant `currency_stat` and `item_key` against catalogs**~~ _(promoted to backlog 2026-06-23 → Queued ▸ Designer Experience)_

- ~~**Stale `spider.hide:{self}` delay timer could hide a newly-respawned spider**~~ _(promoted to backlog 2026-06-23 → Bugs)_

## UI

- **`Label` nodes wrap and overflow with no clipping — easy to author an overlap** _(observed while building `local_coop_demo`'s room2 scene, 2026-07-05)_ — `LabelDef.size` is a fixed-pixel `Node` width/height (`Val::Px`, `scene_loader.rs:1314-1316`) with no `overflow: Overflow::clip()` set, and `font_size` is hardcoded to `22.0` with no per-label RON override; a designer who doesn't do the width/character-count math by hand gets text that silently wraps and visually overflows past the declared box into whatever UI element sits below it (exactly what happened here — two stacked labels overlapped). Consider either adding `overflow: clip` by default (so at worst text is cut off, not overlapping something else) or exposing `font_size` on `LabelDef` so designers can shrink text to fit instead of guessing pixel widths.

## Animation

- **`jump_enter`/`jump_exit` sentinel fallback logs a designer-facing warning for a normal, valid policy state** _(observed while building `local_coop_demo`, 2026-07-04)_ — `player_movement_system` unconditionally fires these two override IDs on every jump/landing for every player prefab; when a policy has no matching override (a legitimate minimal locomotion-only policy, not a typo), `animation_resolver_system` falls through to a literal-clip-name lookup and logs `WARN ... falling back to idle` every single jump — indistinguishable in the log from an actual designer typo in a real clip name. Consider either recognizing these two IDs as engine-internal sentinels and silently no-opping when unmatched, or downgrading the log level specifically for them, so real clip-name typos stay visible without jump-spam noise.

## Camera

- **`PartyOrbitCamera` duplicates `OrbitCamera`'s tuning fields and mouse-orbit block** _(observed at local co-op Stage 1 implementation, 2026-07-04)_ — `capabilities/camera.rs`'s new `PartyOrbitCamera` (added for local co-op's shared camera) repeats ~10 fields from `OrbitCamera` and mirrors `camera_orbit_system`'s mouse-orbit handling almost line-for-line in `party_camera_follow_system`, because unifying it under the existing Icebox "Camera mode unification (v1)" item would have coupled Stage 1 to an unstarted refactor. When that unification work starts, it should fold `PartyOrbitCamera` in as a third mode (distance-derived-radius + multi-target midpoint), not just `OrbitCamera`/`FlyCamera`.

- **Four systems assume exactly one `Camera3d` exists, with no in-code marker of that assumption** _(observed during local co-op Stage 3 planning, 2026-07-05, before any split-screen code was written)_ — `world_label_screen_pos_system` (`lib.rs:501`), `nameplate.rs:212`, `particle_renderer.rs:303`, and `targeting.rs:122` each query `With<Camera3d>` and either `.single()` (returns a graceful no-op on 2+ matches, doesn't panic) or `.iter().find(...)` (arbitrarily picks one camera). None crash, but once split-screen ships (Stage 3+, two real `Camera3d` entities per scene), any project combining split-screen with world labels, nameplates, particles, or click-targeting gets silent, undocumented degradation — e.g. click-targeting would only ever resolve against whichever camera `.find()` happens to return, not whichever half the player actually clicked in. Unlike the `PartyOrbitCamera`/`CameraShake` limitation above (documented at `camera.rs:107-109`), these four sites carry no comment — a future dev debugging "targeting doesn't work right in my split-screen project" will have to rediscover this from scratch. Worth a one-line comment at each site, or a shared multi-camera-aware helper once a second real consumer (beyond split-screen) needs one.

## World Design / Gameplay

- ~~**Item-gated `interactable` (condition on inventory possession)**~~ _(promoted to backlog 2026-06-23 → Queued ▸ Gameplay & Environment)_

- ~~**Zone-based ambient audio swap on TriggerZone enter/exit**~~ _(promoted to backlog 2026-06-23 → maps to existing Queued ▸ Sound zones item)_

- ~~**Conditionally-shown dialogue choices gated on a GameVariable**~~ _(promoted to backlog 2026-06-23 → Queued ▸ Gameplay & Environment)_

