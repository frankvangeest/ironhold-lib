# Feature: Monster corpse loot

_Status: Draft_
_Planned at: `452e2e2` (2026-08-24)_

## Phases

| Phase | Backlog item | Status | Completed |
|---|---|---|---|
| v1 | Same-entity loot-on-death (RON-only, no engine change) | Queued | — |
| v2 | Separate corpse entity via `Action::Spawn.at_entity` (real respawn decoupling) | Queued | — |

**v1 does not fully achieve the "decouple loot from respawn" goal** — see v1's own Why note below.
It ships real loot-on-death now, at zero engine risk, while v2 (which does achieve full decoupling)
gets its own design review. Do not move this file to `features/done/` until v2 also ships.

## What
When a monster's death animation finishes, the player can loot it via the existing container
system (`Inventory` + `Interactable` + `OpenContainer`/`TakeAllFromContainer`). A looted-empty
corpse decays quickly; an unlooted one decays after a longer ambient window.

- **v1**: the monster's own entity becomes the lootable corpse in place (no new entity spawned).
- **v2**: death instead despawns the monster and spawns a new, separate `"<Monster> Corpse"`
  entity at the same position/facing, so a fresh live monster can respawn independently while the
  old corpse still lingers.

## Why
Today (`enemy_zombie.behavior.ron` and siblings) a killed monster is hidden in place at a fixed
delay and revived (`ResetToSpawn`) at a second fixed delay — there is no loot at all for monsters,
only static chests/props authored with `inventory:` directly. Coupling "when this monster comes
back" to "how long the player has to loot it" is the wrong axis: a slow looter shouldn't be raced
by respawn, and a monster's respawn cadence shouldn't have to also double as a loot-availability
window.

**v1 only partially fixes this**: it's still one entity, so reviving it (`ResetToSpawn` back to
`alive`) is what ends the corpse/loot phase — the loot window still has to fit inside the respawn
timer, same structural coupling as today, just with actual loot in the window now instead of
nothing. **v2 is what removes the coupling for real**: a corpse and a freshly-respawned live
monster can coexist as two different entities, so respawn timing and loot timing stop needing to
agree with each other at all.

## Shared groundwork (needed by both v1 and v2)

Found by `ux-gamedesigner-reviewer` during plan review: `container.opened:{id}` /
`container.closed` / `container.looted:{id}` — the events both phases' decay logic depend on — are
not documented anywhere in `docs/`, and no shipped project wires `OpenContainer` by `"{self}"`
(only by a literal id, e.g. `chest_01`). Both are blocking for v1, not just v2:
- [ ] Add the three `container.*` events to `docs/30_runtime_events_and_logic.md`'s event table.
- [ ] `docs/30_runtime_events_and_logic.md`'s existing "hide+restore vs `Despawn`+`Spawn`" guidance
      table gets a "loot corpse" row.

**Two-state decay, not two racing timers** — both `system-architect` and `ux-gamedesigner-reviewer`
independently flagged the original "two `EmitEventAfterDelay` timers, whichever fires first wins"
design as the wrong shape: the loser's action either warns on a stale despawn id (v2) or is merely
harmless-but-sloppy (v1). Both proposed the same fix: give the corpse two states —
`fresh` (ambient decay timer armed) → `looted` (short decay timer armed, transition triggered by
`container.looted:{self}`) — so only the state-appropriate timer's action ever fires. This applies
to both phases below.

**Known gap this doesn't solve**: `system-architect` found `TakeAllFromContainer` early-returns
before emitting `container.looted` when the container has zero items (`action_executor.rs`,
`items_to_transfer.is_empty()` guard) — a corpse authored with no loot never reaches the `looted`
state via that path. Acceptance criteria below account for this: an empty corpse decays via the
ambient timer only, same as an unlooted one. Not a blocker, just means "empty" and "never looted"
are indistinguishable to this design, which is fine for this feature's purpose.

## v1 — same-entity loot-on-death (RON-only)

**Approach**: no schema or code changes. Add `inventory:` / `interactable:` / `trigger_zone:` to
the monster's own prefab from the start (present the whole time, including while alive) — the
`trigger_zone` matches the existing `chest_01` pattern for auto-`CloseContainer` on walk-away,
which the original draft omitted. Split the `dead` state into two states per the shared groundwork
above:
- `dead_full` (entry: existing death actions — `EmitEvent("npc.dead:{self}")`,
  `PlayAnimationOn(death)`, effects, hide-at-10s, arm the long ambient decay event) with
  `on: [(event: "entity.interacted:{self}", do_actions: [OpenContainer("{self}")])]` plus the
  transition to `dead_looted` on `container.looted:{self}`.
- `dead_looted` (entry: `CloseContainer` — closing before anything else changes state, per
  `system-architect`'s "no `CloseContainer` before despawn/reset" finding — then arm the short
  decay delay).

Entity-FSM `on:` handlers are already scoped per-state, so pressing interact on a live monster does
nothing (no handler in `alive`/`attacking`) — looting only becomes possible once the entity reaches
a `dead_*` state.

**Accepted v1 limitation**: the nameplate keeps reading the monster's own name (e.g. "Zombie")
while it's technically a corpse — renaming it live would need a new `SetDisplayName`-style action,
out of scope for a "zero engine change" v1. Revisit in v2, where a genuinely separate prefab gets
its own `display_name` for free.

**v1 tasks**:
- [ ] Add `inventory:` / `interactable:` / `trigger_zone:` to one monster prefab (zombie first) in
      `3rd_person_game_demo/prefabs/prefabs.ron`
- [ ] Split `enemy_zombie.behavior.ron`'s `dead` state into `dead_full` / `dead_looted` per the
      shared groundwork design above; ship this as the reference example for `OpenContainer("{self}")`
      (none exists in any shipped project today)
- [ ] Tune the short (`dead_looted`) and long (`dead_full`) decay delays so both fit comfortably
      inside the existing 20 s respawn delay
- [ ] Playtest: confirm loot is takeable before hide (10 s) and respawn (20 s); confirm walking away
      auto-closes the panel via `trigger_zone`; confirm an empty-loot corpse still decays correctly
      via the ambient timer alone
- [ ] Docs: note the `OpenContainer("{self}")` + two-state decay pattern in
      `crates/ironhold_core/src/CLAUDE.md`'s interactable/dialogue section (no schema change, but a
      new designer-facing pattern worth documenting)

**v1 acceptance criteria**:
- Given a monster's health is depleted, when the player presses interact on the corpse before it's
  hidden, then the container panel opens showing its loot.
- Given the player takes all items, then the corpse transitions to `dead_looted` and
  despawns/hides within its short decay window, well before the existing respawn timer fires.
- Given the player never loots it, or the corpse has no loot at all, then it stays in `dead_full`
  and the existing hide/respawn timers behave exactly as they do today (no regression).
- Given the player walks away with the panel open, then it auto-closes via `trigger_zone` exit,
  matching `chest_01`'s existing behavior.

## v2 — separate corpse entity (needs an engine change)

**Approach**

**Engine change (the only one needed):** `Action::Spawn` currently only supports a static
`position: (f32,f32,f32)` / `spawn_point: String`, plus a static `yaw_deg`. There is no way to
spawn "at wherever entity X currently is" — needed here because monsters wander/patrol, so the
corpse's spawn transform can't be hardcoded in RON. `SpawnEffect` already solves exactly this for
particle bursts via `entity: "{self}"`, resolved through `SpawnRegistry → GlobalTransform`
(`action_executor.rs`). Add the same capability to `Action::Spawn`:

```rust
Spawn {
    prefab: String,
    id: Option<String>,
    position: Option<(f32, f32, f32)>,
    spawn_point: Option<String>,
    yaw_deg: Option<f32>,
    at_entity: Option<String>,   // NEW — resolves position + yaw from this entity's GlobalTransform
}
```

`at_entity` (when set) takes precedence over `position`/`spawn_point` for position, and over
`yaw_deg` for rotation — one field captures both, since the caller doesn't know its own current
yaw to also write a matching `yaw_deg`. Warn (both given, `at_entity` wins) exactly like
`SpawnEffect` warns on `entity` + `position` both set. Supports `{self}`/`{target}` substitution
identically to `SpawnEffect.entity`.

**Confirmed safe by `system-architect`**: `Action::Spawn` already resolves the full `Transform` at
executor time into `QueuedSpawn.transform`, which `drain_spawn_queue_system` only reads later — so
resolving `at_entity` at executor time (same as today's `position`/`spawn_point` resolution) means
a same-frame `Despawn("{self}")` afterward cannot invalidate it. `GlobalTransform` is only
`PostUpdate`-fresh as of last frame, which is irrelevant for an entity that has stopped moving to
die.

**Fixed from the original draft** (`system-architect` finding): do **not** fall back to world
origin when `at_entity` doesn't resolve. `SpawnEffect` skips the spawn entirely in that case
(`action_executor.rs`, "no entity or position resolved; skipping") — placing a lootable corpse at
the origin is worse than not spawning one. `at_entity` should warn-and-skip under the same
condition, falling back to `position`/`spawn_point` only if either was also explicitly given.

**Corpse id uniqueness is load-bearing, not optional** (`system-architect` finding):
`SpawnRegistry.entities` is a plain map — if a respawned monster dies again before its previous
corpse (same derived id, e.g. `"{self}_corpse"`) has despawned, the second `Spawn` silently orphans
the first corpse's registry entry. Rather than invent a new substitution token for uniqueness,
guard it structurally: prepend `Despawn("{self}_corpse")` immediately before the `Spawn(...)` call
in the death entry actions — idempotent no-op if none exists yet, guarantees no orphaning
regardless of timing.

**Everything else is RON-only**, using capabilities that already exist:

- A new `{monster}_corpse` prefab per monster (e.g. `zombie_corpse`), matching `chest_01`'s full
  field set rather than the minimal set in the original draft (`ux-gamedesigner-reviewer` finding):
  `display_name: "Zombie Corpse"`, `inventory: (slots: N, initial_items: [...])`,
  `interactable: (radius: ...)`, `trigger_zone: (radius: ...)` (auto-close on walk-away),
  `colliders`, `nameplate: false`, `click_selectable`, `indicator_category`.
- Each monster's death entry actions, after the existing death-animation-duration delay (same
  `EmitEventAfterDelay` pattern the zombie already uses for its "hide" step):
  ```ron
  Despawn("{self}_corpse"),                                          // guard against id reuse
  Spawn(prefab: "zombie_corpse", id: "{self}_corpse", at_entity: "{self}"),
  Despawn("{self}"),
  ```
- The corpse gets its own two-state behavior file (`fresh` → `looted`, per the shared groundwork
  section) instead of racing timers: `fresh`'s entry arms the long ambient `Despawn`; its `on:`
  list handles `entity.interacted:{self} → OpenContainer("{self}")` and transitions to `looted` on
  `container.looted:{self}`; `looted`'s entry does `CloseContainer` then arms the short decay
  `Despawn`.
- The monster's own respawn (a fresh *live* instance appearing later) moves to whatever already
  spawns that monster in the first place. **`ux-gamedesigner-reviewer` found this has a real
  authoring gap today**: once the monster entity is despawned its own behavior file's timers are
  gone too, so the respawn timer must be armed *before* despawn, from `state_machine.ron` (global),
  not the per-entity behavior file — splitting one monster's lifecycle across two files, with no
  existing worked example. A re-`Spawn`ed zombie also needs a scene `spawn_point` (`main.scene.ron`
  only defines `player_start`/`chest_spawn` today) since `ResetToSpawn`/`NpcAgent.origin` no longer
  apply to a brand-new entity. **Given this gap, v2's own scope is now split further**: ship the
  corpse-swap mechanic first (monster despawns into a corpse, no respawn at all yet) and treat
  "decoupled respawn via a worked spawner example" as its own follow-up task once the corpse half
  is playtested — see Open questions.

**Explicitly not attempted here:** pose/ragdoll continuity. The engine has no mid-animation
pose-baking, so a freshly spawned corpse entity starts in its own model's default pose, not
literally frozen at the death clip's last frame. Getting a visually seamless swap needs either a
dedicated fallen-pose art asset per monster, or accepting the monster's standing/idle-pose model as
a placeholder corpse for now. This is an art/content decision, not an engine one — see Open
questions.

**v2 tasks**:
- [ ] `schema/actions.rs`: add `at_entity: Option<String>` to `Action::Spawn`
- [ ] `action_executor.rs`: resolve `at_entity` via `SpawnRegistry` + `GlobalTransform` (position +
      yaw); warn-and-skip (not origin-fallback) when unresolvable and no other position given
- [ ] `message_interpreter.rs`: add `{self}`/`{target}` substitution for `Action::Spawn.at_entity`
      at **both** `rewrite_self` and `rewrite_target` (two call sites, confirmed by
      `system-architect` — `SpawnEffect.entity` already has both)
- [ ] `capabilities/action_bar.rs`'s `action_needs_target` needs an `at_entity` arm too, or a
      `{target}`-driven corpse spawn won't correctly gate on "no target selected"
      (`system-architect` finding)
- [ ] `ironhold_cli validate`: warn when `at_entity` is a literal id that doesn't resolve to any
      known spawn point/prefab reference at author time (best-effort static check, parity with
      other id-referencing action fields)
- [ ] Author `zombie_corpse` prefab (full `chest_01`-parity field set) + convert
      `enemy_zombie.behavior.ron`'s death handling to the despawn-into-corpse pattern above
- [ ] `Action::PreloadPrefab("zombie_corpse")` at scene load, to avoid a first-death WebGPU
      pipeline-compile stall (`system-architect` finding)
- [ ] Tests: `at_entity` resolves correct position/yaw at spawn time; warn-and-skip on unresolvable
      `at_entity` with no fallback; corpse loot → `looted` state → decay flow; ambient decay for a
      never-looted or empty corpse; `Despawn`-before-`Spawn` guard prevents orphaning on rapid
      re-death
- [ ] Docs: `docs/20_data_formats.md` (`Action::Spawn`), `crates/ironhold_core/src/CLAUDE.md`
- [ ] Follow-up (separate task, not blocking corpse-swap): worked example of a monster respawner
      armed from `state_machine.ron` with a real scene `spawn_point`, to actually deliver decoupled
      respawn (see Open questions)

## Open questions
- Which monster(s) get converted first — just the zombie for playtest, or all three (zombie/spider/
  snake) in one pass?
- Corpse visual for v2: accept the monster's own idle-pose model as a placeholder, or hold for a
  dedicated fallen-pose art asset before shipping this to a real project?
- Loot contents: fixed `initial_items` per corpse prefab (deterministic, matches how
  `InventoryContainerDef` already works), or is randomized/chance-based loot wanted? If so, that's
  a separate follow-on feature (new `loot_table` schema), not v1/v2 scope.
- Default decay windows — proposing 60s ambient (unlooted/empty) / 5s post-loot, tunable per corpse
  prefab; confirm during playtest.
- Does the monster despawn the instant the corpse spawns (same frame, should read as instant given
  matching transforms), or is a brief crossfade/delay wanted between the two?
- Is "ship the corpse-swap first, defer the actual respawner" (see v2 tasks) an acceptable v2 scope
  cut, or does decoupled respawn need to land in the same batch to be worth doing at all?

## Acceptance criteria (v2)
- Given a monster's health is depleted, when its death-animation duration has elapsed, then the
  monster entity is despawned and a `"<Monster> Corpse"` entity is spawned at the same position and
  facing.
- Given `at_entity` can't resolve the source entity and no `position`/`spawn_point` fallback was
  given, then the spawn is skipped with a warning — never placed at the origin.
- Given a corpse has loot, when the player interacts and takes all items, then it transitions to
  `looted` and despawns within its short decay window.
- Given a corpse is never looted, or has no loot at all, then it despawns after the longer ambient
  decay window regardless.
- Given a monster dies again before its previous corpse (same derived id) has despawned, then the
  old corpse is despawned first — no orphaned registry entry.
