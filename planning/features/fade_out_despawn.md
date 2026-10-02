# Feature: Fade-Out-Then-Despawn for `SetDespawnTimer` (`fade_secs` / `fade_mode`)

_Status: Draft — plan-review 2026-10-02: needs more design work (see "Plan-review" section at the end)_
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

## Plan-review (2026-10-02)

Run by `/plan-review` while Frank was away: system-architect and ux-gamedesigner-reviewer, in parallel, read-only, claims checked against the code. **Combined verdict: needs more design work** — the blocking items below must be folded into this plan (and Frank's answers to the open questions recorded) before it can be marked Ready. The reports are reproduced verbatim (headings demoted one level).

### System-architect review

## System-architect plan-review: `planning/features/fade_out_despawn.md`

Reviewed at `34803b1` (integration). The plan's `Planned at` is `29b868d`. Nothing in the affected code changed between the two (the commits since then are planning docs only).

#### Verdict
**Needs more design work.** The plan is small in scope, built on the right seams, and its key decisions hold up: two defaulted fields on `SetDespawnTimer`, a component on the entity rather than a global resource, per-instance clones of the materials, and `Shrink` as the fallback that needs no assets. Findings 1-8 check out against the code. Three items would produce a wrong implementation or a false acceptance criterion. Each one needs only a plan-text change, not a redesign.

Verified claims:
- `despawn_timer.rs:27,46-68`, and `lib.rs:367` (the timer is `Update` and unordered).
- `action_executor.rs:293-325` (Despawn: `try_despawn`, removes the registry entry, closes the container) and `:326-342`.
- `actions.rs:4-5` (`deny_unknown_fields`, `PartialEq`) and `:67-70`.
- `action_substitution.rs:26-29,185-186`, `dialogue.rs:324-325`, `corpse_loot_interact_tests.rs:139-140`.
- `action_bar.rs:416` and `query.rs:545` both use `{ .. }`, so they are safe.
- `material_factory.rs:184-230`: one shared catalog handle per mesh, and the system waits for GLTF children.
- `zombie_corpse` uses `model: "zombie"`, the same GLB as the live zombie, so its materials really are shared.
- `nameplate.rs:303-321` and `stat_display.rs:27-41` are keyed on removal.
- `docs/20_data_formats.md:3820`.

Adding `#[serde(default)]` fields to an existing struct variant is non-breaking under `deny_unknown_fields`. That is correct.

#### Blocking

**B1. Gameplay-state changes are placed in the render-rate visual system, which contradicts the plan's own "read-only on sim state" rule and the D3 / fixed-tick placement.**
- **Where:** Behaviour 2 (fade start, `Update`, `Added<FadingOut>` in `fade_out_visual_system`) removes `Interactable`, `ClickSelectable` and `Targetable`. Behaviour 3 then says the same system is "read-only on sim state". Behaviour 1 also leaves open "(or a sibling `fading_out_tick_system`)" for the system that pushes `Action::Despawn`.
- **Why it matters:**
  - Interactability and targetability are gameplay state. `gameplay_fixed_tick_pipeline.md` §1 and §3 make `interactable` and tab-targeting fixed-tick systems, and say "capture systems only translate and never read sim state". Cosmetic systems stay on the render clock.
  - If the markers are removed in a render-rate system triggered by `Added<>`, the window in which F, Tab or a click can still reach a fading corpse depends on how the frame and tick line up.
  - Separately, a sibling tick system that pushes `ActionQueue` is a new pusher. D3 (`gameplay_pipeline_system_sets.md` §2 table, `EmitSet::DirectActions` = `resolve_pending_behaviors_system → despawn_timer_system`, plus its structure test in §5) hard-fails when a pusher is missing from the declared sets. Whichever of the two features lands second would then have to patch the other.
- **Correction to the plan text:**
  - Behaviour 1: the `FadingOut` tick lives **inside `despawn_timer_system`** as a second query in the same function. Drop "or a sibling". It then inherits D3's `EmitSet::DirectActions` slot and the Phase-1 `FixedUpdate` move with no extra edges.
  - In the expiry branch, issue `commands.entity(e).insert(FadingOut{..}).remove::<(Interactable, ClickSelectable, Targetable)>()` in one go.
  - Behaviour 2 keeps only cosmetic work: material clones, the `FadeStartScale` snapshot, `NameplateTag` removal and despawning `WorldLabel` widgets.
  - Add a test that the markers are gone after the same `app.update()` in which the timer expired, independent of the visual system.

**B2. Fade start is a one-shot `Added<FadingOut>` trigger, which can fire before the meshes and materials it needs exist. The advertised authoring pattern then silently pops instead of fading.**
- **Where:** Behaviour 2 walks `Children::iter_descendants` once, on `Added<FadingOut>`. "What" advertises `SetDespawnTimer(entity: .., delay_secs: 0.0, fade_secs: 1.0)` as "an instant pop that wants a fade".
- **Why it matters:**
  - Mesh children may not exist yet. A GLB spawned via `Action::Spawn` (`model_spawner.rs:51` is `SceneRoot(asset_server.load(..))`) has no `Mesh3d` descendants until `SceneSpawner` instances it, which can take several frames on WASM.
  - The shared material can come back after the clone. `apply_material_overrides` (`material_factory.rs:198-203`) waits for children and then inserts the **shared** catalog handle on every mesh. If it runs after the clone, it overwrites the clone. The per-frame alpha writes then go to an asset nothing displays, and the entity pops at the end.
  - The plan defines no outcome for the "zero meshes found" case.
  - A third mechanism compounds this: the WASM GLTF-hierarchy respawn path documented in `src/CLAUDE.md` (animation section) replaces mesh entities with ones that reference the original shared handles.
- **Correction to the plan text:**
  - Replace `Added<FadingOut>` with a "pending until applied" query: `Query<.., (With<FadingOut>, Without<FadeApplied>)>`.
  - Treat an entity as **not ready** while it has `PendingMaterialOverride` or has zero `Mesh3d` descendants. Retry on the next frame.
  - If it is still not ready after N frames (for example 8), or the fade is already more than about 50% through, degrade to `Shrink`.
  - Add a test that spawns the root, arms `delay_secs: 0, fade_secs: 1`, and adds the mesh children two updates later. It asserts that the child ends up with the cloned handle and alpha below 1.
  - Also define the type fallback as "any `Mesh3d` descendant **without** `MeshMaterial3d<StandardMaterial>` → Shrink". That replaces "carries another material type", which would need a query per material type (Custom, Terrain, Foliage, Flame, PoolFlame, …) and breaks silently when a new one is added.

**B3. The frame-rate acceptance criterion is false against current code.**
- **Where:** Acceptance criteria: "Given identical inputs at 30/60/144 fps, then fade end (and the resulting `Action::Despawn`) occurs on the same tick."
- **Why it matters:** `despawn_timer_system` runs in `Update` (`lib.rs:367`) and decrements by `time.delta_secs()` (`despawn_timer.rs:52`), which is variable-dt. Neither the existing timer nor this fade can meet the criterion until `gameplay_fixed_tick_pipeline.md` Phase 1 moves the system to `FixedUpdate`. An implementer would either write a test that fails or quietly skip it.
- **Correction:** Reword it to: "fade progress is a pure function of accumulated sim `dt` (no wall-clock reads, no query-order dependence); once `gameplay_fixed_tick_pipeline.md` Phase 1 lands, fade end occurs on the same `SimTick` at 30/60/144 fps (that plan's Phase-1 timer test gets one fade case added)". Add the matching task line to the fixed-tick plan rather than to this one.

#### Non-blocking

1. **Shadow and pipeline facts are wrong or vague in "Cost model", verified against vendored `bevy_pbr-0.18.0`.**
   - `Blend` meshes **do** cast shadows. `render/light.rs:1901-1907` adds `MAY_DISCARD` for `Blend`, and `render/pbr_prepass_functions.wgsl:73-78` discards only below `PREMULTIPLIED_ALPHA_CUTOFF = 0.05`. So the shadow stays at full strength for about 97% of the fade and then pops.
   - That shadow pass is a **second** new pipeline variant (shadow + `MAY_DISCARD` + skinned), on top of the `Transparent3d` main-pass variant. The cost is up to 2 compiles, not 1.
   - The depth prepass simply drops `Blend` (`prepass/mod.rs:913-923`), so that adds no variant.
   - Recommendation: decide now, not in the playtest. Insert `NotShadowCaster` on the mesh descendants at fade start. It saves one WASM compile, and a shadow that disappears at fade start reads as part of the fade rather than a late pop.
   - The MSAA note is fine: `Blend` does not depend on MSAA, and no `Msaa` override exists in `src/`.
2. **`Shrink` scales colliders, which contradicts "colliders stay solid".**
   - bevy_rapier3d 0.33 `apply_scale` (`plugin/systems/collider.rs:48-60`) rescales every `Collider` on `Changed<GlobalTransform>`. `Shrink` on a root that has colliders (seal_door, any prop) shrinks its physics geometry every frame, down to 0.001× scale.
   - Because `Shrink` is also the automatic over-budget and custom-material fallback, a render-side budget would decide whether an entity's physics changes mid-fade.
   - No shipped content hits this today: corpses have no colliders, and seal_door's timer stays instant per the plan. Still, fix the plan text. Either strip colliders and sensors at fade start when the mode resolves to `Shrink` (this ties into Open question 3), or never auto-fall-back to `Shrink` for an entity with a `Collider` in its hierarchy. In that case it fades nothing and despawns at the end, with a `warn!`.
3. **The budget is a resource with no release path.** `FadingMaterialBudget` is described as "spent" but never refunded on despawn or `LoadScene`. Follow `particle.rs:272`'s `MAX_FADING_LIGHTS` precedent instead: live count = `Query<(), With<FadeMaterials>>.iter().count()` plus a local counter for this frame. That needs no resource and cannot leak.
4. **Two code paths despawn the same nameplate anchor.** The anchor is a `WorldLabel` with `tracked_entity` (`nameplate.rs:133`). Fade start both removes `NameplateTag`, which makes `nameplate_cleanup_system` despawn the anchor, and despawns every `WorldLabel` tracking the entity. That is the double-despawn shape described in `src/CLAUDE.md` "Despawning". Specify `try_despawn()` for the widget sweep. Alternatively, drop the `NameplateTag` removal and rely on the sweep alone, keeping the tag only to stop `nameplate_setup_system` from re-adding.
5. **A target ring on an already-targeted corpse lingers, which contradicts Playtest item 4.** Behaviour 2 says an already-held target is cleared only at despawn, so the opaque ring decal stays under a corpse that is already invisible. Either have `target_auto_clear_system` also clear on `Has<FadingOut>`, with the same emission path as "hidden", or reword checklist item 4.
6. **The re-arm guard belongs in the tick system, not the executor.**
   - `action_executor_system` already has 15 of 16 params, so `Has<FadingOut>` would have to go into `SpawnParams` or `SceneStateParams`.
   - The executor check also misses the same-frame case: `FadingOut` is inserted through deferred commands, so the executor won't see it.
   - Simpler: in `despawn_timer_system`, an entity with both `DespawnTimer` and `FadingOut` drops the `DespawnTimer` (`debug!`). That fixes itself, needs no executor change, and is order-independent.
7. **Finding 2's inventory is incomplete.** `action_substitution.rs:309-320` (in-file unit tests) builds `SetDespawnTimer { entity, delay_secs }` literals in 4 places. They will fail to compile. Since the compiler will catch them it is harmless, but list them. Also, "overwrites all four fields" should be three (`remaining_secs`, `fade_secs`, `fade_mode`).
8. **Expiry carry-over (optional).** The fade starts one system-run after expiry, because `FadingOut` is inserted via commands. To make `delay + fade` exact, initialise `FadingOut.remaining_secs = fade_secs + timer.remaining_secs` (the remainder is ≤ 0 at that point). It also keeps Finding 10's 20.5 s test margin comfortable.
9. **The cross-schedule ordering edge disappears after Phase 1.** `fade_out_visual_system.after(despawn_timer_system)` has no effect once the timer is in `FixedUpdate`. Say explicitly that the visual must tolerate any order: it only reads `remaining/total`, so it does, but write it down. On >64 Hz displays the fade will step at 64 Hz. That is the accepted aliasing per `src/CLAUDE.md`.
10. **Test detail.** "Cloned handles dropped (`Assets::len` returns to baseline)" needs one or two extra `app.update()` calls after despawn: asset removal happens on the handle-drop event, not at the moment of despawn. The test also needs `init_asset::<StandardMaterial>()`, as `particle_tests.rs` does.
11. **Docs task.** Also update `docs/30_runtime_events_and_logic.md:774` (the `SetDespawnTimer(entity, delay_secs)` signature paragraph) and the `:757-765` RON example, not just "the corpse decay section". Add a `src/CLAUDE.md` note on the per-instance material clone rule. "Never mutate a shared GLB/catalog material in place" is a reusable invariant other features will need.
12. **Optional data-driven hook (defer).** An `entity.fading:{id}` `GameEvent` at fade start would let designers add a sound or effect without Rust. Not needed for v1. Log it in `claude_suggestions.md` if wanted.

On the parts of the brief not raised above:
- **Schema, CLI and determinism:** No CLI validation is needed, and `query actions` is unaffected. Choosing two defaulted fields over a new `Action` variant is the right minimal footprint. Keeping `Action::Despawn` instant and `LoadScene` teardown direct is correct.
- **`SpawnRegistry` during the fade:** keeping the id registered until the real `Despawn` is correct. It preserves the container-close path at `action_executor.rs:313-321`.
- **Clone dedup:** deduping by `AssetId` is deterministic in effect, because the output is written per mesh in `iter_descendants` order.

#### Open questions for Frank

1. **Default `fade_mode`.** Keep `Fade` as the default. It is what was asked for twice. Shrink-by-default would not read as "fading out". But include `NotShadowCaster` at fade start (non-blocking item 1) so the cold-start cost is one pipeline compile, not two. If the dev playtest shows a visible hitch on the first corpse fade, fold the warmup into v1 rather than a follow-up, because the stall would land mid-combat.
2. **Fold in "looted corpse decay 20 s → 2 s"?** Keep it separate, as the plan proposes. It is a tuning decision with its own test change (`corpse_loot_interact_tests.rs:591`), and keeping it out keeps this diff to engine plus one RON line.
3. **Colliders during the fade.** Keep them solid for `Fade`. But this question has to be answered together with non-blocking item 2: `Shrink`, including the automatic fallback, currently shrinks colliders. Recommendation: for `Shrink`, remove `Collider` and `Sensor` at fade start, in the tick-system expiry branch per B1, so the physics change is deterministic and decided by the sim, not by a render budget.

### UX-gamedesigner review

## UX review: planning/features/fade_out_despawn.md (plan review, before any code)

Reviewer: ux-gamedesigner-reviewer. Scope: the RON surface, what designers see, docs/examples, the playtest checklist, and the open questions. I checked it against `docs/20_data_formats.md:3820`, `docs/30_runtime_events_and_logic.md:743-829`, `3rd_person_game_demo/behaviors/{lootable_corpse,enemy_zombie,enemy_snake,enemy_spider}.behavior.ron`, `logic/state_machine.ron:186-193` and `prefabs/prefabs.ron:528` (seal_door).

#### Verdict
**Ready, once two small plan edits land.** The RON surface is the right shape: two optional fields on the existing action, unquoted enum, defaults that keep today's behaviour. It matches the house style (`PlayAnimationOn`'s `start_at_fraction`/`freeze`, `velocity_curve: EaseOut`), and the plan rejected a second `FadeOutAndDespawn` action for the right reason. The two blocking items are both "decide and write it down" fixes, not design rework.

#### Blocking

1. **The plan contradicts itself on the target ring and the target HUD during a fade.** Behaviour §2 says "an already-held target is cleared at despawn as today". Playtest step 4 says "no nameplate/target ring lingers". As written, a corpse you had tab-targeted keeps its ring decal and its HUD name/target bar for the whole fade. Step 4 will then fail, or someone will "fix" it during playtest without a plan. Designer impact: the ring sits on the ground under a corpse that is disappearing, which looks like a bug.
   **Fix:** clear any held target pointing at this entity at fade start, in the same pass that removes `Targetable`/`ClickSelectable`. Add it to the acceptance criteria ("Given a fading entity that is the current target, then the target clears at fade start") and to the test list.

2. **The timing rule (delay_secs + fade_secs = total time on screen) is never stated as the rule designers will read.** The plan's own demo edit shows the trap: to keep the corpse's 20 s, it has to retune to `18.5 + 1.5`. A designer who adds `fade_secs: 1.5` to an existing `delay_secs: 20.0` will silently get 21.5 s. Another reasonable reading of "fade_secs" is "the last N seconds of the delay", which gives a different result. Nothing errors in either case. The plan's docs task only says "new fields, fade_mode table". It does not commit to explaining the timing model.
   **Fix:** make the docs task explicit. The `docs/20` row and the `docs/30` prose must say, word for word: "The fade starts **after** `delay_secs` has elapsed. The entity stays on screen for `delay_secs + fade_secs` in total, and adding a fade to an existing timer makes it last longer." Keep the additive model. It is the right one: it makes "fade_secs larger than delay" a non-issue, keeps `delay_secs` meaning what it means today, and makes `delay_secs: 0.0, fade_secs: X` work as a "fade out now" recipe. It only needs to be written down.

#### Non-blocking

3. **The Shrink fallback is visible, but the reason for it isn't.** On the WASM build, `warn!` only shows in the browser devtools console, which most designers never open. Seeing it shrink instead of fade is better than a silent no-op, so this fits the warn-on-contradictory-intent principle. But "why did this one shrink?" has no answer outside the console. Specify that:
   - the warning names the spawn id and the reason (`custom material` / `premultiplied/additive GLB material` / `fade budget of 16 exceeded`);
   - "once" means once per entity for material reasons, but once per session for the budget reason (otherwise a 20-kill wave spams 4+ lines);
   - docs/20 lists the three fallback triggers in plain language: "custom-shader prefabs, some GLBs with special transparency, and more than 16 things fading at the same moment will shrink instead".
4. **The 16-entity budget makes the look inconsistent, and the docs have to say so.** In a 20-kill wave, corpses 17 and up shrink while their neighbours fade. Looted-corpse fades are naturally staggered, so this is rare, but `fresh`-state corpses from a burst of kills hit their 600 s expiry close together. Add it to the docs/20 fallback note and keep playtest step 7.
5. **Negative or non-finite values are silently clamped.** Under the warn-vs-silent principle, `fade_secs: -1.0` is contradictory intent, not a legitimate fallback. The plan says "no CLI validation exists for delay_secs, so none for fade_secs". Recommend a cheap `ironhold validate` warning for a negative `delay_secs`/`fade_secs` on `SetDespawnTimer`, either here or as a backlog item. Typos are already safe: `fade_sec:` and `fade_mode: Fdae` are hard parse errors because `Action` uses `deny_unknown_fields`, and the ron 0.11 message names the variant and the expected fields (see the error example at docs/20:3800).
6. **Re-arming during a fade is dropped at `debug!` level.** That is fine for the shipped corpse flow, where nothing re-arms after expiry. But "you can't cancel or extend a fade once it has started" is a rule designers should know. Put one sentence in the docs/20 row.
7. **Colliders stay solid while the visuals disappear.** Corpses have no colliders, so the shipped demo isn't affected. Any prop with colliders (a gate, a crate) will block the player while it is visibly gone. Keep the plan's choice, and document "colliders stay solid until the entity is fully gone" in the docs/20 row.
8. **Shrink scales toward the model's origin.** For a lying corpse, the origin is at the monster's feet (`at_entity` transform), so the body slides along the ground toward the feet as it shrinks, rather than toward its own centre. That is probably acceptable for 1-2 s, but playtest step 5's "shrinks into the ground point" should say "toward its origin (the feet)" so Frank judges the right thing.
9. **Demo coverage: add seal_door as a second, non-skinned showcase.** `seal_door` is a static GLB prop (`wooden_gate_01`, `prefabs.ron:528`) despawned with `SetDespawnTimer(delay_secs: 1.5)` at `state_machine.ron:192`. Changing it to `delay_secs: 0.5, fade_secs: 1.0` keeps the 1.5 s total, so the "The seal breaks..." floating text still shows: the entity stays registered during the fade, so the popup keeps tracking it. It also gives designers a second copyable example on a different mesh class (static, has a collider), and exercises item 7 in a real playtest. Update the comment at `state_machine.ron:181-185` if you do this. The plan is right to keep `enemy_zombie/snake/spider`'s 1.0 s swap timers instant: a fade there would show the live model fading through the identical corpse.
10. **Name the duration semantics under Shrink.** `fade_secs` with `fade_mode: Shrink` reads slightly oddly. Don't rename it: `fade_secs` matches the backlog wording and what designers would guess. Add one clause instead: "`fade_secs` is the length of the exit effect, whichever `fade_mode` is used."
11. **Docs scope to cover beyond what the plan lists:**
    - The `docs/30` code block at lines 757/765 has to match the new demo values exactly. The `lootable_corpse.behavior.ron` header comment (lines 5-6, "decays much sooner (20s)") has to change too, or it contradicts the file.
    - Add a short "Fading things out" recipe under docs/20's action table: three examples (corpse decay, an instant fade-now via `delay_secs: 0.0`, and Shrink), plus the fade_mode table. Point to it from the row. That row is already one very long cell, and folding the new rules into it hurts discoverability.
    - State in the `Despawn("id")` row: "always instant; to fade something out, use `SetDespawnTimer(entity, delay_secs: 0.0, fade_secs: N)`". That is where designers will look first.
    - Make sure the docs/20 row's field list shows the defaults: `fade_secs: f32 = 0.0`, `fade_mode: Fade | Shrink = Fade`.
12. **Polish: no event fires when a fade starts or ends.** A designer who wants a "crumble" sound or particles has to compute `delay_secs` by hand with a parallel `EmitEventAfterDelay`. A future `entity.fade_started:{id}` would be the natural hook. Log it as a backlog/suggestion item, not v1 scope.
13. **Shadow pop.** Whichever way the shadow risk is resolved, one pop remains: at the end of the fade (shadow kept) or at its start (`NotShadowCaster`). For a ground-hugging corpse the shadow is tiny, so prefer leaving it as is. If the playtest shows a pop, `NotShadowCaster` at fade start reads better on standing props like seal_door.

#### Open questions for Frank

1. **Default `fade_mode`: Fade or Shrink?** Recommend **Fade** as the default, as the plan proposes. The field is called `fade_secs`. If the default visibly shrinks, the name lies to every designer who never reads the mode table. The one-off pipeline stall is an engine cost the warmup follow-up can solve. It shouldn't be paid for in naming confusion. Keep `Shrink` as an opt-in and the automatic fallback.
2. **Fold in the "looted corpse 20 s -> 2 s" retune?** Recommend **yes**: `delay_secs: 0.5, fade_secs: 1.5`.
   - It is the same RON line and you already asked for it.
   - "Take all, then the corpse dissolves while you're still looking at it" is the payoff this feature exists for. 18.5 s of waiting hides it.
   - It cuts each run through playtest steps 1-4 from about 20 s to about 2 s.
   - Cost: one `advance` change in the `corpse_loot_interact_tests.rs:591` test, plus updating the docs/30 block, which has to change anyway.

   Close the separate backlog item in the same merge so it isn't left dangling. If you'd rather keep scope tight, keep `18.5 + 1.5` and make sure the header comment says "20 s total (18.5 s + 1.5 s fade)".
3. **Colliders during the fade?** Recommend **leave them solid** (the plan's choice) and document it (item 7). It is predictable and needs no new rule. If seal_door's playtest (item 9) shows the player bumping into an invisible gate, revisit it with an opt-in flag. The house style is a bool defaulting to false, e.g. `disable_collision_on_fade: bool`. Don't change behaviour for everyone.
4. **(New) Target clear timing:** see Blocking 1. Recommend clearing at fade start.
