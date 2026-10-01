---
name: perf-priority-list
description: My top-5 perf wishlist in planning/stakeholder_priority_list.md; refreshed 2026-10-01 — still zero of the original 5 shipped; message_interpreter part of item 2 obsoleted by its removal
metadata:
  type: project
---

Original top-5 (snapshot db1ede0, 2026-09-03): terrain first-frame stall, per-frame collection
allocs, scene-transition material cache, frozen-clip evaluation, `format!`-before-guard.

**Status at 2026-10-01 refresh (209 commits later): none shipped.** All five have backlog entries in
`planning/backlog.md` ▸ Performance (frozen clips was promoted there 2026-09-14). Item 2 *changed*:
`message_interpreter_system` no longer exists (removed by rules_to_state_machine_consolidation), so
its Vec is gone; remaining offenders are `stat_modifier_system`'s per-entity key `Vec<String>`
clone (stats.rs ~line 19, always allocates when a StatMap is non-empty) and `player_movement_system`'s
input HashMap (FixedUpdate; `HashMap::new()` is alloc-free on idle ticks, allocates only on ticks
with input). The backlog entry text still names the removed system — stale wording.

Refreshed 2026-10-01 ranking: 1 terrain stall, 2 pipeline warmup for 2D/UI/Sprite (backlog
"Extend pipeline warmup to Text2d and UI" + claude_suggestions Sprite-warmup note — Textured
`Sprite` bars now ship on 3rd_person_game_demo players), 3 frozen clips, 4 material cache,
5 per-frame alloc trio (stat_modifier Vec + format!-before-guard).

**Why:** the Sept window went to CLI hardening, rules→state_machine consolidation, fixed timestep,
action-bar features — authoring/correctness work, not perf. Not neglect of a known regression.

**How to apply:** when a lull appears, propose frozen clips first (smallest diff). Re-read
backlog.md before claiming status; do not trust this snapshot. Binary size is a non-issue
(see [[wasm-size]]; pkg release 32,090,055 bytes on 2026-09-29).
