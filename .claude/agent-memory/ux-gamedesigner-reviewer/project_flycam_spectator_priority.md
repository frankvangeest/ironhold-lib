---
name: flycam-spectator-priority
description: Flycam-vs-player camera priority (spectator mode) — shared-WASD footgun, docs location, and the stale-baseline gap when a scene is added to an existing project
metadata:
  type: project
---

A scene containing both a `tags: ["player"]` entity and a standalone `tags: ["flycam"]` entity is
supported ("spectator mode"): the flycam wins camera priority, the player still plays but gets no
camera. Canonical example: `assets/projects/camera_modes/scenes/flycam_spectator_test.scene.ron`,
reachable from that project's hub via the teal "Spectator" pad. Docs live in
`docs/20_data_formats.md` under `### Special tag: "flycam"` → `#### Spectator mode`.

The non-obvious designer footgun: **both entities read raw input independently, so default WASD
drives the player AND the flycam at the same time.** There is no engine-side arbitration. The
remedy is authoring-side only — re-bind the flycam (`flycam: (forward: "ArrowUp", ...)`) or the
player (`inputs:`). Check this is stated in docs, not just in a scene comment, whenever this
section is touched.

**Why:** implemented 2026-08-17 on `feature/flycam-scene-conflicts`; the shared-key behaviour is
inherent to two independent input consumers and will keep surprising designers.

**How to apply:** when reviewing any change to flycam/camera-priority behaviour, verify the docs
subsection still carries the shared-key note plus the two documented known limitations (runtime
`Action::Spawn` players still get their own camera; `SetCameraMode` without `owner_player` can
convert the flycam itself).

**Flycam diagnostics live in FOUR doc spots that must be kept in sync** (`docs/20_data_formats.md`,
line numbers as of 2026-08-19): the intro sentence "The `model` field (and `children`...) is
ignored — and warns..." (~1882), the bold-lead **Duplicate flycam entities.** note (~1952 — the
canonical tone/format template for any new flycam diagnostic: one bold lead-in, what happens, then
an explicit remedy sentence), the **A flycam-tagged prefab's `model`/`children` never render.** note
with its inline ❌/✅ RON pair (~1956) plus the **Don't put both tags on the same prefab.** note
(~1982), and the spectator-mode what-works/what-doesn't bullet list (~2017, which back-references
the intro sentence and goes stale whenever the intro changes).

Shipped 2026-08-19 (`planning/features/flycam_model_never_renders_warning.md`): a non-empty
`model:`/`children:` on a flycam-tagged prefab, and the dual `tags: ["player","flycam"]` case, now
fire a scene-load `warn!` (`scene_loader.rs`'s `is_flycam` branch) plus `ironhold_cli validate` hard
errors `flycam_model_never_renders` / `flycam_player_tag_conflict`, both prefab-catalog-scoped with
`source_file: "prefabs/prefabs.ron"`. Helpers `PrefabDef::is_flycam()/is_player()/
flycam_ignored_fields()` live in `schema/catalog.rs`.

**CLOSED:** `flycam_ignored_fields()` (`schema/catalog.rs`) now also checks `shape`/`primitive` —
a `kind: Primitive` flycam authoring a body via `shape:` + `primitive:` is caught alongside
`model`/`children`, and `docs/20_data_formats.md`'s flycam intro sentence now explicitly lists
"`model`, `shape`/`primitive`, or `children`" as the ignored body-defining fields. Do not re-flag
this gap.

Note `model:` is a required non-Option field, so `model: ""` is the shipped convention in every
flycam prefab (terrain_demo, custom_materials, foliage_demo `"explorer"`, camera_modes
`"flycam_demo"` — all four are `kind: Prop`) — and because `validate_projects.rs` requires exit 0
for every shipped project, a hard validate error means the counter-example can only ever live inline
in docs, never as a shipped example project. Only `terrain_demo`'s flycam prefab comment mentions
the new diagnostic; `custom_materials`' ("no model, engine detects flycam tag") and `foliage_demo`'s
(which mislabels its flycam-only `"explorer"` as "Player") were not refreshed.

Related review lesson: CLAUDE.md's baseline-screenshot registration step only covers *new
projects*. Adding a new `scenes/*.scene.ron` to an **existing** project silently skips it — check
`screenshot_baselines/scenes/{project}_{scene}.png` exists for the new scene, and that the hub
scene's own baseline is regenerated when a portal pad is added to it. See also
[[camera-config-party-split-nesting]], [[local-coop-demo-room-conventions]] (hint-label character
ceilings).
