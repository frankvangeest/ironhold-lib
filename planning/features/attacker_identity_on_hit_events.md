# Feature: Attacker Identity on Hit Events (`{attacker}` token + `entity.hit:{victim}:{attacker}`)

_Status: Draft_
_Planned at: `09467a2` (2026-10-02)_

Backlog item: **Attacker identity on hit events (replaces the nearest-player heuristic in `npc_hit_relay_system`)**
(`## Queued`/Beta), the deferred half of the Bugs entry "`npc_hit_relay_system` treats the first player in query
order as the attacker". Related, **not** in this plan: Quest `KillCount` (`planning/features/quest_system.md`),
D3/D4 determinism items.

## What
A hit event can say *who* hit. A designer authors a new substitution token, `{attacker}`, which resolves to the
spawn id of the entity performing the action, and emits a new event `entity.hit:{victim}:{attacker}`:

```ron
// action-bar slot do_actions (3rd_person_game_demo/scenes/main.scene.ron)
ModifyStat(key: "{target}.health", delta: -30.0),
EmitEvent("entity.hit:{target}:{attacker}"),
```

`npc_hit_relay_system` then sends the NPC to the **real attacker's** last-known position. The existing
`entity.attacked:{victim}` event keeps working unchanged (no attacker known, falls back to the interim
nearest-player heuristic), so no existing binding or project breaks.

## Why
- Today `entity.attacked:{victim}` carries only the victim; `npc_hit_relay_system` (`npc.rs:170-193`) guesses
  with `player_query.iter().next()` (`:177`) - the first player in query order, and a one-tick-stale
  `GlobalTransform` read in `Update`. In co-op the NPC investigates the wrong player (Bug entry; audit finding E in
  `planning/investigations/hashmap_iteration_order_audit.md`). Frank decided 2026-10-01 to ship a nearest-player
  heuristic first; this is the proper fix.
- It is the only way to attribute anything per player downstream (kill credit, loot/XP, threat) without more
  guessing, and it is a data-driven authoring surface (RON), consistent with the "designer needs no Rust" rule.

## Findings (verified against code at `09467a2`)
- **Only one emitter and one consumer exist.** `entity.attacked` appears in `crates/` only at `npc.rs:11,179`
  (consumer); in assets only at `3rd_person_game_demo/scenes/main.scene.ron:443,458,472,487,520` (five action-bar
  slots: Attack, Heavy Strike, Poke, Mana Blast, Taunt); docs at `docs/30_runtime_events_and_logic.md:109` and
  `assets/projects/CLAUDE.md:140`. **No `state_machine.ron` or behavior file binds it** and no test emits it
  (the backlog's "`state_machine.ron` bindings" worry does not apply today). `npc_tests.rs` seeds `NpcHitQueue`
  directly (`:97,141,180,267`).
- **NPC attacks emit no hit event at all.** Enemy behaviors just `ModifyStat(key: "player_health", ...)`
  (`enemy_zombie.behavior.ron:57`, `enemy_snake:48`, `enemy_spider:49`); `primitive_world/goblin_guard.behavior.ron:31`
  likewise. So "NPC attack = `{self}`" is a future authoring option, not an existing site.
- **FSM event matching is exact string equality** (`fsm_interpreter.rs:60,75,106`). Appending `:{attacker}` to
  `entity.attacked:{victim}` would silently stop matching any binding written for the 1-part name, and a
  wildcard/prefix match does not exist. This kills the "name suffix on the existing event" candidate.
- **`GameEvent` is `enum { Trigger(String) }`** and every reader destructures it irrefutably
  (`let GameEvent::Trigger(name) = event;`, `fsm_interpreter.rs:46`, `npc.rs:178`). Adding a typed
  `GameEvent::Hit { .. }` variant breaks every reader and the ordering story of D3; a separate typed `Message`
  would be a new unordered channel that the D3 schedule tests would have to learn about.
- **The action bar already knows the acting player at the moment it substitutes `{target}`.**
  `action_bar_input_system` resolves `spawn_id` (the owning player's `SpawnId`, via `owns_slot(slot.owner_player,..)`,
  `action_bar.rs:208-210`) and rewrites each action with `rewrite_target(a.clone(), target_id)` (`:267-269`)
  *before* storing it in `PendingIntentActions`. Adding a sibling rewrite here is free and per-player correct in
  split-screen / local co-op (the same mechanism that makes `{target}` per-player, `src/CLAUDE.md` Phase 2 notes).
- **Interpreter-overridden slots lose the player.** If a `state_machine.ron` binding intercepts
  `intent.slot.K:{player_id}` its replacement `do_actions` go through `rewrite_target(.., CurrentTarget)`
  (`fsm_interpreter.rs:62,78,109`) - primary player only (documented out-of-scope boundary). The triggering event
  *name* carries `{player_id}`, though (`action_bar.rs:259-261`), so the interpreter can recover the attacker from
  `intent_slot_key`'s sibling parse, without new plumbing.
- **Substitution is three exhaustive-ish matches that must stay in sync:** `rewrite_self` (`action_substitution.rs:13`),
  `rewrite_target` (`:149`), `action_needs_target` (`action_bar.rs:407`), plus dialogue's `substitute_self_in_action`
  (`dialogue.rs:320`). The `EmitEvent`/`EmitEventAfterDelay` arms are at `:40,62,187-189`.
- **Registry lookup is deterministic.** `SpawnRegistry.entities` is a `BTreeMap<String, Entity>`
  (`scene_manager/mod.rs:360-363`) so resolving attacker id to entity needs no query-order scan. `NpcHitQueue` is
  a `HashMap` but keyed-lookup only (audit, "Keyed-lookup only"), so it can stay.
- **Attacker position must come from `fresh_global_transform`** (`utils.rs:44`), not raw `&GlobalTransform`: the relay
  runs in `Update`, the rule in `src/CLAUDE.md` ("Physics & movement must use `FixedUpdate`").

## Approach
Chosen: **explicit `{attacker}` token + a new, separately named event `entity.hit:{victim}:{attacker}`; the typed
data is the event name, the relay resolves position through `SpawnRegistry`.** Legacy event stays as the fallback.

1. **Token `{attacker}`** - resolves to the spawn id of the entity that caused the action.
   - Action bar (`action_bar_input_system`): new `rewrite_attacker(action, spawn_id.0)` applied next to
     `rewrite_target` at `action_bar.rs:268`. Always resolvable (a firing slot always has an owning player), so
     `action_needs_target` does **not** need an arm and the `no_target` gate is unaffected.
   - Interpreters: `fsm_interpreter_system` resolves `{attacker}` only when the triggering event is
     `intent.slot.K:{player_id}` (the `{player_id}` part becomes the attacker). `entity_fsm_interpreter_system`
     does not resolve it: an NPC that attacks authors `{self}` directly. Anywhere else `{attacker}` stays literal,
     and the executor logs a one-shot `warn!` if an `EmitEvent` still contains it (same shape as
     `action_executor.rs:165-172`'s `{self}`/`{target}` diagnostics).
   - Implemented as one new helper `rewrite_attacker` with the **same variant coverage as `rewrite_target`**
     (copy its match; do not invent a narrower set), plus a matching arm in dialogue's `substitute_self_in_action`
     only if dialogue gets an acting-player concept (it does not today; document, do not add).
2. **Event `entity.hit:{victim}:{attacker}`.** Ids contain no `:` (verify; see Tasks) so the relay does
   `rest.split_once(':')`. New name instead of a suffix on `entity.attacked:` precisely because matching is exact.
   Convention documented as the preferred form; `entity.attacked:{victim}` documented as "attacker unknown".
3. **`npc_hit_relay_system`** (`npc.rs:170`): add `Res<SpawnRegistry>` and a query
   `Query<(Option<&Transform>, &GlobalTransform, Option<&ChildOf>)>`.
   - `entity.hit:{victim}:{attacker}`: resolve attacker -> entity -> `fresh_global_transform`; insert
     `hit_queue.0[victim] = pos`. If attacker == victim, or unresolvable (despawned the same frame), fall through to
     the legacy path.
   - `entity.attacked:{victim}` (legacy): the nearest-player-to-victim, `SpawnId`-tie-broken heuristic from the
     Bug fix (replace `iter().next()` if that fix has not landed; either way it becomes the *only* use of it).
   - `NpcHitQueue` type is unchanged (`HashMap<String, Vec3>`), so `npc_behavior_system` and `npc_tests.rs` need no
     change. Multiple hits on one NPC in a tick: last event wins, event order is fixed by D3 (until D3 lands it is
     the same exposure `entity.attacked` has today; noted, not widened).
4. **Demo + docs.** Switch the five `3rd_person_game_demo` slots to `entity.hit:{target}:{attacker}`; update
   `docs/30` L109, `docs/20_data_formats.md` (substitution-token table), `assets/projects/CLAUDE.md:140`, and
   `src/CLAUDE.md` ("Supported substitutions"). Add a `local_coop_demo` hook only if it already has an NPC
   (open question below).
5. **Perf/WASM:** one `BTreeMap` lookup + one transform read per hit event, only on frames a hit fires; one string
   `replace` per action at slot-fire time (already done for `{target}`). No new per-frame system, no new
   dependency, no WASM-specific code.

### Rejected alternatives
- **Suffix on the existing name (`entity.attacked:{victim}:{attacker}`):** breaks exact-match bindings of the
  1-part name (no wildcard matching exists); would force a migration of every project for no gain.
- **Typed `GameEvent::Hit { victim, attacker }` variant or a separate `Message<HitEvent>`:** breaks every irrefutable
  `let GameEvent::Trigger(..)` reader, is invisible to RON bindings (a designer could not react to who hit), and a
  second message channel needs its own D3 ordering. Revisit only if a payload richer than two ids is needed
  (damage amount, damage type) - that is the combat-event taxonomy work.
- **Companion resource (`LastHit { victim, attacker }`) written by the executor:** one slot per frame loses
  simultaneous hits and re-creates exactly the "who hit whom this tick" implicit state this fixes.
- **Auto-injecting the attacker in `ModifyStat`:** couples damage to attribution in Rust; a heal, DoT or environment
  hazard has no attacker.

## Non-goals (explicit)
- **Kill credit.** `entity.died`/`stat.*.health.depleted` carry no killer today (`npc.rs:186`). Follow-up item:
  `LastHitBy(spawn_id)` per victim, written by the same relay, read by quest `KillCount`; needs the Quest plan
  first. This plan only guarantees the data (`entity.hit:` ordering before depletion) exists.
- **Threat/aggro tables, per-player loot/XP.** Build on this event later; no types are introduced for them here.
- **NPC-attack emission** (making `enemy_*.behavior.ron` emit `entity.hit:{player}:{self}` so players could react):
  the token/event already support it with `{self}`; adding it to shipped behaviors is a separate design call.
- **Interpreter-overridden slots for non-primary players:** stays the documented scope boundary except for the
  `{attacker}` token itself, which does resolve from the `intent.slot.*:{player_id}` event name.

## Tasks
- [ ] Verify spawn ids cannot contain `:` (`tag_spawned_entity`, `Action::Spawn.id`, `{new_id}` composition); if they
      can, reject at scene-load/`validate` and document, or switch the separator the plan uses (decide before coding).
- [ ] `action_substitution.rs`: `rewrite_attacker` (same coverage as `rewrite_target`, incl. `EmitEvent`,
      `EmitEventAfterDelay`); unit tests alongside the existing ones (`:305-394`), including the
      "identity for fieldless variants / leaves catalog keys untouched" cases.
- [ ] `action_bar.rs`: apply `rewrite_attacker(.., &spawn_id.0)` at `:268`; confirm `action_needs_target` unchanged.
- [ ] `fsm_interpreter.rs`: resolve `{attacker}` from an `intent.slot.K:{player_id}` trigger only; leave literal
      otherwise.
- [ ] `action_executor.rs`: one-shot `warn!` if an `EmitEvent` still contains a literal `{attacker}`.
- [ ] `npc.rs`: new `entity.hit:` branch + legacy heuristic fallback in `npc_hit_relay_system`; `SpawnRegistry` and
      transform query params; `fresh_global_transform`; self-hit and despawned-attacker guards.
- [ ] Tests (`npc_tests.rs`/`local_coop_tests.rs`): (a) P2 fires a slot with `EmitEvent("entity.hit:{target}:{attacker}")`
      while P1 is nearer the NPC -> `NpcHitQueue` holds **P2's** position and the NPC walks to P2; (b) legacy
      `entity.attacked:` still queues the heuristic position; (c) unresolvable/self attacker falls back, no panic;
      (d) two simultaneous hits by different players on different NPCs map independently; (e) a slot overridden by
      `state_machine.ron` still resolves `{attacker}` from the intent event name; (f) repeated runs produce the
      identical queue (no query-order dependence).
- [ ] Schema/CLI: no new `Action` variant, so `cargo check -p ironhold_cli` should be a no-op; still run it
      (mandatory gate). `ironhold validate`: decide whether to add a check flagging a literal `{attacker}` in a
      non-slot, non-intent context (open question); if yes add a `cli` test beside the other substitution checks.
- [ ] `ron_lint`/`ron_validation`: re-run after the demo RON edit (step 10 rule).
- [ ] Docs: items in Approach step 4; `planning/claude_suggestions.md` entry for the kill-credit follow-up (Frank
      reviews); add the backlog follow-ups (kill credit, NPC attack emission) at Done-marking time.
- [ ] Code-change workflow: alignment-reviewer, system-architect, debug-detective in parallel; ux-gamedesigner-reviewer
      (RON authoring surface + docs changed); wasm-perf-reviewer **skipped** (no per-frame work), say so.

## Playtest checklist
1. `3rd_person_game_demo`: stand away from an idle Chase-faction enemy outside its detection radius, hit it with
   slot 1 from range -> it enters `Investigating` and walks to you (unchanged single-player behavior).
2. Console: no `{attacker}` literal warnings; `npc.investigating:{id}` fires.
3. `local_coop_demo` (or a scratch 2-player scene with one NPC and a slot on P2's bar using the new event): P1 far
   away, P2 attacks -> NPC walks to **P2**, not P1; swap roles to confirm it is not "nearest" luck.
4. Legacy check: temporarily author `EmitEvent("entity.attacked:{target}")` on one slot -> still aggros (heuristic).
5. Web build: same as 1 (WASM path is identical code).

## Open questions
1. **Event name:** `entity.hit:{victim}:{attacker}` (recommended) vs `entity.attacked_by:...`? Pure naming, but it
   becomes the combat-taxonomy convention (`planning/investigations/rpg_event_taxonomy.md` already lists
   `entity.attacked`, and warns about naming drift).
2. **Deprecate `entity.attacked:{victim}`?** Recommended: keep as documented "attacker unknown" fallback indefinitely.
   Alternative: warn at scene load / `validate --strict` when a slot emits it, nudging migration.
3. **Validation strictness:** should `ironhold validate` hard-error on `{attacker}` outside a slot `do_actions` /
   intent-override context (where it can never resolve), or just warn at runtime?
4. **Should NPC-vs-NPC / environment attackers count as aggro sources** (`entity.hit:{npc}:{other_npc}` makes the
   victim investigate `other_npc`)? The relay as designed accepts any registered attacker; faction filtering is not
   applied. Probably fine for v1, but it is a gameplay call.
5. **Does the interim nearest-player fix ship first** (as decided 2026-10-01)? This plan works either way but is
   cleanest if that lands first so the legacy branch is exactly that heuristic.

## Acceptance criteria
- Given two players and a Chase NPC outside detection range, when P2 hits it via a slot authored with
  `EmitEvent("entity.hit:{target}:{attacker}")`, then the NPC's `last_known_attacker_pos` is P2's position
  regardless of which player is nearer or first in query order, on native and web.
- Given an existing project that only emits `entity.attacked:{victim}`, then behavior is identical to the interim
  heuristic and no binding, RON file, or test needs to change.
- Given `{attacker}` in a slot's `do_actions`, then it is a concrete player spawn id by the time it reaches
  `ActionQueue`, including when `state_machine.ron` overrides the slot's intent.
- `cargo test -p ironhold_core --test '*'`, `cargo check -p ironhold_cli`, `ron_lint` and `ron_validation` pass;
  docs list `{attacker}` and `entity.hit:` and describe `entity.attacked:` as the attacker-unknown fallback.
