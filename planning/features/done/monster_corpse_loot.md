# Feature: Monster corpse loot

_Status: Done — playtest confirmed 2026-08-26_
_Planned at: `452e2e2` (2026-08-24)_

## Phases

| Phase | Backlog item | Status | Completed |
|---|---|---|---|
| v1 | Same-entity loot-on-death (RON-only, no engine change) | Done, superseded by v2 | 2026-08-25 |
| v2 | Separate corpse entity via `Action::Spawn.at_entity` (real respawn decoupling) | Done | 2026-08-26 |

v1 shipped first (zombie, then extended to snake/spider) and was fully playtest-confirmed. It was
then **entirely replaced** by v2 in this same session, once a concrete requirement — an
unconditional fixed 1-minute respawn delay, completely independent of how long the corpse persists
(up to 5 minutes if unlooted) — made v1's inherent same-entity limitation a hard blocker rather
than a documented tradeoff. v1's own RON no longer exists on disk; its section below is kept as
historical review context (real bugs found and fixed there still apply/informed v2's design).

## What
When a monster's health is depleted, its death animation plays, then it despawns and is replaced
by a separate, disposable corpse entity at the exact same position/facing. The corpse is lootable
via the existing container system and disappears once looted (short decay) or after 5 minutes
unlooted (ambient decay). **Independently**, a fresh live instance of the monster respawns exactly
1 minute after death, at its original patrol spot — regardless of whether the corpse has been
looted, is still sitting there, or has already decayed.

## Why
Originally (`enemy_zombie.behavior.ron` and siblings) a killed monster was hidden in place at a
fixed delay and revived (`ResetToSpawn`) at a second fixed delay — no loot existed for monsters at
all, only static chests/props authored with `inventory:` directly. v1 added loot but kept the
same-entity revival model, so the loot window still had to fit inside however long the monster
took to respawn — coupling "when this monster comes back" to "how long the player has to loot it,"
the wrong axis: a slow looter shouldn't be raced by respawn, and a monster's respawn cadence
shouldn't have to also double as a loot-availability window.

The concrete trigger for v2: a real requirement (Frank, 2026-08-26) for corpses to persist up to 5
minutes unlooted while the monster itself respawns after a fixed 1 minute, unconditionally. In a
same-entity model this is a **logical impossibility**, not just an inconvenience — reviving the
monster (`ResetToSpawn`) *is* what ends the corpse, so a corpse can never outlive a shorter
respawn timer. Two independent timers on two independent entities is the only way to satisfy both
numbers simultaneously, which is what v2 delivers.

## Shared groundwork (from v1, still true)

`container.opened:{id}` / `container.closed` / `container.looted:{id}` are documented in
`docs/30_runtime_events_and_logic.md`'s event table (added during v1). The "two-state decay, not
two racing timers" pattern (`fresh` → `looted`, ambient timer vs. short post-loot timer) carries
forward into v2's corpse behavior unchanged — see `behaviors/lootable_corpse.behavior.ron`.

**Known gap, still true**: `TakeAllFromContainer` early-returns before emitting `container.looted`
when the container has zero items — a corpse authored with no loot never reaches `looted` via that
path, and just decays on the ambient timer instead. "Empty" and "never looted" remain
indistinguishable by design.

## v1 — same-entity loot-on-death (superseded, kept for history)

**This RON no longer exists in the project** — `enemy_zombie`/`enemy_snake`/`enemy_spider`'s
prefabs and behavior files were rewritten for v2 (see below). This section documents what shipped
and what was learned, since several findings directly shaped v2's design.

**Original approach**: `interactable:`/`inventory:` added directly to each monster prefab (present
the whole time, including while alive — only reachable once the entity's own behavior file reached
a `dead_*` state, since entity-FSM `on:` handlers are scoped per-state). Death split into
`dead_full` (lootable, ambient hide/respawn timers) → `dead_looted` (fast-decay once emptied).

**Real bugs found and fixed during v1's review, all directly informing v2:**
- `Inventory` is a persistent component, not reset by `ResetToSpawn` — required a manual
  `RemoveItem(..., count: 999)` + `AddItem(...)` re-seed on every revival to avoid a
  permanently-empty or doubled corpse. **v2 eliminates this class of bug entirely** — a fresh
  `Action::Spawn` always gets a fresh `Inventory` from `initial_items`, no re-seeding needed.
- A `dead_looted` state arming its own faster respawn timer alongside `dead_full`'s ambient one
  created a real stale-timer race (system-architect finding): reviving early via the short timer,
  then dying again before the original long timer elapsed, let that stale timer revive the entity
  *again* prematurely from the second death's fresh `dead_full`. **v2 eliminates this too** — decay
  always ends in a real `Despawn`, never a state transition a stale timer could re-trigger against
  a still-live entity.
- `trigger_zone` on a Dynamic-body NPC gets no `ColliderMassProperties` override, making the
  monster ~146x heavier than intended (real engine bug, tracked in `planning/backlog.md`, not
  fixed). v1 and v2 both avoid `trigger_zone` on monster/corpse prefabs entirely as a result.
- `Action::OpenContainer`/`Action::OpenShop` both had a `panels_open` double-increment bug (two
  interactable entities in range of one interact press) — fixed with a guard in both, general
  container-system fixes that v2 also relies on.
- A playtest report ("F does nothing near a corpse") root-caused to an unrelated environmental
  cause (stale dev server serving the wrong worktree) — see `corpse_loot_interact_tests.rs`'s doc
  comment for the full story. No code/RON bug.

v1's own acceptance criteria were all met and playtest-confirmed before being superseded.

## v2 — separate corpse entity

### Engine change: `Action::Spawn.at_entity: Option<String>`

`Action::Spawn` previously only supported a static `position`/`spawn_point`/`yaw_deg` — no way to
spawn "at wherever entity X currently is," needed because these monsters patrol, so a corpse's
spawn transform can't be hardcoded in RON. Mirrors `SpawnEffect.entity`'s existing
`SpawnRegistry → GlobalTransform` resolution, but also copies **rotation**, not just position
(`GlobalTransform::compute_transform()`), since it's meant to faithfully reproduce a whole live
transform:

```rust
Spawn {
    prefab: String,
    id: Option<String>,
    position: Option<(f32, f32, f32)>,
    spawn_point: Option<String>,
    yaw_deg: Option<f32>,
    at_entity: Option<String>,   // resolves position + yaw from this entity's GlobalTransform
}
```

`at_entity` takes precedence over `position`/`spawn_point`/`yaw_deg` entirely (warns if both
given). Supports `{self}`/`{target}` substitution at both `rewrite_self`/`rewrite_target`
(`message_interpreter.rs`) and `action_bar.rs`'s `action_needs_target`. **Skips the spawn with a
warning, never falls back to the origin**, when unresolvable and no `position`/`spawn_point`
fallback was also given (`system-architect` finding against the original draft, which would have
silently placed a lootable corpse at the world origin). Confirmed safe against a same-frame
`Despawn("{self}")`: `Action::Spawn` already resolves the full `Transform` at executor time into
`QueuedSpawn.transform`, which `drain_spawn_queue_system` only reads later.

### Corpse id collisions — handled structurally, not with a uniqueness token

The corpse's derived id (`"{self}_corpse"`) is safe to reuse across every future death of the same
monster slot, because the *live* monster always respawns under its own original, unchanging id
(the global respawn rule always calls `Spawn(..., id: "zombie_01", ...)`), so `{self}` at any
future death is always the same literal string. The one remaining risk — a second death reusing
`"{self}_corpse"` while an *earlier* corpse under that id is still mid-decay (up to 5 minutes if
unlooted) — is closed by an idempotent `Despawn("{self}_corpse")` immediately before every
`Spawn(id: "{self}_corpse", ...)`. Accepted tradeoff: a corpse's actual observed lifetime is
`min(natural decay, time until this slot's next death)`, not an unconditional 5 minutes — bounded
and honestly documented, not a silent bug.

### Monster death sequence (`enemy_zombie.behavior.ron` and siblings)

Single `dead` state now (no more `dead_full`/`dead_looted` on the monster itself — that machinery
moved entirely to the corpse). On death:
```ron
entry_actions: [
    EmitEvent("npc.dead:{self}"), PlayAnimationOn(clip: "death"), /* ...effects... */
    EmitEventAfterDelay(event: "zombie.swap_to_corpse:{self}", delay_secs: 3.0),  // death anim duration
    EmitEventAfterDelay(event: "monster.respawn:{self}", delay_secs: 60.0),      // fixed, unconditional
],
on: [(
    event: "zombie.swap_to_corpse:{self}",
    do_actions: [
        Despawn("{self}_corpse"),
        Spawn(prefab: "zombie_corpse", id: "{self}_corpse", at_entity: "{self}"),
        Despawn("{self}"),
    ],
)],
```
The respawn timer is armed *before* the entity despawns itself, using the global `DelayedEventQueue`
(a plain `(f32, String)` entry, independent of the entity that armed it) — but the entity that
armed it won't exist by the time it fires, so `entity_fsm_interpreter_system` has no live entity
left to match a per-entity `on:` handler against. This is why the respawn is caught globally.

### Corpse behavior — shared across all three monster types

`behaviors/lootable_corpse.behavior.ron` (one file, fully `{self}`-relative, reused by
`zombie_corpse`/`snake_corpse`/`spider_corpse` unchanged): `fresh` arms a 300s
`SetDespawnTimer(entity: "{self}", ...)`, and handles `entity.interacted:{self} →
OpenContainer("{self}")`; transitions to `looted` on `container.looted:{self}`, which does
`CloseContainer` then re-arms a 5s `SetDespawnTimer`. Originally built on `EmitEventAfterDelay` +
an `on:` handler reacting to a `corpse.decay:{self}` event — replaced with `SetDespawnTimer` during
the final review pass once debug-detective proved the event-based version was unsafe under id
reuse (see "Final review pass" below).

### Global respawn rules (`logic/state_machine.ron`'s top-level `global_on:`)

Resolves the authoring gap v1's plan had flagged and deferred: "the respawn timer must be armed
from `state_machine.ron`, not the per-entity behavior file... with no existing worked example."
One rule per scene-placed monster instance (6 total — `zombie_01`/`zombie_02`/`snake_01`/
`snake_02`/`spider_01`/`spider_02`), keyed by literal id, exactly like `chest_01`'s own
`entity.exited:chest_01 → CloseContainer` global rule:
```ron
( event: "monster.respawn:zombie_01", do_actions: [ Spawn(prefab: "enemy_zombie", id: "zombie_01", spawn_point: "zombie_01_spawn", yaw_deg: 200.0) ] ),
```
Originally placed inside `"playing"`'s own state-scoped `on:` list and named per-type
(`zombie.respawn`/`snake.respawn`/`spider.respawn`) — moved to top-level `global_on:` and unified
to one `monster.respawn:{id}` convention during the final review pass (see below).
`spawn_point` (not `at_entity`) is used here deliberately — the replacement should reappear at its
original patrol spot, not wherever the previous instance happened to die. Six matching
`spawn_points` were added to `main.scene.ron`, one per instance, copied from each instance's own
authored `transform.translation`; `yaw_deg` similarly copied from `rotation_euler_deg.y`.

### Corpse prefabs

`zombie_corpse`/`snake_corpse`/`spider_corpse` in `prefabs.ron`: `kind: Prop`, reuse the live
monster's own GLB model as a placeholder fallen pose (no dedicated corpse art exists — accepted,
see Open questions), `display_name`, `nameplate: false`, `click_selectable`, `indicator_category:
"neutral"`, `interactable: (radius: 2.0, hint_text: "Loot")`, `inventory:` (same loot amounts as
v1: zombie 15 gold + 1 potion, snake 5 gold, spider 8 gold + 1 potion). No `trigger_zone` (not
needed — no auto-close-on-exit; the panel's own close button covers it) and no colliders (a corpse
is purely visual + lootable, not a physical obstacle).

### Explicitly not attempted

Pose/ragdoll continuity — the engine has no mid-animation pose-baking, so the corpse renders in its
model's default pose, not frozen at the death clip's last frame. A dedicated fallen-pose art asset
per monster would fix this; out of scope here (art, not engine).

**v2 tasks** (all done):
- [x] `schema/actions.rs`: `at_entity: Option<String>` on `Action::Spawn`
- [x] `action_executor.rs`: resolve `at_entity` (position + yaw), warn-and-skip when unresolvable
      with no fallback
- [x] `message_interpreter.rs`: `{self}`/`{target}` substitution at both `rewrite_self` and
      `rewrite_target`
- [x] `capabilities/action_bar.rs`'s `action_needs_target` gained an `at_entity` arm
- [x] `zombie_corpse`/`snake_corpse`/`spider_corpse` prefabs + shared
      `behaviors/lootable_corpse.behavior.ron`
- [x] All three monsters' behavior files rewritten: single `dead` state, corpse-swap +
      independent fixed respawn timer, no more same-entity revival
- [x] Six `spawn_points` (one per scene-placed monster instance) + six global respawn rules in
      `logic/state_machine.ron`'s `"playing"` state — the respawner example v1 had deferred
- [x] `PreloadPrefab` for all three corpse prefabs at scene load (WASM pipeline-warmup)
- [x] Tests: `at_entity` position+facing resolution, precedence over `position`, warn-and-skip
      on unresolvable with no fallback, fallback-to-position when given (`spawn_tests.rs`);
      monster-death → corpse-swap-at-same-position, corpse-id-reuse-does-not-orphan, corpse
      interact/loot/decay (both paths), two-corpse soft-lock regression
      (`corpse_loot_interact_tests.rs`, rewritten for v2)
- [x] Docs: `docs/20_data_formats.md` (`Action::Spawn.at_entity`),
      `crates/ironhold_core/src/CLAUDE.md` (full v2 design + v1-superseded findings)
- [ ] `ironhold_cli validate` best-effort static check for `at_entity` referencing an unresolvable
      literal id — deferred, non-blocking (nice-to-have, not required for correctness)
- [x] Final playtest of the full v2 flow in-browser (1-minute respawn, 5-minute corpse decay, all
      three monster types) — confirmed working; one real bug found and fixed, one accepted
      tradeoff confirmed and logged (both below)
- [x] Final mandatory 4-agent review pass on the completed diff — one critical bug (global_on
      placement) and one severe bug (corpse-id-reuse compounding via `EmitEventAfterDelay`) fixed
      before merge, plus 4 minor fixes and a docs rewrite — see "Final mandatory review pass" below
- [x] `capabilities/despawn_timer.rs`: new `DespawnTimer` component + `despawn_timer_system`,
      `Action::SetDespawnTimer` wired through all four substitution sites
      (`rewrite_self`/`rewrite_target`/`action_needs_target`/`substitute_self_in_action`)
- [x] `capabilities/targeting.rs`: `target_auto_clear_system` now clears on despawn, not just hide
- [x] `action_executor.rs`: `Action::Despawn` closes the container panel if it was the open one
- [x] 3 new regression tests covering the three fixes above, all passing
      (`corpse_loot_interact_tests.rs`)

**First real playtest bug (2026-08-26), fixed**: killing a monster spammed
`WorldPixelBar: stat_key "zombie_02.health" not found — bar renders empty` every single frame,
severely enough to crash the browser console. Root cause: a genuine, general engine gap, not
specific to this feature — `Action::Despawn` only removes the one entity it's given; it has no
idea a monster's `stat_label`/`world_stat_bar` widgets are separate entities that merely reference
it via `WorldLabel.tracked_entity`. `world_label_screen_pos_system` already degrades gracefully
when a tracked entity disappears (hides the widget), but each bar style's own per-frame
fill-update system does an independent `SpawnId`-string stat lookup regardless of `Visibility`, so
an orphaned fill kept failing and warning forever. This was never reachable before v2's
death→corpse-swap, since no prior feature ever `Despawn`ed a *live* entity carrying these widgets
— previously they were only ever cleared in bulk via a full `LoadScene`, alongside their owner.
Fixed with a new `stat_widget_cleanup_system` (`capabilities/stat_display.rs`), mirroring the
already-existing `nameplate::nameplate_cleanup_system`'s exact `RemovedComponents` pattern
(generalized via `SpawnId` instead of a nameplate-specific marker). Regression test in
`nameplate_tests.rs` (co-located with its nameplate sibling test).

**Playtest re-verification (2026-08-26), confirmed working as designed**: after the fix above,
Frank confirmed an unlooted corpse now correctly persists past the 1-minute respawn and coexists
with the freshly-respawned live monster (the console-crash was indeed masking this — not a
separate bug). One real, but *already-documented*, tradeoff was also confirmed in the same
session: killing the respawned monster again despawns its *previous* still-unlooted corpse the
moment the new one spawns. This is the "Corpse id collisions" guard (see above) doing exactly what
it was designed to do — `{self}_corpse` is always the same literal id across every generation of a
given slot, so a `Despawn("{self}_corpse")` must run before every new corpse spawn to prevent a
silent id collision, at the cost of cutting short whichever older corpse was still there. **Decision:
accept as-is** — a real fix would need a genuinely unique id per death (a monotonic counter), which
the RON action system has no mechanism for today (only `{self}`/`{target}` substitution exists) —
logged as its own `planning/backlog.md` Icebox item rather than blocking this feature on a new
substitution primitive for a narrow edge case (only reachable if the *same* slot is killed twice
within the older corpse's remaining lifetime).

**Correction (final review pass, below): the "decays as designed" half of this was actually
wrong.** debug-detective proved the id-reuse guard interacts badly with `EmitEventAfterDelay`
specifically (not with id reuse itself) — fixed by switching to `SetDespawnTimer`. The narrow,
accepted tradeoff above (an older still-live corpse's loot is lost, not a stale-timer despawn of a
newer one) is what's actually still true post-fix; see "Final mandatory review pass" below for the
full distinction.

## Final mandatory review pass (2026-08-26), fixed before merge

Per this project's own `CLAUDE.md` workflow, the completed v2 diff went through the full parallel
review (`alignment-reviewer`, `system-architect`, `debug-detective`, `ux-gamedesigner-reviewer`)
after the playtest above. Every finding was evaluated individually; the ones below were fixed
rather than logged, since each either broke a real acceptance criterion or was cheap and clearly
correct:

- **CRITICAL — respawn rules were state-scoped, not global (alignment-reviewer + system-architect,
  independently).** The six respawn rules lived in `"playing"`'s own `on:` list.
  `tick_delayed_events_system` ticks on raw `Time` with no pause-gate, so a monster's 60s respawn
  timer could fire while the game was in a `"paused"` state — a state-scoped `on:` handler simply
  never matches in that case, silently and permanently losing that monster's respawn for the rest
  of the session. **Fixed**: moved all six rules to `state_machine.ron`'s top-level `global_on:`
  block, and unified the per-type event names into one `monster.respawn:{self}` convention in the
  same pass (ux-gamedesigner-reviewer: the old per-type names were "a copy-paste trap for a 4th
  monster type").
- **SEVERE — corpse-id reuse compounds over extended play (debug-detective).** The
  "accept as-is" tradeoff logged after the playtest above (a corpse despawning early when the same
  slot dies again) was believed narrow and already accepted. debug-detective proved it is actually
  worse: `EmitEventAfterDelay`'s global, string-matched event has no owner, so a decay timer armed
  by an *older* corpse generation can outlive that corpse's own despawn and later despawn a
  completely different, *newer* corpse sharing the same reused id — and this compounds every kill
  cycle, eventually making a slot's loot permanently unobtainable for the rest of the session, not
  just "cut short if killed twice quickly." This breaks the feature's own core promise, not a
  narrow edge case, so it was fixed before merge rather than logged. **Fixed**: new
  `SetDespawnTimer(entity, delay_secs)` action + `DespawnTimer` component
  (`capabilities/despawn_timer.rs`, modeled on the existing `DamagePopup`/`damage_popup_system`
  self-despawn pattern) replaces `EmitEventAfterDelay` + `corpse.decay` entirely in
  `lootable_corpse.behavior.ron`. The timer lives directly on the corpse entity, so despawning that
  entity (by decay, or by the id-reuse guard) removes the timer with it — a later corpse under the
  same id starts clean, with nothing left over to compete with its own timer. Regression test:
  `a_despawned_corpses_decay_timer_cannot_later_despawn_a_new_corpse_reusing_its_id`
  (`corpse_loot_interact_tests.rs`).
- **Real regression — stale target selection never clears on despawn (debug-detective).**
  `target_auto_clear_system` only checked `Visibility::Hidden` on an entity still present in
  `SpawnRegistry` — correct for the old hide-in-place revival this system predates, but v2's real
  `Despawn` removes the entity from the registry outright, so the check never ran and a targeted,
  killed monster's selection silently survived until the same id was reused by that slot's next
  respawn (up to 60s later), silently re-targeting the player onto an unrelated entity. **Fixed**:
  "not found in `SpawnRegistry`" is now treated the same as "hidden." Regression test:
  `a_despawned_monsters_target_selection_clears_instead_of_surviving_to_the_next_respawn`.
- **Real regression — despawning an open container left ghost UI (debug-detective).**
  `Action::Despawn` didn't check whether the despawned entity was the currently-open container —
  a corpse decaying (or the id-reuse guard) while its own panel was open left `active_container`
  pointing at a gone entity and `panels_open` stuck above 0, permanently blocking interact/pickup/
  tab-targeting. **Fixed**: `Action::Despawn` now runs the same teardown `CloseContainer` does when
  the despawned entity matches `active_container`. Regression test:
  `despawning_the_currently_open_corpse_closes_its_container_panel`.
- **Minor, fixed alongside the above**: `dialogue.rs`'s `substitute_self_in_action` was missing an
  `Action::Spawn` arm entirely (pre-existing gap, widened by `at_entity` — a dialogue choice's
  `do_actions` is a 4th substitution site alongside `rewrite_self`/`rewrite_target`/
  `action_needs_target`); `action_needs_target`'s `at_entity` check used exact-match `==` instead of
  the `.contains()` convention every sibling arm uses; the "both `at_entity` and
  `position`/`spawn_point` set" log was demoted from `warn!` to `debug!` (it's the documented-safe
  fallback pattern, not misconfiguration); the `at_entity` docstring was corrected to say it copies
  the full transform (position, rotation, *and scale*), not just "position + facing."
- **Docs, fixed**: `docs/30_runtime_events_and_logic.md`'s "Lootable corpse" section still described
  the superseded v1 same-entity approach in full (ux-gamedesigner-reviewer: a blocker, since a
  designer following it would author RON that no longer matches any real prefab) — rewritten for
  v2. `docs/20_data_formats.md`'s `Action::Spawn` row corrected to say "full transform" instead of
  "position and facing," and a new row added for `SetDespawnTimer`.

## Post-fix playtest regression (2026-08-26), fixed

Frank playtested the final-review-pass fixes above and confirmed the intended behaviors working
(old corpse/loot/target correctly clear when a new corpse takes over the same reused id; a live
monster's target correctly clears when it dies and swaps to a corpse) — but found a real,
un-caught bug in the newly-added `SetDespawnTimer`: **a corpse's own natural ambient decay left its
loot panel and target UI stuck**, even though the dedicated regression tests for the container-close
and target-clear fixes both passed. Root cause: `despawn_timer_system` despawned its entity
directly via `Commands`, never going through `Action::Despawn`'s own handler — so the registry
removal and container-close teardown those fixes depend on never ran for a *timer-driven* despawn,
only for a manually-pushed `Action::Despawn` (which is exactly what both dedicated tests used,
missing this gap entirely — they proved the teardown logic works when reached, not that every real
despawn path reaches it).

**Fixed**: `despawn_timer_system` (`capabilities/despawn_timer.rs`) now removes its own
`DespawnTimer` component and pushes a real `Action::Despawn(id)` onto the `ActionQueue` instead of
despawning directly, so a timer-driven despawn goes through the identical teardown as any other.
New regression test `a_corpses_own_decay_timer_closes_its_open_container_panel_and_clears_its_target`
reproduces the exact reported scenario (panel open, target set, natural decay fires) rather than a
manually-pushed `Despawn` — the gap the first round of tests missed.

## Open questions
- Corpse visual: accept the monster's own idle-pose model as a placeholder (current state), or
  hold for a dedicated fallen-pose art asset before shipping this to a real project?
- Loot contents: fixed `initial_items` per corpse prefab (current state), or randomized/chance-based
  loot (a separate follow-on feature, new `loot_table` schema, not this feature's scope)?
- `ironhold_cli validate` check for `at_entity` — worth adding before this pattern spreads to more
  projects, or fine to defer indefinitely as a nice-to-have?

## Acceptance criteria (v2) — verify during final playtest
- Given a monster's health is depleted, when its death-animation duration has elapsed, then the
  monster entity is despawned and a `"<Monster> Corpse"` entity is spawned at the same position and
  facing.
- Given `at_entity` can't resolve the source entity and no `position`/`spawn_point` fallback was
  given, then the spawn is skipped with a warning — never placed at the origin. (Automated: covered
  in `spawn_tests.rs`.)
- Given a corpse has loot, when the player interacts and takes all items, then it transitions to
  `looted` and despawns within its short (5s) decay window.
- Given a corpse is never looted, or has no loot at all, then it despawns after 5 minutes
  regardless.
- Given a monster dies again before its previous corpse (same derived id) has despawned, then the
  old corpse is despawned first — no orphaned registry entry.
- Given a monster died at any point, then a fresh instance respawns exactly 1 minute later at its
  original patrol spot, **independent of whether its corpse was looted, is still present, or has
  already decayed**.
