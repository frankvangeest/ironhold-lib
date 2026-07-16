---
name: per-player-action-bar-pattern
description: Phase 2 per-player split-screen action bars — ActionBarDef.owner_player scopes a bar to a PlayerIndex; how per-player {target} resolution stays out of the interpreter pipeline; cross-bar duplicate-key validation
metadata:
  type: project
---

**Phase 2 of `per_player_split_screen_targeting.md` (reviewed 2026-07-16, ALIGNED).** Makes the
action bar per-player. Builds on Phase 1's `PlayerTarget` component (see
[[targeting_capability_pattern]]) and the intent-event layer (see [[intent_event_layer_pattern]]).

**Designer surface — fully RON-reachable, zero recompile:**
- `ActionBarDef.owner_player: Option<u32>` (`schema/scene_v2.rs:917`, `#[serde(default)]`,
  `deny_unknown_fields` on the struct so a typo is a clean parse error). Copied verbatim onto
  `ActionSlotUi.owner_player` at scene load (`scene_loader.rs:1932`).
- The player identity it matches against is `PlayerIndex(n)`, which comes from RON-authored
  `PrefabDef.player_index` → `PlayerConfig` → `spawn_player_entity_core` inserts `PlayerIndex`
  (GLB-only; primitive players never get one). So the whole owner_player→player chain is RON.
- `None`/`Some(0)` both mean "the primary player" via `owns_slot()` (action_bar.rs:219) delegating
  to `is_primary_player(idx)` ("PlayerIndex(0) OR no PlayerIndex at all") — same definition Phase 1
  uses. `Some(n)` matches the player carrying `PlayerIndex(n)`. Backward-compatible: an omitted
  field behaves exactly as pre-feature single-shared-bar.

**Key correctness win — this did NOT need player identity threaded through the pipeline.**
`action_bar_input_system` calls `rewrite_target` itself, LOCALLY, before anything reaches
`ActionQueue`, so `{target}` is already a concrete entity ID by the time the interpreter sees it.
The system still does NOT push to `ActionQueue` — it stores into `PendingIntentActions` and
`flush_pending_intent_system` (the 4th interpreter-tier system) pushes. Intent pattern preserved.
Each fired slot resolves its owning player via `players.iter().find(owns_slot)` and reads THAT
player's own `PlayerTarget` (not global `CurrentTarget`) for: `{target}` rewrite, the no-target
gate, and the `intent.slot.{key}:{player_spawn_id}` event's player id.

**Rewrite of the input loop (flag if regressed):** `action_bar_input_system` was changed from a
single `find`+`return` (fired at most ONE slot per frame) to a loop over EVERY slot whose
`resolved_key.just_pressed`. Required because 2+ independent bars can each have their player press
their own key in the same frame — the old structure silently dropped one. If you ever see this
revert to `find().return`, per-player bars break.

**Two documented scope boundaries that stay global (NOT blockers — both documented in docs/20 and
src/CLAUDE.md):**
1. `SlotCost`/`cost:` reads+deducts the single shared `LoadedStats` — no per-player economy. A P2
   bar's cost slot dims/blocks against the same pool as P1. Backlog item "Per-player stat/resource
   pools". docs/20 line 906-914.
2. A `rules.ron` rule that *intercepts* a non-primary player's slot intent resolves ITS OWN
   replacement `do_actions`' `{target}` via the interpreter against `CurrentTarget` (= primary
   player), NOT the firing player's `PlayerTarget`. Only the slot's OWN built-in do_actions (the
   suppressed-when-a-rule-takes-over path) get per-owning-player resolution. docs/20 line 946-955.

**Cross-bar duplicate slot-key detection (new, additive to the pre-existing per-bar check):**
`CooldownMap`/`PendingIntentActions`/`HandledIntentSlots` are all keyed by the literal slot-key
string alone, SCENE-WIDE — so two bars sharing a resolved key means a rule handling one bar's
intent silently suppresses the other's pending slot, and cooldowns collide. Dual runtime-warn /
CLI-error pattern (see [[keybinding_parse_key_vocabulary]]):
- Runtime: `scene_loader.rs::warn_cross_bar_duplicate_keys` (scene-wide `warn!`).
- CLI: `validate.rs` pushes `error_type: "cross_bar_duplicate_key"` (exit 1), fixture
  `crates/ironhold_cli/tests/fixtures/cross_bar_duplicate_action_bar_key`.
Designers avoid this by using disjoint keys across bars (demo room3 uses KeyG for P1, KeyL for P2).

**Minor gap noted (non-blocking):** the `action_bar.*` notification events (`pressed`, `no_target`,
`on_cooldown`, `insufficient_resource`, `activated`) carry only `{key}`, NOT the player id — only
`intent.slot.{key}:{entity}` carries it. A designer reacting per-player to those relies on
disjoint keys implying the player (which the cross-bar check enforces). Acceptable, but it's the
one place player identity isn't surfaced.

**Coverage:** entity_logic_tests.rs `test_owner_player_slot_resolves_against_its_own_players_target`,
`test_slot_with_unmatched_owner_player_never_fires`, single-player-regression test. Demo:
local_coop_demo room3.scene.ron (2 ActionBar blocks) + prefabs.ron click_target_test.stat_templates
health entry (so ModifyStat/ShowDamagePopup have a real StatMap to act on). NOTE: room3 gained two
visible ActionBar HUD blocks — its baseline screenshot likely needs regeneration (workflow step,
not an alignment issue).
