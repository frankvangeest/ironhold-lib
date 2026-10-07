# Feature: Split `crates/ironhold_core/src/CLAUDE.md` by directory and topic (+ docs audience split)

_Status: Ready (pending the Phase-0 spike gate) — plan-reviewed twice on 2026-10-07 (system-architect, ux-gamedesigner-reviewer, Claude Code docs check; both passes "needs work"); every finding is folded in below (R1-R22) and Frank resolved the open decisions. The second revision has not been re-reviewed._
_Planned at: `b24078b` (2026-10-07); revised at `dd62494` (first pass) and the commit following `1ad337c` (second pass)_

## What

`crates/ironhold_core/src/CLAUDE.md` is 160,039 chars (~40k est. tokens) and loads whenever *any* file under
`crates/ironhold_core/src/` is touched — editing one line of `schema/` costs the same as editing the physics code.
Split it so a typical core edit loads ~5-10k tokens instead: a lean parent for rules that hold crate-wide, a short
`CLAUDE.md` per directory for directory-wide rules, and on-demand topic references that load only when their files
are touched. No rule may be lost, no existing citation may silently dangle, and no safety-critical rule may end up
where it does not load. Because the topic references and the designer-relevant fragments need a home, the docs are
first split by audience (designer docs vs developer docs). Documentation and comment-only changes: no engine
behaviour changes.

## Two deliverables, two branches

1. **`feature/docs_audience_split` (Phase A)** — small, scripted, independently valuable; merges first.
2. **`feature/core_claude_md_split` (Phases 0-5)** — built **additively** (all destinations exist and pass the audit
   before the parent is slimmed, in the last commit). Both branches are cut from `integration` (decision 6).

## Why it is not a mechanical split

The headings do not match the content (map §1): the 52k "Entity FSM" section is mostly per-player targeting,
action bars and animation; "Physics & FixedUpdate" is mostly one 20k `player.rs` jump/ground-detection saga; the
48k "Despawning" section holds the player-construction inventory and ~34k of local co-op/split-screen/gamepad
notes. So content was classified block by block (map §2: ~100 blocks, five read-only slice agents, ~30 rows
re-verified by the architect). About 70k chars are reference material about *one subsystem* each; ~11-12k chars
are derivable or history and can be cut; the rest is directory-wide rules.

## Decisions (resolved with Frank, 2026-10-07)

1. **Mechanism: tool-neutral content + a thin Claude-Code layer.** Verified from the official Claude Code docs
   (code.claude.com/docs/en/memory): `.claude/rules/*.md` with `paths:` frontmatter (globs, `**`, brace expansion)
   loads "when Claude uses the Read, Write, or Edit tool on a file matching the pattern"; nested `CLAUDE.md` files
   load lazily on Read/Write/Edit of a file in their subtree, ancestors load too, and a directory file does **not**
   load for files in its parent (so nothing in `lib.rs`/`det_math.rs`/`inspector.rs`/`utils.rs` ever sees a
   directory file); `/context` lists what loaded. **Not documented:** `@import` inside a rules file (documented for
   `CLAUDE.md`, loaded eagerly), the combined-limit value, per-session reload behaviour, whether block HTML comments
   are stripped from loaded context. OpenCode's docs (not tested): it auto-discovers `AGENTS.md`/`CLAUDE.md` per
   folder, `~/.claude/CLAUDE.md`, `~/.claude/skills/` (global); its `instructions` globs load every match always;
   **no path-scoped rules**. Therefore:
   - the **content** of each topic reference lives in `docs/dev/<topic>.md` (humans, Claude Code and OpenCode);
   - **Claude Code** gets a tiny `.claude/rules/<topic>.md` stub: `paths:` frontmatter naming the exact source files
     + a plain **pointer** to the `docs/dev` file (`@import` only if the Phase-0 spike proves it works and stays lazy);
   - **OpenCode and humans** get a one-line pointer in the directory `CLAUDE.md`;
   - skills only for a topic with no clean file mapping (none identified); `docs/` pages for designer-facing material.
   - **Safety=Y rules never live only in a stub** — only in the parent or a directory file that loads for every
     file they govern. Every row labelled "Y-ish", "Y partial" or "RON-authoring Y" in the map is resolved to Y or N
     (rule: Y if violating it fails silently or breaks determinism/physics/WASM/data); rows whose violation only
     matters to RON authors and is already in a designer doc are N with a pointer.
2. **Directory files:** `capabilities/CLAUDE.md`, `runtime/scene_manager/CLAUDE.md`, `schema/CLAUDE.md` **and a small
   `runtime/CLAUDE.md`** (`runtime/` has six files outside `scene_manager/` — `input.rs`, `actions.rs`,
   `messages.rs`, `model_spawner.rs`, `material_factory.rs`, `mod.rs`; `input.rs` owns `gamepad_bind_system`,
   `unclaimed_gamepad_trigger_system` **and `input_translator_system`**). Rules that cross directories or constrain
   src-root files stay in the **parent**. Each block ID has exactly one owner (the `ActionQueue` FIFO rule is owned by
   the parent's pipeline block, not `runtime/CLAUDE.md`).
3. **Condense moved incident narrative** to the current-state rule plus a test/plan pointer; history lives in
   `planning/` and git.
4. **Docs are split by audience, with minimal churn.** Designer/library-user docs keep their paths (`00_overview`,
   `05_art_style`, `20_data_formats`, `25_custom_shaders`, `30_runtime_events_and_logic`; `STATUS.md` mixed), so
   ~330 inbound references stay valid. Six developer docs move to `docs/dev/` (`10_architecture`,
   `40_determinism_and_networking`, `50_roadmap_and_milestones`, `60_contributing`, `70_profiling`, `browser_tests`).
   `docs/dev/` also hosts the topic references.
5. **CLI documentation stays designer-reachable.** The "CLI tooling (`ironhold`)" section of
   `docs/60_contributing.md` (~lines 138-345) is extracted into a designer-side `docs/15_authoring_tools.md`
   **before** `60_contributing` moves. `docs/15` is the **single canonical list of checks**
   (one row per diagnostic code: code | error or `--strict` | what it means / what breaks if ignored | how to fix +
   a `docs/20` link); `docs/dev/60` keeps only implementation notes keyed by the same codes and a "Adding a validate
   check" note (every new check adds a row to `docs/15`). **Every line citing "Checks performed" / "CLI tooling" is
   repointed by hand in A0** (root `CLAUDE.md` Tools table [already removed from the root, re-check], `tools/asset_checker/CLAUDE.md:7`,
   `planning/features/attacker_identity_on_hit_events.md:391`, `ui_flex_group.md:491`, `docs/20:106`, `docs/20:3880-3882`).
   **Getting the binary:** Frank wants a **prebuilt `ironhold` binary on a GitHub release**, with any committed
   binary in its **own isolated commit** like `pkg/` (never combined with other changes; never cited by hash in
   planning markdown) so old ones can be pruned if the repo grows. Publishing it is a separate backlog item; until it
   exists `docs/15` §2 says honestly "build it with `cargo build -p ironhold_cli`; a prebuilt download is planned"
   and is updated when the release lands.
6. **Release window:** new feature branches are cut from `integration` (extends the 2026-10-05 exception) from the
   start of Phase A until the next batch release.
7. **Parent budget ~18k chars** — achievable only if the `try_despawn` block (2.3k -> ~0.6k) and the determinism
   block (3.8k, heading verbatim, marker mechanics condensed because the lint's failure message teaches them) are
   actually condensed. The moved-sections index (old heading -> new location, base commit, `git blame -C -C -C`)
   lives in `docs/dev/`, with a one-line pointer in the parent.
8. **No subdirectory-split fallback for `capabilities/`** (moving `.rs` modules is a code refactor). Its budget
   (<= ~20k; tally ~23-24k) is met by moving the animation pipeline (622-684, but keep the row-670-675 WASM
   one-liner in CAP) and the viewport-aware saga (1463-1528) into topics; WGSL rules go to `assets/CLAUDE.md`
   (decision 12); condensing rows 1383-1405 (2k -> ~0.8k), 286-314 (2.4k -> ~1k) and 323-351.
9. **Preload guidance** (for the later designer page): standardise on the pattern the shipped demo uses (state
   `entry_actions` of the state entered at gameplay start); mention a `scene.ready` rule as the alternative.
10. **`lib.rs` ordering edges are protected where the code is edited, not only in the parent.** About 15 edges in
    `lib.rs` are guarded today only by `CLAUDE.md` prose, and `lib.rs` loads only the parent. Add a one-line
    `// load-bearing: <why / what depends on it>` comment next to each such edge in `lib.rs` (comment-only change,
    done in Phase 3), and keep in the parent a **3-line rule** plus the handful of edges that cross plugin boundaries.
    Edges to cover (architect list): interpreter chain order `:264-277` (fsm -> entity_fsm -> `flush_pending_intent`
    -> executor -> … -> `drain_spawn_queue_system` -> `drain_dynamic_stat_ui_system`);
    `unclaimed_gamepad_trigger_system.before(fsm_interpreter_system)` `:241`; FixedUpdate chain `:319-329`
    (`gamepad_bind_system` before `input_translator_system`, `player_view_box_clamp_system` after
    `player_movement_system`); `interactable_system.before(fsm)` `:332`; dialogue `.after(button).after(interactable)
    .before(fsm)` `:336-338`; visual chain `:343-369` (`animation_resolver_system` before `animation_playback_system`,
    `dynamic_split_screen_system` after `party_camera_follow_system` and before `split_screen_viewport_system`,
    `split_viewport_player_label_update_system` after `split_screen_viewport_system`, `fly_camera_system` after
    `camera_shake_system`, `camera_blend_system` last in the camera chain `:366-381`);
    `world_label_screen_pos_system.after(camera_blend_system)` `:380` and
    `nameplate_visibility_system.after(world_label_screen_pos_system)` `:383`; targeting-chain consumers `:361`;
    `target_hud_update_system` after `split_screen_viewport_system`; the audio mirror `:257`.
11. **Concurrent edits (Frank: port forward, no freeze).** Other features will edit `src/CLAUDE.md` during the
    window (workflow step 5). At Phase 4, any prose added to `integration` since the base commit is ported forward:
    new block IDs, placed in the right new file, audit re-run. The first commit on the feature branch applies the R10
    fixes to the monolith itself so reviewers who load both old and new files never see contradictory text.
12. **WGSL rules (711-720) go to `assets/CLAUDE.md`** (exists; loads for every `.wgsl` and does not depend on the
    Phase-0 stub outcome), plus the "test in a web build" one-liner in the parent. No `.wgsl` stub.
13. **Docs link (Frank: GitHub-rendered).** The gallery "Docs" link points at
    `https://github.com/frankvangeest/ironhold-lib/blob/main/docs/README.md` (the repo has `.nojekyll`, so a relative
    `docs/README.md` would show raw markdown on Pages), opening in a new tab (`target="_blank" rel="noopener"`); it
    needs a connection and shows `main`, not the local checkout.

## Mitigations for the caveats (R1-R22)

| # | Caveat | Mitigation |
|---|---|---|
| R1 | A rule is lost in the move | **Block-ID audit** (sentence matching cannot pass once text is condensed). Every map row has a stable block ID (`b:<first line>`); rows the map splits across destinations get **sub-IDs** (`b:1530.rule`, `b:1530.ref`; ~13 rows: 108, 284, 286-314, 452-462, 575-585, 856-882, 1121-1200, 1530-1574, 1606-1636, 1657-1667, 1668-1693, 1713-1729, 1731-1752) and the Safety check applies to the part carrying the rule. Each destination carries an HTML-comment anchor `<!-- b:1023 -->` (Phase 0 checks whether Claude Code strips block comments from loaded context; if not, anchors cost ~1.5-2k chars across files and must be budgeted or kept in a sidecar index). Every Safety=Y sub-ID has a `governs:` glob list written **into the map now** (default for "any new system/spawn site/RenderLayers consumer" rules: the widest covering directory, usually the parent). The script asserts: each ID exactly once (a **destinations-only mode** until Phase 4, while the parent still holds everything); each Safety=Y block sits in the parent or a directory file whose subtree covers *every* governed glob (never only in a stub); every topic has a pointer line in its directory file; every `paths:` glob in a stub matches >=1 existing file; every file under `.claude/rules/` has a `paths:` key (a rule without it loads always); no `above`/`below`/`see above`/`line \d+`/`four-site` hits remain unresolved. A modal-verb extraction (must/never/do not/don't/NOT/cannot/load-bearing/deliberate/hard invariant/`.before(`/`.after(`, tables, BAD/GOOD blocks) is a **hint list for the reviewer, not a gate**. Blind spot documented: it only checks consistency with author-written governs globs. |
| R2 | A live citation dangles | **Citation contract** (map §4 + §6/§7): headings cited by live code keep their exact text in the parent (`determinism_lint.rs:8,:43,:201`; `docs/40:102`; `camera.rs:878`, `nameplate.rs:49`; `clippy.toml:4`; `det_math.rs:30`); comments citing moved sections are updated in the same commit (`animation_resolver.rs:30`, `targeting.rs:101`, `validate.rs:1906,2336`, `respawning_gem.behavior.ron:7`, `docs/20_data_formats.md:3292`); hooks rewritten and `python -m py_compile`d (`action_docs_reminder.py` must name the four `{self}`/`{target}` sites `rewrite_self`, `rewrite_target`, `substitute_self_in_action`, `action_needs_target` — that "four" is correct, do not "fix" it to five; `capability_registration_reminder.py` -> `capabilities/CLAUDE.md`). A repo grep for `ironhold_core/src/CLAUDE.md` after each phase may show only intended hits. |
| R3 | Citations in plans/agent memory dangle | Done plans and agent memory are records (the `docs/dev/` moved-sections index keeps them resolvable). **~20 *active* `planning/features/*.md` files citing `src/CLAUDE.md` are live instructions** and are updated in Phase 3 (e.g. `fade_out_despawn.md:65,140`, `ocean_simulation_demo.md:46`, `gameplay_pipeline_system_sets.md:58,203`, `step_offset_auto_step.md`, `airborne_ground_reacquisition.md:85`, `lock_on_camera_mode.md`, `tooltip_system.md`, `deferred_rendering.md`, `draggable_windows.md`). |
| R4 | `capabilities/` is too big | Decision 8; the audit enforces the budget. |
| R5 | Lazy loading misses a rule | Safety=Y rules stay always-loaded (decision 1); every topic has a pointer in its directory file (audited); `lib.rs` edges per decision 10. Seven placements corrected (map §7): the gamepad "consumers take `Option<&BoundGamepad>`" rule gets a CAP line (it governs `action_bar`/`camera`/`interactable`/`targeting`); row 670-675 gets a CAP one-liner; row 1224-1260 also gets an SM line (`Friction` initial value set in `entity_spawner.rs:~1140`); row 575-585's `tick_delayed_events_system` line (it lives in `lib.rs:~758`) goes in the parent or nowhere; row 247-255 also governs the player-construction sites in SM; WGSL -> `assets/CLAUDE.md`. |
| R6 | "see above/below" breaks | The audit greps every new file (R1); known cases are in map §5. |
| R7 | Reviewers cannot see what vanished | Per phase: `debug-detective` runs the rule-loss audit against the CUT table and the hint list; `system-architect` checks no Safety=Y rule loads in fewer files than it governs; `ux-gamedesigner-reviewer` covers designer docs (scope in A4). |
| R8 | OpenCode loads differently | Decision 1. No `AGENTS.md` in subfolders; OpenCode's `instructions` never globs `.claude/rules`. |
| R9 | Hard to roll back | Additive build; one commit per destination; base commit `b24078b` recorded; `git mv` of the six docs and the redirect stubs in **separate** commits. |
| R10 | Known drift is copied into the new files | **Fixed first, in the monolith** (decision 11): FixedUpdate camera-follow wording (the camera chain runs in `Update`), the `audio_volume_var_system` note under "Audio file authoring", "four sites" -> five **for the player-construction inventory only**, "line 560", "chained back-to-back" at `CLAUDE.md:624` (the resolver is `lib.rs:344`, playback `:368`, with 12 camera systems between), `opencode_compatibility.md`'s "~154 KB", `docs/60:171` (`inspect audio` "WAV and MP3" vs `docs/20` recommending OGG), and the stale map header. |
| R11 | Path-scoped stubs rot silently | The audit's glob check (R1) is re-run whenever a core source file is renamed or split. |
| R12 | Spike results unknown | Phase 0 is a hard gate. |
| R13 | Branch timing | Decision 6. |
| R14 | Token math is optimistic | Measure **worst-case** files (`player.rs` = parent + `capabilities/` + ground detection 14.8k + gamepad 9.5k ≈ 60k chars; `camera.rs` adds split-screen ~25k); chars/4 is optimistic for identifier-dense markdown: budget +/-20%. |
| R15 | Wrong citation after the CLI extraction | Decision 5: A0 repoints every `Checks performed`/`CLI tooling` line by hand; A2 uses a **file-level exclusion list plus a reviewed diff**, not "skip hand-edited lines" (line numbers shift after A0). |
| R16 | Gallery link shows raw markdown | Decision 13. |
| R17 | A2's file list is incomplete | Add `planning/claude_suggestions.md:145,407,417,515` and `planning/backlog.md:7,65,149,226` (11 active plan files, not ~12). Nothing reads the six docs programmatically (checked: tests, CLI, hooks, `.githooks`, `.opencode`, root `*.py`, HTML), and the six docs contain no relative markdown links. |
| R18 | Phase-0 gaps | Also test: an agent whose cwd is the primary checkout reading a file in the sibling `../ironhold-lib-{slug}` worktree (do nested files / `paths:` rules load?); whether block HTML comments are stripped. Stop condition corrected (below). |
| R19 | Budget tally | Architect's tally (after §6/§7): parent ~16.8k raw + 1.2k edges -> 18.0k (needs the condensing in decision 7); `schema/` ~5k; `runtime/` ~2.5k; `runtime/scene_manager/` ~20k (at the cap; cut candidates: `{new_id}` prose 3.7k, 1657-1667 4k, audio 1.6k); `capabilities/` ~23-24k raw (most at risk, see decision 8). |
| R20 | Designer path after Phase A | `00_overview` "Where to read next" is rewritten by hand as 20 -> 15 -> 30 -> 25 -> 05; root README links `00_overview#getting-started-in-5-minutes` first in its designer half; the preload advice, `docs/15` and the warmup page are the backlog item. |
| R21 | Reviewer scope creep | A4 narrows the `ux-gamedesigner-reviewer` agent and the root `CLAUDE.md` UX-review trigger to "`docs/` excluding `docs/dev/`", else every `docs/dev/` topic commit would trigger a UX review. |
| R22 | Stub wording | A3 stubs use real markdown links and a heading (below). |

## Approach (phases)

### Phase A — `feature/docs_audience_split` (docs only; merges first)
- **A0** extract the CLI section into `docs/15_authoring_tools.md`; repoint every `Checks performed`/`CLI tooling`
  citation by hand (decision 5); leave a pointer in `docs/60` written with the post-move relative path
  (`../15_authoring_tools.md`) or fixed in A2. Outline of `docs/15`:
  1. what the `ironhold` tool is (optional; `ironhold_cli validate` elsewhere is the same tool) + a "if you only have
     the web build" box (what you lose, where errors show up instead: the DevTools console);
  2. getting the tool (decision 5: build today; prebuilt release planned);
  3. the edit-check loop (`watch`, then `validate`, exit codes, when to add `--strict`);
  4. reading a report (errors vs strict warnings; fix the first parse error first);
  5. what `validate` catches — tables grouped by topic (typos and missing keys, files and path case, logic wiring,
     scenes and UI layout, cameras and input, players and movement, items/dialogue/merchants, text/non-ASCII; a
     separate strict-only table); designer-facing = every command, the diagnostic codes and the symptom each check
     prevents; developer-facing (stays in `docs/dev/60`) = `cargo build`, the Python `glb_inspector`/Blender note, the
     `Path::exists()`/NTFS, asset-root "corroboration", `entity_spawner.rs`, `project_loader.rs`, `schema/project.rs`
     internals, the `claude_suggestions.md` gap pointer and the runtime-warning rationale; check first whether the
     human-readable output prints code names — if not, key the table on message text;
  6. inspect assets before authoring (`inspect glb/texture/audio`, which values to copy into RON);
  7. looking things up (`stats`, `query *`, `--keys-only`, `--filter`, `--json`);
  8. "validate is clean but it still breaks" (known blind spots, e.g. scenes reachable only through an `ActionBar`
     slot's `do_actions`). Small fixes while moving: the machine-specific `C:\git\rust\...` path in the `watch`
     sample output (~line 191).
- **A1** `git mv` the six developer docs into `docs/dev/` — **a pure-move commit** (create `docs/README.md` first or
  accept the brief gap inside the branch).
- **A2** a script rewrites live references (file-level exclusion list for hand-edited files + a reviewed diff;
  `docs/dev/60`'s own design-doc list becomes relative; list per map §6 and R17); `python -m py_compile` on any
  rewritten hook.
- **A3** redirect stubs at the six old paths, in a separate commit, with a heading and real markdown links:
  `# Moved` / "This page is now [docs/dev/60_contributing.md](dev/60_contributing.md) (engine developer docs)." /
  "Making a game? Start at [docs/README.md](README.md)." The 60 stub adds "Using the `ironhold` CLI (validate, watch,
  inspect)? See [15_authoring_tools.md](15_authoring_tools.md)."; the 70 stub adds the DevTools section link. The
  other four need only the first and last lines. Stubs are not listed in `docs/README.md`.
- **A4** `docs/README.md` with a "Start here" line (00 Getting started -> 20 -> 15 -> 30 anchors -> 25/05) and this
  table (verify the hand-derived anchors on GitHub after the merge):

  | Doc | Audience | Purpose |
  |---|---|---|
  | `00_overview.md` | Mixed | "Getting started in 5 minutes" (designer); goals, repo layout, implementation snapshot are developer material and partly stale |
  | `05_art_style.md` | Designer / artist | visual style, palette, texture and asset guidance |
  | `15_authoring_tools.md` | Designer (optional CLI) | check, watch and inspect your project with the `ironhold` tool |
  | `20_data_formats.md` | Designer | full RON reference |
  | `25_custom_shaders.md` | Technical artist | writing WGSL; the alignment/uniform-packing sections are engine detail |
  | `30_runtime_events_and_logic.md` | Mixed | designers: "Project logic: state_machine.ron" and "Entity FSM" by anchor; the rest is vision/planned/execution model |
  | `STATUS.md` | Mixed | what works today (designer); "Engine ABI" and milestones developer |
  | `dev/10_architecture.md` | Developer | crate layout and runtime pipeline |
  | `dev/40_determinism_and_networking.md` | Developer | determinism and multiplayer design |
  | `dev/50_roadmap_and_milestones.md` | Developer | milestones and roadmap |
  | `dev/60_contributing.md` | Developer | workflow, tests, branching, adding validate checks |
  | `dev/70_profiling.md` | Developer (one designer section) | profiling; "Browser DevTools — GPU timing (web)" is designer-usable |
  | `dev/browser_tests.md` | Developer | headless browser test suite |

  Also in A4: split the root `README.md` table into "Using the library" / "Developing the library" (add `05` and
  `15`); the **Docs link** (decision 13): in `index.html` after "Assets" in `.header-nav` and the same in
  `assets.html`; in `play.html` inside `<nav class="top-bar">` after `#project-name` as `<a class="back-link
  docs-link" target="_blank" rel="noopener">Docs</a>` with `margin-left:auto; flex-shrink:0; white-space:nowrap` and
  `min-width:0` on `.project-name` (inside the nav, so `?testing=1` and `?bar=hidden` hide it; the bar stays 44 px, so
  baselines are not at risk); update `.claude/agents/ux-gamedesigner-reviewer.md` scope ("Designer docs:
  `docs/README.md`, the numbered pages directly in `docs/` (00, 05, 15, 20, 25, 30) and `docs/STATUS.md`;
  `docs/dev/` is out of scope except a section a designer doc links to; optionally the `ironhold` CLI (docs/15),
  never assume a designer has it") and its frontmatter trigger; narrow the root `CLAUDE.md` UX-review trigger (R21);
  `system-architect` covers `docs/dev/`.
- Afterwards grep for the six old paths: only stubs and historical records may contain them. No `CLAUDE.md` content
  moves in Phase A.
- **Separate backlog items:** the designer web-loading/warmup page + `00_overview` cleanup + README getting-started
  block; publishing the prebuilt `ironhold` binary (decision 5).

### Phase 0 — spike and tooling (**hard gate**; no content moves)
Verify in Claude Code 2.1.292 with a throwaway `.claude/rules/_spike.md`, checking `/context` each time:
- a `paths:` rule loads on **Read**, **Edit** and **Write of a brand-new file** in `capabilities/`, and not for an
  unrelated file;
- the directory file and the rule load for **subagents**, and for an agent whose cwd is the primary checkout reading
  a file in a sibling `../ironhold-lib-{slug}` worktree (R18);
- `paths:` globs resolve from a worktree cwd;
- a stub with missing or mistyped frontmatter: does it load unconditionally? (If yes, the R1 `paths:`-key check is
  mandatory.)
- `@import` inside a rules file: works? lazy or eager? (If it fails, stubs are pointers only.)
- whether block HTML comments (`<!-- b:NNN -->`) are stripped from loaded context;
- OpenCode (if cheap): touching a core file attaches the directory `CLAUDE.md`; the agent follows the pointer.
**Stop condition:** if path-scoped rules fail for Write-of-new-file, subagents or worktrees, **drop the stubs** and keep
directory files + pointers only — Phases 1-4 still make sense (Safety rules never lived in stubs; topics become
pointer-only), only Phase 2's stub items change. Delete the spike; record results here.
Also: write the audit script (`tools/claude_md_audit.py`, header comment only) with the block-ID/sub-ID/anchor scheme,
run it against the *unchanged* file with every block mapped to itself (0 problems expected), and freeze the base.

### Phase 0 results (2026-10-07, Claude Code 2.1.292, `feature/core_claude_md_split` worktree)
Method: throwaway rules and directory file carrying unique canary tokens (`SPIKE_<WORD><digit>`), headless `claude -p` runs on `--model haiku` (one question per session, the model reports canaries from its own context). `/context` was not used, so every result below rests on the model's self-report; the positive results were consistent across runs, and the controls (an unrelated Read, an import line removed) behaved as expected. Spike files deleted.

| Question | Result | Consequence |
|---|---|---|
| `paths:` rule and a directory `CLAUDE.md` load on **Read** in the subtree | Yes (neither is in context at session start) | stubs and directory files work |
| ... on **Write of a brand-new file** in the subtree | Yes (both loaded, no prior Read) | no stop condition triggered |
| ... on an **unrelated** file (`runtime/mod.rs`) | No | negative control passes |
| ... on **Edit** | Not isolated: Edit requires a prior Read, so it is covered by the Read result | none |
| loaded for a **subagent** reading a subtree file | Yes (the subagent reported both; whether before or after its Read is ambiguous in its report) | reviewers get them when they Read |
| `paths:` globs resolve from a **worktree cwd** | Yes | `.claude/rules` stubs work in `../ironhold-lib-{slug}` |
| cwd = **primary checkout**, reading a file in a sibling worktree via `--add-dir` | **No**: neither the rule nor the directory `CLAUDE.md` loaded, nor any rule from that worktree's `.claude/` | **R18 is real**: an agent must run with cwd inside the worktree, or its prompt must tell it to Read the directory `CLAUDE.md` of each touched directory itself. Applies to every review agent launched from the primary checkout |
| stub with **no frontmatter** | Loads at session start (always) | the R1 "every rule has a `paths:` key" check is **mandatory** |
| stub with a **mistyped key** (`path:` for `paths:`) | Loads at session start (always) | same: the check must verify the exact key, not just that frontmatter exists |
| `@import` inside a path-scoped rule | **Eager**: the imported file loaded at session start even though the importing rule did not; removing the import line removed it | **stubs are pointers only, never `@import`** (an import defeats the lazy loading the stubs exist for) |
| block HTML comments `<!-- b:NNN -->` in a rule | **Stripped** from loaded context (the rule's quoted text had no comment line) | anchors cost no context tokens; the audit script reads the files, not the loaded context |
| OpenCode directory-file attach | Not run (not cheap here) | stays an assumption from its docs; check once when OpenCode is next used |

**Gate verdict: PASS, with three plan adjustments** (none stops the work): (1) stubs are pointer-only, no `@import`; (2) the R1 audit rejects any `.claude/rules/*.md` whose frontmatter lacks the exact key `paths:`; (3) R18 becomes a concrete rule: review agents and any other agent that reads worktree files must have cwd inside the worktree (or be told to Read the directory `CLAUDE.md` files for what they touch) — to be written into `ship-feature.md` / the review prompts in Phase 3.

### Phases 1-5 — `feature/core_claude_md_split` (additive, parent slimmed last)
- **Phase 1 — build the destinations** (first commit: the R10 fixes in the monolith; then one commit each, audit
  after each): `schema/CLAUDE.md` (adds "a new rendering `PrefabDef` field must be checked against the player path"
  and the `{self}`/`{target}` four-site rule), `runtime/CLAUDE.md` (gamepad invariants for `input.rs`: never bind to
  an already-claimed entity, ascending-`PlayerIndex` order, reset `PendingJoinGamepad` to `None` each run, one pad
  per frame, `input_translator_system` notes; `material_factory` notes), `runtime/scene_manager/CLAUDE.md`,
  `capabilities/CLAUDE.md` (<= ~20k; includes the gamepad "consumers take `Option<&BoundGamepad>`" line),
  `assets/CLAUDE.md` additions (WGSL rules), then the parent's new content in a draft file.
- **Phase 2 — topic references**, one commit per topic (content in `docs/dev/<topic>.md`, the `.claude/rules` stub,
  the directory pointer): `player-ground-detection` (hard rules stay in CAP), `split-screen-cameras-and-widgets`
  (also absorbs ring visibility and the viewport saga), `action-bar-input-routing`, `gamepad-routing`,
  `player-spawn-sites`, `animation-pipeline`, `lootable-corpse`. The SFX block (1063-1066) is CUT: `docs/20:1928`
  covers it — add "OGG/MP3 decoder start-up is most noticeable on first play in WASM" to its `.wav` row.
- **Phase 3 — references:** map §4 citations and the active plans (R2/R3), the `// load-bearing` comments in `lib.rs`
  (decision 10), hooks (+ `py_compile`), `tests/CLAUDE.md` dedupe (the `SpawnRegistry` fixture rule is already
  there), `AGENTS.md`, `ship-feature.md`.
- **Phase 4 — port forward and slim the parent (last commit):** port forward concurrent edits (decision 11), slim
  the parent, add the pointer to the `docs/dev/` moved-sections index; full audit (non-destinations-only mode);
  `cargo test -p ironhold_core --test determinism_lint`.
- **Phase 5 — verify and measure** with `/context` on worst-case files (R14) and one file per directory; record
  before/after chars and est. tokens. No WASM build or `pkg/` work (documentation and comment-only changes), so this
  follows the lightweight path of the code workflow.

## Tasks
- [x] Phase A on `feature/docs_audience_split`: A0-A4, reviews (R7), merge (`c188550`, 2026-10-07)
- [ ] Backlog items: designer web-loading/warmup page + overview cleanup + README block; publish prebuilt `ironhold` binary
- [ ] Phase 0 spike (hard gate) + audit script + freeze the base
- [ ] Phase 1: R10 fixes in the monolith, then schema, runtime, scene_manager, capabilities, assets, parent draft
- [ ] Phase 2: topic references (one commit each) + SFX cut
- [ ] Phase 3: live citations, active plans, `lib.rs` load-bearing comments, hooks, `tests/CLAUDE.md`, `AGENTS.md`, command files
- [ ] Phase 4: port forward, slim the parent (last), moved-sections index in `docs/dev/`, full audit, determinism_lint
- [ ] Phase 5: `/context` measurement (worst-case files), per-phase reviews and a final full-file reviewer pass
- [ ] Backlog items for splitting `docs/20_data_formats.md` and auditing `docs/30_runtime_events_and_logic.md` (logged 2026-10-07)

## Acceptance criteria
- Every map block ID/sub-ID appears exactly once in the new set or in the CUT table, and every Safety=Y rule loads for
  every source file it governs (the audit's governs-glob test, not text presence).
- Every live citation (map §4, §6, §7) resolves to text that exists at the cited location; the ~20 active plans cite
  valid locations; each `lib.rs` edge guarded only by prose now has a `// load-bearing` comment.
- Budgets (measured with `/context`, +/-20%): parent <= ~18k chars; `schema/` <= ~6k; `runtime/` <= ~4k;
  `runtime/scene_manager/` <= ~20k; `capabilities/` <= ~20k; the worst-case files (`player.rs`, `camera.rs`) are recorded.
- `cargo test -p ironhold_core --test determinism_lint` passes and its failure message still names a section that exists.
- OpenCode touching a core file attaches the parent and the directory file; no `AGENTS.md` exists in a subfolder; its
  `instructions` does not glob `.claude/rules`.
- Docs: every developer doc is under `docs/dev/`; designer docs have not moved; no live file references an old
  developer-doc path; the CLI documentation is reachable from `docs/20` without entering `docs/dev/` and `docs/15` is
  the single canonical list of checks; the gallery Docs link opens the GitHub-rendered `docs/README.md` in a new tab;
  `docs/README.md` tags every doc honestly.
