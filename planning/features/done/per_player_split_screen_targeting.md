# Feature: Per-Player Independent Targeting for Split-Screen

_Status: Done (Phase 1 + Phase 2 both shipped)_
_Planned at: `f275996` (2026-07-13)_
_Plan review (2026-07-13): system-architect + ux-gamedesigner-reviewer, verdict Needs-more-design-
work. Both reviewers independently flagged the same core contradiction (see Approach) and
converged on the same resolution; their findings are incorporated below. Frank resolved the
remaining 3 open decisions (HUD format block, ring color/dedup, legacy label suppression) the
same day — see Approach and Open questions._
_Phase 1 code review (2026-07-13): alignment-reviewer (ALIGNED), system-architect (found and fixed
a real bug — `Action::SetTarget`/`ClearTarget` weren't mirroring into the primary player's
`PlayerTarget`), debug-detective (found and fixed a duplicate-`player_index: 0` gap via a runtime
warning; 3 low-severity findings logged to `claude_suggestions.md`), ux-gamedesigner-reviewer
(found and fixed a real WASM playtest blocker — `"Tab"` is browser-intercepted, plus 3 doc/
playtest-aid gaps), wasm-perf-reviewer (OK, 1 negligible nit logged). Playtest confirmed by Frank,
no console errors._
_Phase 2 plan review (2026-07-15): system-architect + ux-gamedesigner-reviewer, verdict
Needs-more-design-work on both passes. Confirmed the core design (no interpreter-pipeline
threading needed; `owner_player` field; manual dual-`ActionBar`-block authoring) is sound, but both
independently caught that `action_bar_input_system`'s `find`+`return` structure would silently drop
one player's same-frame press (Major), that action bars are keyboard-only so a gamepad co-op player
would get a permanently non-functional bar (Major), and that the shared-`LoadedStats` cost pool and
cross-bar slot-key collisions need explicit runtime warnings/doc placement, not just a plan-file
footnote. All incorporated below — see Approach, Not in scope, Tasks, and Phase 2 acceptance
criteria. One open question remains for Frank (custom-hotkeys sequencing, cost-free demo choice)._
_Phase 2 code review (2026-07-17): alignment-reviewer (ALIGNED), system-architect (found and fixed
a real bug — the cross-bar duplicate-key check compared `ActionBar.id`, which nothing enforces as
unique, instead of positional bar index; fixed in both the runtime `warn!` and the CLI check),
debug-detective (no blocking bugs; flagged the damage-popup viewport gap that playtest then
actually reproduced), ux-gamedesigner-reviewer (found and fixed 3 stale "`player_index` not read
by any system yet" doc claims, now false since this phase reads it), wasm-perf-reviewer (OK, the
`find`→loop restructure is cost-neutral on idle frames). Playtest surfaced a real bug beyond the
reviews: `ShowDamagePopup`/`ShowFloatingText` are single-instance (no `WorldLabelRank`), so a
popup only ever rendered in the single highest-priority active split viewport regardless of which
player's action triggered it — fixed by extending the same `WorldLabelRank` duplication mechanism
`stat_label`/`world_stat_bar` already use to both actions, gated on split-screen exactly like
those. Playtest re-confirmed by Frank after the fix (popup now correctly duplicates per visible
viewport), no console errors — `928fc58`._

## Phases

| Phase | Backlog item | Status | Completed |
|---|---|---|---|
| 1 | Per-player target **selection & display** — `PlayerTarget` component, tab/click resolve to the acting player only, per-player target indicator/HUD display | Done | `04c1785` (2026-07-13) |
| 2 | Per-player action-bar ability execution against each player's own target | Done | `928fc58` (2026-07-17) |

Phase 2 is scoped separately (see "Not in scope") because the action bar has its own,
pre-existing single-player limitation unrelated to targeting — see Research findings.

**Phase 1 does not change what `{target}` resolves to for gameplay actions** (`ModifyStat`,
`Despawn`, etc. fired from `rules.ron`/`state_machine.ron`/behaviors) — see Approach's
"What Phase 1 does NOT do" note. It delivers independent target *selection and visual feedback*
per player (each can see and Tab/click their own focus — useful for co-op coordination even before
abilities diverge), not independent gameplay consequences. That gap is real and is Phase 2's job.

## What

Today, `CurrentTarget` (`capabilities/action_bar.rs`) is a single global resource. In a
split-screen local co-op scene, player 1 and player 2 fight over the same target: whichever
player last clicked or Tab-cycled overwrites the other's selection, and `tab_targeting_system`
hardcodes `controllers.iter().next()` — Tab always acts on whichever `CharacterController` the
query finds first, never player 2's, regardless of that player's own `target_next` keybinding
(each player already has an independent one via their own `InputMap`). This feature gives each
player their own target: their own Tab-cycle, their own target indicator ring, their own
target-name HUD readout — so two players in the same split-screen room can each be tracking a
different enemy at once.

## Why

Surfaced 2026-07-12 during `split_screen_camera_followups.md` Phase 2's playtest (click-to-select
became viewport-aware — clicking correctly resolves *which entity* a click in a given viewport
hits — but the result still overwrites the one shared `CurrentTarget`). Local co-op is a shipped,
playable mode (`local_coop_demo`, Stages 1-6) and any project with `targetable`/`click_selectable`
enemies plus 2+ players hits this today. Logged as its own backlog item rather than folded into
that feature, since fixing *which camera a click resolves against* (Phase 2) is a narrower,
already-shipped fix than *giving two players independent target state* (this feature).

## Research findings

- **`CurrentTarget` is read directly by 4 systems, not just the 2 targeting systems that write
  it.** `capabilities/targeting.rs` (`click_select_system`, `tab_targeting_system`,
  `target_auto_clear_system`) and `capabilities/action_bar.rs` (`action_bar_input_system`, for the
  `{target}` cost/rewrite check) all take `Res<CurrentTarget>`/`ResMut<CurrentTarget>` directly.
  Any per-player conversion touches all of these call sites, not just the two that assign it.
- **`{target}` substitution runs inside all three interpreter systems**
  (`message_interpreter_system`, `fsm_interpreter_system`, `entity_fsm_interpreter_system` — all in
  `runtime/scene_manager/message_interpreter.rs`), each independently reading the same
  `Res<CurrentTarget>` and calling the shared `rewrite_target(action, target_id)` helper. None of
  these systems have any notion of "which player" triggered the message being interpreted —
  `UiEvent::ButtonPressed`, `GameEvent::Trigger`, and `InputActionMessage` carry no player
  identity today. Making `{target}` itself resolve per-acting-player would require threading
  player identity through the entire Message → Interpreter → Action → Executor pipeline, not just
  the targeting capability — a materially larger change than giving each player their own
  selection *state*.
- **The action bar is already single-player-hardcoded, independent of this feature.**
  `action_bar_input_system` reads `Res<ButtonInput<KeyCode>>` directly (fixed digit keys 1-9, not
  routed through any player's `InputMap`) and `player_query: Query<&SpawnId, With<CharacterController>>`
  is only used for `{self}`-style substitution, not to disambiguate which player pressed a key —
  there is exactly one shared action bar today, matching the existing single shared `CurrentTarget`
  it reads. A genuinely per-player action bar (each player's own ability slots, each executing
  against their own target) is a separate, larger feature than "each player can select a different
  enemy" — this is why it's split into Phase 2 and explicitly out of scope for Phase 1's own
  acceptance criteria.
- **Each player already has independent input for Tab-cycling.** `CharacterController.inputs:
  InputMap` is per-entity (each GLB player prefab authors its own `inputs:` block —
  `target_next`/`target_range` included), so the missing piece for Phase 1 is purely
  `tab_targeting_system` iterating `controllers.iter().next()` instead of all players, plus
  `CurrentTarget` itself needing to be per-player storage.
- **`click_select_system` (Phase 2 of `split_screen_camera_followups.md`) already resolves which
  camera/viewport a click lands in** (`is_active` + `logical_viewport_rect().contains(cursor)` +
  `camera_priority_key` tiebreak). What it's missing is mapping that camera back to the player who
  owns it (`OrbitCamera.target` → the `CharacterController`/`PlayerIndex` entity for that camera's
  `SplitViewportSlot`) so the resolved entity is written to *that player's* target, not the shared
  resource. A real, unavoidable limitation stays regardless: one physical mouse means only one
  player can click-target in any given frame — this feature does not and cannot change that
  (surfaced and accepted during Phase 2's playtest already).
- **A precedent for per-player UI duplication already exists and should be reused.**
  `capabilities/camera.rs`'s `split_viewport_player_label_spawn_system`/`_update_system` spawn one
  standalone UI `Text` node per `SplitViewportSlot` camera (reacting to `Added<SplitViewportSlot>`,
  positioned against that camera's live viewport, visibility synced to `Camera.is_active`). A
  per-player target-name HUD readout and target indicator ring should follow the same pattern
  rather than inventing a new one — see Approach.
- **`local_coop_demo` authors no `target_indicator:`, `targetable`/`click_selectable` prefab, or
  action-bar slots anywhere today** (confirmed: same "nothing to observe" situation
  `split_screen_camera_followups.md` found for nameplates/particles/click-select before its own
  playtest aids were added) — a demo-project addition will be needed before this can be
  dev-build playtested, following the same pattern.

## Approach

**Phase 1 — per-player target state, tab-cycle, click resolution, and visual feedback.**

**The core design decision (both plan-review agents independently flagged this as a
contradiction in the first draft): `PlayerTarget` is added *alongside* `CurrentTarget`, not a
replacement.** `CurrentTarget` stays as-is and becomes "the primary player's (player 0's) target,
mirrored" — every other reader of `Res<CurrentTarget>` needs zero changes:

- **Unchanged**: `action_bar.rs` (`action_bar_input_system`'s `{target}` cost/no-target gate),
  all three interpreter systems (`message_interpreter_system`, `fsm_interpreter_system`,
  `entity_fsm_interpreter_system` — `{target}` substitution via `rewrite_target`). Every existing
  `rules.ron`/`state_machine.ron` binding using `{target}` keeps resolving against the primary
  player exactly as today. This is what makes the "Not in scope: rewriting `{target}`" boundary
  below actually hold — deleting the resource would have silently forced that rewrite.
- **Changed**: `targeting.rs` (`click_select_system`, `tab_targeting_system`,
  `target_auto_clear_system` — see below), `target_indicator.rs` (per-player rewrite, see Open
  questions), plus one new HUD system (per-player readout).

Concretely:
- Add `PlayerTarget(pub Option<String>)` as a **component on each player entity** (not a resource
  keyed by player index — a component follows the entity lifecycle, auto-cleans on despawn, and
  matches how `tab_targeting_system`/`click_select_system` already query per-player data).
  Inserted alongside `CharacterController`/`PlayerIndex` at each of the four player-construction
  sites (see `crates/ironhold_core/src/CLAUDE.md`'s "four player-construction sites" inventory —
  this change touches all of them, same class of divergence risk `tag_spawned_entity` exists to
  prevent). Single-player scenes get exactly one `PlayerTarget`, always in lockstep with
  `CurrentTarget` — regression-tested.
- `tab_targeting_system` iterates every `(CharacterController, PlayerTarget, GlobalTransform)`
  instead of `.next()`, resolving each player's own `target_next` key press against their own
  `PlayerTarget`, independently. When the acting player is the primary player, also mirror the
  result into `CurrentTarget` (so `{target}`-driven rules/action-bar gating for the primary player
  keep working unchanged).
- `click_select_system` maps the resolved camera (already viewport-aware since Phase 2 of
  `split_screen_camera_followups.md`) to its owning player — via `OrbitCamera.target` → that
  entity's `PlayerTarget` — and writes the click result there (mirroring into `CurrentTarget` only
  when the acting player is primary). A non-split (single-camera or `party`) scene has exactly one
  player-owning-camera mapping, unaffected.
- `target_auto_clear_system` iterates all `PlayerTarget`s instead of the one resource (mirroring
  into `CurrentTarget` when the cleared target belongs to the primary player).
- `apply_target`'s global pipeline events (`target.changed`/`target.changed:{id}`,
  `target.cleared`) currently carry no player identity and would otherwise fire once per player
  with no way for a `rules.ron` rule to tell which player triggered it. **Only mirror the primary
  player's changes into these global events** — non-primary players' target changes update their
  own `PlayerTarget` and HUD/indicator, but do not emit into the global event pipeline. (Revisit if
  a real project need for per-player pipeline events surfaces — out of scope here.)
- **Target indicator ring** (`capabilities/target_indicator.rs`) and the **target-name HUD
  readout** need a per-player duplication story for split-screen — reuse the
  `split_viewport_player_label_spawn_system` pattern (one instance per `SplitViewportSlot`, synced
  to that camera's viewport/`is_active`), per both reviewers' recommendation — **not** suffixed
  `GameVariables` keys.
  - **HUD readout is designer-authorable via a new scene-level `target_hud:` block** (Frank's
    decision — a sibling to `target_indicator:`, not an extension of it, since it drives a
    different widget: text vs. ring). Minimal shape:
    ```ron
    target_hud: (
      show: Full,        // enum: Full ("prefab_key id"), NameOnly, IdOnly — mirrors the existing
                          // target_display/target_name/target_id GameVariables shapes
      font_size: 16.0,
      color: (0.9, 0.9, 0.9, 1.0),
    )
    ```
    One instance per `SplitViewportSlot`, positioned/visibility-synced like the P1/P2 corner
    labels. Absent block → feature simply doesn't spawn the per-viewport readout (opt-in, matching
    `target_indicator:`'s own opt-in pattern) — a project that only wants Phase 1's ring, or
    neither, isn't forced into it.
  - **Rings are tinted per-player** (Frank's decision) using the existing `PLAYER_LABEL_COLORS`
    palette, reused from the P1/P2 corner-label precedent — but **only when 2+ players are
    actually present** (`PlayerIndex` populated via split-screen). A single-player scene's ring
    keeps today's exact color precedence (prefab override → category → scene default) — no visual
    regression. If 2 players target the same entity, both rings render, coincident, each tinted —
    no dedup logic needed.
  - **The existing global `target_display`/`target_name`/`target_id` `GameVariables` (and any RON
    `Label` bound to them) go blank in split-screen scenes** (Frank's decision) — mirroring
    `clear_target_vars`'s existing "write empty strings" pattern rather than adding new Label-
    hiding logic. There is no single meaningful "the" target across 2+ independent players, so a
    designer who bound a `Label` to `target_display` sees it correctly go blank once split-screen
    activates, rather than silently showing only the primary player's value with no indication why
    the second player's target isn't reflected. Single-player scenes are completely unaffected —
    the vars keep populating exactly as today.
- **`{target}` substitution in `rules.ron`/`state_machine.ron`/behavior files still resolves
  against `CurrentTarget` (the primary player) only — this is explicitly unchanged in Phase 1.**
  See "What Phase 1 does NOT do" and Not in scope.

**What Phase 1 does NOT do:** a non-primary player's `PlayerTarget` is never consulted by
`{target}` substitution, the action bar's cost/no-target gate, or any `rules.ron`/behavior-driven
gameplay action. Phase 1 delivers independent *selection and visual feedback* (each player sees
their own ring/readout) — not independent *gameplay consequences*. Two players can each be
visually tracking a different enemy, but any ability fired via the shared action bar still only
ever affects the primary player's target. This is a real, designer-visible gap and must be
documented (see Tasks) — not silently left to be discovered as "a bug."

**Phase 2 — per-player action-bar ability execution against each player's own target.**

**Correction to Phase 1's own speculation (re-verified going into Phase 2):** Phase 1's Research
findings assumed Phase 2 would need to thread player identity through the entire Message →
Interpreter → Action → Executor pipeline. Re-reading `action_bar_input_system` shows this is
**not** actually true for the common case — the action bar already calls `rewrite_target(action,
target_id)` itself, locally, *before* pushing anything to `ActionQueue` (`capabilities/
action_bar.rs`). The interpreter chain never sees `{target}` in a slot's inline `do_actions` — it's
already a concrete entity ID by the time it's queued. So Phase 2 only needs the action bar itself
to resolve the *correct* `target_id` per bar/player; it does **not** need `GameEvent`/`UiEvent`/
`InputActionMessage` to carry player identity, and does **not** need any interpreter-system change.
This makes Phase 2 materially narrower than originally scoped. (The one place this doesn't hold:
see "Not in scope" below — a `rules.ron` rule overriding a slot's intent still resolves `{target}`
via the interpreter against the primary player only.)

**Hard dependency: `planning/features/done/action_bar_custom_hotkeys.md` — shipped 2026-07-16
(`6305237`), no longer blocking.** Action bars previously shared one hardcoded `DIGIT_KEYS` table
(`1`-`9`, `i`) — two players sharing one keyboard couldn't have two independent action bars without
colliding on the same physical keys (the exact class of problem `target_next` hit before Phase 1
gave each player prefab its own key). Now any `InputMap::parse_key()`-recognised key name can be
bound per slot, plus a `key_hint` field and both a runtime `warn!` and an `ironhold_cli validate`
check for unparseable/duplicate-within-a-bar keys — see that feature's own "Relationship to Phase
2" section for two things this Phase 2 implementation must still account for: don't over-invest in
matching its fire-first single-match semantics (Phase 2 restructures the loop anyway, see Approach
below), and its duplicate-key check is per-bar only, not scene-wide (logged to
`claude_suggestions.md` — Phase 2 introduces multiple bars per scene, so this phase should extend
that check to be scene-wide rather than leaving the per-bar gap in place).

**Approach:**
- **New field `ActionBarDef.owner_player: Option<u32>`** (`#[serde(default)]`, mirrors
  `PlayerIndex`). `None` (the default) → today's exact behavior: single shared bar, resolves
  against the primary player. `Some(n)` → this bar's slots act on whichever player entity carries
  `PlayerIndex(n)`. Copied onto `ActionSlotUi` per slot at scene-load time (same pattern
  `slot_key`/`cooldown_secs`/`cost` already follow).
- **No auto-duplication engine mechanism** — unlike the target ring/HUD (Phase 1), which needed an
  engine-automatic per-`SplitViewportSlot` spawn because they're camera-anchored overlays a
  designer has no other way to author. An `ActionBar` is already a fully designer-authored UI
  subtree with `position: (f32, f32)` ("always absolute" per its own doc comment) — a split-screen
  scene simply authors two `ActionBar` blocks in its `ui:` list, one per player, each positioned in
  that player's half and tagged `owner_player`. Same manual-duplication pattern Phase 1's own
  playtest RON already used for the `legacy_target_var`/`targeting_hint` `Label`s in `room3.scene.ron`
  — no new engine spawn/position-sync system needed.
- **`action_bar_input_system` rewritten to resolve per-slot, not once globally, AND to stop
  dropping same-frame presses (system-architect finding, Major — verified against code):** today
  it does `DIGIT_KEYS.iter().find(|(kc,_)| keys.just_pressed(*kc))` then handles that **one** slot
  and returns (`action_bar.rs:118-122`) — there is no loop. With two independent per-player bars,
  if both players press their own key in the same frame, one press is silently dropped. This
  directly undercuts the headline Phase 2 acceptance criteria ("player 1 fires… player 2 fires
  independently"), so the rewrite must restructure this into a loop over **every** slot whose
  resolved key is `just_pressed` this frame, not just a target-lookup swap on the existing
  single-match structure. Once restructured: for each fired slot, resolve its owning player
  (`owner_player` → matching `PlayerIndex` entity; `None` → primary player, the same "PlayerIndex(0)
  or no PlayerIndex at all" definition Phase 1 established) and read **that player's own
  `PlayerTarget`** instead of `CurrentTarget` for the `{target}` rewrite and the no-target gate. For
  the primary player this is a no-op change in practice, since Phase 1 already mirrors
  `PlayerTarget` → `CurrentTarget` for the primary player — single-player projects see zero
  behavior change. (`player_query.single()` for the intent-event player ID has the same "silently
  wrong with 2+ players" problem today — `.single()` returns `None` on ambiguous match, falling
  back to the literal string `"player"` — replaced by the same per-slot owning-player resolution.)
- **Action bars remain keyboard-only — a real, documented usability gap for the realistic co-op
  configuration (system-architect + ux-gamedesigner-reviewer, both independently flagged this):**
  slots read `Res<ButtonInput<KeyCode>>` directly; there is no gamepad/`InputMap`-routed activation
  path, in this phase or in `action_bar_custom_hotkeys.md`. Phase 1's own playtest notes call out
  "co-op's common case is one keyboard + one gamepad player" — under that configuration, after
  Phase 2 ships, the gamepad player's action bar UI renders fully but can **never fire** (no key
  presses reach it). The only fully-usable Phase 2 configuration is two players sharing one
  keyboard with disjoint slot keys — matching how Phase 1's own actual playtest was authored
  (`KeyT`/`KeyM`, both keyboard, despite the "keyboard + gamepad" language elsewhere in this doc).
  Must be stated plainly in docs (not left for a designer to discover as "my gamepad player's
  abilities are broken"), and a "gamepad-routed action-bar slots" follow-up logged to the backlog
  as the item that would make co-op abilities actually usable in the common configuration.
- **Cross-bar duplicate slot-key collisions need a runtime `warn!`, not just an opt-in CLI
  `validate()` check (ux-gamedesigner-reviewer: an opt-in check a designer has to remember to run
  is not a real safety net; system-architect: the collision is worse than "wrong slot fires" —
  `CooldownMap`/`PendingIntentActions`/`HandledIntentSlots` are keyed by the literal `slot_key`
  string alone, scene-wide, and `intent_slot_key()` strips the event down to just that key
  (`message_interpreter.rs`) before checking `HandledIntentSlots` — so a `rules.ron` rule that
  handles **one** player's intent on a colliding key silently suppresses the **other** player's
  pending slot too, not just picks the wrong target).** Add a scene-load-time `warn!` when 2+
  action bars in the same scene share a slot key, mirroring Phase 1's duplicate-`player_index: 0`
  runtime-warning precedent (`entity_spawner.rs`) — in addition to, not instead of, extending
  `action_bar_custom_hotkeys.md`'s own proposed within-bar duplicate-key `validate()` check to also
  cover across-bar collisions in the CLI tool.
- **Cost/resource gating stays global/shared — explicitly out of scope for this phase, and this is
  more than a documentation footnote (ux-gamedesigner-reviewer: "actively misleading," not just a
  shared economy).** `SlotCost` checks/deducts against the single global `LoadedStats` resource, not
  any per-entity `StatMap` (per-entity stats exist and are used for NPCs via `"{self}.stat"`
  addressing, but the action bar's cost check was never wired to it). Concretely: `action_bar_
  visual_system`'s cooldown-overlay dim (`action_bar.rs`) reads the same shared `LoadedStats`, so
  when player 1 spends the shared resource, player 2's bar visually dims too — each bar looks
  per-player (separately positioned, separately tinted-if-we-choose) but silently shares one
  invisible pool, which reads as a bug, not a documented limitation, from in-game symptoms alone.
  **Mitigation for this phase (no schema rework):** the playtest-aid slot should use a cost-free
  ability (omit `cost:` entirely) specifically to avoid surfacing this during the demo, and
  `docs/20_data_formats.md`'s action-bar section must state the shared-pool behavior explicitly,
  next to the `SlotCost` field docs, not just in this plan file. A genuinely separate per-player
  economy is future work — flag as its own backlog item if a real project needs it.
- **Pipeline events stay as today**: `intent.slot.{key}:{player_id}` already embeds the acting
  player's `SpawnId` in the event string (`action_bar_input_system` already does this via
  `player_query.single()` — just needs the per-owner-player lookup above instead of `.single()`).
  `action_bar.activated:{key}` / `.on_cooldown:{key}` / `.no_target:{key}` /
  `.insufficient_resource:{key}` are unchanged.

### Not in scope (Phase 2)
- **Per-player stats/resources** (health, mana, cooldown-affecting buffs, etc.) — see Approach.
  Both players' action bars share one global resource pool; a genuinely separate per-player economy
  is future work.
- **`rules.ron`-overridden slot intents still resolve `{target}` against the primary player only.**
  When a designer's own rule matches a slot's `intent.slot.*` event and defines its own
  `do_actions` (rather than letting the slot's inline `do_actions` fire), those actions go through
  `message_interpreter_system`'s `rewrite_target`, which still reads the global `CurrentTarget` —
  this is the one path Phase 2 does *not* make per-player, consistent with Phase 1's existing
  "not in scope: rewriting `{target}` in the interpreter" boundary. **Doc placement matters here
  (ux-gamedesigner-reviewer)**: `docs/20_data_formats.md`'s existing intent-interception example
  (around the action-bar section) already teaches designers this exact `rules.ron`-override
  pattern with `{target}`-using actions — the caveat must go inline right next to that example, not
  only in this plan file, or a designer copying the documented pattern in a split-screen project
  will hit the gap with zero warning.
- **Gamepad/InputMap-routed action-bar activation** — slots stay keyboard-`parse_key`-bound (per
  the custom-hotkeys dependency), not routed through a player's `InputMap` the way movement/
  `target_next` are. See the keyboard-only limitation called out in Approach — logged as its own
  backlog follow-up ("gamepad-routed action-bar slots") rather than solved here.

### Not in scope

- **Rewriting `{target}` substitution in `message_interpreter_system`/`fsm_interpreter_system`/
  `entity_fsm_interpreter_system` to resolve per-acting-player** — would require threading player
  identity through the entire Message → Interpreter → Action → Executor pipeline (every message
  type gaining a player-origin field). Global `rules.ron`/`state_machine.ron` rules keep reading
  `CurrentTarget` (the primary player) exactly as today; only the targeting *capability's own
  per-player state* (which entity is "my" target, for selection/display purposes) becomes
  per-player in Phase 1. **Note (system-architect, plan review):** this same missing primitive —
  player identity threaded through the Message pipeline — is also what Phase 2's action bar and
  Beta 0.6's `PlayerOwnership` multiplayer story will eventually need. Worth designing holistically
  when one of those becomes concrete, not speculatively here; logged to
  `planning/claude_suggestions.md`.
- **Per-player action bar / ability execution** — was Phase 2, deferred, at the time Phase 1 was
  planned; now scoped concretely in Phase 2's own "Approach"/"Not in scope (Phase 2)" subsections
  above.
- **Fixing the single-shared-mouse limitation for click-to-select** — physically impossible with
  one mouse; already accepted during Phase 2 of `split_screen_camera_followups.md`'s playtest.
  Tab-cycling is the only simultaneous-for-both-players mechanism this feature provides. Must be
  documented as expected behavior (see Tasks), not left for a designer to rediscover as a bug.

## Playtest setup — `local_coop_demo` changes needed

`local_coop_demo` authors no `target_indicator:`, `targetable`/`click_selectable` prefab, or
action-bar slots today. A `targetable: true` (and/or `click_selectable: true`) test prop plus a
`target_indicator:` block will need to be added to room3 (or a new room) so both players can
independently Tab-cycle/click a different target and the fix can be visually confirmed — same
"nothing to observe yet" pattern `split_screen_camera_followups.md` hit for its four sites.

**Both players' `InputMap.target_next` must be bound to a device-appropriate key** (ux-reviewer
finding) — co-op's common case is one keyboard + one gamepad player. If the playtest aid only
wires a keyboard `Tab` binding, the gamepad-driven player will appear to have "broken targeting"
during the playtest itself. Bind each player's `target_next` to their own input device (keyboard
Tab for the keyboard player, a gamepad button for the gamepad player) and cover both in the
playtest checklist.

## Tasks

- [x] Phase 1: `PlayerTarget` component added alongside `CurrentTarget` (not replacing it) at the
      four player-construction sites; `tab_targeting_system`/`click_select_system`/
      `target_auto_clear_system` made per-player with primary-player mirroring into
      `CurrentTarget`; per-player target indicator (tinted via `PLAYER_LABEL_COLORS` when 2+
      players present) + new `target_hud:`-driven per-viewport HUD readout; existing global
      `target_display`/`target_name`/`target_id` `GameVariables` blanked whenever 2+ players
      present; `local_coop_demo` playtest addition (distinct `target_next` keys per player —
      `KeyT`/`KeyM`, not the browser-intercepted `"Tab"` default) — `04c1785`. 2 code-review-driven
      fixes: `Action::SetTarget`/`ClearTarget` now mirror into the primary player's `PlayerTarget`
      (previously only wrote `CurrentTarget`, silently breaking the ring for that action path); a
      runtime `warn!` fires when 2+ players share `player_index: 0` (both would be treated as
      primary, causing `CurrentTarget` stomping). All 5 reviews clean/addressed; full
      `ironhold_core` test suite (16 binaries) + `cargo check -p ironhold_cli` green. WASM dev
      build clean, no console errors. Playtest confirmed by Frank.
- [x] Tests: per-player tab-cycle independence
      (`test_tab_targeting_each_player_cycles_independently`), click resolving to the clicking
      player only (`test_click_select_only_changes_the_clicking_players_target`), single-player
      regression (`test_legacy_target_vars_populate_when_single_player`), target auto-clear per
      player (`test_target_auto_clear_is_per_player`), non-primary player's target changes do NOT
      mirror into `CurrentTarget`/emit global events
      (`test_only_primary_player_target_mirrors_into_current_target_and_global_events`), per-player
      ring tinting (`test_target_indicator_tints_rings_per_player_when_multiplayer`), per-viewport
      HUD readout (`test_target_hud_shows_each_players_own_target_independently`), and the
      `Action::SetTarget`/`ClearTarget` regression that caught the primary-mirroring bug
      (`test_set_target_and_clear_target_actions_mirror_into_primary_player`)
- [x] Docs: added a "Per-player split-screen targeting" subsection to `docs/20_data_formats.md`
      (beside "Split-screen player HUD labels"), covering the `target_hud:` block's fields/RON
      example, per-player ring tinting, legacy `GameVariables` blanking (including the party-mode
      gap), the single-shared-mouse limitation, and the `{target}`/`target.clicked` primary-player
      carve-out. Updated `crates/ironhold_core/src/CLAUDE.md`'s `{target}` substitution and target
      indicator sections.
- [x] Logged the per-player-`{target}`/Phase-2-action-bar/Beta-0.6-`PlayerOwnership` convergence
      insight to `planning/claude_suggestions.md` (system-architect finding, see Not in scope),
      plus 3 debug-detective findings (divergent player-count query shapes, no system ordering
      vs. the camera chain, the duplicate-`player_index` footgun) and 1 wasm-perf-reviewer nit
      (`target_hud_update_system`'s uncached `format!`)
- [x] WASM dev build + playtest checklist — clean, playtest confirmed by Frank, no console errors
- [x] Phase 2: ship `action_bar_custom_hotkeys.md` first — shipped 2026-07-16 (`6305237`), see the
      "Hard dependency" note in Approach for what it delivered and what Phase 2 still owes it
      (cross-bar duplicate-key check, fire-first loop restructuring)
- [x] Phase 2: `ActionBarDef.owner_player: Option<u32>` schema field (`#[serde(default)]`), copied
      onto `ActionSlotUi` at scene-load time
- [x] Phase 2: rewrite `action_bar_input_system` to (a) loop over **every** slot whose resolved key
      is `just_pressed` this frame instead of `find`+`return`ing on the first match
      (system-architect finding — the current structure silently drops one player's press if both
      fire in the same frame), and (b) resolve the acting player per slot (`owner_player` →
      matching `PlayerIndex` entity, `None` → primary player) and read that player's own
      `PlayerTarget` for the `{target}` rewrite/no-target gate/intent-event player ID, replacing
      the `player_query.single()` call (already silently wrong with 2+ players)
    - [x] Cross-bar duplicate-slot-key check: a scene-load-time runtime `warn!` (mirroring Phase 1's
        duplicate-`player_index: 0` precedent), in addition to extending `action_bar_custom_
        hotkeys.md`'s own within-bar `validate()` warning to also cover across-bar collisions in
        the CLI tool (system-architect + ux-gamedesigner-reviewer — an opt-in CLI check alone
        isn't a real safety net, and the collision silently suppresses the *other* player's slot
        via `HandledIntentSlots`, not just picks the wrong target)
- [x] `local_coop_demo` playtest addition: two `ActionBar` blocks (one per player, `owner_player`
      tagged, disjoint hotkeys, positioned in each player's half), **both players on the keyboard**
      (not keyboard+gamepad — the gamepad player's bar would render but never fire, see Approach),
      with at least one **cost-free** slot (no `cost:` field, to avoid the shared-`LoadedStats`
      dimming-both-bars confusion during playtest) whose `do_actions` uses `{target}` against a
      `targetable` prop, so per-player resolution is visually confirmable
- [x] Tests: two players' bars fire independently against their own `PlayerTarget`; both players'
      slots firing in the same frame both take effect (regression for the find+return bug);
      primary-player bar behavior is unchanged in a single-player scene (regression); duplicate
      slot key across two bars triggers the scene-load `warn!`; a `rules.ron`-overridden slot
      intent still resolves `{target}` against the primary player only (documents the accepted
      gap, not a silent inconsistency)
- [x] Docs: `docs/20_data_formats.md` — `ActionBarDef.owner_player` field + per-player action-bar
      RON example; the shared-`LoadedStats` cost-pool caveat placed next to the `SlotCost` field
      docs (not just this plan file); the `rules.ron`-override `{target}` gap placed inline next to
      the existing intent-interception example (ux-gamedesigner-reviewer: designers copying that
      documented pattern need the caveat right there); the keyboard-only limitation stated plainly.
      Update `crates/ironhold_core/src/CLAUDE.md`'s action-bar/targeting sections.
- [x] Backlog: log "gamepad-routed action-bar slots" as a new follow-up item (both reviewers
      independently flagged that without it, the realistic keyboard+gamepad co-op configuration
      leaves one player's action bar permanently unusable).

## Open questions

**Phase 2 (resolved 2026-07-16 — plan moves to Active):**
- ~~Sequencing of `action_bar_custom_hotkeys.md`~~ — **resolved: shipped first, on its own branch,
  2026-07-16 (`6305237`)**, per Frank's confirmation. Phase 2 now branches off the updated `main`.
- ~~Shared vs. per-player stats/resources~~ — **resolved: confirmed acceptable as a documented
  interim limitation, not a blocker.** `LoadedStats` is touched by 13 files across the codebase
  (stat bars/HUD, dialogue conditions, inventory costs, stat radar, `ModifyStat` action executor —
  not just the action bar's cost check), so making it per-player would be a substantial
  cross-cutting change in its own right, not a small Phase 2 prerequisite, and no current project
  needs a per-player economy. Logged as its own future backlog item (see `backlog.md` ▸ Gameplay &
  Environment ▸ "Per-player stat/resource pools") to design properly if/when a real need surfaces,
  rather than building it speculatively now.
- ~~`local_coop_demo` demo design~~ — **resolved: `ModifyStat(key: "{target}.health", delta: -10.0)`
  against the existing `click_target_test` prop** (already `targetable: true` since Phase 1) — the
  minimal option, reusing Phase 1's playtest aids rather than inventing new ones.

**Phase 1 (resolved — plan reached Ready):**

- ~~Per-player target indicator/HUD readout mechanism~~ — **resolved: engine-automatic per-viewport
  spawn/positioning (reuse `split_viewport_player_label_spawn_system`'s pattern) with a new
  designer-authored `target_hud:` RON block controlling text format/font/color.** Both reviewers
  converged on engine-automatic *spawning* (the ring is a viewport-anchored overlay a designer
  cannot author via RON `Label` nodes at all); Frank decided the display *content* should still be
  designer-configurable via a new block rather than fully hardcoded.
- ~~Does `PlayerTarget` belong on the `CharacterController` entity or a separate index-keyed
  component~~ — **resolved: component on the player entity directly** (follows entity lifecycle,
  matches existing per-player query patterns; no designer-facing impact either way).
- ~~Should Phase 2 (per-player action bar) happen at all~~ — **resolved: stays deferred.** No
  current project uses the action bar in split-screen; it needs the same player-identity-in-the-
  pipeline primitive that per-player `{target}` and Beta 0.6 multiplayer will eventually need, so
  it's worth designing holistically later rather than speculatively now.
- ~~Display-format RON block shape~~ — **resolved: new `target_hud:` scene-level block** (sibling
  to `target_indicator:`, not an extension of it), see Approach for the field shape.
- ~~Target indicator ring color/dedup policy for 2+ players~~ — **resolved: per-player tint via
  `PLAYER_LABEL_COLORS`** (only when 2+ players are present — single-player keeps today's exact
  per-target color precedence), **two players targeting the same entity render two coincident
  tinted rings, no dedup.**
- ~~Single-player `target_display` Label binding in a co-op-promoted scene~~ — **resolved: the
  existing global `target_display`/`target_name`/`target_id` `GameVariables` go blank in
  split-screen scenes** (mirrors the existing `clear_target_vars` "write empty strings" pattern),
  rather than silently showing only the primary player's value. Single-player scenes are
  unaffected.

## Acceptance criteria

- ~~Given a split-screen scene with 2 players and 2+ `Targetable` entities, when player 1 presses
  their `target_next` key, then only player 1's target changes — player 2's target is unaffected.~~
  **Met — confirmed by `test_tab_targeting_each_player_cycles_independently` and Frank's playtest.**
- ~~Given the same scene, when player 2 clicks a `ClickSelectable` entity in their own viewport,
  then only player 2's target changes.~~ **Met —
  `test_click_select_only_changes_the_clicking_players_target`, confirmed by playtest.**
- ~~Given a single-player (non-split) scene, when the player Tab-cycles or clicks a target, then
  behavior is unchanged from today (regression guard) — `PlayerTarget` and `CurrentTarget` stay in
  lockstep.~~ **Met — `test_legacy_target_vars_populate_when_single_player` and the click-select/
  tab-targeting single-camera regression tests.**
- ~~Given 2 players each with a different current target, when either target becomes hidden/despawned,
  then only that player's target auto-clears — the other player's target is unaffected.~~ **Met —
  `test_target_auto_clear_is_per_player`.**
- ~~Given a non-primary player selects a target (Tab or click), when a `rules.ron`/behavior action
  using `{target}` fires, or the action bar's `{target}`-gated cost check runs, then it resolves
  against the primary player's target only — the non-primary player's selection has no gameplay
  effect (Phase 1's documented scope boundary, not a bug).~~ **Met —
  `test_only_primary_player_target_mirrors_into_current_target_and_global_events`.**
- ~~Given a split-screen scene with a `target_hud:` block authored, when either player selects a
  target, then a per-viewport HUD readout appears in that player's own viewport showing the
  configured format, independent of the other player's readout.~~ **Met —
  `test_target_hud_shows_each_players_own_target_independently`, confirmed by playtest.**
- ~~Given a split-screen scene, when a `Label` bound to the legacy `target_display`/`target_name`/
  `target_id` `GameVariables` is present, then it renders blank — not the primary player's value
  with no explanation of why the second player's target isn't shown.~~ **Met —
  `test_legacy_target_vars_blank_when_multiplayer`, confirmed by playtest via the room3 Label.**
- ~~Given a split-screen scene where 2 players select different `Targetable` entities, when their
  target indicator rings render, then each is tinted per the `PLAYER_LABEL_COLORS` palette; if
  both players select the same entity, two coincident tinted rings render.~~ **Met —
  `test_target_indicator_tints_rings_per_player_when_multiplayer`, confirmed by playtest.**

### Phase 2 acceptance criteria

- ~~Given a split-screen scene with two `ActionBar`s, each tagged with a different `owner_player`
  and disjoint slot keys, when player 1 fires a slot whose `do_actions` includes
  `ModifyStat(key: "{target}.health", ...)`, then it resolves against player 1's own `PlayerTarget`
  — player 2's target is unaffected, regardless of which entity player 2 currently has selected.~~
  **Met — `test_owner_player_slot_resolves_against_its_own_players_target`, confirmed by playtest.**
- ~~Given the same scene, when player 2 fires their own bar's slot, then it resolves against player
  2's own `PlayerTarget` independently of player 1's.~~ **Met — same test, confirmed by playtest.**
- ~~Given both players press their own bar's key in the **same frame**, when the system updates, then
  both slots fire and both actions resolve against their respective players' targets — neither
  press is silently dropped (regression guard for the `find`+`return` bug).~~ **Met —
  `test_both_players_bars_firing_same_frame_neither_press_dropped`.**
- ~~Given a single-player (non-split) scene with one `ActionBar` and no `owner_player` authored, when
  the player fires a slot, then behavior is unchanged from today (regression guard).~~ **Met —
  `test_single_player_slot_with_no_owner_still_resolves_via_player_target` and the pre-existing
  intent-layer tests, all still passing unchanged.**
- ~~Given two action bars in the same scene that share a slot key, when the scene loads, then a
  runtime `warn!` fires identifying the collision (mirrors Phase 1's duplicate-`player_index: 0`
  precedent).~~ **Met — `warn_cross_bar_duplicate_keys` (runtime) +
  `cross_bar_duplicate_action_bar_key_exits_1` (CLI, hard error).**
- ~~Given a slot's cost check, when either player's bar fires a cost-gated slot, then the shared
  global `LoadedStats` resource is checked/deducted (documented shared-pool limitation, not a bug)
  — not a per-player pool.~~ **Met — unchanged `SlotCost` behavior, documented in
  `docs/20_data_formats.md`.**
- ~~Given a `rules.ron` rule that overrides a slot's intent event with its own `{target}`-using
  `do_actions`, when a non-primary player fires that slot, then the rule's actions resolve against
  the primary player's target, not the firing player's (documented gap, matches Phase 1's existing
  interpreter-level boundary).~~ **Met —
  `test_rule_overridden_intent_still_resolves_target_against_primary_player_only`.**
- **(Found during playtest, not originally listed)** Given a split-screen scene where a target is
  visible in 2+ active viewports, when either player's slot deals damage, then the damage popup
  renders in every viewport the target is visible in — not just the single highest-priority active
  camera regardless of which player triggered it. **Met —
  `test_show_damage_popup_duplicates_ranks_when_split_screen_active`, confirmed by Frank's playtest
  after the `WorldLabelRank` duplication fix.**
