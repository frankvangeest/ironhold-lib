Scaffold a new ironhold project from the blank_project template and register it everywhere a project must appear. This is THE way to add a project under `assets/projects/` — never hand-copy the template or hand-register one (root `CLAUDE.md` points here).

Project name: $ARGUMENTS

If no name was provided, ask Frank for one before continuing. The name is a snake_case folder name (`ui_demo`, not `UI Demo`).

Steps:

1. **Copy the template** — Copy the entire `assets/projects/blank_project/` directory to `assets/projects/<name>/` and rename `blank_project.project.ron` to `<name>.project.ron`. If the project is UI-only (no player or world), delete the `player_01`/`ground` entities from `scenes/main.scene.ron` and the unused prefabs.

2. **Update project identity** — In `<name>.project.ron`:
   - Set `project_id: "<name>"`
   - Set `display_name: "<Friendly Name>"` (title-case the name, underscores to spaces)

3. **Write the scene and logic** — Author the content. Every `Button` needs a matching `ui.button_pressed:<trigger>` binding in `logic/state_machine.ron`, or `ironhold validate` reports `unreachable_trigger` and the button does nothing. Use plain ASCII in comments and on-screen text (the engine font has no glyph for em-dashes). Never write `Some(...)` in RON (`ron_lint` rejects it).

4. **Register the project** (all of these, each is a separate place a project must appear):
   - `test_web.py` — append `"<name>"` to the `PROJECTS` list near the top.
   - `crates/ironhold_cli/tests/validate_projects.rs` — add `#[test] fn validate_<name>() { validate("<name>"); }`.
   - `README.md` — add a row to the "Example projects" table.
   - `index.html` — add a card: copy an existing `<a class="project-card">` block and update `id` (`card-<name-with-dashes>`), `href` (`play.html?project=<name>`), `data-keywords`, `img src` (`screenshot_baselines/scenes/<name>_main.png`), `img alt`, the title, description, and tags.
   - `python tools/build_asset_manifest.py` — regenerate `assets_manifest.json` if the project added asset files.

5. **Validate** — Run all of these and confirm they pass before continuing:
   ```
   cargo run -p ironhold_cli -- validate --strict assets/projects/<name>
   python tools/asset_checker/check.py
   cargo test -p ironhold_core --test ron_lint --test ron_validation
   cargo test -p ironhold_cli --test validate_projects
   ```
   (One cargo invocation at a time — never from two worktrees at once.)

6. **Baseline screenshot** — needs a finished WASM dev build (`wasm-pack build crates/ironhold_web --target web --out-dir ../../pkg --dev --features webgpu --features inspector`), so do it only once that build exists, and tell Frank first because it opens a visible browser window:
   ```
   python test_web.py --project <name> --real-gpu --update-baselines --skip-build
   ```
   This writes `screenshot_baselines/scenes/<name>_main.png` (one file per scene for a multi-scene project) and, on the first run, skips the console-error check — so also load `play.html?project=<name>` once and check the browser console.
   - **Use `--real-gpu` (WebGPU, visible Chromium).** Headless `--webgpu` finds no GPU adapter on this machine, and the default headless GL mode needs a *WebGL2* build, not the dev build above. Both are fallback only.
   - The suite runs its own server on port **8001** (`--port` to change), so a manual `python serve.py` on 8000 never collides. If a run times out with an empty `#debug-state`, check `netstat -ano | grep :800`.
   - Only update this project's baselines (`--project <name>`), never a blanket `--update-baselines` across all projects — older baselines were captured on headless GL and would show unrelated diffs.
   - After the run: `git checkout -- pkg` (the dev build leaves tracked `pkg/` modified; never commit it on a feature branch) — but only once any play-test is finished.

7. **Report** — List every file created or edited, and whether the baseline step (6) is still pending.
