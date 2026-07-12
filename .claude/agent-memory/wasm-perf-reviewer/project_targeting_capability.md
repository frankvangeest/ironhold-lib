---
name: project-targeting-capability
description: targeting.rs hot-path cost profile — both systems are input-gated, cheap on idle frames
metadata:
  type: project
---

`crates/ironhold_core/src/capabilities/targeting.rs` adds two Update systems: `click_select_system` and `tab_targeting_system`.

**Why:** Targeting (click-select + Tab-cycle) for 3rd_person_game_demo; deliberately uses `camera.world_to_viewport` screen-space projection instead of bevy_picking mesh raycast (raycast hits bind-pose geometry, misses animated skinned GLB).

**How to apply:**
- Both systems early-return on non-input frames: `click_select_system` after `mouse.just_pressed(Left)`, `tab_targeting_system` after `keys.just_pressed(tab_key)`. On idle frames the only cost is ECS system-param fetch (Res/Query borrows) — negligible, no allocation. This is correct and WASM-safe.
- Allocations (Vec of candidates in tab_targeting, String clones of spawn ids) happen ONLY on the click/Tab frame. Entity counts are small (handful of targetables). Not a per-frame concern.
- `world_to_viewport` is per-selectable but only on click; fine.
- Split-screen viewport-aware camera pick (feature/targeting-viewport-click): camera query grew to `(Entity, &Camera, &GlobalTransform, Option<&SplitViewportSlot>)`, filtered by `is_active` + `logical_viewport_rect().contains(cursor)`, then `.min_by_key(camera_priority_key)`. Bounded to ≤MAX_SPLIT_PLAYERS(4) cameras, evaluated ONLY on click frames. `logical_viewport_rect()` is a couple float ops per camera; `camera_priority_key` (camera.rs) is a zero-alloc `(u32, Entity)` tuple. Trivially fine on WASM. Added read-only query fields are archetype-access only (web is single-threaded, no parallelism cost).
- No threads, no blocking I/O, no GPU features. Pure CPU + ECS.

Link: [[project-dynamic-labels-system]] (drives the target UI label), [[project-rewrite-target]] ({target} substitution feeds off CurrentTarget set here).
