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
- No threads, no blocking I/O, no GPU features. Pure CPU + ECS.

Link: [[project-dynamic-labels-system]] (drives the target UI label), [[project-rewrite-target]] ({target} substitution feeds off CurrentTarget set here).
