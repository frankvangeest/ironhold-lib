---
name: perf-priority-list
description: My top-5 perf wishlist in planning/stakeholder_priority_list.md; 3rd edition 2026-10-13 — still zero of the original perf items shipped; canvas pixel budget is new #1
metadata:
  type: project
---

Original top-5 (snapshot db1ede0, 2026-09-03): terrain first-frame stall, per-frame collection
allocs, scene-transition material cache, frozen-clip evaluation, `format!`-before-guard.

**Status at 3rd edition (HEAD 929b7d1, 2026-10-13, 213 commits after ce77bdd): still none shipped.**
Oct window went to docs/CLAUDE.md split, OpenCode/GLM tooling, flex Group, responsive UI, SetUiVisible.
All original items still in `planning/backlog.md` ▸ Performance (the per-frame-collections entry
still names the deleted `message_interpreter_system`). stats.rs has the key-Vec clone TWICE
(:19 modifier, :118 threshold system).

3rd-edition ranking: 1 cap total physical pixels of the fitted canvas (responsive_ui Phase 4,
see [[responsive-canvas-render-cost]]), 2 terrain stall, 3 2D/UI/Sprite warmup, 4 frozen clips,
5 warmup must only strip the NoFrustumCulling it added (lib.rs ~495) + ocean-demo perf gates.
Material cache and alloc trio demoted (still open).
2026-10-13: #1 (canvas pixel cap) and #5 (warmup WarmupNoCull marker) implemented on feature/prerelease_render_fixes
(reviewed, not yet merged at time of writing) — verify on integration before calling them shipped.

**Why:** authoring/tooling work dominates; perf items get no lull. Responsive UI shifted the worst
case to large desktops.

**How to apply:** re-read backlog.md before claiming status; do not trust this snapshot. pkg/ release
was built at ebc338c (2026-10-10), BEFORE responsive UI landed — the canvas regression is not live
until the next release. Binary size non-issue (32,161,056 bytes); see [[wasm-size]].
