---
name: aim-target-mouse-slot-pattern
description: first_person_starter v2 — FirstPersonCameraDef.aim_target (crosshair PlayerTarget) + ActionSlotDef.mouse_button (capture-gated); SelectAimHeight-only-with-click_selectable footgun; validate now feeds slot do_actions into all_actions
metadata:
  type: project
---

Reviewed 2026-10-22 (feature/first_person_starter_v2).

- `aim_target_system` (capabilities/targeting.rs) reads `Option<&SelectAimHeight>` on `Targetable` entities, but
  `tag_spawned_entity` (scene_manager/mod.rs) inserts `SelectAimHeight` ONLY when `click_selectable` is true.
  A targetable-only prefab's authored `select_aim_height` is silently dead (falls back to 1.0). catalog.rs doc
  still says "Only meaningful when click_selectable: true". Check this whenever a new consumer of SelectAimHeight
  or Targetable appears.
- `target_name` GameVariable = prefab catalog KEY (write_target_vars), never `display_name`: crosshair labels
  show "training_dummy". Pre-existing, but now a headline UI in the FPS starter.
- `mouse_button` fires only while `CursorLockState.locked`; no runtime warn twin for "no capture_cursor
  player" (CLI --strict only). Strict check reads only the prefab's camera_mode, not scene/registry modes.
- validate.rs now appends ActionBar slot `do_actions` to `all_actions` (after discover_extra_scenes) — this
  feeds ERROR checks too (missing effect/sound keys), not just orphan "uses". Stale comments then remain at
  set_ui_visible_checks (~910, which now double-collects slot SetUiVisible) and pause_checks (~3235).
- Starter respawn = one global_on rule per dummy with copied positions (EmitEventAfterDelay armed before
  SetDespawnTimer). Delayed event survives LoadScene -> duplicate-id Spawn if a designer adds a restart.
- aim range/radius is per-camera, not per-slot: a second weapon cannot have its own range.

**Why:** these are the extension points designers copy from the FPS starter.
**How to apply:** when v3 (`fire` slot variant, weapons) lands, re-check per-weapon range and the
SelectAimHeight insertion; see [[targeting-capability-pattern]], [[validate-cross-file-blind-spots]].
