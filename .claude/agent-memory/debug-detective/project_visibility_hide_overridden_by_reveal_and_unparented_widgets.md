---
name: project-visibility-hide-overridden-by-reveal-and-unparented-widgets
description: Hiding an entity via root Visibility::Hidden is undone by the animation_policy reveal and never reaches nameplate/WorldLabel anchors (unparented)
metadata:
  type: project
---

Two facts that break any "hide the player/entity by setting `Visibility::Hidden` on its root" feature (found reviewing first_person_starter `hide_own_body`, 2026-10-17):

1. `animation_playback_system` (capabilities/animation.rs ~102 failsafe, ~387 pose-confirmed) does `insert(Visibility::Inherited)` on any entity with `awaiting_reveal` — i.e. every GLB player/prefab with an `animation_policy` (entity_spawner hides it at spawn for the bind-pose flash). Any other spawn-time hide on that entity is silently reverted once the pose loads (or after 5 s).
2. Nameplate anchors (`nameplate.rs` ~125) and other `WorldLabel` widgets are deliberately **unparented** (they track `tracked_entity`), so hiding the root does NOT hide them, contrary to what a doc comment may claim.

**Why:** both are designed for other purposes (bind-pose flash; avoiding inherited scale/animation) and nobody owns a "hidden reason" bitmask.
**How to apply:** when reviewing any feature that hides an entity by root Visibility, check for an `animation_policy` on the entity and for WorldLabel/nameplate/stat-bar anchors; ask for a marker the reveal respects. Also: `bevy_winit` `changed_cursor_options` re-applies grab_mode on every Changed (no cache compare) but compares `visible` against its cache. Related: [[project-ui-vs-text2d-compositing]].
