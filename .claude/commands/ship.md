Walk through the ironhold-lib 12-step code change workflow for the current feature. Execute each step in order, report its status, and do not skip steps.

Steps:

1. **Feature plan complete** — Check whether the relevant feature file exists in `planning/features/` and is filled in (approach, schema changes, RON examples). If there is no plan doc for a non-trivial change, flag it and stop.

2. **Feature is Active in backlog** — Read `planning/backlog.md` and confirm the item is listed under `## Active`. If it is still Queued or Icebox, move it to Active and commit before continuing.

3. **Code changes implemented** — Confirm the implementation is complete. Ask the user to describe what was changed if it is not clear from context.

3b. **Alignment review** — Invoke the `alignment-reviewer` agent to verify the change follows the data-driven design philosophy: designer-reachable from RON without recompiling, no hardcoded asset paths, no capability pushing directly to ActionQueue.

4. **Tests pass** — Run:
   ```
   cargo test -p ironhold_core --test integration_tests --test ron_validation --test ron_lint
   ```
   All must pass. Fix any failures before continuing.

5. **Docs updated** — Check that `docs/20_data_formats.md` and any relevant `CLAUDE.md` files reflect the changes. New schema fields, new action types, and new events each need a doc entry. Also check `crates/ironhold_core/src/CLAUDE.md` for capability-level notes.

5b. **UX review** — If any files in `assets/`, `docs/`, or schema RON files changed, invoke the `ux-gamedesigner-reviewer` agent to verify the designer experience is clear, documented, and consistent.

6. **Schema/CLI check** — If any file in `crates/ironhold_core/src/schema/` was modified, run:
   ```
   cargo check -p ironhold_cli
   ```
   Also verify `query actions` and `query events` output still formats correctly if `Action` or event types changed.

7. **WASM dev build** — Run:
   ```
   wasm-pack build crates/ironhold_web --target web --out-dir ../../pkg --dev
   ```
   Report the size of `pkg/ironhold_web_bg.wasm`. Warn if ≥ 95 MB.

8. **Play-test checklist** — Provide a concrete checklist for Frank to verify the feature in the browser: which project to load, what to interact with, what to look for. Include golden path and at least one edge case.

9. **Await play-test confirmation** — Stop here. Do not proceed to step 10 until Frank explicitly confirms the feature works in the browser.

   If Frank reports a bug or regression during play-testing:
   - Return to **step 3** (implement the fix)
   - Re-run **steps 4 → 5 → 6 → 7** (tests, docs, schema check, dev WASM build)
   - Then return to **step 8** (updated play-test checklist) and **step 9** (await confirmation again)
   - Repeat this loop until Frank confirms all is well.

10. **WASM release build** — Run:
    ```
    cargo clean && wasm-pack build crates/ironhold_web --target web --out-dir ../../pkg
    ```
    Report binary size. If ≥ 95 MB, warn Frank clearly — GitHub Pages hard-blocks at 100 MB.

11. **Move feature to Done** — In `planning/backlog.md`, mark the item `[x]` and move its feature file from `planning/features/` to `planning/features/done/` if one exists.

12. **Commit** — Stage all changed files and commit with a descriptive message in conventional-commit format. Include a summary of what changed and why.

After all 12 steps are complete, propose the next feature to activate from the backlog `## Queued` section.
