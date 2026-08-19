---
name: diagnostic-only-feature-pattern
description: Pure warn!/validate diagnostic features (flycam model-never-renders) — the runtime-warn + CLI-error twin, prefab-catalog vs per-scene scoping rule, and PrefabDef tag helpers now living in schema/catalog.rs
metadata:
  type: project
---

Reviewed 2026-08-19 (verdict ALIGNED). Feature: `planning/features/flycam_model_never_renders_warning.md`.

**A "diagnostic-only" change is the easiest ALIGNED verdict in this codebase, and the checklist is
short:** no new RON field means no new designer surface to audit — instead verify (1) the fix the
message prescribes is itself pure-RON (here: `model: ""` in `prefabs.ron`), (2) the detector
iterates the catalog/scene generically with no project name anywhere, (3) runtime behavior is
byte-identical (the only new statements are `warn!`s inside an existing branch). Cite this feature
as the precedent when a plan proposes a new schema field to "opt out of" a footgun that a warning
would fix.

**Scoping rule for the runtime-warn + CLI-error twin (established here, worth reusing):** scope the
`ironhold_cli validate` check to whatever the *condition* actually depends on, not to whatever the
runtime warn iterates. `duplicate_flycam_entity` is per-scene because 2-flycams-in-one-scene is
genuinely scene-dependent; the ignored-fields/dual-tag checks are **prefab-catalog-scoped**
(`source_file: "prefabs/prefabs.ron"`, iterating `catalog.prefabs`) because the condition is
entirely prefab-local — scene-scoping would emit one error per instantiation, all pointing at the
wrong file. Consequence to expect and accept: a catalog prefab never placed in any scene fails
`validate` but never produces a runtime warn (design-time is deliberately stricter). Precedent for
catalog-wide checks already existed: `missing_stat_widget_template`, `missing_file` (behavior path),
foliage `leaf_texture`.

**`PrefabDef::is_flycam()`/`is_player()` + `pub const TAG_FLYCAM`/`TAG_PLAYER` now live in
`schema/catalog.rs`, not `scene_loader.rs`.** That move is what makes them CLI-reachable
(`ironhold_cli` only ever imports `ironhold_core::schema::*`, never `runtime::`) — put any future
tag/field predicate that both the runtime and `validate` need in the same `impl PrefabDef` block for
the same reason. `TAG_COLLECTABLE` stayed private in `scene_loader.rs` (no CLI consumer), and ~9
sites (`action_executor.rs`, several `validate.rs` checks, `query.rs`) still compare
`tags.iter().any(|t| t == "player")` by literal — adoption is partial, so don't assume the helper is
the only tag-check path when auditing.

**`flycam_ignored_fields()` covers only `model` + `children`, but the flycam branch `continue`s
before *every* other `PrefabDef` field is consulted.** `motion`, `material`, `primitive`,
`colliders`, `interactable`, `trigger_zone`, `stat_templates`, `stat_label`, `world_stat_bar`,
`display_name`, `nameplate`, `behavior`, `dialogue`, `inventory`, `merchant`, `npc`,
`animation_policy`, `click_selectable`, `targetable`, `player_index` are all equally, silently
discarded on a flycam-tagged prefab — only `components.flycam` and `components.camera_mode` survive.
The helper is the single extension point for widening this (both the runtime warn and the CLI check
call it), so a future report of "my flycam's `motion:` rail does nothing" is a one-function fix, not
a new detector.

See also [[flycam_spectator_mode_pattern]] for the flycam/player camera-priority behavior these
diagnostics sit on top of.
