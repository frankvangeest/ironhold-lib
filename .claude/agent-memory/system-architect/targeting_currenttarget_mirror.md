---
name: targeting-currenttarget-mirror
description: Per-player targeting design — CurrentTarget is the primary player's PlayerTarget mirror; SetTarget/ClearTarget actions diverge from it
metadata:
  type: project
---

Per-player split-screen targeting (Phase 1, `planning/features/per_player_split_screen_targeting.md`) added `PlayerTarget(Option<String>)` per player entity (`capabilities/player.rs`) and kept `CurrentTarget` (resource, `capabilities/action_bar.rs`) as "the primary player's `PlayerTarget`, mirrored." Primary = `PlayerIndex(0)` or no `PlayerIndex`.

**Why:** Full per-player gameplay-action execution (Phase 2) needs player identity threaded through the whole Message→Interpreter→Action→Executor pipeline — deliberately out of Phase 1 scope. So only the primary player's selection reaches `{target}` substitution + the action-bar cost gate.

**How to apply:**
- The mirror is **one-directional**: `apply_player_target`/`clear_player_target` in `targeting.rs` write PlayerTarget then mirror into CurrentTarget when primary. Anything that writes `CurrentTarget` directly desyncs it.
- **Known divergence gap (flag on any targeting work):** `Action::SetTarget`/`ClearTarget` (`action_executor.rs`) write `CurrentTarget.0` directly and never touch the primary player's `PlayerTarget`. Since the ring (`target_indicator_system`, keyed on `Changed<PlayerTarget>`) and `target_auto_clear_system` (loops PlayerTarget) both moved off CurrentTarget, a rule/dialogue-driven `SetTarget` no longer spawns a ring or gets auto-cleared — a silent regression even in single-player. Fix = route those actions through the primary player's PlayerTarget. As of the Phase 1 review this was unfixed and untested (the `test_target_indicator_spawns_on_set_target...` test was rewritten to mutate PlayerTarget directly rather than dispatch the action).
- "is this multiplayer?" is computed three different ways (`player_targets` count without CharacterController filter in click_select; `With<CharacterController>` count in tab/indicator) — identical sets today, but standardize if touched.
- Camera→owning-player resolution: split/Grid cameras have `OrbitCamera` (target = player); `PartyOrbitCamera` has none → falls back to primary. Robust across all modes because click_select pre-filters to `is_active` cameras.
