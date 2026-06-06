Scaffold a new ironhold project from the blank_project template.

Project name: $ARGUMENTS

If no name was provided, ask Frank for one before continuing.

Steps:

1. **Copy the template** — Copy the entire `assets/projects/blank_project/` directory to `assets/projects/<name>/`.

2. **Update project identity** — In the new `<name>/<name>.project.ron`:
   - Set `project_id: "<name>"`
   - Set `display_name: "<Friendly Name>"` (title-case the project name, replace underscores with spaces)

3. **Register in test_web.py** — Append `"<name>"` to the `PROJECTS` list near the top of `test_web.py`.

4. **Validate** — Run:
   ```
   cargo run -p ironhold_cli -- validate assets/projects/<name>
   ```
   Confirm all files are valid before continuing.

5. **Remind Frank** of the two manual steps that require a running build:
   - Generate a baseline screenshot:
     ```
     python test_web.py --project <name> --update-baselines --skip-build
     ```
     This writes `screenshot_baselines/scenes/<name>_main.png`.
   - Add a card to `index.html`: copy an existing `<a class="project-card">` block and update `id`, `href`, `data-keywords`, `img src`, `img alt`, the title, description, and tags.

Do not generate the baseline or edit `index.html` automatically — those steps require a confirmed WASM build.
