---
name: same-frame-event-order
description: Effective same-frame FSM event order a designer actually observes (UI stream > carried-over > FixedUpdate > Emit chain > Scene); D3 plan only fixes the Emit-chain slice; which demo bindings are order-sensitive
metadata:
  type: project
---

Verified 2026-10-02 (plan-review of `planning/features/gameplay_pipeline_system_sets.md`, D3):

**Effective order the interpreter sees in one frame** (fsm_interpreter.rs reads UI, then Game, then Scene;
each event applied one at a time, state change immediate, actions all executed after):
1. UI stream: key bindings (`global_input_system`, internal order = LoadedKeyBindings HashMap -> D1 sorts by
   key name) -> unclaimed gamepad -> Button -> IconButton.
2. Game stream, in buffer order: (a) events written AFTER last frame's interpreter — EmitEvent, stat
   thresholds (`stat_threshold_system` in Execute), `action_bar.activated`, dialogue/audio/container events
   — "one frame late" and they come FIRST; (b) this frame's FixedUpdate (collectible, trigger_zone, npc.*,
   player.jumped; 0/1/2 batches); (c) the Update Emit chain (D3: Targeting -> ActionBar -> Interact ->
   Delayed -> modifier-expiry).
3. Scene stream (scene.ready etc.) last.
- Action-bar slot built-in `do_actions` are flushed AFTER all FSM reactions (flush_pending_intent) and are
  NOT state-gated (inventory slot "i" toggles even while paused).
- `dialogue_tick_system` pushes to ActionQueue BEFORE the interpreters (a de-facto third interpreter).

**Why it matters:** "input, then delayed, then stat" (Frank 2026-10-01) is only true inside slice 2c;
threshold events (2a) precede input in the Game stream. Three conflicting order statements existed at
review time: plan step 3, backlog D3 entry, docs/30 "System ordering" (~873).

**Order-sensitive shipped content:** pause overlay resume-button + Esc (key-first = intuitive);
Esc + playing-scoped click (purchase dropped); primitive_world `game_over` transition is `from: "playing"`
only, so death while paused never reaches game over (edge-triggered; pause is cosmetic, see
[[pause-is-cosmetic]]); entity_logic_demo respawning_gem interacted vs gem.reappear; enemy_* behaviors are
order-proof (transitions authored from both alive and attacking) — the pattern to recommend.

**D1 review (2026-10-02) facts:** slot `key`/binding keys sort as the LITERAL authored string, case-sensitive
byte order (digits < capitals < lowercase): `"0"` < `"1"`, `"F10"` < `"F2"`, `"KeyE"` < `"i"` (3rd_person
inventory slot is bare `"i"`). `"10"` is not a valid key name; the real tenth-slot trap is `"0"`. Threshold order
= stats.ron stat order, then each stat's `thresholds` list order (the common case: 30->0 crosses `low` and
`depleted` together; shipped files list `depleted` first). All modifier-expiry events precede all threshold
events; global stats precede `stat_templates` stats; cross-entity instance-stat order still unordered (D4).
Unclaimed-gamepad: only ONE trigger per frame across ALL pads (break after first match).

**How to apply:** for D1/D3/D4/fixed-tick reviews, check the designer doc states the whole model, not just
the slice the feature touches; reliable manual repro = bind one key to two emitters (scene_key_binding +
action-bar slot, or action-bar slot on the interact key). Related: [[fsm-designer-traps]].
