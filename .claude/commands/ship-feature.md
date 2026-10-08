Walk the current feature through the ironhold-lib code change workflow up to and including its merge into `integration` (steps 1-10). Execute each step in order, report its status, and do not skip steps. Steps marked _(conditional)_ run only when their trigger applies — say so explicitly when you skip one.

These steps happen **on a `feature/{slug}` branch**, in its own git worktree — parallelizable, one per feature. Getting the combined `integration` batch onto `main` is a separate, once-per-batch job: `/release`. See root `CLAUDE.md` → Branching Model and Code change workflow for the full reference; if this file and root `CLAUDE.md` disagree, root `CLAUDE.md` wins — fix this file.

1. **Feature plan complete** — Check whether the relevant feature file exists in `planning/features/` and is filled in (approach, schema changes, RON examples). If there is no plan doc for a non-trivial change, flag it and stop. Equivalent to running `/plan-review`. For a bug fix, **reproduce it first** against current `HEAD` (see `CLAUDE.md` step 1) before writing any fix.

2. **Create the feature branch + worktree, mark it Active in the backlog, commit before coding:**
   ```
   git worktree add ../ironhold-lib-{slug} -b feature/{slug} main
   ```
   - **Exception:** if `main` lags `integration` by a whole unreleased batch the plan was written against (`git diff --stat main integration -- crates` shows dozens of files), cut the branch from `integration` instead. `.githooks/post-checkout` only copies a plan file that is *missing* — `main` may carry a stale older copy, so confirm the worktree's `planning/features/{slug}.md` is the current one.
   - Read `planning/backlog.md`, move the item to `## Active`, and commit that change before writing code.

3. **Code changes implemented** — Confirm the implementation is complete (code, CLI, tests). Any playtest-aid RON/asset change belongs here, not bolted on before the WASM build, so step 4's tests cover it. If the new work adds a project under `assets/projects/`, use `/new-project <name>` — do not hand-register it.

4. **Parallel code review + tests** — Equivalent to running `/code-review` alongside the test suite. Launch in a single message (multiple tool calls, so everything runs concurrently):
   - `alignment-reviewer` _(always)_ — designer-reachable from RON without recompiling, no hardcoded asset paths, no capability pushing directly to `ActionQueue`.
   - `system-architect` _(always)_ — crate boundaries, the Message→Interpreter→Action→Executor pipeline, schema stability, capability coupling, WASM compatibility.
   - `debug-detective` _(always)_ — adversarial review of the diff for latent bugs and edge cases.
   - `ux-gamedesigner-reviewer` _(conditional — if any files in `assets/`, `docs/`, or schema RON files changed)_.
   - `wasm-perf-reviewer` _(conditional — runtime systems, rendering, the render/update hot path, asset-loading, per-frame work, a new dependency, or schema that drives per-frame processing)_.
   - The test suite, at the same time as the review agents. On this machine run it **one test file at a time**, checking cargo's own exit code and `df -h /c` between files (see `CLAUDE.md` → Build & Run Commands for the loop), then:
     ```
     cargo check -p ironhold_cli
     ```
     The CLI check is unconditional — it catches `Action`/schema changes that would silently break `query.rs`. All must pass before continuing.

   **Evaluate every review finding individually**: fix it now (return to step 3) or, if non-blocking, log it in `planning/backlog.md` or `planning/claude_suggestions.md`.

   **Then commit the review agents' memory right away, on `integration`, from the primary checkout** (`git add .claude/agent-memory && git commit -m "chore(agent-memory): update from {slug} review cycle"`). Review agents always write into the primary checkout's working tree, whichever branch the change is on.

5. **Docs updated** — `docs/20_data_formats.md` and any relevant `CLAUDE.md` files (the folder file for the area you touched: `capabilities/`, `runtime/`, `runtime/scene_manager/` or `schema/` under `crates/ironhold_core/src/`, plus the crate-wide `crates/ironhold_core/src/CLAUDE.md` for pipeline-wide rules; long-form notes go in a `docs/dev/` topic page) reflect the change. New schema fields, action types and events each need a doc entry.

6. **Schema/CLI spot-check** _(conditional)_ — If any file in `crates/ironhold_core/src/schema/` was modified, run `cargo run -p ironhold_cli -- query actions assets/projects/3rd_person_game_demo` (the freshly built CLI, **not** the cached `tools/bin/ironhold`) and verify new kinds appear and nothing crashes.

7. **WASM dev build** — Run:
   ```
   wasm-pack build crates/ironhold_web --target web --out-dir ../../pkg --dev --features webgpu --features inspector
   ```
   `inspector` is a standing default for every dev build (F9 collider wireframes, `` ` `` egui world inspector). The dev build is not a proxy for the release size. **`pkg/` is tracked, so this leaves it modified — never commit it on a feature branch** (`.githooks/pre-commit` blocks it), and run `git checkout -- pkg` only **after** Frank has confirmed the play-test, not before.
   - Screenshots / browser suite: `python test_web.py --project <name> --real-gpu --update-baseline <scene> --skip-build` (a visible Chromium window; the suite uses port 8001, so a manual `python serve.py` on 8000 never collides). Update only the baselines this change affected, never a blanket `--update-baselines`. Headless modes are the fallback only.

8. **Play-test checklist** — Provide a concrete checklist for Frank: which project to load, what to interact with, what to look for. Include the golden path and at least one edge case. Tell him to run `python serve.py` from the feature worktree.

9. **Await play-test confirmation** — Stop here. Do not proceed to step 10 until Frank explicitly confirms the feature works in the browser.

   If Frank reports a bug or regression: return to **step 3**, re-run **step 4** (reviews only where their trigger still applies, plus the tests), then **steps 5 → 7**, and repeat 8 → 9 until Frank confirms.

10. **Mark Done, commit, merge into `integration`:**
    - If **any** `assets/projects/` file changed since step 4's test run, re-run at least `cargo test -p ironhold_core --test ron_lint --test ron_validation` first (they are the only checks for RON authoring style).
    - In `planning/backlog.md`, **physically move the item's bullet into `## Done (reference)`** under the current `### <Month> <Year>` heading (a `[x]` left in `## Active`/`## Bugs` does not count), and move `planning/features/{name}.md` into `planning/features/done/` (a multi-phase plan stays put until its last phase ships — update its Phases table instead). Commit (code + tests + docs — **never `pkg/`**).
    - Stop Frank's dev server (`python serve.py` with its cwd in the worktree locks the directory on Windows), then `git checkout -- pkg`.
    - Merge **from the primary checkout** (it stays on `integration` permanently — never `git checkout integration` from the worktree): `git merge feature/{slug}`. Expect an occasional `planning/backlog.md` conflict; resolve by hand (a `merge=union` driver was tried and rejected). If moving the feature file into `done/` causes a modify/delete conflict, `git rm` the stale old-path copy and carry any content edits across.
    - Check `git status` in the primary checkout and commit any remaining `.claude/agent-memory/` changes separately, then `git push origin integration`.
    - Clean up: `git worktree remove ../ironhold-lib-{slug}` then `git branch -d feature/{slug}`.

When this feature is merged and Frank wants the combined batch live, run `/release`.
