---
name: system-ordering-determinism-pattern
description: How to alignment-review a Bevy system-ordering change (bare .chain()/.before() fixes and the D3 GameplaySet structure) - designer-visible effects, the current set map, and the doc traps when the docs promise a same-frame order
metadata:
  type: project
---

Reviewing a change that only orders existing systems: there is no schema, no RON, no new capability, so
the alignment question narrows to **"does making the order deterministic change any behavior a designer
can already reach from RON, and is that behavior documented correctly?"** Never a RON knob: designers
must never order systems. Spend the review on second-order effects and doc accuracy.

**Current `Update` map (D3, `runtime/schedule.rs`, branch feature/pipeline_system_sets, 2026-10-14):**
`GameplaySet::Emit -> Interpret -> Execute -> PostExecute`. Emit = `EmitSet::Scene` (check_project_loaded
-> spawn_scene_v2) -> {`Ui` (global_input -> gamepad triggers -> button -> icon_button), `Game` =
`GameStreamSet` Targeting -> ActionBar -> Interact -> DelayedEvents -> Stats} -> `DirectActions`
(resolve_pending_behaviors -> despawn_timer) -> `Dialogue`. Execute = flush_pending_intent -> executor ->
apply_sim_clock -> stat_effective(2nd) -> stat_threshold -> drain/pool/decal. PostExecute = npc_hit_relay,
inventory/container UI, audio_volume_var, SceneEvent readers (preload_*, clear_pool). New writers join a
set; a bare `.before(fsm_interpreter_system)` is now stale wherever it still appears in docs/comments
(dev docs `gamepad-routing.md`, `runtime/CLAUDE.md`, `capabilities/CLAUDE.md` interactable/dialogue
lines, `input.rs`/`dialogue.rs` doc comments were still stale at review time).

**Doc traps when docs promise a same-frame order (found in the D3 review):**
- Interpreters read ALL `UiEvent`s, then all `GameEvent`s, then all `SceneEvent`s, so the real read order
  is UI input (keys, pads, buttons) -> game carry-over from last frame's Execute -> FixedUpdate events ->
  this frame's Game chain -> scene carry-over -> this frame's scene events. A numbered list that puts key
  presses after the carry-over is wrong even with a "(UI before game)" parenthetical.
- In the scene stream, last frame's executor events (Requested/Unloading) come BEFORE this frame's
  spawn_scene_v2 Loaded/Ready (buffer write order), not after.
- "Same order on every machine" overclaims while D4 is open: entity_fsm_interpreter is entity-major
  over query order, and FixedUpdate writers iterate queries, so order *between entities* is not fixed.
- "Avoid binding a key and a button to the same trigger": primitive_world ships Escape + a HUD "Menu"
  button both on `toggle_pause`; advice like that must be scoped to "only matters in one frame".
- "Put must-not-lose reactions in global_on" conflicts with the pause docs' "bind gameplay triggers in
  playing's on:" - flag the tension.

**Older bare-edge-fix lessons that still hold:** an edge that transitively pulls systems in front of the
interpreters is a designer-visible latency change (needs a docs/30 line). `Action::Spawn` is deferred
(`SPAWNS_PER_FRAME = 2`), so a pre-interpreter "not in SpawnRegistry = gone" check deterministically acts
on just-spawned ids: a WARNING; the fix belongs in the spawn path. Test fixtures going red after an ordering
fix is the sign the fix is real (shared helper + why-comment, e.g. `local_coop_tests.rs::spawn_targetable_at`).
Not worth flagging: engine constants the fix merely touches, cross-capability imports.

Related: [[same-frame-order-determinism-pattern]], [[real-pause-pattern]], [[intent-event-layer-pattern]].
