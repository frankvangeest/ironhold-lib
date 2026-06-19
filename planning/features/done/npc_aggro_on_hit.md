# Feature: NPC aggro-on-hit — Investigating state + kiting

_Status: Active_
_Planned at: `6bb3f2f` (2026-06-19)_
_Architect-reviewed at: `22260da` (2026-06-19)_
_Phase 2 planned at: see `git rev-parse --short HEAD` at commit time (2026-06-19)_

## What

When the player damages an enemy from outside its `detection_radius`, the enemy should
respond intelligently:

1. Record the attacker's last-known world position.
2. Enter a new **`Investigating`** state — walk toward that position to try to get the
   attacker into visual/detection range.
3. If the attacker enters detection range during the walk, transition to normal **Chase**.
4. If the abandon timeout elapses without a new hit *and* without visual contact, give up
   and return to spawn (**Return**).
5. Each subsequent hit refreshes the last-known position and resets the abandon timer —
   enabling **kiting**: player retreats while attacking, each hit buys a little more time
   before the NPC gives up.

## Why

The previous approach (aggro → Alerted → Chase immediately) was too simple. "Chase" means
"I can see you and am following you." Investigating means "I know you were over there — let
me check." The distinction matters because:

- Chase with `dist_opt.or(target_dist_opt)` (the workaround we added) stretched Chase's
  meaning to cover visibility-blind pursuit. The architect flagged this as a semantic hack.
- Kiting is a genuine RPG gameplay tactic. The new state makes it possible and rewards
  player skill (timing retreats to stay just ahead of the NPC's investigation radius).
- `npc.investigating:{id}` / `npc.investigation_failed:{id}` events let RON rules react
  (e.g. play a confused/alert sound) without any Rust changes.

## Approach (architect-reviewed, phase 2)

### Data changes

**`NpcHitQueue`** — change from `HashSet<String>` to `HashMap<String, Vec3>`:
- Key: NPC id (the entity that was attacked).
- Value: attacker's world position at the moment the relay system runs.

**`npc_hit_relay_system`** — add `Query<&GlobalTransform, With<CharacterController>>` to
snapshot the player position alongside the event. Stays in Update, after
`action_executor_system`. No schedule change.

**`NpcDef`** — add one optional field:
```ron
investigate_timeout_secs: 5.0,  // how long to pursue before giving up (default 5 s)
```
Use the `default_npc_investigate_timeout` named-default-fn pattern (`catalog.rs:1065`)
so existing RON files keep parsing without change.

**`NpcAgent`** — two new runtime fields (not serialised):
```rust
pub last_known_attacker_pos: Option<Vec3>,
pub investigate_timer: f32,
```

**`NpcState`** — new variant:
```rust
Investigating,
```

### State machine

**Pre-match hit block** — lift hit-handling out of the per-state match arms into a single
block before the match. Currently `was_hit` / `can_aggro` logic is duplicated in
`Idle | Patrol` and `Return`. One block is cleaner and prevents future drift:

```rust
// Hit from range: record attacker position regardless of current state,
// except when already Chasing (player is visible — no need to investigate).
let hit_pos: Option<Vec3> = hit_map.get(npc.npc_id.as_str()).copied();
let can_aggro = matches!(npc.on_player_near, NpcOnPlayerNear::Chase | NpcOnPlayerNear::Interact);
```

**`Idle | Patrol` and `Return`** — on hit:
```rust
} else if hit_pos.is_some() && can_aggro {
    npc.last_known_attacker_pos = hit_pos;
    npc.investigate_timer = 0.0;
    next_state = Some(NpcState::Investigating);
}
```
On detect (player visible): Alerted as before (unchanged).

**`Investigating`** arm:
- Increment `investigate_timer` by `dt`.
- If player enters detection range (`in_detect`): transition to `Alerted` (normal chase path).
- If another hit arrives: update `last_known_attacker_pos`, reset `investigate_timer`.
- If `investigate_timer >= npc.investigate_timeout_secs` AND no new hit: emit
  `npc.investigation_failed:{id}`, transition to `Return`.
- If NPC reaches `last_known_attacker_pos` (within `waypoint_reach_radius`) without
  spotting player: emit `npc.investigation_failed:{id}`, transition to `Return`.
- Movement: walk toward `last_known_attacker_pos` at `patrol_speed` (not chase speed —
  investigating, not sprinting).
- On entry: emit `npc.investigating:{id}`.

**`Chase`** — revert to visibility-only `in_chase` (remove `dist_opt.or(target_dist_opt)`
workaround). Chase = player is genuinely visible and within `chase_radius`. When player
drops out of visual range during Chase: transition to `Investigating` (treat it as "I know
they went that way") rather than snapping straight to Return.

**`Alerted`** — unchanged; brief pause before acting.

### New events emitted

| Event | When |
|---|---|
| `npc.investigating:{id}` | NPC enters Investigating state |
| `npc.investigation_failed:{id}` | Timeout or arrival without visual contact |

Both are emitted via `game_events.write(GameEvent::Trigger(...))` — fully RON-reactable.

### No new actions or schema version bumps

`NpcDef.investigate_timeout_secs` is an optional field with a named default; existing
RON files parse without change. `NpcState` is never serialised.

### Event latency

`npc_hit_relay_system` runs in Update (after `action_executor_system`). `npc_behavior_system`
runs in FixedUpdate. Hits populate `NpcHitQueue` in Update frame N; `npc_behavior_system`
drains it in FixedUpdate frame N+1. ≤1-tick latency is intentional and imperceptible.

## Tasks

**Phase 1 (done)**
- [x] RON (signal): `EmitEvent("entity.attacked:{target}")` in attack slots 1–4 in `main.scene.ron`.
- [x] `npc.rs`: `NpcHitQueue` resource + `npc_hit_relay_system` (Update, after executor).
- [x] `npc.rs`: aggro-on-hit → Alerted → Chase in Idle/Patrol and Return arms.
- [x] Integration tests: aggro, Flee-NPC no-op, unknown-id no-op.
- [x] Docs: `entity.attacked:{id}` in `30_runtime_events_and_logic.md` and `assets/projects/CLAUDE.md`.
- [x] `in_chase` fallback using `target_dist_opt` (intermediate fix, to be replaced by phase 2).

**Phase 2 (current)**
- [ ] `NpcHitQueue`: change `HashSet<String>` → `HashMap<String, Vec3>`; update relay to
  snapshot player `GlobalTransform`.
- [ ] `NpcDef`: add `investigate_timeout_secs: f32` with named default `5.0`.
- [ ] `NpcAgent`: add `last_known_attacker_pos: Option<Vec3>` and `investigate_timer: f32`.
- [ ] `NpcState`: add `Investigating` variant.
- [ ] `npc_behavior_system`: lift hit-handling to a pre-match block; implement `Investigating`
  arm; revert Chase to visibility-only; Chase→Investigating when LOS lost.
- [ ] Events: emit `npc.investigating:{id}` and `npc.investigation_failed:{id}`.
- [ ] Integration tests: investigating entry, timeout → Return, hit-refresh resets timer,
  visual-contact → Alerted during investigating.
- [ ] `NpcDef` docs: `docs/20_data_formats.md` `investigate_timeout_secs` field.

## Acceptance criteria

- Attacked from far range → NPC enters Investigating, walks toward last-known attacker position.
- Player steps into detection radius during Investigating → NPC transitions to Alerted → Chase.
- No further hits + timeout (5 s default) → NPC returns to spawn; `npc.investigation_failed` fires.
- Repeated hits reset the timer — player can kite indefinitely by attacking before timeout.
- Chase state requires visual contact — NPC losing LOS pivots to Investigating, not Return.
- `on_player_near: Flee` NPCs do not investigate on hit.
- Normal proximity detection and existing patrol/return loop unaffected.
