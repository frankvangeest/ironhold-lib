---
name: core-claude-md-split-review
description: Plan-review findings (2026-10-07, HEAD 1d616bc) for splitting crates/ironhold_core/src/CLAUDE.md into parent + dir files + docs/dev topics with .claude/rules stubs; load-scope traps to re-check at code review
metadata:
  type: project
---

Plan `planning/features/core_claude_md_split.md` + map `planning/investigations/core_claude_md_split_map.md` reviewed 2026-10-07 -> Needs work.

Load-scope traps found (re-check these when the split is implemented):
- `runtime/` has 6 files OUTSIDE `scene_manager/` (input.rs, material_factory.rs, actions.rs, messages.rs, model_spawner.rs, mod.rs). The plan's "no runtime/CLAUDE.md, everything is in scene_manager" claim is false. `runtime/input.rs` owns `gamepad_bind_system` + `unclaimed_gamepad_trigger_system` (the `claimed` invariant, `PendingJoinGamepad` reset) -> those Safety=Y rules must be in the PARENT, not CAP/TOPIC.
- `lib.rs` (parent-only) holds the load-bearing Update ordering edges: targeting chain consumers, `camera_blend_system` last in camera chain, `target_hud_update_system` after `split_screen_viewport_system`, dialogue `.after(button).after(interactable).before(fsm)`, audio_volume_var ordering, `mark_dirty_trees` before SyncBackend. Map row 1383-1405 refers to a nonexistent "risk 3" for this. Wants a parent "load-bearing schedule edges" list.
- `PrefabDef` lives in `schema/catalog.rs` -> the "new rendering PrefabDef field must be checked against the player path" rule needs a SCH line, not SM-only.
- `{self}` substitution has FOUR enumeration sites across two dirs: `rewrite_self`/`rewrite_target` (action_substitution.rs, SM), `substitute_self_in_action` (dialogue.rs), `action_needs_target` (action_bar.rs). Cutting the `{self}` list is fine only if the hook `action_docs_reminder.py` and SCH name all four.
- R9 says old file never rewritten until all blocks placed, but Phase 1 slims the parent first -> contradiction; build destinations additively, slim parent last.
- `git mv` + stub at old path in one commit kills rename detection; separate commits.
- capabilities raw ~43-45k; map's own condensed estimate 25k > 20k budget. player.rs/camera.rs worst case loads parent+CAP+ground+gamepad/split-screen topics.
- R4's "split capabilities/ into subfolders" fallback is a code move, not a doc refactor.

**Why:** split CLAUDE.md files only protect files under their directory; src-root and runtime-root files load only the parent.
**How to apply:** at code review of the split, verify each Safety=Y rule's governed files vs destination scope; see [[capability-patterns]], [[schedule-ordering-mechanism]], [[gamepad-input-pattern]].
