---
name: dev-tooling-feature-review
description: How to scope an alignment review of a dev-tooling-only change (tools/*.py, .opencode/, CLAUDE.md maintenance docs) where designer-reachability does not apply
metadata:
  type: feedback
---

For dev-tooling-only changes (first seen: feature/opencode_compat_probe, 2026-10-08) the caller narrows the review to:
designer-leak check, project-rule bookkeeping, and plan-vs-built drift. Reach for these checks:

- `docs/README.md`: new dev doc must be a row with a "Developer (...)" Audience, below the designer rows; the bold
  "Making a game" opener (lines 3-7) must not mention it.
- Root `CLAUDE.md`: budget-conscious (about 14.8k tokens, every session) so at most one bullet; the "Updating
  documentation" list is file-list shaped, an imperative bullet there reads oddly but was the plan's chosen spot.
- `planning/claude_suggestions.md` entries: `_(observed at `<hash>` <date>, <source>)_` header line, body indented.
- Plan acceptance criteria phrased as "each row's re-verify cell is a command that can be pasted" are easy to half-meet:
  grep the table for prose cells ("the probe prints...", "probe a file under a temporary...").
- Plan "Open questions" sections are often left stale after implementation notes settle them.
- No shell tool in this agent: read the worktree files directly instead of `git diff`.
- Model-routing changes (feature/opencode_glm_driver_pilot, 2026-10-09): cross-check root CLAUDE.md wording against
  `.opencode/opencode.json` top-level `model` AND the `.opencode/README.md` facts table (V-rows); fact rows describing
  "the current default" go stale when the tiers table is updated. "Free subagents" wording omits the paid `-deep` twins.

**Why:** these reviews are cheap only if scoped; designer-reachability checklists produce noise here.
**How to apply:** when the caller says "no engine code, no schema, no RON", use this list, not the full methodology.
Related: [[dev-doc-split-designer-leak-audit]].
