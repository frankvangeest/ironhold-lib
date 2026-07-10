Walk through the ironhold-lib code change workflow for the current feature. Execute each step in order, report its status, and do not skip steps. Steps marked _(conditional)_ run only when their trigger applies — say so explicitly when you skip one.

Steps 1–10 happen **on a `feature/{slug}` branch**, in its own git worktree — parallelizable, one per feature. Steps 11–17 happen **on `integration`**, once per batch of merged feature branches, not once per feature. See root `CLAUDE.md` → Branching Model for the full branch-tier reference.

## On a feature branch

1. **Feature plan complete** — Check whether the relevant feature file exists in `planning/features/` and is filled in (approach, schema changes, RON examples). If there is no plan doc for a non-trivial change, flag it and stop. Equivalent to running `/plan-review`.

2. **Create the feature branch + worktree, mark it Active in the backlog, commit before coding:**
   ```
   git worktree add ../ironhold-lib-{slug} -b feature/{slug} main
   ```
   Read `planning/backlog.md`, move the item to `## Active` if it isn't already, and commit that change before writing code.

3. **Code changes implemented** — Confirm the implementation is complete. Ask the user to describe what was changed if it is not clear from context.

4. **Parallel code review + tests** — Equivalent to running `/code-review` alongside the test suite. Launch in a single message (multiple tool calls, so everything runs concurrently):
   - `alignment-reviewer` _(always)_ — verifies the change follows the data-driven design philosophy: designer-reachable from RON without recompiling, no hardcoded asset paths, no capability pushing directly to ActionQueue.
   - `system-architect` _(always)_ — verifies crate boundaries, the Message→Interpreter→Action→Executor pipeline, schema stability, capability coupling, and long-term maintainability.
   - `debug-detective` _(always)_ — adversarially reviews the diff for latent bugs and edge cases, not just symptoms the user already reported.
   - `ux-gamedesigner-reviewer` _(conditional — if any files in `assets/`, `docs/`, or schema RON files changed)_ — verifies the designer experience is clear, documented, and consistent.
   - `wasm-perf-reviewer` _(conditional — if the change touches runtime systems, rendering, the render/update hot path, asset-loading paths, per-frame work, adds a dependency, or adds schema that drives per-frame processing)_ — verifies no WASM frame-time or binary-size regressions.
   - The test suite, at the same time as the review agents (independent of them):
     ```
     cargo test -p ironhold_core --test '*'
     cargo check -p ironhold_cli
     ```
     Both are unconditional every time — the CLI check catches `Action`/schema changes that would silently break `query.rs` otherwise. All must pass before continuing.

   **Evaluate every review finding individually**: fix it now (return to step 3) or, if it's non-blocking, log it as its own item in `planning/backlog.md` or a `planning/claude_suggestions.md` entry for later triage. Don't let a non-blocking observation stall this feature.

5. **Docs updated** — Check that `docs/20_data_formats.md` and any relevant `CLAUDE.md` files reflect the changes. New schema fields, new action types, and new events each need a doc entry. Also check `crates/ironhold_core/src/CLAUDE.md` for capability-level notes.

6. **Schema/CLI spot-check** _(conditional)_ — If any file in `crates/ironhold_core/src/schema/` was modified (the `cargo check -p ironhold_cli` in step 4 already ran unconditionally), also verify `query actions` and `query events` output still formats correctly if `Action` or event types changed.

7. **WASM dev build** — Run:
   ```
   wasm-pack build crates/ironhold_web --target web --out-dir ../../pkg --dev --features webgpu
   ```
   Report the size of `pkg/ironhold_web_bg.wasm`. Warn if ≥ 95 MB.
   ⚠️ **Never commit `pkg/` on a feature branch** — release builds only happen on `integration` (enforced by `.githooks/pre-commit`). This is a dev build for local play-testing only.

8. **Play-test checklist** — Provide a concrete checklist for Frank to verify the feature in the browser: which project to load, what to interact with, what to look for. Include golden path and at least one edge case.

9. **Await play-test confirmation** — Stop here. Do not proceed to step 10 until Frank explicitly confirms the feature works in the browser.

   If Frank reports a bug or regression during play-testing:
   - Return to **step 3** (implement the fix)
   - Re-run **step 4** (parallel review + tests, each conditional review only if its trigger still applies) and **steps 5 → 7** (docs, schema check, dev WASM build)
   - Then return to **step 8** (updated play-test checklist) and **step 9** (await confirmation again)
   - Repeat this loop until Frank confirms all is well.

10. **Mark Done, commit, merge into `integration`:**
    - In `planning/backlog.md`, mark the item `[x]` and move its feature file from `planning/features/` to `planning/features/done/` if one exists.
    - Commit (code + tests + docs — **never `pkg/`**).
    - Merge **from the primary checkout** (it stays checked out on `integration` permanently — do not `git checkout integration` from inside the feature worktree, git will refuse since `integration` is already checked out elsewhere): `git merge feature/{slug}`. Expect an occasional `planning/backlog.md` conflict when several features land close together (resolve by hand — a `merge=union` auto-resolver was tried and rejected: it silently duplicates section headers and reverts moved lines instead of flagging a real conflict). Confirm the merge succeeded, then `git push origin integration`.
    - Clean up: `git worktree remove ../ironhold-lib-{slug}` then `git branch -d feature/{slug}`.

## On `integration` (once per batch, after one or more feature branches have merged in)

11. **Full test suite across the combined batch** — run again on `integration` to catch cross-feature regressions the individual feature branches couldn't see:
    ```
    cargo test -p ironhold_core --test '*'
    cargo check -p ironhold_cli
    ```

12. **WASM release build** — Run:
    ```
    cargo clean && wasm-pack build crates/ironhold_web --target web --out-dir ../../pkg --features webgpu
    ```
    Report binary size. If ≥ 95 MB, warn Frank clearly — GitHub Pages hard-blocks at 100 MB.

13. **Await release play-test confirmation** — Stop here. Ask Frank to do a quick smoke-test of the combined batch (`python serve.py`) — confirm no console errors and every merged feature still works. Do not proceed to step 14 until Frank confirms.

    If the release build reveals a regression:
    - Return to **step 3** on the relevant `feature/{slug}` branch (recreate its worktree if it was already removed) and fix it there
    - Re-run that branch's steps 4 → 9, then re-merge into `integration`
    - Repeat from **step 11**
    - If the regression is hard to isolate to one feature branch, reset instead: `git branch -f integration <last-good-sha>` (typically `main`'s tip), then re-merge whichever finished feature branches were dropped by the reset.

14. **Commit `pkg/` on `integration`** — use `git add -f pkg/`, not plain `git add`: `pkg/.gitignore` is a blanket `*`, so a new filename `wasm-pack` emits (as opposed to an already-tracked one) would otherwise be silently skipped.

15. **Promote to `main`** — fast-forward only, then push (this is the step that updates the live GitHub Pages demo), then switch back to `integration` (its permanent home):
    ```
    git checkout main && git merge --ff-only integration && git push origin main && git checkout integration
    ```
    `.githooks/pre-push` blocks this push unless `main` exactly matches `integration`'s tip.

16. **Post cleanup** — `cargo clean`; prompt the user to run `/compact`.

17. **Propose the next feature(s) to activate from the backlog `## Queued` section** — one per available worktree.
