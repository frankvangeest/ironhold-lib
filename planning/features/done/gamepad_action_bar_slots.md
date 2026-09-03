# Feature: Gamepad-Routed Action-Bar Slots

_Status: Done (shipped 2026-07-31)_
_Planned at: `e9e4c87` (2026-07-19)_

**Hard dependency: `gamepad_controller_input.md` must merge first.** This plan reuses that
feature's `InputMap::parse_gamepad_button` helper and the `resolve_gamepad(sorted_slice, index)`
shared resolver directly — do not begin implementation before that feature lands on `integration`.

**Plan-review note (2026-07-19):** **system-architect** returned Needs-more-design-work, resolved
as: (1) the originally-proposed `owns_slot` refactor (return an `Entity`, not a `bool`) was wrong —
`owns_slot` is also used by `action_bar_visual_system`, whose query has no `Entity`/`SpawnId` to
return, so that refactor would have forced unwanted churn into a system this plan explicitly said
it wouldn't touch. Corrected: `owns_slot` stays an unchanged `bool` predicate; `action_bar_input_
system`'s existing single `.find()` widens its tuple to include `&CharacterController` and is
reused as-is for the fire-check and the existing cost/target resolution — no second lookup, no
signature change, `action_bar_visual_system` genuinely untouched. (2) Added a fast-path ordering
(below) that resolves `keyboard_fired` before any player lookup, so a slot with no gamepad binding
skips player resolution entirely, exactly matching today's performance profile and today's
cooldown-event-on-unmatched-owner behavior byte-for-byte — no observable behavior change to
document, once ordered this way. (3) Added tasks: demo wiring, a per-slot warn on unparseable
`gamepad_key`, and a `crates/ironhold_core/src/CLAUDE.md` doc update (previously docs/20 only).
**ux-gamedesigner-reviewer** returned Needs-more-design-work, resolved as: (a) the "phantom
keyboard binding" concern (`key` stays required, so a gamepad-primary slot can still be pressed by
anyone's keyboard) is **pre-existing, universal behavior for every action-bar slot today** —
`owner_player` has always routed target/cost, never gated who's physically allowed to press a
shared key — so no schema fork is needed; clarified in docs instead (task added) so it doesn't
read as a new gap; (b) added a demo-wiring task targeting `local_coop_demo/scenes/room3.scene.ron`
(which already has two `owner_player`-scoped bars and commented `// gamepad_index` seams — a
ready-made host); (c) added a `key_hint` gamepad-glyph docs note (no auto-derivation exists, must
be authored manually); (d) folded in fixing an adjacent already-stale doc claim (the "not currently
cross-checked" cross-bar line, which is simply false today — the checks already exist); (e) added
a concrete before/after collision example to the docs task.

## What
Adds an optional per-slot `gamepad_key: Option<String>` field to `ActionSlotDef`, so a gamepad-
routed player's action bar can actually fire — today action-bar slots are 100% keyboard
(`ActionSlotUi.resolved_key: Option<KeyCode>`, checked against the single global
`Res<ButtonInput<KeyCode>>`), and `ActionSlotDef.key`'s own doc comment states gamepad buttons are
explicitly "not supported."

## Why
In a realistic local-coop pairing (one keyboard player, one gamepad player), the gamepad player's
action bar renders fully (icons, cooldown overlay, cost gating all already work per-player per
`per_player_split_screen_targeting.md` Phase 2 and `per_player_stat_pools.md`) but that player can
never press a slot — confirmed by this session's investigation: `action_bar_input_system` reads
only `Res<ButtonInput<KeyCode>>`, and `ActionSlotUi` has no gamepad-button field at all. Flagged
during Phase 2 plan review of per-player targeting (2026-07-15) as blocking the realistic mixed-
input local-coop configuration.

## Approach

**Schema**: `ActionSlotDef` (`schema/scene_v2.rs`) gains `#[serde(default)] pub gamepad_key:
Option<String>` — unbound by default, parsed at scene load via `InputMap::parse_gamepad_button`
into a new `ActionSlotUi.resolved_gamepad_button: Option<GamepadButton>` field, at the same call
site `resolved_key` is already resolved (`scene_loader.rs:1883`). An unparseable `gamepad_key`
gets its own per-slot `warn!` identifying bar + slot, mirroring the existing keyboard warn at
`:1885` (not just `parse_gamepad_button`'s own generic warn — the existing keyboard path names the
bar/slot for diagnosability and the gamepad path should match). `key` itself stays required and
unchanged in role (see the "phantom keyboard binding" note below); fix the stale "not supported: …
gamepad buttons" doc line — gamepad buttons move to their own field, not `key`.

**A slot fires on EITHER device, but the two devices resolve differently — this is the load-
bearing design point.** Keyboard is genuinely shared hardware: any player's keyboard-bound slot
fires from the one global `ButtonInput<KeyCode>` (this is pre-existing, unchanged by this feature —
`owner_player` has always routed *target/cost*, never gated *who may physically press the key*;
see the docs clarification task below so this doesn't read as a new gap). A gamepad is *not* shared
the same way — each player who has one gets their own, routed via their own `InputMap.
gamepad_index`. So a gamepad-bound slot must resolve specifically **against its owning player's own
gamepad**: `owner_player: Some(1)`'s slot with `gamepad_key: "South"` must only fire from player
1's own pad — player 0 pressing South on a *different* pad must never fire it.

**`action_bar_input_system` change** (`capabilities/action_bar.rs`) — ordered specifically to avoid
any observable behavior change to the existing keyboard/cooldown-event path:
```rust
for slot in slots.iter() {
    let keyboard_fired = slot.resolved_key.is_some_and(|kc| keys.just_pressed(kc));
    // Fast path: unchanged perf profile for the common case (no gamepad binding, not pressed).
    if !keyboard_fired && slot.resolved_gamepad_button.is_none() { continue; }

    // Existing single `.find()`, tuple widened to include `&CharacterController` — reused below
    // for the fire-check AND the existing cost/target resolution. `owns_slot` itself is untouched.
    let Some((spawn_id, target, idx, stat_map, controller)) =
        players.iter().find(|(_, _, idx, _, controller)| owns_slot(slot.owner_player, *idx))
    else {
        if keyboard_fired { /* existing on_cooldown/no-target event path, byte-for-byte unchanged */ }
        continue;
    };

    let gamepad_fired = slot.resolved_gamepad_button.is_some_and(|btn| {
        resolve_gamepad(&sorted_gamepads, controller.inputs.gamepad_index)
            .is_some_and(|gp| gp.just_pressed(btn))
    });
    if !keyboard_fired && !gamepad_fired { continue; }
    // ... existing cooldown/cost/target logic, unchanged, using the already-resolved tuple
}
```
This preserves today's behavior exactly for every keyboard-only slot (identical fast-path skip,
identical on-unmatched-owner event emission) — the only new branch is the gamepad check, which
only ever executes for slots that actually declare `gamepad_key`. `sorted_gamepads:
Vec<(Entity, &Gamepad)>` is built **once per system call** (mirroring the sibling plan's "sort
once, resolve many" contract), from a new `Query<(Entity, &Gamepad)>` param. This system already
runs in `Update` (confirmed) — it takes its own live gamepad query and resolves `just_pressed`
directly, mirroring `camera_orbit_system`'s established pattern rather than depending on
`input_translator_system`'s `FixedUpdate`-scheduled reads. No change to `action_bar_visual_system`
— purely cost/cooldown-driven visuals, no input reads, and its query shape is untouched since
`owns_slot`'s signature doesn't change.

**Cross-bar duplicate-binding detection needs a *different* scoping rule for gamepad than for
keyboard — and it's a different failure mode, not just a different scope.** The existing keyboard
check (`warn_cross_bar_duplicate_keys`, `scene_loader.rs:1335`; `ironhold_cli validate`,
`validate.rs:268`) exists because `CooldownMap`/`PendingIntentActions`/`HandledIntentSlots` are
keyed by the literal `slot.key` string — two bars claiming the same key can cross-suppress each
other's intent handling. The gamepad case has no such pipeline entanglement (the pipeline is never
keyed by `gamepad_key`) — its failure mode is a same-player **double-fire** (one physical button
press triggers two different abilities for the same player) if that player's own bar(s) bind the
same button to two slots. Both detectors get a **second, separate pass** keyed by
`(bar.owner_player.unwrap_or(0), GamepadButton)` — matching the existing `unwrap_or(0)`
normalization precedent already used elsewhere in the same file
(`warn_missing_player_stat_templates`, `scene_loader.rs:1422`) — flagging a collision only when the
*same normalized owner* binds the same button twice; two different players sharing a button name
is correctly not a collision (different physical pads).

## Tasks
- [x] `ActionSlotDef`: add `#[serde(default)] pub gamepad_key: Option<String>` (schema/scene_v2.rs);
      fix the stale "gamepad buttons not supported" doc comment on `key`
- [x] `ActionSlotUi`: add `resolved_gamepad_button: Option<GamepadButton>`; parse at scene load via
      `InputMap::parse_gamepad_button`, with its own per-slot `warn!` (bar + slot name) on an
      unparseable name, mirroring the existing keyboard warn's diagnostic shape
- [x] `action_bar_input_system`: widen the existing `players` query tuple to include
      `&CharacterController`; add `Query<(Entity, &Gamepad)>`; restructure per the ordered
      fast-path/fire-check shown in Approach — preserve the existing behavior for keyboard-only
      slots exactly
- [x] Extend both `warn_cross_bar_duplicate_keys` (`scene_loader.rs`) and `ironhold_cli validate`'s
      action-bar collision check (`validate.rs`) with a second pass keyed by
      `(bar.owner_player.unwrap_or(0), GamepadButton)` — a same-player double-fire check, distinct
      from (not replacing) the existing scene-wide keyboard pipeline-collision check
- [x] Demo wiring — `local_coop_demo/scenes/room3.scene.ron` already has two `owner_player`-scoped
      bars (`action_bar_p1`/`action_bar_p2`) and `prefabs.ron` already has commented-out
      `// gamepad_index: 0,`/`// gamepad_index: 1,` seams on the matching player prefabs. Add a
      commented-out `gamepad_key: "South",` (or similar) to one of `action_bar_p2`'s slots, with a
      one-line comment cross-referencing the sibling plan's button-names table and noting that
      uncommenting it also requires uncommenting `player_p2_split`'s `gamepad_index` — kept
      commented so `ron_lint`/`validate` pass without requiring a connected pad by default
- [x] Tests — new: a gamepad-routed `owner_player: Some(1)` slot fires from player 1's own gamepad
      button and does *not* fire from player 0's press of the same button name on a *different*
      pad (the core correctness case); new: a slot with both `key` and `gamepad_key` bound fires
      from either device; new: the per-player gamepad collision check fires only when the same
      player owns both colliding slots, and does *not* false-positive across different players
      sharing a button name; new: an unparseable `gamepad_key` warns with bar/slot context and the
      slot never fires from gamepad (keyboard binding, if any, still works); regression: existing
      keyboard-only action-bar tests (`entity_logic_tests.rs`'s Phase 2 section, `local_coop_tests.
      rs`'s cost-pool section) are unaffected. **Reuses whatever headless gamepad-test harness
      pattern the dependency (`gamepad_controller_input.md`) establishes** — no `Gamepad`-spawning
      test pattern exists in this repo yet; do not re-derive it here, inherit it
- [x] `cargo check -p ironhold_cli` — schema change gate
- [x] Docs — `docs/20_data_formats.md`: `ActionSlotDef` table gets `gamepad_key`; fix the stale
      "not supported: … gamepad buttons" line *and* the adjacent already-false "not currently
      cross-checked" cross-bar claim in the same sentence (both checks already exist today,
      independent of this feature); one worked RON example showing a slot with both `key` and
      `gamepad_key` bound, cross-referencing the sibling plan's gamepad button-names table; a
      concrete before/after collision example ("two players' bars both using `gamepad_key:
      \"South\"` → fine, different pads; one player's bar with two slots both `\"South\"` → error");
      a note that `key_hint` has no gamepad-glyph auto-derivation — for a gamepad-routed slot it
      must be manually authored (e.g. `"Ⓐ"`/`"South"`), a known limitation, not an oversight; a
      clarification that `owner_player` has always routed target/cost only, never gated *which
      physical device* may press a slot's keyboard `key` — pre-existing behavior, not new
- [x] Docs — `crates/ironhold_core/src/CLAUDE.md`'s Phase 2 action-bar / gamepad-routing sections:
      update to describe the widened query shape and the new per-player gamepad collision pass

## Open questions
None outstanding.

## Acceptance criteria
- Given a 2-player local-coop scene with player 0 on keyboard and player 1 on a gamepad, each with
  their own action bar (`owner_player: Some(0)`/`Some(1)`), when player 1 presses their bound
  gamepad button, then only player 1's slot fires — player 0's identically-keyed slot does not
  (**browser-observable, requires a physical controller** — the integration test is the primary
  correctness proof since headless browser tests can't synthesize a Gamepad).
- Given two different players' bars both defaulting `gamepad_key: "South"`, when this ships, then
  `ironhold_cli validate` does **not** report a false collision.
- Given two slots *for the same player* both bound to `gamepad_key: "South"`, then `validate`
  reports a collision, matching the existing keyboard duplicate-key error shape.
- Given any pre-existing keyboard-only action bar (no `gamepad_key` authored anywhere), when this
  ships, then behavior — including the on-unmatched-owner cooldown-event path — is unchanged.

## Amendment (2026-07-31, post-implementation review findings)

All 4 post-implementation reviews (alignment, system-architect, debug-detective, ux-gamedesigner)
found real, fixed issues:

- **Vacuous test (debug-detective, HIGH)**: `test_gamepad_action_bar_slot_with_both_key_and_
  gamepad_key_fires_from_either_device` released the keyboard key but never called `clear_just_
  pressed` — with no `InputPlugin` in this test harness (`MinimalPlugins` only), `just_pressed`
  latches forever, so the "gamepad alone fires it" assertion passed for the wrong reason (a stale
  keyboard bit, not the actual gamepad press). Fixed; also strengthened with a same-frame
  both-devices-pressed check (a stat tally proving exactly one fire, not two) to directly verify
  the "cannot double-fire" property debug-detective proved by code reading but that wasn't tested.
- **Untested branch (debug-detective, HIGH)**: the new gamepad-only-fire cooldown gate (the one
  piece of genuinely new control flow — a slot on cooldown pressed only via gamepad) had zero
  coverage; deleting that whole `if` block would have left every existing test passing while a
  gamepad press silently bypassed the cooldown gate. Added `test_gamepad_only_action_bar_slot_on_
  cooldown_emits_event_and_does_not_fire`.
- **Real button collision in the shipped demo/docs (alignment + ux-gamedesigner)**: the demo's and
  docs' worked examples all used `gamepad_key: "South"` — which collides with `InputMap.gamepad_
  jump`'s own default (`"South"`), so a designer following the instructions verbatim would jump
  *and* cast on one press, with nothing flagging it. Changed every example to `"RightTrigger"` and
  added a callout sentence warning against reusing the owning player's own `gamepad_jump`/
  `gamepad_run`/`gamepad_interact`/`gamepad_target_next` buttons (that overlap still isn't
  detected — logged as a documented limitation, not silently left unmentioned).
- **Silent no-op gap (alignment + ux-gamedesigner, both ranked this their #1/#2 fix)**: a
  `gamepad_key` on a player whose prefab sets no `gamepad_index` at all was completely silent —
  no crash, no warning, the binding just never fires. Added `warn_gamepad_key_without_gamepad_
  index` (scene_loader.rs, mirrors `warn_missing_player_stat_templates`'s exact shape) plus a
  matching `ironhold_cli validate` error (`gamepad_key_without_gamepad_index`), each with their own
  fixture/test.
- **Headline acceptance criterion strengthened (system-architect)**: the original test proved
  "unclaimed pad doesn't fire" but not the plan's actual headline claim (two *live* players, two
  pads, identical `gamepad_key`, only the pressing player's slot fires). Added `test_two_players_
  two_pads_same_gamepad_key_each_fires_only_their_own_slot` to prove this directly at runtime.
- **Minor cleanups**: hoisted `warn_same_player_gamepad_duplicate_slots` to a sibling call instead
  of a tail-call inside the keyboard check (was invisible to a future reader of the keyboard
  function); dropped a redundant `With<CharacterController>` query filter now that the tuple
  already fetches `&CharacterController`; softened a doc-comment perf claim that overstated the
  fast path's coverage (it only covers slots with no `gamepad_key` at all, not every frame for a
  gamepad-bound slot); added a doc note that `key` must stay scene-globally unique even for a
  gamepad-routed slot. Added a fixture/test closing the `None`/`Some(0)` owner_player-normalization
  gap for the new gamepad collision check (mirrors a pre-existing untested gap in the keyboard
  check, not introduced by this feature).

Two systemic, pre-existing gaps were surfaced but **not** fixed in this feature (all reviewers
agreed these are not merge blockers — logged to `planning/backlog.md` instead): (1) two players
sharing the same `gamepad_index` is undetected and would defeat the new same-player collision
check's premise; (2) a mid-session gamepad disconnect re-indexes the positional `resolve_gamepad`
slice, which the action bar's new consumption of it makes gameplay-visible (a wrong-player ability
activation) rather than just a camera/movement oddity. Both trace back to the same root cause
already tracked under "Positional `gamepad_index` → resolved-`Entity` binding."

Full test suite (129 `ironhold_core` + 24 `ironhold_cli` cross-file tests) green after all fixes;
`cargo check -p ironhold_cli` clean; `ironhold_cli validate` on `local_coop_demo` clean (no false
positives from either new check against the shipped, still-commented-out demo wiring).
