# Feature: Attacker Identity on Hit Events (`{attacker}` token + `entity.hit:{victim}:{attacker}`)

_Status: Draft — plan-review 2026-10-02: needs more design work (see "Plan-review" section at the end)_
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

## Plan-review (2026-10-02)

Run by `/plan-review` while Frank was away: system-architect and ux-gamedesigner-reviewer, in parallel, read-only, claims checked against the code. **Combined verdict: needs more design work** — the blocking items below must be folded into this plan (and Frank's answers to the open questions recorded) before it can be marked Ready. The reports are reproduced verbatim (headings demoted one level).

### System-architect review

## system-architect plan-review: attacker_identity_on_hit_events.md

Reviewed on `integration` @ `34803b1` (plan's `Planned at: 09467a2` resolves; no relevant code drift since).

#### Verdict
**Needs more design work** (two scoped plan-text corrections; the core approach is sound).

The overall shape is right and the footprint is minimal: a string token plus a new event name, decoded by the single
consumer (`npc_hit_relay_system`). There is no new `Action` variant, no new `GameEvent` variant and no new message
channel, so D3's ordering and the interpreter's `let GameEvent::Trigger(name) = event;` readers are untouched. Most
findings check out against the code:
- the relay and its `iter().next()` guess: `npc.rs:170-193` / `:177`
- exact-match FSM: `fsm_interpreter.rs:60,75,106`
- the slot-time `rewrite_target`: `action_bar.rs:267-269`
- the intent event built with the owning player's id: `action_bar.rs:259-261`
- `SpawnRegistry` is a `BTreeMap`: `scene_manager/mod.rs:360-363`
- players are registered, because `tag_spawned_entity` always inserts into the registry: `mod.rs:443-461`
- the `fresh_global_transform` signature: `utils.rs:44`
- `rewrite_target` is exhaustive with no wildcard: `action_substitution.rs:149-258`

Two design points are wrong as written.

#### Blocking

**B1. Moving the demo to `entity.hit:{victim}:{attacker}` removes the only "this entity was hit" hook a behavior file can bind, which is the same objection the plan uses to reject the alternatives.**
- Evidence: `entity_fsm_interpreter.rs:53,67,82` builds `pattern = binding.event.replace("{self}", id)` and
  exact-matches it. So `on: "entity.attacked:{self}"` in a `.behavior.ron` is a working pattern today (hurt
  reaction, flee-on-hit, alert allies). No shipped file uses it, but designers can author it.
- `entity.hit:{self}:{attacker}` can't be bound that way: the attacker id isn't known when the behavior is
  written, and there is no wildcard matching (the plan's own finding).
- Approach step 4 switches all five `3rd_person_game_demo` slots to the new event, and the docs task relabels
  `entity.attacked:` as "attacker unknown". Together these teach designers to drop the bindable event. The plan
  rejects the typed `GameEvent::Hit` partly because it is "invisible to RON bindings", and its own choice has the
  same problem for victim-scoped bindings.
- Correction to the plan text:
  - Approach step 4 / demo: slots emit **both**, `EmitEvent("entity.attacked:{target}")` followed by
    `EmitEvent("entity.hit:{target}:{attacker}")`.
  - Docs: `entity.attacked:{victim}` = "victim was hit; bind this from behaviors". `entity.hit:{victim}:{attacker}`
    = "attribution channel; consumed by the engine and by global bindings that know the attacker".
  - Approach step 3: add a precedence rule so the order in which the two events arrive can't matter. Inside one
    `reader.read()` pass, collect attributed hits and legacy hits into two local maps keyed by victim. Then insert
    the legacy (heuristic) position only for victims with **no** attributed hit this frame. Without this, the
    legacy event can overwrite the correct attributed position.
  - Add test (g): a slot emits both events, legacy first, and `NpcHitQueue` holds the attacker's position.
  - Rewrite the acceptance criteria to match.
  - Answer Open question 2 with "keep, not deprecated; it is the victim-scoped event", not "fallback".

**B2. Interpreter-side `{attacker}` resolution is under-specified, and the matching acceptance criterion pairs the wrong victim for P2.**
- Evidence: an override binding exact-matches `intent.slot.K:{spawn_id}` (`fsm_interpreter.rs:60` against the name
  built at `action_bar.rs:259-261`). So the designer has already written the literal player id in `on:`, and
  `{attacker}` only saves retyping it.
- `{target}` in the same binding resolves to `CurrentTarget`, which is the **primary** player's target
  (`fsm_interpreter.rs:30`, then `rewrite_target` at `:62,78,109`). For a P2 override,
  `EmitEvent("entity.hit:{target}:{attacker}")` therefore becomes `entity.hit:<P1's target>:<P2>`. That is a
  correctly attributed hit on the wrong victim.
- Acceptance criterion 3 and test (e) as written would certify that.
- The plan also doesn't say which of the four push sites resolve the token (global_on, in-state `on`, transition
  exit actions, transition entry actions).
- It is silent on `entity_fsm_interpreter_system`, which imports `intent_slot_key` and can bind intents too
  (`entity_fsm_interpreter.rs:7`).
- Correction to the plan text (minimal footprint, recommended): drop interpreter resolution from v1. `{attacker}`
  resolves **only** in action-bar slot `do_actions`. FSM overrides author the literal player id they already
  bound, and the CLI flags `{attacker}` anywhere else (see N1). Remove test (e) and the "including when
  `state_machine.ron` overrides" clause from acceptance criterion 3.
- If Frank wants it kept: list all four push sites, state the primary-player `{target}` pairing caveat explicitly
  in the docs, and make test (e) assert the P1-target pairing as a documented limitation rather than as success.

#### Non-blocking

- **N1. The executor-warn citation is wrong; put the guard in the CLI.**
  - `action_executor.rs:165-172` is the `Action::Spawn` id leftover-`{` check, not general `{self}`/`{target}`
    diagnostics for `EmitEvent`. That same check already catches a leftover `{attacker}` in a Spawn id.
  - Better guard: a `misplaced_attacker_token` CLI error, a near-copy of `misplaced_new_id_token`
    (`crates/ironhold_cli/src/commands/validate.rs:1062-1077`), allowed only in `ActionBarDef` slot `do_actions`,
    plus a `cli` test.
  - A runtime one-shot warn needs `Local` state in the executor. A cheaper option is a `warn!` in the relay when
    the parsed attacker equals `"{attacker}"`.
- **N2. The fresh-transform rationale is schedule-specific; reword it to be schedule-neutral.** The relay is
  re-homed to `GameplaySet::PostExecute` by D3 (`gameplay_pipeline_system_sets.md:95`) and later to `FixedUpdate` by
  `gameplay_fixed_tick_pipeline.md:89,99,256`. The rule "use `Transform` via `fresh_global_transform`, because
  `GlobalTransform` propagation only runs in `PostUpdate`" holds in both schedules. Add no ad-hoc
  `.before/.after` edges; the relay's existing placement after the executor is all it needs. No conflict with
  D3/D4 otherwise.
- **N3. Separator parsing.**
  - Generated ids are `{prefab}_{counter}` (`action_executor.rs:~150-160`), so they contain no `:`. Hand-authored
    scene/Spawn ids are free strings.
  - Resolve the Tasks item by adding a CLI check that rejects `:` in authored ids, rather than switching the
    separator.
  - If a victim id could ever contain `:`, decide between `split_once` and `rsplit_once` explicitly in the plan.
- **N4. Test (f) is weak.** "Repeated runs produce the identical queue" passes even if the code depends on query
  order. Instead, run test (a) twice with the two players spawned in reversed order and assert the same result.
- **N5. Despawned attacker.** `SpawnRegistry` can still hold an id whose entity has been despawned; there is
  deferred-despawn history. Resolve as `registry.get(id)` followed by `query.get(entity).ok()`, and fall back on
  `Err`. The plan says "unresolvable", so make the `query.get` failure path explicit.
- **N6. Sequencing.** The interim nearest-player fix (`backlog.md:19`) has not landed. If both are picked up
  together, implement the heuristic inside this feature's legacy branch so it isn't written twice.
- **N7. Wording note.** Rejected alternative "typed `Message<HitEvent>`": the reason that holds is the extra D3
  ordering surface. The "invisible to RON" reason now applies equally to the chosen design without B1's fix, so
  drop it or qualify it.
- WASM/perf: confirmed negligible. There is no new system and no dependency; it adds one string `replace` per slot
  fire and one `BTreeMap` lookup per hit. Skipping wasm-perf-reviewer is justified.

#### Open questions for Frank
1. **Event name:** recommend `entity.hit:{victim}:{attacker}`. It is short, and it is distinct from
   `entity.attacked` so the meanings don't drift.
2. **Deprecate `entity.attacked:`?** Recommend **no**. Keep it permanently as the victim-scoped, bindable "was hit"
   event, and keep emitting it alongside `entity.hit:` (B1).
3. **Validation strictness:** recommend a hard CLI error that mirrors `misplaced_new_id_token`, allowing `{attacker}`
   only in slot `do_actions` (B2). Skip a runtime warn, or keep it to a relay-side literal check.
4. **NPC/environment attackers as aggro sources:** recommend accepting any registered attacker in v1, with no faction
   filter, and documenting it. Faction-aware threat belongs to the later threat/aggro item.
5. **Interim heuristic first?** Either order works. If both land in the same batch, fold the heuristic into this
   feature's legacy branch (N6).

### UX-gamedesigner review

## UX plan review: attacker_identity_on_hit_events.md

Reviewer: ux-gamedesigner-reviewer (pre-code plan review, 2026-10-02, `integration` @ `34803b1`)

The core idea works for designers. A token named after a role, sitting next to `{target}` in the same
slot `do_actions`, is the right shape. Keeping `entity.attacked:` working with no migration is
the right call, and the plan correctly rejects the typed-event alternatives, which designers could not see.
`action_needs_target` staying untouched is also good: a slot that uses only `{attacker}` (for example
a self-buff) will not be blocked by the no-target gate.

Three gaps need closing before coding. Each one causes a silent failure on the most natural
designer path:
- what happens when a designer emits both events,
- how a designer reacts to "this entity was hit by anyone",
- whether a misplaced `{attacker}` gets caught.

#### Verdict
**Needs more design work.** The changes are small and mostly decisions, not a redesign.

#### Blocking

**B1. Emitting both events silently brings the wrong-player bug back.**
- Plan, Approach step 3: the relay writes `hit_queue.0[victim] = pos` and the last event wins.
- The docs will keep describing `entity.attacked:{victim}` as supported. So the most natural
  migration is to *add* `EmitEvent("entity.hit:{target}:{attacker}")` and *keep* the old line.
  Designers will also do this on purpose, because they need the one-part event for RON reactions
  (see B2).
- `assets/projects/3rd_person_game_demo/scenes/main.scene.ron:443,458,472,487,520` put the old
  `EmitEvent` last in each list. A designer who adds the new line above it gets this result: the
  legacy branch runs second and overwrites the real attacker's position with the nearest-player
  guess. There is no warning, and in single-player nothing looks different.
- **Fix:** state a precedence rule in the plan. For the same victim in the same frame, an
  `entity.hit:` entry always beats an `entity.attacked:` entry, in either order. Add test (g): both
  events in either order, and P2's position wins. Document it in one sentence: "Emitting both is safe;
  the attacker-aware event takes priority."

**B2. After the migration, RON has no way to say "when this entity is hit by anyone".**
- FSM matching is exact string equality (plan Findings). So `entity.hit:{self}:...` can only be bound
  per attacker id, for example `entity.hit:{self}:player_01` plus `entity.hit:{self}:player_02`.
- Today `docs/30_runtime_events_and_logic.md:109` presents `entity.attacked:<id>` as an ordinary
  bindable event. That is the obvious hook for a hit-flinch, a hit sound or a "first hit starts the
  boss music" rule.
- Moving all five demo slots to `entity.hit:` removes the only shipped emitter of an attacker-free
  hit event.
- Worse, a designer will naturally write `on: "entity.hit:{self}:{attacker}"` in a victim's
  `.behavior.ron`, expecting `{attacker}` to capture whoever hit. That pattern can never match, and
  nothing reports it (see B3).
- **Fix:** decide and document the two-event model explicitly:
  - `entity.hit:{victim}:{attacker}` is for *attribution*: the engine uses it, and it can be bound per known attacker.
  - `entity.attacked:{victim}` is the *"hit by anyone" reaction* event, not a "legacy fallback".
  - Docs recommend emitting both when you want RON reactions (safe because of B1).
  - At least one demo slot shows both lines, with a one-line comment explaining why.
- Also add a sentence to the `{attacker}` docs: "`{attacker}` is filled in when the action *fires*.
  It is not a wildcard and cannot capture a value from an incoming event name."

**B3. A misplaced or misspelled `{attacker}` is mostly silent.**
- The plan's only safety net is a one-shot runtime `warn!` when an *`EmitEvent`* still contains the
  literal. The `validate` check is left as an open question. These cases get no signal at all:
  - `{attacker}` in any `on:`/`event:`/transition pattern (the B2 capture mistake). It is never an
    action, so the executor never sees it.
  - `{attacker}` in non-`EmitEvent` fields, for example a lifesteal
    `ModifyStat(key: "{attacker}.health", delta: 5.0)` or
    `ShowFloatingText(entity: "{attacker}", ...)` in a `.behavior.ron`, a dialogue choice, or a
    non-intent `state_machine.ron` binding. These fail as "entity not found" with no hint about
    why.
  - Typos such as `{atacker}` or `{Attacker}`. These pass through as a literal id.
- No existing `validate` check covers "a token used where it can't resolve": I grepped
  `crates/ironhold_cli/src/commands/validate.rs`, and the `{self}` hits are skip-logic, not checks.
  So the plan's "add a cli test beside the other substitution checks" has nothing to sit beside.
- **Fix (decide now, it changes the task list):**
  - (a) `ironhold validate` gives a hard error for `{attacker}` anywhere it can never resolve. That is
    statically decidable:
    - allowed in ActionBar slot `do_actions`, and in `do_actions` of a `state_machine.ron` binding whose `event:` starts with `intent.slot.`
    - an error anywhere else: `.behavior.ron`, `.dialogue.ron`, other `state_machine.ron` bindings, and every `on:`/`event:`/transition pattern
  - (b) The runtime warning covers every string field `rewrite_attacker` touches, not only
    `EmitEvent`. Use the same variant coverage the plan already mandates.
  - (c) Cheap and high value: an `unknown_token` validate warning for any `{word}` in an
    action/event string that is not one of the known tokens (`{self}`, `{target}`, `{new_id}`,
    `{attacker}`). This catches typos for all four tokens at once.
  - Add whatever ships to the "Checks performed" list in `docs/60_contributing.md` (~244-256).

#### Non-blocking

**N1. The docs task list points at a table that doesn't exist and misses several places.**
- Plan step 4 says "`docs/20_data_formats.md` (substitution-token table)". There is no such table.
  The token docs are spread out:
  - `{target}`: `docs/20_data_formats.md:1202`
  - `{self}`: `docs/30_runtime_events_and_logic.md:498-515`
  - `{new_id}`: `docs/30_runtime_events_and_logic.md:517-536`
- The full list of places to update:
  1. `docs/30_runtime_events_and_logic.md` ~536: a new `### {attacker} substitution` section next to
     `{new_id}`. Say where it resolves, where it doesn't, and that it is not a capture wildcard.
  2. `docs/30_runtime_events_and_logic.md:109`: add the `entity.hit:<victim>:<attacker>` bullet and
     reword the `entity.attacked` bullet per B2. Line 113 (`npc.investigating`) says "last-known
     attacker position"; add "(the real attacker when `entity.hit:` is used, otherwise the nearest
     player)".
  3. `docs/20_data_formats.md:1202`: add a sibling `{attacker}` paragraph right after the `{target}`
     paragraph.
  4. `docs/20_data_formats.md:1191-1200`, the co-op intent-override callout. **This one matters most.**
     After this feature, an override rule resolves `{attacker}` per firing player, but `{target}`
     still resolves to the *primary* player. A designer will assume both behave the same. Say the
     difference explicitly, side by side.
  5. `docs/20_data_formats.md:1181`: the rage-strike example uses `EmitEvent("combat.hit:player_01")`,
     a third naming style. Change it to `entity.hit:{target}:{attacker}` so the docs show one
     convention.
  6. `docs/20_data_formats.md` NPC section (~3230-3262): there is no "how do I make an NPC react
     when I hit it" recipe. Damage alone (`ModifyStat`) does **not** aggro an NPC; the designer must
     emit the event. Today that fact lives in only one bullet in docs/30. Add a three-line recipe and
     link it from the `investigate_timeout_secs` row (3262).
  7. `docs/20_data_formats.md` action table rows for `EmitEvent` (3828) and `EmitEventAfterDelay`
     (3836): they list only "`{self}` substituted in behavior files". Add `{target}`/`{attacker}`.
  8. `docs/STATUS.md`: the entity messages list (~110-114) and any NPC/combat feature row. This is
     the place that keeps getting missed.
  9. `docs/60_contributing.md` checks list, per B3.
- `planning/investigations/rpg_event_taxonomy.md:107` proposes `combat.hit:{attacker}:{target}`. That
  is a different namespace *and* the reverse argument order. Update it in the same change so the
  "convention" this plan sets is not contradicted by the taxonomy doc it cites.

**N2. No shipped example demonstrates the part of this feature that actually changes behavior.**
- `3rd_person_game_demo` is single-player, so switching its five slots changes nothing visible. It
  is a good regression check, not an example.
- `local_coop_demo` has no NPCs (no `on_player_near` anywhere in the project). The plan's co-op
  playtest relies on "a scratch 2-player scene", which would be thrown away.
- **Recommend:** add one Chase NPC to `local_coop_demo/scenes/room3.scene.ron`. That room already has
  per-player bars (`action_bar_p1` G, `action_bar_p2` L, lines 243-289) and per-player targeting (T/M).
  Add `EmitEvent("entity.hit:{target}:{attacker}")` to both slots. That becomes the canonical co-op
  attribution example, and playtest step 3 becomes reproducible.
- The room3 comments are already long. Keep any new hint `Label` under the ~82-character
  one-line limit for that project's 22px font, and use ASCII `-` rather than an em dash in in-game
  text (the engine font has no em-dash glyph).

**N3. The demo's Taunt slot is the clearest showcase, but the token name works against it.**
- `main.scene.ron:516` hardcodes `ShowFloatingText(entity: "player_01", text: "Taunt!")`. That is
  exactly the per-player id `{attacker}` exists to replace, and it is broken today in any co-op
  reuse of that bar.
- Converting it is a good demo. But `ShowFloatingText(entity: "{attacker}", text: "Taunt!")`, or a
  heal slot `ModifyStat(key: "{attacker}.health", delta: +20)`, reads oddly. The token really means
  "the player who fired this slot", whatever the slot does. See open question 6.

**N4. The plan's non-goal suggests a token that doesn't exist.**
- Non-goals: "making `enemy_*.behavior.ron` emit `entity.hit:{player}:{self}`". There is no `{player}`
  token. The shipped enemy behaviors damage the global `player_health` and don't know which player
  they hit.
- Fine as a planning note, but make sure that example never reaches `docs/`. A designer would copy
  it and get a literal `{player}` (which B3(c) would at least catch).

**N5. Playtest checklist gaps.**
- Add a misuse step. Put `on: "entity.hit:{self}:{attacker}"` in a scratch behavior and run
  `tools/bin/ironhold validate`. Expect the B3 error and a clear message.
- Add a both-events step: one slot emits `entity.hit` *and* `entity.attacked`, in each order. The NPC
  still walks to the firing player (B1).
- Step 3 should use the room3 setup from N2. Activate P2's slot once by its key and once by its
  gamepad `RightTrigger`, so the owner_player path is covered on both input sources.
- Step 2 says "no `{attacker}` literal warnings". Also confirm `npc.investigating:{id}` fires
  exactly once per hit when both events are emitted.

**N6. Self-hit and unresolvable attackers fall back quietly. That is correct, but document it.**
- One sentence in the `{attacker}` docs section: "If the attacker no longer exists, or is the victim
  itself, the NPC falls back to the nearest player."
- No warning is needed: this is a legitimate runtime situation, not contradictory authoring.

#### Open questions for Frank (with recommendations)

1. **Event name: `entity.hit:{victim}:{attacker}` or `entity.attacked_by:`?**
   - **Recommend `entity.hit:{victim}:{attacker}`, victim first.**
     - It keeps the `entity.<verb>:<the entity it happened to>` shape of the existing
       `entity.interacted:`/`entity.entered:`/`entity.attacked:` family.
     - A victim's behavior then reads `entity.hit:{self}:...`, the same as `entity.attacked:{self}`.
   - Avoid `attacked_by`. It shares a prefix with `entity.attacked`, and a near-miss typo between
     the two silently never matches.
   - In the same change, reconcile `rpg_event_taxonomy.md:107` (`combat.hit:{attacker}:{target}`)
     and the docs/20:1181 `combat.hit:player_01` example (see N1).

2. **Deprecate `entity.attacked:{victim}`?**
   - **Recommend no.** Keep it permanently, reframed as the "hit by anyone" reaction event (B2), not
     a "legacy, attacker unknown" fallback.
   - No `validate --strict` nudge. Warning about a legitimate, documented choice trains designers
     to ignore warnings. The engine's existing rule is to warn only when authored intent
     contradicts itself.

3. **Validation strictness for `{attacker}`?**
   - **Recommend a hard `validate` error in the statically unresolvable contexts**, plus the broader
     runtime warning and the unknown-token warning (B3).
   - The allowed contexts are exactly two and both are easy to detect in the files, so a hard error
     will not produce false positives.

4. **Should NPC-vs-NPC / environment attackers count as aggro sources?**
   - **Recommend yes for v1, documented as "the NPC investigates whichever entity the event names."**
   - No shipped content emits that today, so the risk is zero. Faction filtering would be hidden
     engine behavior that a designer could not see or override from RON. If it is ever needed, it
     should be an `NpcDef` field.

5. **Ship the interim nearest-player fix first?**
   - **Recommend yes.** Then the docs can describe the fallback once, accurately ("nearest player to
     the victim"), instead of describing first-in-query-order behavior that is about to change.

6. **(New) Token name: `{attacker}`, or something role-neutral like `{caster}`/`{actor}`?**
   - The token resolves to the slot's owning player for *every* action in the slot, including heals,
     buffs and taunt text (N3).
   - **Recommend:** keep `{attacker}` only if its docs open with "the player who fired this slot,
     whatever the slot does". Otherwise choose `{caster}`, which RPG designers already read as
     "who used this ability".
   - Decide before docs and demos lock in the name. Renaming a token later means a migration for
     every project.
