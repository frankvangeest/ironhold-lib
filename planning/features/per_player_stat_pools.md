# Feature: Per-Player Stat Pools (Action-Bar Costs)

_Status: Ready_
_Planned at: `612111e` (2026-07-17)_
_Plan review (2026-07-17): system-architect (Ready — every specific claim verified against real
code; flagged one Major correctness footgun, folded into Approach/Tasks below) +
ux-gamedesigner-reviewer (Needs-minor-design-work — no rework, just scoping: resolve the warning
question, expand the docs checklist, pair the RON example with an explicit fallback callout; all
incorporated below)._

## What
Split-screen players who each own an `ActionBar` with a `cost:`-gated slot (mana, stamina, etc.)
currently share one invisible resource pool: `SlotCost`'s check and deduct always read/write the
single scene-global `LoadedStats` resource, so player 1 spending mana also drains and dims player
2's bar, even though the two bars look independently positioned and owned. This feature gives each
player who declares their own stat template an independent pool, while any player/project that
doesn't declare one keeps today's exact global-pool behavior.

## Why
This is the concrete, demonstrated bug behind the "Per-player stat/resource pools" backlog item
(`planning/backlog.md`, surfaced 2026-07-16 during per-player action-bar execution Phase 2
planning, documented as an interim limitation in `docs/20_data_formats.md`'s `SlotCost` caveat and
`crates/ironhold_core/src/CLAUDE.md`'s "Per-player action bars (Phase 2)" section). It blocks any
further local-coop ability authoring that wants real cost gating (both `local_coop_demo` action
bars currently either omit `cost:` or would silently share a pool). It's also a prerequisite in
spirit for inventory/dialogue ever going per-player, though those two systems are explicitly **not**
touched by this plan (see Not in scope) since neither exists in any split-screen project yet.

## Approach
**The real gap is narrower than "global `LoadedStats`" everywhere.** Per-entity stat storage
already exists and is already fully wired: `StatMap` (`schema/stats.rs`) is a `Component`,
populated from `PrefabDef.stat_templates` by `attach_prefab_features` (`entity_spawner.rs:40-114`)
for every NPC/prop/composite prefab, ticked every frame by all of `capabilities/stats.rs`'s systems
identically to the global resource, and already read via dot-routed `"{id}.stat"` keys by both the
`ModifyStat`/`SetStat` action executor (`action_executor.rs:397-452`) and the `stat_display.rs`/
`stat_radar.rs` `resolve_stat` helper. **Player entities are the only spawn path that never
receives a `StatMap`**, and `SlotCost`'s two read sites (`action_bar_input_system`'s check
`action_bar.rs:160-172` + deduct `:199-203`, and `action_bar_visual_system`'s dim check
`:268-275`) never dot-route — they always resolve the plain stat key against `LoadedStats`.

This means the fix needs **zero new RON schema** — it reuses `PrefabDef.stat_templates` verbatim,
the exact field NPCs already declare stats with, just forwarded onto player prefabs too:

1. Factor the `StatMap`-building block already inside `attach_prefab_features`
   (`entity_spawner.rs:81-114`) into a small shared helper (e.g. `build_stat_map_from_templates`)
   so both the generic prefab path and the player spawn path call the same code — matches this
   codebase's established "single source of truth" convention (`attach_prefab_features`,
   `tag_spawned_entity`, `assemble_player_config` are all named for exactly this reason).
2. Add `stat_templates: Vec<StatTemplateDef>` to `PlayerConfig` (`schema/player.rs`), forwarded
   from `prefab.stat_templates.clone()` in `assemble_player_config` (`entity_spawner.rs:931-964`) —
   the same one-place-to-edit function that already forwards `player_index` and `material`.
3. `spawn_player_entity_core` (`entity_spawner.rs:715-840`) inserts the built `StatMap` on the
   player entity when non-empty, alongside the existing `PlayerTarget::default()`/`PlayerIndex`
   inserts. The primitive/capsule player path (site 2 of the four-site inventory) is **not**
   touched — per existing `CLAUDE.md` documentation it's single-player-only and never gets local
   co-op components (`PlayerIndex`, `owner_player` action bars) either.
4. Rewire the three `SlotCost` sites in `action_bar.rs` to resolve **per-player-first, global-
   fallback**: given the acting player's own `Option<&StatMap>` (added to `action_bar_input_
   system`'s existing player query, and newly added to `action_bar_visual_system`, which currently
   has no player-resolution query at all), if that player's `StatMap` contains `cost.stat`, check/
   deduct against it via the dot-routed key `"{spawn_id}.{stat}"` (reusing the executor's existing
   dot-routing — **no executor change needed**); otherwise fall back to `LoadedStats` exactly as
   today. A shared small helper (e.g. `resolve_cost_current(stat, player_stat_map, loaded_stats)`)
   avoids duplicating this branch across the input and visual systems.
   **Correctness requirement (system-architect finding):** the check (synchronous, direct read)
   and the deduct (deferred — pushed as an `Action::ModifyStat` and executed later off the
   `ActionQueue`) are two different code paths and must not each independently decide "own pool
   vs. global" — compute that decision **once** in `action_bar_input_system` per firing slot (i.e.
   resolve whether `cost.stat` exists in the acting player's `StatMap`) and use that single
   resolution to build both the gate check's read and the deduct action's key, rather than letting
   the two paths potentially disagree (e.g. a race where a value change between check-time and
   deduct-time flips which pool the key resolves against).
5. Regen, thresholds, and modifiers need no changes — `stats.rs`'s tick systems already process
   `StatMap` components generically. HUD display (`stat_label`/`world_stat_bar`) needs no changes
   either — `resolve_stat` already supports a dot-routed key; a designer can already bind
   `"player_p1.mana"` today.
6. **Missing-template warning, load-time and scoped** (resolves the former Open Question, per
   both reviewers): warn once at scene load — mirroring `warn_cross_bar_duplicate_keys` — when an
   `ActionBar` with `owner_player` set (i.e. explicitly scoped to one of 2+ players) has a `cost:`
   slot whose `stat` is declared in *that player's* `stat_templates` list under a different key, or
   when the player declares `stat_templates` at all but not the one this slot costs. Do **not**
   warn when the player declares no `stat_templates` at all and `cost.stat` matches a global
   `stats.ron` entry — that is the ordinary, unchanged single-player fallback path and must stay
   silent, or every existing single-player `cost:`-gated project starts logging spurious warnings.
   Add the equivalent `ironhold_cli validate` error alongside the runtime warning, same pairing as
   the cross-bar duplicate-key check.

**Backward compatibility is the load-bearing property of this design**: a player prefab that
declares no `stat_templates` gets no `StatMap`, so `SlotCost` falls through to the global
`LoadedStats` branch exactly as before this feature. Every existing single-player project (and any
split-screen bar that doesn't opt in) is byte-for-byte unaffected.

**Two side-effect interactions to document, not fix (system-architect findings, both existing
behavior a designer should just know about):**
- A `rules.ron` rule that intercepts a slot's intent event suppresses its **entire** pending entry
  — built-in `do_actions` *and* the cost deduct together (`flush_pending_intent_system` drops the
  whole `(actions, cooldown)` tuple when `HandledIntentSlots` contains the key). This was already
  true before this feature; it now simply applies per-player too — a rule-overridden per-player
  slot never drains that player's pool either.
- A player prefab that has both a nameplate and a `stat_templates` key matching what the nameplate
  system looks for will surface a nameplate stat bar automatically (`nameplate.rs` already reads
  `Option<&StatMap>`) — not a compatibility break, but worth calling out in the demo authoring so
  it isn't mistaken for a bug.

## Tasks
- [ ] Extract the `StatMap`-building block from `attach_prefab_features` into a shared helper
      callable from both the generic prefab path and player spawn.
- [ ] Add `stat_templates: Vec<StatTemplateDef>` to `PlayerConfig`; forward in
      `assemble_player_config`.
- [ ] Insert the built `StatMap` (when non-empty) in `spawn_player_entity_core`.
- [ ] Rewire `action_bar_input_system` to resolve "own pool vs. global" **once** per firing slot
      and use that single resolution for both the cost check and the deduct action's key (not two
      independent checks — see Approach's correctness requirement). Falls back to global
      `LoadedStats` when the player has no matching `StatMap` entry.
- [ ] Add a player-resolution query and the same fallback logic to `action_bar_visual_system`.
- [ ] Add the load-time missing-template `warn!` (scoped per Approach point 6) plus the matching
      `ironhold_cli validate` error.
- [ ] Update `local_coop_demo`: give `player_p1`/`player_p2` their own `stat_templates` (e.g.
      `mana`, with **visibly different base values**, e.g. 100 vs. 60, so independence is obvious
      at a glance), author a `cost:`-gated slot on each existing action bar, and a per-player
      `world_stat_bar`/`stat_label` bound to `"{spawn_id}.mana"` so each pool is visually
      confirmable independently in playtest. Comment the cost slot pointing at its owning prefab's
      `stat_templates` block, and note in the demo comment that `local_coop_demo` has no
      `stats/stats.ron` at all, so mana here is fully per-player by design (no global fallback to
      muddy the example).
- [ ] Tests: cost check/deduct resolves per-player when the acting player has a matching
      `StatMap` entry; falls back to global when absent (regression covering existing
      single-player behavior); two players with independent pools don't cross-drain or cross-dim;
      regen on one player's pool doesn't affect the other's cooldown-overlay dim state; the
      missing-template warning fires when expected and stays silent on the ordinary single-player
      global-fallback case.
- [ ] Docs — full checklist (ux-gamedesigner-reviewer finding: more spots go stale than just the
      two obvious ones):
  - [ ] `docs/20_data_formats.md`'s `SlotCost` "global, not per-player" caveat prose
  - [ ] `docs/20_data_formats.md`'s `SlotCost.stat` field-table row (currently says the key
        "matches a key in `stats.ron`" — needs the per-player alternative added right there, not
        just in the prose above/below it)
  - [ ] `docs/20_data_formats.md`'s "Instance stats (`stat_templates`)" section — note players may
        now carry it too, and what that unlocks
  - [ ] `docs/20_data_formats.md`'s nameplate example comment that currently says a player using a
        global stat "silently fails to match `{self}`" — a player with matching `stat_templates`
        now *does* resolve it; update so the note doesn't actively mislead post-feature
  - [ ] `CLAUDE.md`'s "Per-player action bars (Phase 2)" paragraph (the `SlotCost`
        deliberately-global note)
  - [ ] `CLAUDE.md`'s "four player-construction sites" section — record that
        `spawn_player_entity_core` now conditionally inserts `StatMap`, the same class of
        "every player gets X component" note that section exists to track
  - [ ] A paired RON example: the player prefab's `stat_templates` block **and** the cost slot
        that consumes it, shown together, with an explicit callout — *"omit the `stat_templates`
        block on this player's prefab and `cost` silently falls back to the shared global pool,
        the exact behavior this feature exists to fix."*

## Acceptance criteria
- Given two split-screen players, each with their own `ActionBar` and a `cost:`-gated slot
  referencing a stat declared in their own player prefab's `stat_templates`, when player 1 fires
  their ability, then only player 1's pool is deducted and player 2's slot/cooldown-overlay dim
  state is unaffected.
- Given a single-player project, or any player prefab with no `stat_templates` declared, with a
  `cost:`-gated action-bar slot, when the ability fires, then behavior is unchanged from before
  this feature (global `LoadedStats` deduction).
- Given a player's stat pool regenerates via `regen_rate`, when time passes, then the action bar's
  cooldown-overlay dim clears once that player's own pool (not the global one) crosses the cost
  threshold.
