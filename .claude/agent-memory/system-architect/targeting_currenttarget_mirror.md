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
- **Divergence gap — FIXED.** `Action::SetTarget`/`ClearTarget` (`action_executor.rs`) now route through the primary player's `PlayerTarget` via `apply_player_target`/`clear_player_target` (the same helpers `targeting.rs` uses), and only fall back to writing `CurrentTarget.0`/vars directly when there is no player entity in the scene at all (e.g. a menu). A code comment at the `SetTarget` arm explicitly documents why: "otherwise the ring (target_indicator_system) and target_auto_clear_system, both now driven by PlayerTarget, would never react to a rule-driven SetTarget." So the ring/auto-clear regression this section warned about is closed.
- "is this multiplayer?" is computed three different ways (`player_targets` count without CharacterController filter in click_select; `With<CharacterController>` count in tab/indicator) — identical sets today, but standardize if touched.
- Camera→owning-player resolution: split/Grid cameras have `OrbitCamera` (target = player); `PartyOrbitCamera` has none → falls back to primary. Robust across all modes because click_select pre-filters to `is_active` cameras.
