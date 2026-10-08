# Lootable corpses, container panels and respawn

The corpse design, its respawn rules and the `{new_id}` id scheme.

> Moved from `crates/ironhold_core/src/CLAUDE.md` on 2026-10-08 (the core CLAUDE.md split). The hard rules for this area stay in the folder `CLAUDE.md` files; this page keeps the long-form reference and history verbatim.

## Lootable corpse design summary

<!-- b:478 -->

**Lootable corpse (loot-on-death), `planning/features/done/monster_corpse_loot.md`** — on death, a
monster despawns itself and is replaced by a separate, disposable corpse entity at the same
position/facing, so a fresh respawned monster and its still-lootable corpse can coexist as two
independent entities (v1 shipped a same-entity version first; this superseded it once a real
requirement — an unconditional fixed respawn delay, fully decoupled from how long the corpse
persists — made the same-entity model's inherent limitation a blocker, not just a documented
tradeoff). See `assets/projects/3rd_person_game_demo/behaviors/enemy_zombie.behavior.ron` +
`zombie_corpse`'s shared `behaviors/lootable_corpse.behavior.ron` for the full reference
implementation, and `docs/30_runtime_events_and_logic.md`'s "Lootable corpse (loot-on-death)"
section for the designer-facing walkthrough.

## A dying entity can't catch its own respawn timer; global rule needed

<!-- b:563 -->

**The dying entity's own per-entity behavior file cannot catch its own respawn timer, because it
won't exist anymore when that timer fires — and the catching rule must be pause-proof.** A
monster's `dead` state arms `EmitEventAfterDelay(event: "monster.respawn:{self}", delay_secs:
60.0)` *before* despawning itself (the delayed event is a plain `(f32, String)` entry in the global
`DelayedEventQueue`, entirely independent of the entity that armed it — see "Despawning" notes
elsewhere in this file). But once that entity is gone, `entity_fsm_interpreter_system` has no live
entity with that `SpawnId` left to match a per-entity `on:` handler against — the event needs a
**global** rule, one per scene-placed instance, keyed by the monster's *literal* scene id, exactly
the same convention `chest_01`'s own `entity.exited:chest_01 → CloseContainer` global rule already
uses. `spawn_point` (not `at_entity`) is used for this respawn `Spawn` — the replacement should
reappear at its original patrol spot, not wherever the previous instance happened to die.

## Respawn rules must be in `global_on`, not state-scoped `on:`

<!-- b:575.rule -->
<!-- b:575.ref -->

These six rules **must live in `state_machine.ron`'s top-level `global_on:` block, not inside
`"playing"`'s own state-scoped `on:` list** (found by both `alignment-reviewer` and
`system-architect`, independently, during the final review pass — a critical bug, not a style
preference). `tick_delayed_events_system` ticks on raw `Time` with no pause-gate, so a monster's
30s respawn timer can fire while the game is in a non-`"playing"` state (e.g. paused); a
state-scoped `on:` handler simply never matches in that case, silently and permanently losing that
monster's respawn for the rest of the session. `global_on` fires "regardless of state, no state
change," so it always catches the event no matter what state the interpreter is in when it lands.
The event name is also unified to a single `monster.respawn:{id}` convention rather than
per-type (`zombie.respawn`/`snake.respawn`/`spider.respawn`) names, so adding a 4th monster type is
one copy-pasted rule line, not a new event-name convention to keep in sync everywhere.
