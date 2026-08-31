---
name: cli-runtime-mirror-check-pairs
description: The recurring "CLI validate check + matching scene-load warn!" pair pattern — its three standing failure modes (logic duplication, band-direction asymmetry, and diagnostics that quietly change runtime behavior)
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

**Why:** these pairs are cheap to add and land often, so the same three mistakes recur; all three
are invisible in a green test run.

**How to apply:** when reviewing any new validate-check + `warn!` pair, ask in order: (a) is the
shared predicate duplicated, and could it live in `schema/`? (b) which side's band is wider, and
does the doc describe the subset direction correctly? (c) did a clamp/default change ride along,
and are the message texts still true after it?
