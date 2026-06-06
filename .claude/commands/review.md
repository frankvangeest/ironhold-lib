Run a full pre-commit review of the current feature changes.

Feature or context: $ARGUMENTS

Invoke the following three agents in order, passing the feature name/context to each:

1. **alignment-reviewer** — Verify data-driven design compliance. Can a game designer use this feature entirely from RON without recompiling? Check for hardcoded asset paths, unreachable schema types, and pipeline violations.

2. **ux-gamedesigner-reviewer** — Verify designer UX and documentation completeness. Are assets/, docs/, and RON files clear and usable for a non-programmer? Check for missing doc entries, inconsistent naming, and undocumented fields.

3. **system-architect** — Verify architectural integrity and long-term maintainability. Check crate boundaries, WASM compatibility, schema stability, and capability coupling.

After all three agents complete, produce a consolidated summary:

- **Ready to ship** — if all three give no blocking issues
- **Needs fixes** — list all blocking issues across all three reviews, grouped by severity
- **Warnings** — list all warnings, noting which agent flagged each

If there are blocking issues, recommend which ones to fix before proceeding to WASM build and which can be deferred to a follow-up.
