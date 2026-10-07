Promote the combined `integration` batch to `main` (steps 11-17): full test suite, WASM release build, release play-test, commit `pkg/`, fast-forward `main`. Execute each step in order, report its status, and do not skip steps. Everything here happens **on `integration`**, once per batch of merged feature branches — not once per feature (that is `/ship-feature`).

This is the step that updates the live GitHub Pages demo, so stop at every "Await" step and wait for Frank. If this file and root `CLAUDE.md` disagree, root `CLAUDE.md` wins — fix this file.

Precondition: the primary checkout is on `integration` with a clean working tree, and every feature in the batch has been merged and pushed (`/ship-feature` step 10).

11. **Full test suite across the combined batch** — run again on `integration` to catch cross-feature regressions the individual branches couldn't see. One test file at a time, checking cargo's exit code and `df -h /c` between files (see `CLAUDE.md` → Build & Run Commands), then:
    ```
    cargo check -p ironhold_cli
    cargo test -p ironhold_cli
    ```

12. **WASM release build** — Run (the `cargo clean` clears the shared target dir for every worktree, which is why this happens once per batch; check `df -h /c` first):
    ```
    cargo clean && wasm-pack build crates/ironhold_web --target web --out-dir ../../pkg --features webgpu
    ```
    Never add `inspector` to the release build. Report `ls -lh pkg/ironhold_web_bg.wasm`. If ≥ 95 MB, warn Frank clearly — GitHub Pages hard-blocks at 100 MB.

13. **Await release play-test confirmation** — Stop here. Ask Frank to smoke-test the combined batch (`python serve.py`): no console errors, and every merged feature still works. Do not proceed to step 14 until Frank confirms.

    If the release build reveals a regression:
    - Return to `/ship-feature` step 3 on the relevant `feature/{slug}` branch (recreate its worktree if it was removed) and fix it there, then re-merge into `integration` and repeat from **step 11**.
    - If the regression is hard to isolate to one feature, reset instead: `git branch -f integration <last-good-sha>` (typically `main`'s tip), then re-merge whichever finished feature branches the reset dropped.

14. **Commit `pkg/` on `integration`, in its own commit** — never combined with any code, docs, backlog or agent-memory change. Use `git add -f pkg/` (not plain `git add`): `pkg/.gitignore` is a blanket `*`, so a new filename `wasm-pack` emits would otherwise be skipped. **Never cite a `pkg/` commit's hash in any planning markdown** — cite the nearest code/docs-only commit, or describe it in words.

15. **Promote to `main`** — fast-forward only, then push, then return to `integration` (the primary checkout's permanent home):
    ```
    git checkout main && git merge --ff-only integration && git push origin main && git checkout integration
    ```
    `.githooks/pre-push` blocks this push unless `main` exactly matches `integration`'s tip. Do not fast-forward `main` before steps 12-14 are complete.

16. **Post cleanup** — `cargo clean`; prompt the user to run `/compact`.

17. **Propose the next feature(s) to activate from the backlog `## Queued` section** — one per available worktree.
