# Feature: Room 3 click targets with a health bar, death and respawn

_Status: Done (2026-10-10, play-tested by Frank)_
_Planned at: `ebc338c` (2026-10-10)_

## What
`local_coop_demo` room 3's two `click_target_test` spheres get a visible health bar, die at 0 health and respawn after a delay,
so the per-player targeting playtest shows a complete hit, kill, respawn loop. RON-only; no engine change.

## Why
Found in the October 2026 release play-test: pressing the action bar slot showed a floating damage number above the selected target, but
nothing else visibly happened, and the player's own mana bar drained at the same time. That reads as "the action hurt me, not the target".

**Premise check (2026-10-10, before planning):** the RON does what it says. Both slots in `room3.scene.ron` run
`ModifyStat(key: "{target}.health", delta: -10.0)` and `ShowDamagePopup(entity: "{target}", ...)`, and `click_target_test` already declares a
`health` stat (base 100). What the designer cannot see is that damage: the stat has no `stat_label`/`world_stat_bar` (the prefab comment says
"invisible on its own" on purpose), the only visible bar is the caster's own mana, and at 0 health nothing happens, so a target takes
hits forever. Frank confirmed (2026-10-10): the target's popup showed while his own mana dropped, which read as self damage. He asked for a health bar on every player and target, health red and mana blue.

## Approach
Reuse the documented hide + delay + restore pattern (`docs/30_runtime_events_and_logic.md`, "attack_dummy"), no new schema.
1. `prefabs.ron` `click_target_test`:
   - add `behavior: "behaviors/click_target.behavior.ron"`;
   - **add a threshold to the health template**: `thresholds: [( when: BelowOrEqual(0.0), emit: "stat.{self}.health.depleted" )]`. Per-entity stats emit only the thresholds
     written on the template (`capabilities/stats.rs:127-147`); without it `dead` can never be entered (architect review, verified). The docs' attack_dummy example omits it (logged in `claude_suggestions.md`);
   - health `base`/`max` **30** (was 100), so three 10-damage hits kill: P1's mana affords 5 casts and P2's only 3 (60 mana, 20 per cast, slow regen), and 100 HP made a kill take about 45 s for P2;
   - add `stat_label` and a red **Pixel** `world_stat_bar` (`fill_color: (0.85, 0.15, 0.15, 0.95)`, same size and style as the players' mana bars) on `{self}.health`, with
     `offset: (0.0, 2.4, 0.0)`, the scene-wide nameplate offset the forced "Click Target" nameplate uses, stacked with `screen_offset` (name, number, bar) per docs/20 "Label depth scaling".
     (The UX review preferred Ascii because it believed Pixel bars do not depth-scale; docs/20 lines 165 and 4534 say they now do and line 4582 still says they don't. The play-test decides; the stale line is logged.);
   - **players (changed from the Ready plan, Frank informed 2026-10-10):** health bars on players are NOT in this feature. A prefab has a single `world_stat_bar` slot (players use it for mana) and
     the nameplate's multi-bar widget shows in only one split viewport (docs/20 nameplate section). Queued instead as an engine feature (several bars per prefab). What this feature does for the colour request: every
     player mana bar in `local_coop_demo` is blue (`player_p2_split`, `player_p2_primitive`, `player_p2_split_ring`, `player_p2_primitive_split_ring` were pink or red), so red means health everywhere;
2. New `behaviors/click_target.behavior.ron`: `alive` (entry: `SetStat` health 30, then `SetEntityVisible` true, in that order) and `dead` (entry: hide, `EmitEventAfterDelay` `entity.respawned:{self}` 5 s).
   Transitions on `stat.{self}.health.depleted` (from `alive`) and the respawn event with `from: Some("dead")`, so a stale delayed event after a room change (delayed events are not cleared on LoadScene and rooms 9 and 10 reuse the ids) does nothing.
3. Update the stale prefab/scene comments ("invisible on its own", "no damage source in this demo").

**Shared prefab (accepted):** `click_target_test` is also placed in `room9.scene.ron` and `room10.scene.ron`, which have damaging action bars of their own, so they get the bar and the loop too.
That is the point (every damage demo shows the same loop), so no room 3-only variant; their baselines are checked and updated with room 3's.

## Risks to check in code before building
- Edge-triggered once per crossing (verified), further hits at 0 are clamped and emit nothing. Accepted: the threshold event reaches the entity FSM one frame after the hit, so one more press in that frame costs mana and makes a popup that is never seen.
- `behavior:` on a `kind: Primitive` prop is honoured and `{self}` is the scene-placed id (verified, `entity_spawner.rs:53-58`, `scene_loader.rs:477-482`). Hidden targets auto-clear per player (`targeting.rs:381-391`) and WorldLabels of a hidden entity hide, including the nameplate anchor and popup (`lib.rs:688-691`); confirm in the play-test.
- Labels and the bar must not collide with `stat_widget_test` (centre line, 4 m away), the nameplate, `target_hud` or the hints; check at max zoom-out (radius 9) before re-baselining.

## Tasks
- [x] Prefab: behavior, threshold, base 30, stat_label, red Pixel world_stat_bar at offset 2.4, comments
- [x] Every player mana bar blue (4 prefabs changed); player health bars deferred (backlog)
- [x] Behavior file
- [x] Tests: `ron_validation`/`ron_lint` pass; a `local_coop_tests` case if the harness can drive health to 0 (otherwise note why not)
- [x] Docs: none expected (no schema change); fix the stale comments
- [x] `local_coop_demo_room3`, `_room9` and `_room10` baselines, only the ones whose screenshot changes (`--real-gpu --update-baseline`)

## Open questions
- (answered) What Frank saw: the target's popup with the caster's mana dropping.
- Respawn delay: 5 s and 30 health unless Frank prefers otherwise (both reviewers think 5 s is right for a play-test).

## Acceptance criteria
- Given room 3 with a selected target, when a slot is fired, then the target's health bar drops 10 per hit and the popup shows above that target; the third hit kills it.
- Given a target at 0 health, then it disappears, its player's selection clears, and it returns at full health after the delay, selectable again.
- Given both players hitting different targets, then each viewport shows the right bars and popups.

## Result
Shipped at `d9f9ae0` (RON-only). Differences from the plan: no player health bars (a prefab has one `world_stat_bar` slot; multi-bar queued); the label stack runs *downward* from the name (negative `screen_offset`) because upward put the bar under the HUD text; every player mana bar is blue (4 prefabs), not just room 3's. Review findings applied: the `thresholds` entry, health 30, shared-prefab rooms, bare `from:` (`ron_lint`). Logged: delayed events survive `LoadScene`, docs/30 examples (`Some(` in `from:`, entry-actions scope, missing thresholds), docs/20 Pixel depth-scaling contradiction.
