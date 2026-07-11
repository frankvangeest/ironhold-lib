# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Run Commands

```bash
# Ironhold CLI — inspect, validate, and query project assets (no engine startup required)
cargo run -p ironhold_cli -- validate <project_dir>              # parse & cross-check all RON files; exit 0=ok 1=errors 2=tool-error
cargo run -p ironhold_cli -- validate --strict <project_dir>    # also report defined keys never referenced anywhere (orphan detection)
cargo run -p ironhold_cli -- inspect glb     <path.glb>          # animations, meshes, materials, root nodes
cargo run -p ironhold_cli -- inspect texture <path.png|jpg|webp> # dimensions, format, channels, file size
cargo run -p ironhold_cli -- inspect audio   <path.wav|mp3>      # duration, sample rate, channels, file size
cargo run -p ironhold_cli -- query prefabs <project_dir>         # list prefabs (kind, model, tags, behavior)
cargo run -p ironhold_cli -- query effects <project_dir>         # list particle effects (count, layers, flags)
cargo run -p ironhold_cli -- query scenes   <project_dir>        # list scenes (entities, ui, player, overlay)
cargo run -p ironhold_cli -- query rules    <project_dir>        # list rules.ron and/or state_machine.ron
cargo run -p ironhold_cli -- query actions  <project_dir>        # list all action types used across logic files
cargo run -p ironhold_cli -- query events   <project_dir>        # list all event triggers used across logic files
cargo run -p ironhold_cli -- query prefabs <project_dir> --keys-only             # one key per line (pipe-friendly)
cargo run -p ironhold_cli -- query effects <project_dir> --filter additive=true  # filter by field=value
cargo run -p ironhold_cli -- watch  <project_dir>               # watch for .ron changes and re-validate on every save
cargo run -p ironhold_cli -- stats  <project_dir>               # compact summary: scenes, prefabs, effects, logic, catalog size
cargo run -p ironhold_cli -- --json <command>                    # any command accepts --json for machine-readable output
cargo build -p ironhold_cli --release   # produces target/release/ironhold.exe

# Run native (desktop) build
cargo run -p ironhold_native

# Run with inspector UI (debug overlay)
cargo run -p ironhold_native --all-features

# Run a specific project by name
cargo run -p ironhold_native -- --project 3rd_person_game_demo

# ⚠️ Before any cargo/wasm-pack command: verify CARGO_TARGET_DIR is actually set in THIS shell.
# `export` does not persist across separate tool invocations/shells, and this machine has no
# .cargo/config.toml or shell profile file to fall back on -- if the variable is empty, cargo
# silently creates a full local target/ (multi-GB) inside whichever directory you're in, instead
# of the shared one. A persistent Windows user env var was set via `setx` (2026-07-12), but
# already-running shells/sessions (including one still open from before that point) do NOT pick
# it up until restarted. Check first:
echo $CARGO_TARGET_DIR   # empty output -> prefix every command below with:
export CARGO_TARGET_DIR="/c/git/rust/ironhold-cargo-target-shared"

# Run all tests -- ⚠️ this machine chronically runs low on disk (single-digit GB free is common,
# see One-time machine setup below); a full parallel `--test '*'` build has already exhausted the
# disk more than once building 16 separate debug test binaries at once. Default to the
# one-file-at-a-time loop below rather than treating it as a last-resort fallback.
cargo test -p ironhold_core --test '*' -- --nocapture

# Preferred on this machine: compile/run one test file at a time, checking cargo's own exit code
# (not tail's -- piping through `tail` masks a real compile failure unless you check
# PIPESTATUS/pipefail, which is how a previous run kept "succeeding" through several disk-full
# compiler crashes before anyone noticed):
for t in fsm_tests entity_logic_tests scene_lifecycle_tests spawn_tests action_tests npc_tests \
         nameplate_tests ui_tests audio_tests stats_tests particle_tests ron_validation ron_lint \
         ui_panel_blocker assets_schema_version_regression; do
  echo "=== $t ==="
  cargo test -p ironhold_core --test "$t" | tail -15
  [ "${PIPESTATUS[0]}" -ne 0 ] && { echo "FAILED: $t"; break; }
  df -h /c/git 2>/dev/null | tail -1   # watch free space; stop and investigate if it's dropping fast
done

# If a single test binary alone still fails with an IO/no-space or compiler-panic error, cap
# build parallelism as a further fallback (single flag, keeps one command):
cargo test -p ironhold_core --test '*' --jobs 1

# Run a single test file
cargo test -p ironhold_core --test fsm_tests
cargo test -p ironhold_core --test ron_validation

# Run a single test by name
cargo test -p ironhold_core --test ui_tests test_ui_button_to_load_scene_action

# Ironhold CLI tests (spawn the binary — no Bevy required)
cargo test -p ironhold_cli                              # run all CLI tests (smoke + cross-file)
cargo test -p ironhold_cli --test validate_projects     # smoke: validate each example project
cargo test -p ironhold_cli --test validate_cross_file   # cross-file: reference errors reported correctly

# Build for WASM — WebGPU backend (default; requires Chrome 113+ / Edge 113+)
wasm-pack build crates/ironhold_web --target web --out-dir ../../pkg --features webgpu

# Build for WASM — WebGL2 fallback (broader browser support, more GPU fallback warnings)
wasm-pack build crates/ironhold_web --target web --out-dir ../../pkg

# Serve WASM locally (no-cache, port 8000)
python serve.py

# Full browser test suite (builds WASM, starts server, runs headless Chromium)
python test_web.py

# Skip wasm-pack build and test against existing pkg/
python test_web.py --skip-build

# Overwrite stored screenshot baselines after intentional visual changes
python test_web.py --update-baselines

# Overwrite baseline for a single project (or 'pause_nav' for navigation steps)
python test_web.py --update-baseline quick_scene
python test_web.py --update-baseline pause_nav
```

## Architecture Overview

Three-crate workspace:
- **`ironhold_core`** — platform-agnostic game library; contains all logic, rendering, physics, and the scene pipeline. Must never have platform-specific code.
- **`ironhold_native`** — thin desktop runner; parses `--project` CLI arg, calls `ironhold_core::start_app()`.
- **`ironhold_web`** — thin WASM runner; `#[wasm_bindgen(start)]` calls `ironhold_core::start_app(None)`.

### Core internal structure (`ironhold_core/src/`)

- **`schema/`** — RON-serializable data types (`ProjectConfig`, `GameSceneV2`, `AssetCatalog`, `PrefabCatalog`, `Action`, etc.). These are the source of truth for all data-driven content.
- **`runtime/`** — systems that run at engine boot/update: scene loading (`scene_manager`), model spawning, material creation, input translation, message interpreter, action executor.
- **`capabilities/`** — modular gameplay systems: player controller, orbit camera, flycam, animation, animation resolver, NPC AI, collectible triggers, motion (rotate/bob), custom material, terrain mesh generation, terrain material, physics (Rapier3D).

### Data-driven game loop

The engine uses a **Message → Interpreter → Action → Executor** pipeline:

1. Capabilities emit `UiEvent`, `GameEvent`, `InputActionMessage`, or `SceneEvent` events.
2. `message_interpreter_system` reads those events plus the data-defined `LogicRules` (from `logic/rules.ron`) to produce `Action` values placed on the `ActionQueue` resource.
3. `action_executor_system` dispatches each `Action` (e.g., `LoadScene`, `Spawn`, `PlayAnimation`) to the appropriate capability systems.

This means game behavior can be authored entirely in RON without recompiling the engine.

### Asset & project layout

```
assets/projects/{name}/
  {name}.project.ron          ← ProjectConfig (entry point, initial scene ref)
  scenes/*.scene.ron          ← GameSceneV2 files (models, UI, lighting, player); projects can have multiple scenes
  logic/rules.ron             ← event → action rules (simple projects)
  logic/state_machine.ron     ← FSM-based logic (used by projects with multiple states/scenes)
  overrides/model_fixes.ron   ← per-model transform corrections
  prefabs/prefabs.ron         ← reusable component definitions
  prefabs/animation/*.ron     ← AnimationPolicy per character
  assets.ron                  ← AssetCatalog
```

Note: projects may have `rules.ron`, `state_machine.ron`, or both. Simple projects use only `rules.ron`; projects with multiple scenes/states use `state_machine.ron` (sometimes alongside `rules.ron`). See the interpreter notes in `crates/ironhold_core/src/CLAUDE.md`.

Example projects: `quick_scene`, `3rd_person_game_demo`, `terrain_demo`, `custom_materials`, `primitive_world`, `entity_logic_demo`, `particles_demo`. Test data lives in `assets/projects/integration_tests/`.

## Tools

Python CLI tools live in `tools/`. Always run them from the repo root.

| Tool | When to use |
|---|---|
| `tools/asset_checker/check.py` | After editing any `assets.ron` or moving/renaming asset files — verifies all referenced paths resolve on disk |
| `tools/texture_gen/generate.py` | Generate seamless noise textures or per-project terrain heightmaps |
| `tools/avif2png/convert.py` | Batch-convert AVIF preview images to PNG |
| `tools/glb_inspector/inspect_glb.py` | Inspect a GLB for exact node names, animation clips, and materials before authoring RON |
| `tools/glb_preview/preview.py` | Render a 3/4-view preview PNG for GLB models using Blender headless |
| `tools/build_asset_manifest.py` | After adding, removing, or renaming any asset files — regenerates `assets_manifest.json` for the `assets.html` browser |

Each tool has its own `CLAUDE.md` with full usage examples. Run `python <tool> --help` for a quick reference.

```bash
# Always run after changing any assets.ron or moving asset files
python tools/asset_checker/check.py

# Also check for unreferenced files in assets/shared/
python tools/asset_checker/check.py --orphans

# Regenerate the asset browser manifest after adding/removing asset files
python tools/build_asset_manifest.py
```

## Planning

All work items live in `planning/`. See `planning/CLAUDE.md` for the full folder reference.

### Backlog (`planning/backlog.md`)
The canonical priority queue — features and bugs in one place. Items flow: **Icebox → Queued → Active → Done**. Do not duplicate items into GitHub issues or `docs/`.

### Bugs
Log known bugs in the `## Bugs` section of `planning/backlog.md` as a one-liner with reproduction and suspected cause. If the bug needs investigation before it can be fixed, also create `planning/investigations/{name}.md` and link to it from the backlog entry.

### Feature files (`planning/features/`)
Create `planning/features/{name}.md` (copy `_template.md`) when a feature needs design discussion before coding: new schema fields, new event/action types, cross-capability changes, or anything where the approach is unclear. Always fill in `Planned at: <hash> (<YYYY-MM-DD>)` at the top — run `git rev-parse --short HEAD` to get the hash.

### Claude suggestions (`planning/claude_suggestions.md`)
While implementing features, if you notice something worth revisiting — a latent bug, a pattern that could be improved, a follow-up optimisation — add a brief entry. Format:
```
- **Title** _(observed at `<hash>` <YYYY-MM-DD>)_
  What (one sentence) + Why (one sentence, concrete basis).
```
Only add things with a concrete technical basis. Frank reviews these periodically and promotes good ones to the backlog.

## Adding a new asset project

When a new project is added under `assets/projects/{name}/`, three registration steps are required:

1. **`test_web.py`** — append the project name to the `PROJECTS` list at the top of the file.

2. **Baseline screenshot** — generate the project's scene screenshot so it can be used in the gallery:
   ```bash
   python test_web.py --project {name} --update-baselines --skip-build
   ```
   This writes `screenshot_baselines/scenes/{name}_main.png` (and one file per scene if the project has multiple scenes).

3. **`index.html`** — add a card to the project grid. Copy an existing `<a class="project-card">` block and update:
   - `id` attribute (`card-{name}`)
   - `href` → `play.html?project={name}`
   - `data-keywords` → space-separated search terms
   - `img src` → `screenshot_baselines/scenes/{name}_main.png`
   - `img alt`, card title, description, and tags

---

## Proactive Agent Reviews

Beyond the mandatory post-implementation reviews below, use specialized agents proactively whenever a question is complex enough that a second perspective would change the answer: design decisions before coding, architectural tradeoffs, feasibility investigations, or any time the right approach is genuinely unclear. The system-architect, alignment-reviewer, and ux-gamedesigner-reviewer are all available for pre-implementation consultation, not just post-change audits.

For bugs that are hard to reproduce, have an unclear root cause, or span multiple systems, invoke the **debug-detective** agent rather than investigating inline. Complex bugs benefit from systematic isolation — the agent works methodically through hypotheses without accumulating context debt in the main conversation.

After implementing any feature, capability, or schema change, always invoke these specialized agents — do not wait to be asked:

| Agent | When to invoke |
|---|---|
| **`alignment-reviewer`** | After any code change — verifies RON designer-reachability and no hardcoded behavior |
| **`system-architect`** | After any code change — verifies crate boundaries, WASM compatibility, and long-term maintainability |
| **`debug-detective`** | After any code change — general adversarial review for latent bugs/edge cases in the diff, not only known reproducible bugs |
| **`ux-gamedesigner-reviewer`** | *(conditional)* After any change to `assets/`, `docs/`, or RON schema — verifies the designer experience is clear and documented |
| **`wasm-perf-reviewer`** | *(conditional)* For changes to runtime systems, rendering, the render/update hot path, asset-loading, per-frame work, new dependencies, or schema that drives per-frame processing — verifies no WASM frame-time or binary-size regressions |

`alignment-reviewer`, `system-architect`, and `debug-detective` run after every code change — launch all three **in parallel** (single message, multiple tool calls), alongside `ux-gamedesigner-reviewer`/`wasm-perf-reviewer` when their conditional triggers apply, and alongside the test suite (steps 4/11 in the Code change workflow below — reviews and tests are independent of each other). Skip the conditional two for pure RON/asset/doc tweaks. Use `/code-review` to run the full set for a consolidated pre-commit verdict on code changes, or `/plan-review` for the feature plan before any code is written.

**Evaluate each finding individually** once the reviews come back: either fix it now (loop back to the Code changes step) or, if it's non-blocking, log it as its own item in `planning/backlog.md` or a `planning/claude_suggestions.md` entry for later triage — don't let a minor observation stall the feature it wasn't blocking.

---

## Shell tool preference

Always prefer the **Bash tool** over the PowerShell tool for shell commands. Most commands in this project (`cargo`, `wasm-pack`, `python`, `git`, `ls`, `grep`, `find`) work correctly in Bash on Windows. Only fall back to the PowerShell tool when a command genuinely requires PowerShell-specific syntax (e.g. `Get-ChildItem -Recurse` pipelines, registry access, or `$env:` variables) and cannot be expressed in Bash.

---

## Branching Model (GitOps, parallel features)

Ironhold uses a three-tier branch model so multiple features can be developed in parallel (e.g. several Claude Code worktrees at once) without relying on GitHub Actions or any other platform automation. Enforcement is via local git hooks in `.githooks/` (portable to Forgejo — plain git, no platform lock-in).

- **`main`** — always deployable. GitHub Pages serves `pkg/` directly from this branch, so `pkg/` on `main` must always match the RON/assets/code on `main`. `main` only ever advances by fast-forwarding to `integration`'s tip — never a direct commit, never a merge from a `feature/*` branch.
  ⚠️ Confirm in GitHub repo Settings → Pages that the source is `main` / `(root)` — this can't be verified from files in the repo. Note also that GitHub's "deploy from branch" Pages hosting runs a hidden, GitHub-managed `pages-build-deployment` Action under the hood — this one piece of the pipeline is *not* plain-git and will need a Forgejo Pages equivalent at migration time.
- **`integration`** — the batching branch. Finished `feature/*` branches merge here. This is the *only* branch where the expensive gates run — full test suite across the whole combined batch, `cargo clean` + WASM **release** build, full regression playtest — once per batch, not once per feature. `pkg/` is committed here, then promoted to `main`.
- **`feature/{slug}`** — one branch per backlog item, developed in its own git worktree so several proceed at once:
  ```bash
  git worktree add ../ironhold-lib-{slug} -b feature/{slug} main
  ```
  A feature branch only runs the cheap part of the workflow (plan → code → tests → docs → WASM **dev** build → dev playtest) and never touches `pkg/`. Once the dev playtest is confirmed, it merges into `integration`.

(Not to be confused with `.claude/worktrees/agent-*` — those are ephemeral worktrees the Agent tool's `isolation: "worktree"` option creates and cleans up on its own; `../ironhold-lib-{slug}` is the separate, long-lived convention for a feature's whole branch lifetime.)

**Primary checkout ownership:** the main repo directory (`C:\git\rust\ironhold-lib`) is permanently the `integration`/`main` home — it stays checked out on `integration` at all times, only switching to `main` briefly to fast-forward and push (see step 15). Git refuses to check out a branch that's already checked out in another worktree, so `feature/*` work always happens in its own separate worktree, never in the primary checkout, and merges into `integration` happen *from* the primary checkout, not by trying to `checkout integration` inside a feature worktree.

### Recovering a poisoned `integration`

If a bad batch lands on `integration` (e.g. a release playtest reveals a regression that's hard to isolate): `git branch -f integration <last-good-sha>` (typically `main`'s current tip, or an earlier `integration` commit), then re-merge whichever finished feature branches were dropped by the reset.

### Fresh-clone / new-machine bootstrap

A fresh clone has no local `integration` branch and no `core.hooksPath` set — `.githooks/pre-push` will block every push to `main` until both exist:
```bash
git branch integration origin/main    # or origin/integration if it's already been pushed
git config core.hooksPath .githooks
```
These hooks are local guardrails only (client-side, advisory) — they don't stop a machine that hasn't run this setup from pushing directly. Real enforcement needs server-side branch protection (Forgejo) once this moves off GitHub.

### One-time machine setup

```bash
git config core.hooksPath .githooks
```

`core.hooksPath` activates `.githooks/pre-commit` (blocks `pkg/` being committed on a `feature/*` branch) and `.githooks/pre-push` (blocks pushing `main` unless it exactly matches `integration`'s tip).

**`CARGO_TARGET_DIR` must be a persistent environment variable, not a per-shell `export`.** This
machine has no shell profile file (`.bashrc`/`.bash_profile`/`.profile` don't exist) for an
`export` line to live in, and tool-invoked shells don't share state between separate invocations —
so "add it to your shell profile" silently never took effect here, and any cargo/wasm-pack command
run without the variable explicitly set created a full local `target/` (multi-GB) inside whichever
directory it ran from, duplicating the shared cache and exhausting disk (root-caused 2026-07-12: a
~12 GB stray `target/` inside a feature worktree, found and deleted manually). Set it as a real
persistent Windows user environment variable instead:
```bash
setx CARGO_TARGET_DIR "C:\git\rust\ironhold-cargo-target-shared"
```
This only takes effect for **new** shells/processes started after the `setx` call — verify with
`echo $CARGO_TARGET_DIR` before relying on it in an already-open session, and explicitly
`export CARGO_TARGET_DIR=...` for that session if it's still empty (see the verification step
above the test commands).

Point every worktree of this repo at the **same** `CARGO_TARGET_DIR` — this repo's `target/` is very large (tens of GB observed), and this machine runs low on disk (single-digit GB free is common); a separate `target/` per worktree is not viable here.

⚠️ **Never run `cargo build`/`test`/`check` from two worktrees at the same moment**, even with a shared target dir — concurrent cargo invocations against the same target dir have corrupted builds on this machine before. Editing, planning, and playtesting can happen in parallel across worktrees; compiling cannot — coordinate so only one worktree runs cargo at a time. (A `cargo clean` on `integration`, per step 12, clears this shared cache for every worktree at once — the next build anywhere after that is a full rebuild.)

**Don't add a `cargo clean` to a feature branch's own cleanup (step 10).** It's tempting after a
disk scare, but it throws away the shared incremental cache every other worktree relies on,
turning the *next* feature's build into a full ~20+ minute rebuild — directly undermining the
"several features in parallel" point of this whole model. The mandatory clean stays scoped to step
12 (once per batch, before the release build). If disk is genuinely critical mid-feature, check
`df -h` and confirm with Frank before cleaning outside that step — see the `CARGO_TARGET_DIR`
verification note above for the far more common actual cause (a stray per-worktree `target/`, not
the shared cache itself, per the 2026-07-12 incident).

---

## Critical Rules

### Code change workflow
Every code change follows this order. Steps 1–10 happen **on a `feature/{slug}` branch** (parallelizable — one per worktree); steps 11–17 happen **on `integration`**, once per batch of merged features, not once per feature. See Branching Model above for the branch tiers.

**On a feature branch:**

 1. **Verify feature plan is complete and up-to-date** — Check if the plan for the feature is:
    - planned out enough - Require more input or decisions from the Frank or not?
    - project goal aligned - Goal alignment review for the feature plan
    - follows proper UX design - UX review for the feature plan
 2. **Create the feature branch + worktree from latest `main`, mark it Active in the backlog, and commit before coding:**
    ```bash
    git worktree add ../ironhold-lib-{slug} -b feature/{slug} main
    ```
 3. **Code changes** 
      - implement the feature or fix
      - update cli 
      - update tests
 4. **Parallel code review + tests** — launch in a single message (multiple tool calls, so they run concurrently):
    - `alignment-reviewer`, `system-architect`, `debug-detective` — always
    - `ux-gamedesigner-reviewer`, `wasm-perf-reviewer` — conditional (see Proactive Agent Reviews above); skip and say so if their trigger doesn't apply
    - the test suite, at the same time as the review agents (independent of them):
      ```
      cargo test -p ironhold_core --test '*'
      cargo check -p ironhold_cli
      ```
      The CLI check is unconditional — new `Action` variants and schema changes silently break `query.rs` without it.

    **Evaluate every review finding individually**: fix it now (go back to step **Code changes**) or, if it's non-blocking, log it as its own item in `planning/backlog.md` or a `planning/claude_suggestions.md` entry — don't let a non-blocking finding stall this feature. All tests must pass before continuing regardless of review outcome.
 5. **Docs updated** — `docs/20_data_formats.md` and any relevant `CLAUDE.md` files
 6. **Schema/CLI verify** — if any `schema/` type was added, renamed, or had a field type changed, also spot-check the query output:
    ```
    cargo run -p ironhold_cli -- query actions assets/projects/3rd_person_game_demo
    ```
    Verify new action kinds appear in the output and nothing crashes.
 7. **WASM dev build** — `wasm-pack build crates/ironhold_web --target web --out-dir ../../pkg --dev --features webgpu`
    Fast (~2 min). For local play-testing only — **never commit `pkg/` on a feature branch** (enforced by `.githooks/pre-commit`).
 8. **Provide a play-test checklist** — A checklist on how to check the changes and with what project.
 9. **User play-tests** — Frank runs `python serve.py` and confirms the feature works in the browser
    - If the user requests changes or changes are required we go back to step **Code changes** to implement them, then re-run step 4 (review + tests) before playtesting again.
10. **Mark the feature Done, commit, and merge into `integration`:**
    - On the feature branch/worktree: move the feature from Active to Done in `planning/backlog.md` and in its own `planning/features/{name}.md`, then commit (code + tests + docs — never `pkg/`).
    - From the **primary checkout** (already on `integration` — do not run `git checkout integration` from the feature worktree, it will fail since `integration` is checked out there permanently): `git merge feature/{slug}`. Expect an occasional `planning/backlog.md` conflict when several features land close together — resolve it by hand; a `merge=union` driver was tried and rejected here (tested: it silently duplicates section headers and reverts moved lines instead of flagging a real conflict, since backlog.md entries move *between* sections rather than only being appended).
    - Confirm successful merge, then `git push origin integration` (so a batch in progress isn't only local).
    - Clean up: **stop any dev server started for this feature's playtest first** (`python serve.py`
      running with its cwd inside the worktree holds a lock on Windows — `git worktree remove` will
      fail with "Device or resource busy"/"Permission Denied" until it's killed; this has happened
      more than once). Then `git worktree remove ../ironhold-lib-{slug}` then `git branch -d feature/{slug}`.

**On `integration` (once per batch, after one or more feature branches have merged in):**

11. **Full test suite across the combined batch** — same test commands as step 4 (`cargo test -p ironhold_core --test '*'` + `cargo check -p ironhold_cli`), run again on `integration` to catch cross-feature regressions the individual feature branches couldn't see.
12. **WASM release build** — `cargo clean && wasm-pack build crates/ironhold_web --target web --out-dir ../../pkg --features webgpu`
    Full clean + size-optimised release build (~8 min).
    ⚠️ Check binary size after build: `ls -lh pkg/ironhold_web_bg.wasm`. Warn at **95 MB** — GitHub Pages hard-blocks at **100 MB**.
13. **Release play-test** — Frank confirms the combined batch in the browser, no console errors, basics working.
    - If changes are needed, go back to step **Code changes** on the relevant `feature/{slug}` branch, re-merge into `integration`, and repeat from step 11.
14. **Commit `pkg/` on `integration`** — use `git add -f pkg/` (not plain `git add`): `pkg/.gitignore` is a blanket `*`, so any *new* filename `wasm-pack` emits (as opposed to ones already tracked) would otherwise be silently skipped.
15. **Promote to `main`** — fast-forward only, then push (this is the step that updates the live GitHub Pages demo), then switch back to `integration` since that's the primary checkout's permanent home:
    ```bash
    git checkout main && git merge --ff-only integration && git push origin main && git checkout integration
    ```
    (`.githooks/pre-push` blocks this push unless `main` exactly matches `integration`'s tip.)
16. **Post cleanup**
      - Do a cargo clean
      - Compact session. Prompt the user to do a /compact.
17. **Propose the next feature(s) to add to Active in the backlog** — one per available worktree.

Do not start coding before the feature plan is finalized and reviewed.
On a feature branch, do not commit past step 9 (dev play-test confirmed) — and never commit `pkg/` there at all.
On `integration`, do not fast-forward `main` before steps 12–14 (release build + `pkg/` committed + release play-test confirmed) are complete.
Do not commit a `--dev` WASM build — it bloats the repo and may exceed GitHub Pages limits.

If any code changes are made to the ironhold_core, check that we are using the code workflow properly.

### After changes
When ever you make changes in the code, give the summery of the changes in a nice git commit message format.

### Web Performance
When making new features, performance and compatibility with WASM web builds must be considered. Avoid using features not supported in web builds. Test web builds frequently (`python test_web.py`).

### Updating documentation
When asked to update or audit documentation, check **all** of the following — not just CLAUDE.md files:
- `CLAUDE.md` (root)
- `crates/ironhold_core/src/CLAUDE.md`
- `crates/ironhold_core/tests/CLAUDE.md`
- Every `.md` file in `docs/` (`00_overview.md`, `10_architecture.md`, `20_data_formats.md`, `25_custom_shaders.md`, `30_runtime_events_and_logic.md`, `40_determinism_and_networking.md`, `50_roadmap_and_milestones.md`, `60_contributing.md`, `70_profiling.md`, `browser_tests.md`, `STATUS.md`)

> Rust-specific rules (GPU/WGSL alignment, physics, terrain, inspector) live in
> `crates/ironhold_core/src/CLAUDE.md`.
> Integration test setup rules live in `crates/ironhold_core/tests/CLAUDE.md`.
> Browser test suite documentation lives in `docs/browser_tests.md`.
