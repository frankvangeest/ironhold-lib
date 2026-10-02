# Feature: Fade-Out-Then-Despawn for `SetDespawnTimer` (`fade_secs` / `fade_mode`)

_Status: Draft_
_Planned at: `29b868d` (2026-10-02)_

Backlog item: **Fade-out-then-despawn for `SetDespawnTimer`/`Action::Despawn`** (`## Rendering & Assets`), re-requested
for looted corpses "instead of just popping out". Related, **not** in this plan: "Reduce looted-corpse decay timer from
20s to 2s" (same RON line, see Open question 2), `gameplay_fixed_tick_pipeline.md` (re-homes the timers this plan
extends), a dedicated corpse `AlphaMode`/dissolve shader (rejected below).
Written autonomously while Frank was away; not plan-reviewed yet.

## What
A designer adds two optional fields to the existing timer action and the entity eases out instead of popping:

```ron
// lootable_corpse.behavior.ron, "looted" state
SetDespawnTimer(entity: "{self}", delay_secs: 18.5, fade_secs: 1.5),                 // default fade_mode: Fade
SetDespawnTimer(entity: "{self}", delay_secs: 18.5, fade_secs: 1.5, fade_mode: Shrink),
```

After `delay_secs` the entity spends `fade_secs` fading out (`Fade`: all its standard materials go transparent;
`Shrink`: it scales to nothing around its origin, no transparency, no new assets), then despawns through the
unchanged `Action::Despawn` path. Both fields are optional; omitting them is byte-for-byte today's behaviour. An
instant pop that wants a fade is `SetDespawnTimer(entity: .., delay_secs: 0.0, fade_secs: 1.0)`. `Action::Despawn`
itself stays instant by contract.

## Why
Corpses (and anything else despawned by timer: the live monsters after their death swap, `seal_door`) vanish in a
single frame. Frank asked twice (2026-08-31, 2026-09-14). It is the only polish gap in the loot loop, and it must be
authored in RON (no corpse-specific Rust) to stay inside the designer-reachability rule.

## Findings (verified against code at `29b868d`)
1. **The timer is a component on the entity, and its expiry already routes through `Action::Despawn`.**
   `DespawnTimer { remaining_secs }` (`despawn_timer.rs:27`), `despawn_timer_system` (`:46-68`) decrements by
   `time.delta_secs()` (`Update`, `lib.rs:367`), removes the component, and pushes `Action::Despawn(id)`. The executor's
   `Action::Despawn` (`action_executor.rs:293-325`) does `try_despawn`, removes the id from `SpawnRegistry.entities`
   and closes the container panel if that entity was `active_container`. The fade must hang off this seam so the
   registry/panel teardown stays in one place.
2. **`SetDespawnTimer` has 6 exhaustive-ish touch points.** Declaration `actions.rs:67-70`; executor arm
   `action_executor.rs:326-342`; `rewrite_self` `action_substitution.rs:26-29` and `rewrite_target` `:185-186`;
   `dialogue.rs:324-325` (`substitute_self_in_action`, destructures by name); the test-local mirror
   `corpse_loot_interact_tests.rs:139-140`. Not affected: `action_bar.rs:416` and `query.rs:545` (both use `{ .. }`).
   All name-destructure and rebuild, so adding fields is a compile error at each, not a silent drop. `Action` is
   `deny_unknown_fields` and derives `PartialEq` (`actions.rs:4-5`); a new `#[serde(default)]` field on an existing
   struct variant is not a breaking RON change (only renames/removals are, `src/CLAUDE.md` "Adding new actions").
3. **GLB materials are shared and never cloned.** `model_spawner.rs:51` spawns `SceneRoot(asset_server.load(path))`;
   a grep of `src/` finds no `StandardMaterial` clone/`materials.add` on spawned GLB children (only
   `material_factory.rs`' catalog build and the decal/indicator/particle materials). `apply_material_overrides`
   (`material_factory.rs:~205-225`) inserts the *same* catalog handle on every target mesh. So a zombie corpse shares
   its handles with every live zombie of the same GLB: mutating `Assets<StandardMaterial>` alpha in place would fade
   all of them. Confirms the backlog's question (1).
4. **The decal precedent fades a private material only.** `fading_decal_system` (`decal.rs:~102-153`) mutates the
   plane's own `MeshMaterial3d<StandardMaterial>` (built per instance at `:58-67`, `AlphaMode::Blend` from creation).
   `damage_popup.rs` fades `TextColor`; `fading_light.rs` fades `PointLight.intensity` and caps at
   `MAX_FADING_LIGHTS = 16` (`:8`), a cap precedent reused below.
5. **Corpses are cheap to fade.** `zombie_corpse` (`prefabs.ron:439-456`): `kind: Prop`, `nameplate: false`, no
   `colliders`, no `stat_templates`/world bars, `click_selectable: true`, `interactable: (radius: 2.0)`, `inventory`.
   A *live* monster in the same file carries nameplate/stat widgets, so fading non-corpse entities needs widget handling.
6. **Cosmetic widgets are keyed on removal.** `nameplate_cleanup_system` (`nameplate.rs:303-321`) despawns anchors on
   `RemovedComponents<NameplateTag>`; `stat_widget_cleanup_system` (`stat_display.rs:~28-40`) does the same on
   `RemovedComponents<SpawnId>`. During a fade the entity still has both, so widgets would linger until the final despawn.
7. **Selection/targeting survive a fade unless removed.** `target_auto_clear_system` (`targeting.rs:373-393`) clears only
   on `Visibility::Hidden` or registry absence; `ClickSelectable` (`:25`) and `Targetable` (`:36`) are plain marker
   components; `Interactable` (`interactable.rs:19`) is distance-based, not collider-based.
8. **Pipeline-variant risk is documented.** `src/CLAUDE.md:1644`: every new mesh+material combination costs a
   synchronous `createRenderPipeline()` (~100-300 ms) on first draw on WASM; `pipeline_warmup_system` (`lib.rs:446`) only
   toggles `NoFrustumCulling`, it does not pre-create blend variants. No `Msaa` override exists in `src/` (grep), so the
   Bevy default applies.
9. **Timers are slated for `FixedUpdate`.** `gameplay_fixed_tick_pipeline.md` Phase 1 moves `despawn_timer_system` there
   (`Res<Time>` becomes `Time<Fixed>`, constant dt). Whatever is added must work unchanged under either clock.
10. **Existing tests are not broken by this**: `looted_corpse_transitions_to_looted_and_decays_quickly`
    (`corpse_loot_interact_tests.rs:591-620`) and `unlooted_corpse_decays_after_ten_minutes` only change if the demo RON
    gets `fade_secs` (they advance 20.5 s / 601 s; a 1.5 s fade on top of `delay_secs: 18.5` still lands inside 20.5 s).

## Approach

### Decisions
| # | Question | Decision | Rejected |
|---|---|---|---|
| 1 | Shared-handle materials | **Clone per instance at fade start**, deduped per distinct source handle, written back to that entity's mesh descendants only | Mutating the shared handle (Finding 3: fades every sibling). Swapping to a "fade" catalog material via `MaterialOverride` (loses the GLB's textures). A custom dissolve shader/`ExtendedMaterial` with a global uniform (new pipeline family, WGSL alignment rules, far more code than the feature warrants) |
| 2 | Alpha mode | `Opaque`/`Mask` -> `Blend`; already-`Blend` kept; `Premultiplied`/`Add`/`Multiply` or any non-`StandardMaterial` mesh (`CustomMaterial`, terrain, foliage, flame) -> whole entity falls back to `Shrink` with one `warn!` | `AlphaToCoverage` (relies on MSAA being on in the browser, still a new pipeline variant, dither look; revisit only if Blend self-overlap looks bad). `Mask` ramp (binary pop). Cloning custom materials (each has bespoke uniforms; out of proportion) |
| 3 | Schema surface | **Two defaulted fields on `SetDespawnTimer`**: `fade_secs: f32` (default `0.0`) and `fade_mode: DespawnFadeMode` (`Fade` default, `Shrink`) | A separate `Action::FadeOutAndDespawn` (a second timer concept, forces `rewrite_self`/`rewrite_target`/`dialogue.rs`/`action_needs_target`/CLI arms for a duplicate of the same fields, and two ways to author "despawn later"). A prefab/component field (a hidden global behaviour change for every despawn path; also `LoadScene` teardown must stay instant). A `fade_secs` on `Action::Despawn(String)` (tuple variant, would break every call site, see the `PlayAnimation`/`freeze` note in `docs/20`) |
| 4 | Where fade state lives | A `FadingOut` component on the entity, ticked in `despawn_timer.rs` next to `DespawnTimer` (so it re-homes to `FixedUpdate` with it) | A global resource of fading entities (id-reuse hazard that motivated `DespawnTimer`, see its doc comment) |
| 5 | Offer a non-blending mode? | **Yes, `Shrink`**: zero assets, zero pipeline variants, valid for every material type, also the automatic fallback and the over-cap fallback | `Sink` into the ground (needs the model's height and ground contact; no authoring data for either) |

### Types (`schema/actions.rs`, `capabilities/despawn_timer.rs`)
```rust
#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq)]
pub enum DespawnFadeMode { #[default] Fade, Shrink }

SetDespawnTimer {
    entity: String,
    delay_secs: f32,
    #[serde(default)] fade_secs: f32,
    #[serde(default)] fade_mode: DespawnFadeMode,
}

#[derive(Component)] pub struct DespawnTimer { pub remaining_secs: f32, pub fade_secs: f32, pub fade_mode: DespawnFadeMode }
#[derive(Component)] pub struct FadingOut { pub total_secs: f32, pub remaining_secs: f32, pub mode: DespawnFadeMode }
```
Progress `t = 1 - remaining/total` is a pure function of accumulated dt (same f32 subtraction pattern as
`DespawnTimer`), never of query order or wall time.

### Behaviour
1. **Timer expiry** (`despawn_timer_system`): if `fade_secs > 0` (clamped, non-finite -> 0), remove `DespawnTimer`, insert
   `FadingOut`, and do **not** push `Despawn` yet. Otherwise unchanged. A second system arm ticks `FadingOut.remaining_secs`
   in the same function (or a sibling `fading_out_tick_system` registered next to it) and, at `<= 0`, removes the component
   and pushes `Action::Despawn(id)` (same `SpawnId`-less fallback `try_despawn` as today).
2. **Fade start** (cosmetic, `Update`, `Added<FadingOut>` in `fade_out_visual_system`, new file `capabilities/fade_out.rs`):
   - `Fade`: walk `Children::iter_descendants` (deterministic child order) collecting `(Entity, Handle<StandardMaterial>)`;
     if any descendant mesh carries another material type, or any source material is Premultiplied/Add/Multiply, or the
     global `FadingMaterialBudget` (cap `MAX_FADING_MATERIAL_ENTITIES = 16`, mirrors `MAX_FADING_LIGHTS`) is spent,
     switch this entity's mode to `Shrink` and `warn!` once. Otherwise clone each *distinct* source material
     (`HashMap<AssetId, Handle>`), set `alpha_mode = Blend`, insert the new `MeshMaterial3d` on each mesh, and store
     `FadeMaterials { handles: Vec<(Handle<StandardMaterial>, f32 /*base alpha*/)> }`. Old handles drop; the clones are freed
     automatically when the entity despawns (strong handles on the dying entity).
   - `Shrink`: store the root's `Transform.scale` as `FadeStartScale`.
   - Both: remove `Interactable`, `ClickSelectable`, `Targetable`, `NameplateTag` (anchor goes via the existing cleanup), and
     despawn any `WorldLabel` widgets whose `tracked_entity` is this entity (one query pass per *fade start*, not per frame).
     The entity **stays in `SpawnRegistry`** until the real `Action::Despawn`, so id-based actions (`SetEntityVisible`,
     `PlayAnimationOn`) and the container-close teardown at despawn keep working; `target_auto_clear_system` is *not* relied on
     (the markers are gone, so tab/click cannot re-acquire it; an already-held target is cleared at despawn as today).
3. **Each frame** (`fade_out_visual_system`): `Fade` writes `base_color.alpha = base_alpha * (1 - t)` into each cloned
   asset (`get_mut`, one per distinct material); `Shrink` writes `Transform.scale = start * (1 - t).max(0.001)`.
   Read-only on sim state; touches only the entity's own components/assets, so it needs no ordering against anything
   except running after the tick (`.after(despawn_timer_system)`).
4. **Re-arming:** `SetDespawnTimer` on an entity that already has `FadingOut` is ignored (`debug!`); re-arming during a plain
   countdown overwrites all four fields, matching today's "overwrite, don't stack". `LoadScene` teardown despawns directly:
   components and cloned handles go with the entity.
5. **Unchanged on purpose:** colliders/sensors stay solid during the fade (a fading `seal_door` still blocks until it is
   gone; corpses have none); animation keeps running (a frozen corpse pose stays frozen); shadows are left to the engine
   (see risk below).

### Cost model (WASM)
- Per fading entity: one `Assets::add` per distinct material at fade start (typically 1-3 for a monster GLB; no image
  copies, textures are refcounted handles), then one `get_mut` per distinct material per frame. Bevy re-prepares changed
  materials, so ~N materials of bind-group work per frame; cap 16 entities bounds it, anything beyond degrades to `Shrink`.
- Pipeline: first `Fade` of a mesh class (skinned + `Blend`) compiles one extra variant (~100-300 ms stall, Finding 8).
  Not pre-warmed in v1: measure in the dev playtest; if it is visible, add a warmup following the particle-warmup pattern
  (`src/CLAUDE.md:1724-1744`) as a follow-up task, not a prerequisite. `Shrink` has no such cost.
- Known visual risks to check in the playtest: Blend on a multi-primitive skinned mesh with depth write off can show limbs
  through the torso (acceptable for 1-2 s; if ugly, offer `AlphaToCoverage` as a third mode); the shadow of a `Blend`
  mesh may stay full strength until despawn (if so, insert `NotShadowCaster` at fade start, `scene_loader.rs:572` pattern).

## Tasks
- [ ] Re-verify Findings 2 and 3 with `grep` for `SetDespawnTimer` / `StandardMaterial` clone sites at implementation time.
- [ ] `schema/actions.rs`: `DespawnFadeMode`, the two defaulted fields + doc comment; update the six touch points in Finding 2
  (including the test-local mirror) to carry the fields through; `action_executor.rs` arm stores them on `DespawnTimer`
  and skips arming when `FadingOut` is present.
- [ ] `capabilities/despawn_timer.rs`: `fade_secs`/`fade_mode` on `DespawnTimer`, `FadingOut`, expiry hand-off, tick + final
  `Action::Despawn`; update module docs. Register in `capabilities/mod.rs` and `lib.rs` next to `despawn_timer_system`.
- [ ] `capabilities/fade_out.rs`: `fade_out_visual_system`, `FadeMaterials`, `FadingMaterialBudget`, fallback rules, widget and
  marker removal at fade start.
- [ ] Demo: `lootable_corpse.behavior.ron` both states get `fade_secs` (keep total visible time unchanged: `18.5 + 1.5`);
  `enemy_zombie/snake/spider.behavior.ron` death-swap timers stay instant (bit-identical overlap with the corpse; a fade
  there would flash the live model through the corpse).
- [ ] Tests (new `tests/fade_out_tests.rs`, `setup_test_app()`; synthetic entities with child `Mesh3d` + `MeshMaterial3d`,
  as `particle_tests.rs` does for decals; one `app.update()` = one 1/64 s tick): shared-handle isolation (two entities share
  one material; fading A leaves B's asset and handle untouched), alpha monotonic and exactly 0 at end, `Opaque`/`Mask` become
  `Blend`, `Premultiplied`/custom-material -> `Shrink` fallback, over-cap fallback, `Shrink` scale curve (starts at the
  authored scale, ends near 0), `fade_secs: 0` identical to today (same tick as before), id leaves `SpawnRegistry` only at
  the end, open container panel on a fading corpse still closes at despawn (extend the existing corpse test pattern),
  `Interactable`/`ClickSelectable`/`Targetable` gone at fade start, re-arm during fade ignored, cloned handles dropped
  after despawn (`Assets::len` returns to baseline), RON round-trip of both field forms in `ron_validation.rs`.
- [ ] Update `corpse_loot_interact_tests.rs` expectations only if timings in the demo RON change (Finding 10).
- [ ] Docs: `docs/20_data_formats.md:3820` (new fields, `fade_mode` table, "`Action::Despawn` stays instant", fallback rules),
  `docs/30_runtime_events_and_logic.md` (corpse decay section), `src/CLAUDE.md` (despawn-timer paragraph ~L535: fade state and
  per-instance clone rule), `assets/projects/CLAUDE.md` if it lists the action.
- [ ] CLI: no new validation exists for `delay_secs` today, so none for `fade_secs` (runtime clamps); `query actions` prints
  kind names only. Run `cargo check -p ironhold_cli` (mandatory) and spot-check
  `cargo run -p ironhold_cli -- query actions assets/projects/3rd_person_game_demo` plus `validate` on the demo project, then
  rebuild the cached `tools/bin/ironhold`.
- [ ] Reviews in one parallel message: `alignment-reviewer`, `system-architect`, `debug-detective`, `ux-gamedesigner-reviewer`
  (RON/docs change), `wasm-perf-reviewer` (per-frame material mutation, new blend variant); then full test suite one file at a time.
- [ ] `ron_lint` + `ron_validation` after any `assets/projects/` edit; WASM dev build (`--features inspector`) for the playtest.

## Playtest checklist
Project: `3rd_person_game_demo`. Serve with `python serve.py`.
1. Kill a zombie, wait for the corpse, loot it fully: after the delay it fades smoothly over ~1.5 s and disappears; no pop.
2. Kill two zombies close together; fade one corpse while the other (and a still-living zombie of the same GLB) stays fully opaque.
3. Open a corpse's loot panel and let its fade run out: panel closes at despawn, no stuck UI, interact/targeting work after.
4. While a corpse fades: F does not open it, tab/click do not select it, no nameplate/target ring lingers.
5. Temporarily set `fade_mode: Shrink` in `lootable_corpse.behavior.ron`: corpse shrinks into the ground point, no transparency.
6. Watch the first-ever fade on a cold load (browser): note any visible hitch (pipeline compile), then F9/inspector check for limb-through-torso and shadow behaviour.
7. Kill 20+ monsters quickly with corpses fading together: no frame-time spike beyond the first fade; overflow corpses shrink.

## Open questions
1. **Default `fade_mode`.** `Fade` looks right but costs a one-off pipeline compile on WASM and has Blend sorting artefacts;
   `Shrink` is free and robust. Plan defaults to `Fade` with automatic `Shrink` fallback; say if you would rather ship
   `Shrink` as the default and `Fade` opt-in.
2. **Fold in "looted corpse decay 20s -> 2s"?** It edits the same RON line. Plan keeps total visible time at 20 s
   (`18.5 + 1.5`) to avoid pre-empting that item; if you want 2 s, it becomes `delay_secs: 0.5, fade_secs: 1.5` and the test at
   `corpse_loot_interact_tests.rs:591` changes its `advance`.
3. **Colliders during the fade.** Plan leaves them solid. If you want a fading door/crate to stop blocking at fade start,
   that is a one-line addition but changes physics mid-fade (not deterministic-sensitive, but visible).

## Acceptance criteria
- Given `SetDespawnTimer(.., delay_secs: D, fade_secs: F)`, when `D` elapses, then the entity fades/shrinks for `F` seconds and
  is despawned (registry entry removed, panel closed) on the tick the fade ends; with `fade_secs` omitted nothing changes.
- Given two entities sharing a GLB material handle, when one fades, then the other's material is untouched.
- Given an entity with a custom/terrain/premultiplied material or the fade budget exhausted, then it shrinks instead and logs once.
- Given a fading entity, then it cannot be interacted with, tab-targeted, click-selected or nameplated.
- Given identical inputs at 30/60/144 fps, then fade end (and the resulting `Action::Despawn`) occurs on the same tick.
- `cargo test -p ironhold_core` (one file at a time) and `cargo check -p ironhold_cli` pass; docs updated.
