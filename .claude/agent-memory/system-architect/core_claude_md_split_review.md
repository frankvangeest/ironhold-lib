---
name: core-claude-md-split-review
description: Plan-review findings (1st pass 2026-10-07 @1d616bc, 2nd pass @dd62494) for splitting crates/ironhold_core/src/CLAUDE.md into parent + dir files + docs/dev topics with .claude/rules stubs; load-scope traps to re-check at code review
metadata:
  type: project
---

Plan `planning/features/core_claude_md_split.md` + map `planning/investigations/core_claude_md_split_map.md`. 1st pass -> Needs work; 2nd pass (same day, after revision) -> still Needs work, but narrower.

Load-scope facts (verified, re-check at code review of the split):
- `runtime/` has 6 files OUTSIDE `scene_manager/`. `runtime/input.rs` holds `global_input_system`, `unclaimed_gamepad_trigger_system`, `gamepad_bind_system` AND `input_translator_system`. `runtime/actions.rs` = `ActionQueue`. A `runtime/CLAUDE.md` also loads for scene_manager files (ancestor), but NOT for capabilities/.
- `tick_delayed_events_system` lives in `lib.rs` (not runtime/) -> any rule about it must be PARENT.
- Gamepad consumer rule (`Option<&BoundGamepad>`, never required) governs capabilities/{action_bar,camera,interactable,targeting}.rs + runtime/input.rs -> needs a CAP line, runtime/ alone misses it.
- Friction rule (1224-1260) governs entity_spawner.rs (SM) too, not only player.rs.
- `assets/CLAUDE.md` exists -> natural always-loaded home for WGSL rules (instead of a `.wgsl` path stub, which would put a Safety=Y rule in a stub).
- lib.rs edges that CLAUDE.md prose guards (2nd pass inventory): interpreter chain (fsm->entity_fsm->flush_intent->executor->...->drain_spawn_queue->drain_dynamic_stat_ui), unclaimed_gamepad.before(fsm), FixedUpdate chain (gamepad_bind before input_translator, view_box_clamp after movement, mark_dirty_trees before SyncBackend), interactable.before(fsm), dialogue edges, audio_volume_var edges, visual chain (animation_resolver first ... camera_blend ... animation_playback last; dynamic_split after party before split_viewport; split_viewport_player_label_update + target_hud after split_viewport; fly_camera after camera_shake), target_hud.after(target_auto_clear), world_label.after(camera_blend), nameplate_visibility.after(world_label). Plan listed only 5. Many already have inline lib.rs comments; the uncommented chain-position ones are the gap.
- CLAUDE.md:624 "chained back-to-back" is stale (resolver lib.rs:344, playback :368, 12 camera systems between).
- "four sites" ambiguity: player-construction "four->five" fix vs the {self}/{target} four-site rule (correctly four) — no mechanical replace.
- Block-ID audit gaps: ~13 rows are split across destinations (needs sub-IDs), "exactly once" can't hold during additive Phases 1-3 (needs a destinations-only mode), Y-ish/partial labels must be resolved before scripting.
- Phase A: no programmatic readers of the six docs; moved docs contain zero relative md links. Map missed live refs in backlog.md (65,149,226) and claude_suggestions.md.

**Why:** split CLAUDE.md files only protect files under their directory; src-root (lib.rs) and runtime-root files load only the parent/runtime file.
**How to apply:** at code review of the split, check every Safety=Y rule's governed files vs destination scope; see [[capability-patterns]], [[schedule-ordering-mechanism]], [[gamepad-input-pattern]].
