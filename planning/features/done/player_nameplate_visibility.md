# Feature: Player nameplate visibility — Player marker + own-nameplate toggle

_Status: Done_
_Planned at: `48889f1` (2026-07-03)_

## Phases

| Phase | Backlog item | Status | Completed |
|---|---|---|---|
| v1 | `Player`/`PlayerOwnership` marker + `show_player_nameplate` scene field + close the `action_executor.rs:163` bug | Done | 2026-07-03 |
| v2 | Runtime `ToggleOwnNameplate` action (player preference, independent of scene authoring) | Done | 2026-07-03 |

## What

Today, nameplate visibility for "not an NPC" entities is governed by `faction_filter: FriendlyOnly`, which really means "lacks `NpcAgent`" — this silently lumps the player in with any other non-NPC friendly entity (props, decorative characters). There is no explicit `Player` marker component anywhere in the engine; `CharacterController` presence is used ad-hoc across ~6 capabilities to informally mean "this is the player." This feature adds a real `Player` marker (carrying a `PlayerOwnership` field, `Local` or `Remote`, defaulting `Local`) and an orthogonal scene-level `show_player_nameplate: bool` field (default **false**) so a project can independently control "does my own character show a nameplate" without touching the existing `show_nameplates`/`faction_filter` NPC-facing controls. It also closes the known bug where the character-select dynamic player-spawn path (`action_executor.rs:163`) ignores `show_nameplates` entirely.

v2 adds a `ToggleOwnNameplate` action so a player can flip their own nameplate visibility at runtime as a personal preference (mirrors the existing `ToggleMute` pattern), independent of the scene-authored default.

## Why

Prompted by Frank asking whether `show_nameplates` should split into `show_npc_nameplates` + `show_player_nameplates`, and how that should work once multiplayer lands. Consulted `system-architect` and `game-world-designer` before deciding the shape:

- **Architect**: a 2-way NPC/Player split is the wrong move — `show_nameplates` already *is* the NPC/world toggle, so `show_npc_nameplates` would be a rename with no gain. The actual gap is the missing `Player` marker (today `faction_filter: FriendlyOnly` wrongly includes the player). Recommended adding the marker (with `PlayerOwnership::{Local, Remote}` as a cheap multiplayer forward-compat hook) plus an orthogonal `show_player_nameplate` field — no migration, `show_nameplates` untouched.
- **Game designer**: the category that matters to players is *relationship to me* (self / other players / NPCs), not entity type. Recommended `show_own_nameplate` defaulting **off**, per genre convention (WoW/GW2/ESO all hide your own plate — it only occludes your view of your own character with no informational payoff). `show_other_player_nameplates` should be reserved as a dormant category for when multiplayer exists, rather than guessed at now.
- These converge cleanly: `PlayerOwnership::{Local, Remote}` is exactly the mechanism that later makes "own vs. other players" real — today every player is `Local`, so `show_player_nameplate` **is** "show own nameplate" until Beta 0.6 (LAN co-op) introduces `Remote` players, at which point `show_other_player_nameplates` can be added alongside it without another schema break.
- **Frank's decision**: `show_player_nameplate` defaults to **false** (industry convention wins), not "inherit from `show_nameplates`." `3rd_person_game_demo`'s player prefabs keep showing their nameplate today via the existing per-prefab `nameplate: Some(true)` override — this feature does not change that project's shipped, play-tested behavior, only the engine-wide default for *new* projects that don't set anything.

## Approach

**New `Player` marker + `PlayerOwnership` component** (home: alongside other spawn-metadata types in `scene_manager/mod.rs`, inserted via `tag_spawned_entity` so all spawn paths get it automatically — GLB actor/prop, primitive, composite, dynamic `Action::Spawn`):
```rust
#[derive(Component)]
pub struct Player;

#[derive(Component, Clone, Copy, PartialEq, Default)]
pub enum PlayerOwnership {
    #[default]
    Local,
    Remote, // unused until multiplayer; reserved now to avoid a later breaking change
}
```
Inserted only for entities with the `"player"` tag (mirrors the existing tag check in `action_executor.rs` and `scene_loader.rs` player-spawn branches) — not a new designer-facing RON field, purely an ECS-internal signal.

**`faction_filter` fix**: `nameplate_visibility_system`'s `HostileOnly`/`FriendlyOnly` check switches from `With<NpcAgent>` absence to an explicit `Without<Player>` check (or equivalent), so "FriendlyOnly" no longer accidentally includes the player.

**New scene field** — `NameplateOptionsDef.show_player_nameplate: bool` (`#[serde(default)]` = `false`). Orthogonal to `show_nameplates`/`faction_filter`, which continue to govern NPCs and other non-player entities unchanged.

**Gating**: for entities with the `Player` marker (and `PlayerOwnership::Local`), `should_insert_nameplate`'s `show` argument becomes `nameplate_options.show_player_nameplate` instead of `scene.show_nameplates`. For all other entities, nothing changes. The per-prefab `nameplate: Option<bool>` tri-state override continues to apply uniformly on top, as today.

**Bug fix**: `action_executor.rs:163` (character-select dynamic player-spawn) is rewired to call `should_insert_nameplate(prefab_def.nameplate, show_player_nameplate)` instead of its current truncated `nameplate != Some(false)` check — closing the `planning/backlog.md` Bugs entry "Character-select player nameplate ignores `show_nameplates`."

**v2 — `ToggleOwnNameplate` action**: a new resource (`PlayerNameplatePreference(bool)`, mirroring `AudioState`'s shape) initialized from `show_player_nameplate` at scene load. `nameplate_visibility_system` (which already re-evaluates `Visibility` every frame for faction/distance filtering) additionally checks this preference for `Player` entities with no per-prefab override — no change needed to spawn-time `NameplateTag` insertion, since visibility toggling happens in the existing per-frame system. `Action::ToggleOwnNameplate` flips the resource; executor pushes `nameplate.own_shown`/`nameplate.own_hidden` for RON-side label/icon updates, mirroring the `ToggleMute` → `audio.muted`/`audio.unmuted` pattern.

**Multiplayer (explicitly out of scope for this feature)**: `show_other_player_nameplates` and any `Remote`-player handling are deferred until Beta 0.6's library spike defines real requirements — building them now would be guessing at a shape with no replication code to validate against.

## Approach — v1 deviations from plan (as shipped)

Implementation surfaced two things the design above didn't anticipate, both handled during the same pass rather than deferred:

- **A 6th, previously-undiscovered nameplate-gating site.** The "Primitive player" scene-placed spawn block in `scene_loader.rs` (~line 774) had its own inline copy of the old predicate under differently-named local variables (`np_override`/`np_display_name`), so it was missed entirely by the earlier `should_insert_nameplate` extraction refactor (which was grepped for `prefab.nameplate`-style names). Found and routed through `should_insert_nameplate(np_override, show_player_nameplate)` alongside the other sites, plus given the `Player`/`PlayerOwnership` marker insertion the plan called for.
- **Two additional per-entity gates were required, not just the `faction_filter` tweak.** `Player`/`PlayerOwnership` alone wasn't sufficient:
  - `nameplate_setup_system`'s own redundant `!config.enabled && prefab_override != Some(true)` re-check (independent of the spawn-time gate) would have silently re-suppressed a correctly-tagged player nameplate whenever `show_nameplates` was `false`, since it only knew about `config.enabled`, not `config.player_enabled`. Fixed by adding `Option<&Player>` to its query and picking the right config field per entity. `NameplateSceneConfig` gained a `player_enabled: bool` field for this (parallel to `enabled`).
  - `nameplate_visibility_system`'s default `faction_filter: HostileOnly` would otherwise force-hide the player's plate every frame (no `NpcAgent` = fails the filter), regardless of `show_player_nameplate`. Rather than special-casing `FriendlyOnly` alone (the plan's original framing), `Player` entities now bypass `faction_filter` entirely — treated the same as an explicit `Some(true)` prefab override (distance-only gating). This is correct for all three filter variants, not just `FriendlyOnly`.
- `SpawnParams` (the bundled `SystemParam` used to stay within Bevy's 16-param limit) gained a `nameplate_config: Res<NameplateSceneConfig>` field so `action_executor.rs`'s `Action::Spawn` arm could read `player_enabled`.

## Approach — v2 deviations from plan (as shipped)

- **Two distinct events, not one `nameplate.own_toggled`.** The plan originally sketched a single generic toggle event; shipped code instead emits `nameplate.own_shown`/`nameplate.own_hidden`, exactly mirroring `ToggleMute`'s `audio.muted`/`audio.unmuted` shape. Alignment review confirmed this is the correct pattern — two semantic events let a `global_on` RON bridge map each to a distinct `SetVariable` without conditional logic, matching every other two-state toggle in the engine.
- **The v1 `Some(true) || player_q.contains(entity)` bypass branch had to be split in two**, not left combined: an explicit per-prefab override must ignore the runtime preference entirely (always wins, distance-only), while a `Player` entity with *no* override is gated by `PlayerNameplatePreference`. Combining them would have made the runtime toggle override an explicit designer `nameplate: Some(true))`, which is backwards from the intended precedence.
- **A real regression surfaced during verification**: `nameplate_visibility_system` gaining a `Res<PlayerNameplatePreference>` dependency broke the v1 test `test_nameplate_visibility_player_bypasses_faction_filter`, which constructs `NameplateSceneConfig` directly (bypassing `spawn_scene_v2`) and therefore never seeded the new resource — it sat at its `Default` of `false`, hiding the player entity the test expected to see. Fixed by having that test explicitly seed `PlayerNameplatePreference(true)`, isolating the faction-filter-bypass behavior it actually tests from the newly-added, separately-tested toggle behavior.
- **`cargo check -p ironhold_cli` caught a real non-exhaustive-match compile error** (`E0004`) in `query.rs::action_kind`, exactly the failure mode the project's mandatory CLI check exists to catch for new `Action` variants. Fixed by adding the missing arm.

## Tasks
- [x] Add `Player` marker + `PlayerOwnership` component; wire into `tag_spawned_entity`-adjacent spawn sites for player entities (`spawn_player_entity`, primitive-player block)
- [x] Fix `nameplate_visibility_system`'s `faction_filter` check — `Player` entities bypass it entirely (all three variants), not just a `FriendlyOnly` tweak
- [x] Add `NameplateOptionsDef.show_player_nameplate: bool` (default `false`)
- [x] Route all 3 actual player-nameplate-gating call sites (`scene_loader.rs` ×2 — GLB + primitive scene-placed, `action_executor.rs` ×1 — dynamic character-select) through `show_player_nameplate` instead of `show_nameplates`/`nameplate_config.enabled`
- [x] Fix `action_executor.rs:163` to use `should_insert_nameplate`; close the backlog bug entry
- [x] `ron_validation.rs` — 2 parse tests for the new `show_player_nameplate` field (default + explicit)
- [x] Domain test coverage (`nameplate_tests.rs`) — 3 new tests: `player_enabled`-vs-`enabled` gating (both directions) + faction_filter bypass
- [x] Update `docs/20_data_formats.md` and `crates/ironhold_core/src/CLAUDE.md` nameplate section
- [x] **v2**: `Action::ToggleOwnNameplate`, `PlayerNameplatePreference` resource, `nameplate.own_shown`/`nameplate.own_hidden` events, docs + tests
- [x] v1 alignment review — ALIGNED, no blocking issues
- [x] v1 `wasm-perf-reviewer` — negligible per-frame cost, no concerns (added since both touched systems run every frame/per-spawn)
- [x] v1 `ux-gamedesigner-reviewer` — no blockers; two doc-clarity gaps found and fixed (two-toggle callout, `player_warrior` example annotation)
- [x] v1: Full `ironhold_core` test suite (200+ tests) + `cargo check -p ironhold_cli` — all green
- [x] v1: WASM dev build + play-test confirmed; WASM release build (58 MB) + smoke-test confirmed
- [x] v2 alignment review — ALIGNED, no blocking issues
- [x] v2 `wasm-perf-reviewer` — negligible cost (one extra bool check per nameplated entity per frame), no concerns
- [x] v2 `ux-gamedesigner-reviewer` — no blockers; three doc-clarity gaps found and fixed (scene-reset promoted to its own warning callout, events reference table added, override-precedence "toggle does nothing visible" nuance documented)
- [x] v2: 6 new `nameplate_tests.rs` tests + 1 `ron_validation.rs` parse test; fixed 1 v1 test regression; `cargo check -p ironhold_cli` (caught and fixed a real missing-match-arm compile error) + `query actions` CLI spot-check — all green
- [x] Worked example added to `3rd_person_game_demo`: a "Toggle Nameplate" HUD button wired through `state_machine.ron` to `Action::ToggleOwnNameplate`. This required converting `player_male`/`player_female`'s hard per-prefab `nameplate: true` override into the scene-level `show_player_nameplate: true` default (same default visual result — nameplate shown — but now actually toggleable; a `Some(true)` override would have made the button a permanent no-op). Closes both v1's and v2's "no shipped example" gap in one pass. Validated via `ironhold_cli validate`, dev + release WASM play-tested and confirmed.

## Open questions
- Should `PlayerOwnership::Remote` handling (or the `show_other_player_nameplates` field) be stubbed now even though nothing sets it yet, or added only when Beta 0.6 actually lands? Leaning toward **not stubbing** — an enum variant with zero call sites is dead code with no test coverage possible today.
- (raised by v2 alignment review, non-blocking) `PlayerNameplatePreference` resets on every scene transition rather than persisting for the session (unlike `AudioState`'s mute toggle). Deliberate scoping choice to keep v2 simple and match `player_enabled`'s own per-scene-authored behavior — revisit only if this becomes a real reported UX complaint.

## Acceptance criteria
- A project that sets neither `show_nameplates` nor `show_player_nameplate` shows no nameplates on the player and behaves exactly as before for NPCs.
- A project with `show_nameplates: true` and `show_player_nameplate: false` (or unset) shows NPC nameplates but not the player's own.
- A project with `show_player_nameplate: true` shows the player's own nameplate regardless of `show_nameplates`.
- `3rd_person_game_demo`'s existing `nameplate: true` per-prefab override on `player_male`/`player_female` continues to force-show their nameplate, unaffected by the new default.
- The character-select dynamic player-spawn path (`action_executor.rs:163`) now respects `show_player_nameplate` identically to the scene-placed player path — the tracked bug is closed.
- `Action::ToggleOwnNameplate` flips the local player's nameplate visibility at runtime without affecting NPC or (future) other-player nameplates, and without affecting a player prefab that has an explicit `nameplate: Some(true)`/`Some(false)` override.
- The preference resets to `show_player_nameplate` on the next scene load (documented behavior, not a bug).
