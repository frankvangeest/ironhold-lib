---
name: core-claude-md-split-review
description: Review history (plan 2026-10-07 x2, final code review 2026-10-08 @17b65f4) of splitting crates/ironhold_core/src/CLAUDE.md into parent + folder files + docs/dev topics + .claude/rules stubs; the load-scope gap classes to re-check whenever a CLAUDE.md rule is added or moved
metadata:
  type: project
---

Plan `planning/features/core_claude_md_split.md`; disposition table `planning/investigations/core_claude_md_split_dispositions.md` (block -> destination, Safety label). Old file: `git show d222d91:crates/ironhold_core/src/CLAUDE.md`.

Final review (2026-10-08) verdict: nothing truly LOST (every must-rule exists either in a loaded CLAUDE.md or verbatim in a docs/dev topic page), audit 0 problems; the residual risk is LOAD SCOPE, not content. Gap classes found:
- **Topic-only must-rules**: blocks labelled N but containing load-bearing "must/never" (ground_cast "both not-underfoot AND not-walkable" exclusion, jumps_used never reset on an edge, local co-op ActiveSplitSlotCount never derived live / never one Orbit per player without viewports, ring_layer_for_player sole owner, resolve_cost_source single decision). Safety label was assigned per block, so a mostly-narrative block hid a must-sentence. Re-check modal verbs, not labels.
- **Rule placed by subject, not by the file that implements it**: camera rules (SetCameraMode/AuthoredCameraMode, CameraShake query) sit in capabilities/ but the code is in runtime/scene_manager (action_executor.rs, mod.rs SceneStateParams.orbit_cameras, entity_spawner.rs apply_camera_mode). Per-rank WorldLabelRank spawn rule sits in capabilities/ but the spawn sites are scene_loader.rs / action_executor.rs. Nameplate invariants sit in SM but nameplate.rs is capabilities/. The Option<&BoundGamepad> consumer rule is in capabilities/ only, yet input_translator_system is runtime/input.rs.
- **Stub globs list exact files** and miss secondary implementers (particle_renderer.rs, stat_display.rs, entity_spawner.rs, lib.rs for split-screen; scene_loader.rs for action-bar duplicate-key warns).
- lib.rs now carries `// load-bearing:` markers on every guarded edge group (verified); world_label_screen_pos_system's own doc comment documents the NameplateCameraDistance stash contract.
- Verbatim topic docs carried stale `planning/features/X.md` paths that are now under `done/` (pre-existing drift, not new).

**Why:** a folder CLAUDE.md loads only for files in its subtree; lib.rs/det_math.rs/inspector.rs/utils.rs see only the parent; `.claude/rules` stubs are pointers.
**How to apply:** when any rule is added to a split CLAUDE.md, place it in the file that loads for the code that would violate it (grep the fn's location first), and treat a docs/dev-only "must" as unloaded. See [[capability-patterns]], [[schedule-ordering-mechanism]], [[gamepad-input-pattern]].
