---
name: world-label-cleanup-and-at-entity
description: WorldLabel widget cleanup now has two overlapping owners (nameplate_cleanup_system + stat_widget_cleanup_system); Action::Spawn.at_entity copies the full GlobalTransform incl. scale
metadata:
  type: project
---

Two facts from the monster_corpse_loot v2 review (2026-08-26), both about entity-lifecycle
plumbing that is easy to mis-scope.

**1. `WorldLabel`-widget teardown has two owners.** `Action::Despawn` removes only the one entity
it names; every unparented `WorldLabel` widget that merely *references* it via
`WorldLabel.tracked_entity` must be cleaned up explicitly. There are now two systems doing this:
`nameplate::nameplate_cleanup_system` (`RemovedComponents<NameplateTag>` + `With<NameplateAnchorWidget>`)
and `stat_display::stat_widget_cleanup_system` (`RemovedComponents<SpawnId>`, **no** `With<...>`
filter). Because nameplate anchors carry `WorldLabel` too, the second is a strict superset of the
first, and it also reaches `ShowDamagePopup`/`ShowFloatingText` labels. Both use `try_despawn()` so
the overlap is benign, but the invariant has no single owner.

No code path removes `SpawnId` from a live entity (zero `remove::<SpawnId>` sites), so
`RemovedComponents<SpawnId>` is a safe despawn proxy — the false-positive risk is scope, not timing.

**Why:** whoever adds the next `WorldLabel`-based widget will guess wrong about which system owns
its teardown, and a future `remove::<SpawnId>` (e.g. an "unregister id" action) would silently turn
this into a real false-positive.

**How to apply:** when reviewing a new world-space widget or a new despawn path, push toward one
generic `world_label_cleanup_system` rather than a third per-widget copy. See
[[world_space_widgets]] and [[nameplate_gating]].

**2. `Action::Spawn.at_entity` transplants the whole `GlobalTransform`.** It resolves via
`SpawnRegistry → GlobalTransform::compute_transform()` and the result is passed wholesale to
`spawn_prefab_instance`, so **scale and pitch/roll come across too**, not just position+yaw as the
schema doc claims — and `model_fixes.ron`'s own scale is then applied on top of that at the child
model. Harmless in `3rd_person_game_demo` (every fix is `scale: (1,1,1)`), a double-scale bug in any
project that scales a GLB via `model_fixes`.

**Why:** the docs promise "position + facing"; the code delivers a full transform copy.

**How to apply:** flag it if `at_entity` spreads to a project with non-unit `model_fixes` scale, or
if someone uses it for a non-corpse case (thrown-projectile origin, portal, etc.).
