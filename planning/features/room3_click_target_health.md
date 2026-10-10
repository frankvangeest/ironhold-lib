# Feature: Room 3 click targets with a health bar, death and respawn

_Status: Draft_
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
hits forever. Frank's report needs a one-line confirmation of what he saw (see open questions); the fix below addresses both readings.

## Approach
Reuse the documented hide + delay + restore pattern (`docs/30_runtime_events_and_logic.md`, "attack_dummy"), no new schema.
1. `prefabs.ron` `click_target_test`: add `behavior: "behaviors/click_target.behavior.ron"`, a `stat_label` and a Pixel or Ascii `world_stat_bar`
   on `{self}.health` (offsets like `stat_widget_test`, same label-depth-scale rules).
2. New `behaviors/click_target.behavior.ron`: `alive` (entry: `SetStat` health 100, `SetEntityVisible` true) and `dead` (entry: hide, `EmitEventAfterDelay`
   respawn 5 s), transitions on `stat.{self}.health.depleted` and the respawn event.
3. Update the stale prefab/scene comments ("invisible on its own", "no damage source in this demo").

## Risks to check in code before building
- The depleted event for a per-entity `StatMap` is emitted once per crossing, and a hidden target is auto-cleared by `target_auto_clear_system` (it already skips hidden
  entities for Tab and click). Verify a hit on the last frame cannot re-trigger `dead`.
- `behavior:` on a `kind: Primitive` prop: confirm `entity_spawner` honours it (the docs example uses an interactable prop).
- Two instances share one behavior file via `{self}`; the popup/label duplication across the two split viewports (`WorldLabelRank`) must still work for the bar.
- `stat_widget_test` sits on the room's centerline and the new bars must not overlap its label or the target hud text.

## Tasks
- [ ] Prefab: behavior, stat_label, world_stat_bar, comments
- [ ] Behavior file
- [ ] Tests: `ron_validation`/`ron_lint` pass; a `local_coop_tests` case if the harness can drive health to 0 (otherwise note why not)
- [ ] Docs: none expected (no schema change); fix the stale comments
- [ ] `local_coop_demo_room3` baseline only if the screenshot changes (`--real-gpu --update-baseline`)

## Open questions
- What exactly did Frank see: the target's popup with the *caster's* mana dropping (reads as self damage), or did the caster's health change? Frank to confirm. If a stat on the caster really dropped, that is a separate bug.
- Respawn delay: 5 s unless Frank prefers otherwise.

## Acceptance criteria
- Given room 3 with a selected target, when a slot is fired five times, then the target's health bar drops 10 per hit and the popup shows above that target.
- Given a target at 0 health, then it disappears, its player's selection clears, and it returns at full health after the delay, selectable again.
- Given both players hitting different targets, then each viewport shows the right bars and popups.
