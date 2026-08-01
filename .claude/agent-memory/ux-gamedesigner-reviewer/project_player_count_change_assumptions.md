---
name: player-count-change-assumptions
description: Live player-count changes (hot join/leave) break two hidden assumptions — seat index vs viewport slot, and "2+ players" gating evaluated as if count were constant per scene
metadata:
  type: project
---

Two engine assumptions predate live player-count changes and keep biting any join/leave feature:

**1. Seat index (`PlayerIndex`) vs viewport slot (`SplitViewportSlot`) are conflated at join time.**
`Action::JoinPlayer` computes `next_slot = ActiveSplitSlotCount + queued_hot_joins` and uses that
one number for three different things: the viewport slot, the `PlayerIndex`, and the
`join_prefab_keys[..]` lookup. That is only safe while player count never *decreases*. The moment
a leave exists, a middle-seat departure frees `PlayerIndex 1` while the count drops to N-1 — the
next join then reuses an index a survivor still holds (duplicate ring tint, duplicate "P{n}" HUD
label, duplicate `ActionBar.owner_player`, duplicate keyboard scheme, and the
`own_viewport_only` duplicate-`player_index` warn). Correct shape: join should claim the **lowest
free seat index** and index `join_prefab_keys` by that seat; slot renumbering stays separate.
Spawn-point lookup (`player_{n+1}_start`) has the same seat-vs-slot ambiguity.

**2. The "2+ players present" gating family is written as if count is fixed for a scene's
lifetime.** Ring tinting (per-player palette vs per-target color), `target_display`/`target_name`/
`target_id` blanking, and per-viewport `target_hud` all branch on a live `CharacterController`
count, but several apply their effect only at *spawn/target-switch* time. Crossing 2→1 live (only
possible once leave exists) leaves a live ring still player-tinted while the vars un-blank —
mixed state with no scene reload to resolve it. Widget rank duplication
(`stat_label`/`world_stat_bar`/`world_labels`/popups) is the exception: it re-derives from the
live active-camera list each frame and self-corrects.

**How to apply:** on any plan that changes live player count, require an explicit answer for
(a) which number is the seat vs the layout slot, and (b) what happens at the 1↔2 threshold
crossing. See [[hot-join-input-prefab-coupling]], [[per-player-targeting-gating]],
[[player-index-owner-player-wiring]].
