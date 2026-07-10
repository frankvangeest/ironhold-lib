Run a full pre-commit review of the current feature's code changes (implementation, not the plan — use `/plan-review` for the feature plan).

Feature or context: $ARGUMENTS

Launch the following agents **in parallel** (single message, multiple tool calls), passing the feature name/context to each. `alignment-reviewer`, `system-architect`, and `debug-detective` always run; the rest are conditional — run each only when its trigger applies, and note in the summary which ones you skipped and why.

1. **alignment-reviewer** _(always)_ — Verify data-driven design compliance. Can a game designer use this feature entirely from RON without recompiling? Check for hardcoded asset paths, unreachable schema types, and pipeline violations.

2. **system-architect** _(always)_ — Verify architectural integrity and long-term maintainability. Check crate boundaries, the Message→Interpreter→Action→Executor pipeline, schema stability, and capability coupling.

3. **debug-detective** _(always)_ — Adversarially review the diff for latent bugs and edge cases the implementer might have missed, not just symptoms already reported.

4. **ux-gamedesigner-reviewer** _(if `assets/`, `docs/`, or RON schema changed)_ — Verify designer UX and documentation completeness. Are assets/, docs/, and RON files clear and usable for a non-programmer? Check for missing doc entries, inconsistent naming, and undocumented fields.

5. **wasm-perf-reviewer** _(if runtime systems, rendering, render/update hot path, asset-loading, per-frame work, new dependencies, or per-frame-driving schema changed)_ — Verify there are no WASM frame-time or binary-size regressions. Check for unconditional per-frame allocation/work, hot-path costs, and dependency/binary-size impact.

Once the agents complete, evaluate every finding individually: recommend fixing it now, or logging it as its own `planning/backlog.md` item / `planning/claude_suggestions.md` entry if it's non-blocking. Then produce a consolidated summary:

- **Ready to ship** — if all invoked agents give no blocking issues
- **Needs fixes** — list all blocking issues across the reviews, grouped by severity
- **Warnings** — list all warnings, noting which agent flagged each
- **Skipped** — list any conditional agents not run and the reason (e.g. "wasm-perf-reviewer skipped — no runtime/perf surface")

If there are blocking issues, recommend which ones to fix before proceeding to WASM build and which can be deferred to a follow-up.
