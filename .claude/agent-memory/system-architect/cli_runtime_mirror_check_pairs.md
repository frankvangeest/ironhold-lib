---
name: cli-runtime-mirror-check-pairs
description: The recurring "CLI validate check + matching scene-load warn!" pair pattern — its four standing failure modes (logic duplication across the crate boundary, band-direction asymmetry, diagnostics that quietly change runtime behavior, and shared-helper messages whose remedy is only true at one call site), plus the validate_camera_mode_def instance
metadata:
  type: project
---

Ironhold has a standing pattern: every designer-facing misconfiguration gets **two** diagnostics —
an `ironhold_cli validate` check (`cross_file_checks` hard error or `strict_checks` `--strict`
warning) *and* a matching one-time `warn!` in `runtime/scene_manager/scene_loader.rs`, because a
designer on a prebuilt WASM build has no CLI access. Instances: `jump_cannot_clear_ground_sensor`,
`invalid_walkable_slope_limit`, `negative_coyote_time_secs`, `missing_stat_widget_template`,
`duplicate_gamepad_index`, `label_depth_scale_min_scale_out_of_range`,
`label_depth_scale_reference_distance_outside_camera_range`. See
[[cli-validate-coverage-model]].

**Three failure modes to check on every new pair:**

1. **Duplicated helper logic across the crate boundary.** The two sides routinely re-implement the
   same predicate. Sometimes justified (`resolve_jump_velocity` vs the CLI's height-targeted math —
   genuinely *different-shaped inputs*), sometimes not (pure enum-variant classification, which is
   identical on both sides). **The right home for a pure schema-shaped predicate is
   `ironhold_core::schema::` itself** — `ironhold_cli` already depends on the schema module, so no
   visibility gymnastics or runtime dependency is needed. Do not accept "the core copy is
   `pub(crate)`" as a rationale — verify the actual visibility, and note that `pub`-ifying is
   already precedent (`entity_spawner::default_camera_config` was made `pub` for exactly this).

2. **Band-direction asymmetry — check which side is NOISIER, not which is more complete.** When
   the CLI can see project-wide data the runtime can't (e.g. scanning `Action::Spawn` for
   player-tagged prefabs, which `spawn_scene_v2` cannot do — it only has `player_configs` for
   *scene-placed* players), the CLI's tolerance band is *wider*, so CLI-warnings are a **subset**
   of runtime-warnings. The failure the designer actually hits is therefore "`validate --strict`
   passes clean but the console warns anyway", not the reverse. Plan docs and CLAUDE.md notes
   consistently frame this backwards as "the runtime misses some warnings". Always work out the
   subset direction explicitly.

3. **A "diagnostic-only" change that quietly alters rendering/behavior.** Validation features
   frequently smuggle in a clamp or a default change alongside the warnings. Two live examples from
   `label_depth_scale_validation`: a `min_scale` clamp added inside `resolve_label_depth_scale`
   (previously `min_scale: 1.5` really did render labels at 150% via
   `depth_scale_factor_from`'s `.min(1.0).max(min_floor)`), and
   `default_label_ref_distance()` `50.0` → `20.0`. Both were inert against shipped RON, but they
   make the *diagnostic messages themselves* false (the CLI still says "pins at 150% forever" when
   the engine now renders 100%) and can staleify defensive comments elsewhere.

4. **A shared CLI helper parameterized only by a `context:` *prefix* ships one call site's
   *remedy* to every call site.** The message body — "…they must be siblings of `camera_mode`,
   e.g. `components: (camera_mode: Orbit(...), split: (...))`" — is correct for a prefab, wrong
   for a scene `camera_modes:` registry entry (which has no `components:` block, and where
   `split`/`party` aren't authorable at all). Same shape for behavioral claims: "the camera will
   sit at `position` with no rotation applied (facing -Z)" is true only at spawn
   (`Transform::from_translation` = identity rotation); on the `Action::SetCameraMode` switch path
   `apply_camera_mode` deliberately never touches rotation and `fixed_camera_system` only calls
   `look_at` when a target resolves, so the camera **inherits the outgoing mode's rotation**. The
   predicate genuinely is shared; the message is not. Signature fix: return a kind enum (or
   `(kind, remedy)`) and let each call site render its own remedy, rather than a
   `Vec<(String, &'static str)>` whose `String` is pre-baked.

**Concrete instance to reason from — `validate_camera_mode_def` (`3d74da2`, 2026-09-06).**
`CameraModeDef` has exactly **two** authoring surfaces (`PrefabComponents.camera_mode`,
`catalog.rs`~1407; `GameSceneV2.camera_modes`, `scene_v2.rs`~66 — `PlayerConfig.camera_mode` is
runtime-assembled, not authored), and this helper covers both, so the set is complete. But the
runtime side is *three* separate, narrower sites: the nested-split/party `warn!` lives in
`assemble_player_config` (**player-tagged prefabs only**), the `Fixed` both/neither `warn!` lives
in `spawn_active_camera_for_player` (**single-player fallback branch only** — silent in local
co-op), and `warn_camera_modes_registry` (`scene_loader.rs`~1423) checks *neither*. Net: CLI band
strictly wider than runtime at every call site, and the registry half of the pair has **no runtime
counterpart at all**. Note also `prefab.components.camera_mode` has a **third** live consumer —
`tags: ["flycam"]` prefabs (`scene_loader.rs`~273) — so "camera_mode on a prefab" is not
synonymous with "player prefab".

**Why:** these pairs are cheap to add and land often, so the same mistakes recur; all of them
are invisible in a green test run.

**How to apply:** when reviewing any new validate-check + `warn!` pair, ask in order: (a) is the
shared predicate duplicated, and could it live in `schema/`? (b) which side's band is wider, and
does the doc describe the subset direction correctly? (c) did a clamp/default change ride along,
and are the message texts still true after it? (d) if a helper is shared across call sites, is
every clause of its message — especially the suggested fix — true at *all* of them?
