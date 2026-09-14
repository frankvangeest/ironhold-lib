---
name: perf-priority-list
description: My top-5 perf wishlist lives in planning/stakeholder_priority_list.md (snapshot db1ede0, 2026-09-03); status as of 2026-09-14 review — zero shipped
metadata:
  type: project
---

My top-5 web-perf wishlist is recorded in `planning/stakeholder_priority_list.md` ▸ "WASM-Perf-Reviewer":
terrain first-frame stall, per-frame collection allocs, scene-transition material cache, frozen-clip
evaluation, `format!`-before-guard. Four of the five map to `planning/backlog.md` ▸ Performance.

**Item 4 (paused/frozen animation clips still fully evaluated) has never been logged in
`backlog.md`** — it exists only in the priority list and in [[animation-hot-path]]. It is the
cheapest of the five (drop `AnimationGraphHandle` when a clip freezes) and the one most likely to be
forgotten because it has no backlog entry to be triaged.

**Why:** Frank asked for a stakeholder progress check on 2026-09-14; the two-week window since the
snapshot went entirely to `ironhold_cli` validate hardening (build-time tooling), so nothing on the
perf list moved. That is a deliberate authoring-correctness push, not neglect of a known regression.

**How to apply:** When Frank asks "what should we do next" or a lull appears between feature
batches, propose item 4 first (smallest diff, no new cfg branches, self-contained in the animation
capability). Do not re-derive backlog status from this note — re-read `backlog.md`; check whether
item 4 has since been promoted before claiming it is unlogged. Binary size remains a non-issue
(see [[wasm-size]]).
