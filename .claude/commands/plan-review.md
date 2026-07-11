Review a feature's plan doc before any code is written — not the implementation (use `/code-review` for that).

Feature or context: $ARGUMENTS

1. **Locate the plan** — Find `planning/features/{name}.md`. If it doesn't exist for a non-trivial change (new schema fields, new event/action types, cross-capability changes, or anything where the approach is unclear), stop and say so: copy `planning/features/_template.md` first.

2. **Completeness check** — Confirm it has `Planned at: <hash> (<YYYY-MM-DD>)`, a concrete approach, any schema changes called out, and RON authoring examples. Flag anything vague enough that it needs more input or a decision from Frank before coding starts.

3. **Review in parallel** (single message, multiple tool calls):
   - **system-architect** — goal/architecture fit: does this belong in the data-driven pipeline as-is, or does it need a new primitive? What's the minimal architectural footprint? Flag risks — schema breaking changes, WASM/perf implications, capability coupling, determinism.
   - **ux-gamedesigner-reviewer** — UX/design fit: will a non-programmer designer be able to use this once built? Is the planned RON surface consistent with existing patterns and documentable?

4. **Consolidated verdict**:
   - **Ready** — plan is complete, goal-aligned, and UX-sound. Set the plan file's `_Status:_` line
     to `Ready`. Do **not** move the backlog item to `## Active` here — that happens in step 2 of
     the Code change workflow (`CLAUDE.md`), at the moment the `feature/{slug}` branch/worktree is
     actually created, not at plan-review time. Moving it here caused a real merge conflict once
     (2026-07-11/12): the plan-review edit landed on `integration` before any branch existed, and
     the feature branch — cut from a `main` that predates that edit — made the same edit
     independently, so they collided on merge.
   - **Needs more design work** — list the specific open questions for Frank, grouped by reviewer.

Do not proceed to code changes until the plan reaches **Ready**.
