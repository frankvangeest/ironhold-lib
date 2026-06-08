Run a full pre-commit review of the current feature changes.

Feature or context: $ARGUMENTS

Invoke the following agents in order, passing the feature name/context to each. `alignment-reviewer` always runs; the rest are conditional — run each only when its trigger applies, and note in the summary which ones you skipped and why.

1. **alignment-reviewer** _(always)_ — Verify data-driven design compliance. Can a game designer use this feature entirely from RON without recompiling? Check for hardcoded asset paths, unreachable schema types, and pipeline violations.

2. **ux-gamedesigner-reviewer** _(if `assets/`, `docs/`, or RON schema changed)_ — Verify designer UX and documentation completeness. Are assets/, docs/, and RON files clear and usable for a non-programmer? Check for missing doc entries, inconsistent naming, and undocumented fields.

3. **system-architect** _(if architectural / schema / capability / crate-boundary / WASM-compat changes)_ — Verify architectural integrity and long-term maintainability. Check crate boundaries, the Message→Interpreter→Action→Executor pipeline, schema stability, and capability coupling.

4. **wasm-perf-reviewer** _(if runtime systems, rendering, render/update hot path, asset-loading, per-frame work, new dependencies, or per-frame-driving schema changed)_ — Verify there are no WASM frame-time or binary-size regressions. Check for unconditional per-frame allocation/work, hot-path costs, and dependency/binary-size impact.

After the relevant agents complete, produce a consolidated summary:

- **Ready to ship** — if all invoked agents give no blocking issues
- **Needs fixes** — list all blocking issues across the reviews, grouped by severity
- **Warnings** — list all warnings, noting which agent flagged each
- **Skipped** — list any conditional agents not run and the reason (e.g. "wasm-perf-reviewer skipped — no runtime/perf surface")

If there are blocking issues, recommend which ones to fix before proceeding to WASM build and which can be deferred to a follow-up.
