# Feature: Split `crates/ironhold_core/src/CLAUDE.md` by directory and topic (+ docs audience split)

_Status: Ready (pending the Phase-0 spike gate) — plan-reviewed 2026-10-07 by system-architect, ux-gamedesigner-reviewer and the Claude Code guide (all "needs work"); every finding is folded in below (R1-R14) and Frank resolved the open decisions the same day. The revised text has not been re-reviewed._
_Planned at: `b24078b` (2026-10-07); plan revised at `23e5424`_

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
2. **`feature/core_claude_md_split` (Phases 0-5)** — the `CLAUDE.md` split, built **additively** (all destinations
   exist before the parent is slimmed, in the last commit). Both branches are cut from `integration` (decision 6).

## Why it is not a mechanical split

The headings do not match the content (map §1): the 52k "Entity FSM" section is mostly per-player targeting,
action bars and animation; "Physics & FixedUpdate" is mostly one 20k `player.rs` jump/ground-detection saga; the
48k "Despawning" section holds the player-construction inventory and ~34k of local co-op/split-screen/gamepad
notes. So content was classified block by block (map §2: ~100 blocks, five read-only slice agents, ~30 rows
re-verified by the architect review). About 70k chars are reference material about *one subsystem* each; ~11-12k
chars are derivable or history and can be cut; the rest is directory-wide rules.

## Decisions (resolved with Frank, 2026-10-07)

1. **Mechanism: tool-neutral content + a thin Claude-Code layer.** Verified from the official Claude Code docs
   (code.claude.com/docs/en/memory): `.claude/rules/*.md` with `paths:` frontmatter (globs, `**`, brace expansion)
   loads "when Claude uses the Read, Write, or Edit tool on a file matching the pattern"; nested `CLAUDE.md` files
   load lazily on Read/Write/Edit of a file in their subtree, ancestors load too, and a directory file does **not**
   load for files in its parent (so nothing in `lib.rs`/`det_math.rs`/`inspector.rs`/`utils.rs` ever sees a
   directory file); `/context` lists what loaded. **Not documented:** `@import` inside a rules file (documented for
   `CLAUDE.md`, loaded eagerly), the combined-limit value, per-session reload behaviour. OpenCode's docs (not tested
   here): it auto-discovers `AGENTS.md`/`CLAUDE.md` per folder, `~/.claude/CLAUDE.md`, `~/.claude/skills/` (global);
   its `instructions` globs load every match always; **no path-scoped rules**. Therefore:
   - the **content** of each topic reference lives in `docs/dev/<topic>.md` (readable by humans, Claude Code and OpenCode);
   - **Claude Code** gets a tiny `.claude/rules/<topic>.md` stub: `paths:` frontmatter naming the exact source files
     + a plain **pointer** to the `docs/dev` file (an `@import` only if the Phase-0 spike proves it works and stays
     lazy);
   - **OpenCode and humans** get a one-line pointer in the directory `CLAUDE.md`;
   - skills only for a topic with no clean file mapping (none identified); `docs/` pages for designer-facing material.
   - **Safety=Y rules never live only in a stub** — only in the parent or a directory file that loads for every
     file they govern.
2. **Directory files:** `capabilities/CLAUDE.md`, `runtime/scene_manager/CLAUDE.md`, `schema/CLAUDE.md` **and a small
   `runtime/CLAUDE.md`** (reverses the earlier "no `runtime/CLAUDE.md`": `runtime/` has six files outside
   `scene_manager/` — `input.rs`, `actions.rs`, `messages.rs`, `model_spawner.rs`, `material_factory.rs`, `mod.rs` —
   and `input.rs` owns the gamepad-claim invariants). Rules that cross directories or constrain src-root files stay
   in the **parent**.
3. **Condense moved incident narrative** to the current-state rule plus a test/plan pointer; history lives in
   `planning/` and git.
4. **Docs are split by audience, with minimal churn.** Designer/library-user docs keep their paths (`00_overview`,
   `05_art_style`, `20_data_formats`, `25_custom_shaders`, `30_runtime_events_and_logic`; `STATUS.md` mixed, stays),
   so ~330 inbound references stay valid. Six developer docs move to `docs/dev/` (`10_architecture`,
   `40_determinism_and_networking`, `50_roadmap_and_milestones`, `60_contributing`, `70_profiling`,
   `browser_tests`; ~61 live references). `docs/dev/` also hosts the topic references.
5. **CLI documentation stays designer-reachable (Frank: some designers use the CLI).** `docs/60_contributing.md`
   "CLI tooling (`ironhold`)" (lines ~138-345: `validate`, `validate --strict`, `watch`, `stats`, `query`,
   `inspect glb|audio|texture`, the "Checks performed" list) is extracted into a designer-side page,
   `docs/15_authoring_tools.md`, **before** `60_contributing` moves; the page also says how to get/build the binary
   (`cargo build -p ironhold_cli` stays developer-side). `docs/20:106` and `docs/20:3880-3882` are repointed to it
   **by hand**; `docs/dev/60_contributing.md` keeps a one-line pointer.
6. **Release window:** new feature branches are cut from `integration` (extends the 2026-10-05 exception) from the
   start of Phase A until the next batch release, so nobody works against the old monolith or the old doc paths.
7. **Parent budget ~18k chars.** The moved-sections index (old heading -> new location, plus "`git blame -C -C -C`"
   and the base commit) lives in `docs/dev/`, with a one-line pointer in the parent (it only serves historical
   lookups and must not cost tokens on every core edit).
8. **No subdirectory-split fallback for `capabilities/`.** Moving `.rs` modules is a code refactor (git-blame churn,
   full workflow). The `capabilities/` budget is met by moving the animation pipeline (622-684, ~4.6k) and the
   viewport-aware widget saga (1463-1528, ~5.9k) into topics, leaving ordering/gotcha one-liners; per-file `paths:`
   stubs already give file granularity.
9. **Preload guidance** (for the later designer page): standardise on the pattern the shipped demo uses (state
   `entry_actions` of the state entered at gameplay start) and mention a `scene.ready` rule as the alternative;
   today `docs/20:3891`, `docs/30:50/359/368`, `STATUS.md:92` say "on `scene.ready`" while the core `CLAUDE.md` says
   `entry_actions`.

## Mitigations for the caveats (R1-R14)

| # | Caveat | Mitigation |
|---|---|---|
| R1 | A rule is lost in the move | **Block-ID audit** (replaces the earlier sentence-matching idea, which cannot pass once text is condensed): every map row has a stable block ID (`b:<first line>`); each destination carries an HTML-comment anchor `<!-- b:1023 -->`, or the block is in an explicit CUT table with a reason. Every Safety=Y row has a `governs:` glob list. The script asserts: each ID appears exactly once; each Safety=Y block sits in the parent or a directory file whose subtree covers *every* governed glob (never only in a stub); every `paths:` glob in a stub matches >=1 existing file; every file under `.claude/rules/` has a `paths:` key (a rule without it loads always — a silent token regression); no `above`/`below`/`see above`/`line \d+`/`four-site` hits remain unresolved. A modal-verb extraction (must/never/do not/don't/NOT/cannot/load-bearing/deliberate/hard invariant/`.before(`/`.after(`, tables, BAD/GOOD blocks) produces a **hint list for the reviewer**, not a gate. |
| R2 | A live citation dangles | **Citation contract** (map §4, extended): headings cited by live code keep their exact text in the parent (`determinism_lint.rs:8,:43,:201` incl. a failing-test message; `docs/40:102`; `camera.rs:878`, `nameplate.rs:49`; `clippy.toml:4`; `det_math.rs:30`); comments citing moved sections are updated in the same commit (`animation_resolver.rs:30`, `targeting.rs:101`, `validate.rs:1906,2336`, `respawning_gem.behavior.ron:7`); the hooks are rewritten and `python -m py_compile`d (`action_docs_reminder.py` must name the four `{self}`/`{target}` sites: `rewrite_self`, `rewrite_target`, `substitute_self_in_action`, `action_needs_target`; `capability_registration_reminder.py` -> `capabilities/CLAUDE.md`). A repo grep for `ironhold_core/src/CLAUDE.md` after each phase may show only intended hits. |
| R3 | Citations in plans/agent memory dangle | Done plans and agent memory are records and are not edited (the `docs/dev/` moved-sections index keeps them resolvable). **The ~20 *active* `planning/features/*.md` files that cite `src/CLAUDE.md` are live instructions** (e.g. `fade_out_despawn.md` `:1644`/`:1724-1744`, `ocean_simulation_demo.md` `:718`, `gameplay_pipeline_system_sets.md` `:30`, `step_offset_auto_step.md`, `airborne_ground_reacquisition.md`, `lock_on_camera_mode.md`, `tooltip_system.md`, `deferred_rendering.md`, `draggable_windows.md`): their pointers are updated in Phase 4. |
| R4 | `capabilities/` is too big | Budget <= ~20k chars (~55% reduction of the raw ~45k, not 30-40%) via decision 8; the audit enforces the budget. |
| R5 | Lazy loading misses a rule | Safety=Y rules stay always-loaded (decision 1); every on-demand topic gets a one-line pointer in its directory file; the **`lib.rs` schedule edges** get a parent section ("Load-bearing schedule edges in `lib.rs`", ~1.2k chars, one line per edge: targeting-chain consumers `lib.rs:~361`, `camera_blend_system` last in the camera chain `:~366-381`, `target_hud_update_system` after `split_screen_viewport_system`, dialogue `.after(button).after(interactable).before(fsm)` `:~336-338`, the audio mirror `:~257`) pointing at the detail, because `lib.rs` loads only the parent. |
| R6 | "see above/below" breaks | The audit greps every new file (R1); known cases are in map §5 ("four sites" is now five, "line 560", "Player-construction sites", "Gamepad-triggered hot join", `PlayerIndex "(see above)"`, the `OpenContainer` pointer). |
| R7 | Reviewers cannot see what vanished | Per phase: `debug-detective` runs the rule-loss audit adversarially against the CUT table and the hint list; `system-architect` checks no Safety=Y rule loads in fewer files than it governs; `ux-gamedesigner-reviewer` covers `docs/README.md`, the new authoring-tools page and any `docs/dev/` text designer docs link to, and its scope wording is updated accordingly (its current text says "`docs/` folder" and would silently include `docs/dev/`). |
| R8 | OpenCode loads differently | Decision 1. Do **not** add `AGENTS.md` files in subfolders (an `AGENTS.md` wins over `CLAUDE.md` in that folder and shadows ours) and do **not** point OpenCode's `instructions` at `.claude/rules/*.md`. Pointers in directory files are how OpenCode finds topics. |
| R9 | The change is big and hard to roll back | Additive build on a feature branch: every destination exists and passes the audit **before** the parent is slimmed (last commit); one commit per destination; base commit `b24078b` recorded; `git mv` of the six docs in its own commit and the redirect stubs in a **separate** commit (a move plus a stub at the old path in one commit breaks rename detection and `git log --follow`). |
| R10 | Known drift is copied into the new files | Fix while moving: FixedUpdate camera-follow wording (the camera chain runs in `Update`), the `audio_volume_var_system` note filed under "Audio file authoring", "four sites" -> five, "line 560", `opencode_compatibility.md`'s "~154 KB" note, `docs/60:171` (`inspect audio` "WAV and MP3" vs `docs/20` recommending OGG). |
| R11 | Path-scoped stubs rot silently | The audit's glob check (R1) runs in Phase 5 and is documented as the check to re-run whenever a core source file is renamed or split. |
| R12 | Spike results unknown | Phase 0 is a **hard gate** (below). |
| R13 | Branch timing | Decision 6. |
| R14 | Token math is optimistic | Measure **worst-case** files, not one per directory: `player.rs` would load parent + `capabilities/` + ground detection (14.8k) + gamepad routing (9.5k) = ~60k chars; `camera.rs` adds split-screen (~25k). chars/4 is optimistic for identifier-dense markdown: budget +/-20%. |

## Approach (phases)

### Phase A — `feature/docs_audience_split` (docs only; merges first)
- **A0** extract the CLI section from `docs/60_contributing.md` into `docs/15_authoring_tools.md` (designer-side;
  how to get/build the binary, validate/watch/inspect/stats/query, the "Checks performed" list); repoint
  `docs/20:106` and `:3880-3882` by hand; leave a pointer in `docs/60`.
- **A1** `git mv` the six developer docs into `docs/dev/` — **a pure-move commit**.
- **A2** a script rewrites live references (README, root `CLAUDE.md` "Updating documentation" list, agent prompts,
  hooks, code comments, `docs/*`, active plan files, `tools/asset_checker/CLAUDE.md`, `planning/backlog.md`); it
  **skips** lines hand-edited in A0, `docs/dev/60`'s own design-doc list (becomes relative) and `00_overview`'s
  "Where to read next" list, which is rewritten by hand (20 -> 30 -> 25 -> 05 -> the new tools page) so designers are
  not sent into `docs/dev/`.
- **A3** redirect stubs at the six old paths, in a separate commit; each stub's first line says who it is for
  ("Moved to `docs/dev/60_contributing.md` (engine developer docs). Designers: see `docs/README.md`."); stubs are not
  listed in `docs/README.md`.
- **A4** `docs/README.md` (audience-tagged index: designer / developer / mixed, honestly — `00_overview`, `30_*`,
  `STATUS` are mixed, with anchors for designers into `30`), the root `README.md` table split into "Using the
  library" / "Developing the library", and a minimal "Docs" link in `index.html` and `play.html` pointing at
  `docs/README.md` (the gallery has no docs links today); update the `ux-gamedesigner-reviewer` agent scope (R7).
- Grep afterwards for the six old paths: only stubs and historical records may contain them. Run `py_compile` on any
  rewritten hook. No `CLAUDE.md` content moves in Phase A.
- **Separate backlog item** (not Phase A): the new designer page for web loading/warmup (GLB preload + particle
  warmup + the `ParticleBudget` footgun, with the standardised preload guidance), `00_overview` cleanup, a README
  "Making a game (no Rust needed)" block.

### Phase 0 — spike and tooling (**hard gate**; no content moves)
Verify in Claude Code 2.1.292 with a throwaway `.claude/rules/_spike.md`, checking `/context` each time:
- a `paths:` rule loads on **Read**, **Edit** and **Write of a brand-new file** (adding a new capability is exactly
  when these rules matter) in `capabilities/`, and does not load for an unrelated file;
- the directory file and the rule load for **subagents** (the review agents);
- `paths:` globs resolve from a `../ironhold-lib-{slug}` **worktree** cwd;
- a stub with missing or mistyped frontmatter: confirm whether it loads unconditionally (then the audit check in R1 is
  mandatory);
- `@import` inside a rules file: works? lazy or eager? (If it fails, stubs are pointers only.)
- OpenCode (if cheap): touching a core file attaches the directory `CLAUDE.md`; the agent follows the pointer.
**Stop conditions:** if path-scoped rules fail for Write-of-new-file, subagents or worktrees, drop the stubs entirely
and use directory files + pointers only, then re-plan Phase 3 before continuing. Delete the spike; record results here.
Also: write the audit script (`tools/claude_md_audit.py`, header comment only) with the block-ID/anchor scheme, run
it against the *unchanged* file with every block mapped to itself (0 problems expected), and freeze the base.

### Phases 1-5 — `feature/core_claude_md_split` (additive, parent slimmed last)
- **Phase 1 — build the destinations** (one commit each, audit after each): `schema/CLAUDE.md` (adds the
  "new rendering `PrefabDef` field must be checked against the player path" rule and the `{self}`/`{target}`
  four-site rule), `runtime/CLAUDE.md` (gamepad invariants for `input.rs`: never bind to an already-claimed entity,
  ascending-`PlayerIndex` order, reset `PendingJoinGamepad` to `None` each run, one pad per frame; `ActionQueue` FIFO;
  `material_factory` notes), `runtime/scene_manager/CLAUDE.md`, `capabilities/CLAUDE.md` (<= ~20k), then the parent's
  new content (schedule-edges section, condensed determinism/try_despawn blocks) in a draft file.
- **Phase 2 — topic references**, one commit per topic (content in `docs/dev/<topic>.md`, the `.claude/rules` stub,
  the directory pointer): `player-ground-detection`, `split-screen-cameras-and-widgets` (also absorbs the ring
  visibility rows and the viewport-aware saga), `action-bar-input-routing`, `gamepad-routing`,
  `player-spawn-sites`, `animation-pipeline`, `lootable-corpse`; a WGSL stub with `paths: assets/**/*.wgsl` (today
  editing a shader loads no core rule at all). The SFX block (1063-1066) is CUT: `docs/20:1928` covers it — add the
  one missing clause ("OGG/MP3 decoder start-up is most noticeable on first play in WASM") to its `.wav` row.
- **Phase 3 — references:** map §4 citations and the active plans (R2/R3), hooks (+ `py_compile`), `tests/CLAUDE.md`
  dedupe (the `SpawnRegistry` fixture rule is already there), `AGENTS.md`, `ship-feature.md`.
- **Phase 4 — slim the parent (last commit)** and add the pointer to the `docs/dev/` moved-sections index; run the
  full audit; `cargo test -p ironhold_core --test determinism_lint`.
- **Phase 5 — verify and measure** with `/context` on worst-case files (R14) and one file per directory; record
  before/after chars and est. tokens. No WASM build or `pkg/` work (documentation and comment-only changes), so this
  follows the lightweight path of the code workflow.

## Tasks
- [ ] Phase A on `feature/docs_audience_split`: A0-A4, reviews (R7), merge
- [ ] Backlog item for the designer web-loading/warmup page + `00_overview` cleanup + README getting-started block
- [ ] Phase 0 spike (hard gate) + audit script + freeze the base
- [ ] Phase 1: schema, runtime, scene_manager, capabilities, parent draft
- [ ] Phase 2: topic references (one commit each) + WGSL stub + SFX cut
- [ ] Phase 3: live citations, active plans, hooks, `tests/CLAUDE.md`, `AGENTS.md`, command files
- [ ] Phase 4: slim the parent (last), moved-sections index in `docs/dev/`, full audit, determinism_lint
- [ ] Phase 5: `/context` measurement (worst-case files), per-phase reviews and a final full-file reviewer pass
- [ ] Backlog items for splitting `docs/20_data_formats.md` and auditing `docs/30_runtime_events_and_logic.md` (logged 2026-10-07)

## Acceptance criteria
- Every map block ID appears exactly once in the new set or in the CUT table, and every Safety=Y rule loads for every
  source file it governs (checked by the audit's governs-glob test, not by text presence).
- Every live citation (map §4 plus the review additions) resolves to text that exists at the cited location; the
  ~20 active plans cite valid locations.
- Budgets (measured with `/context`, +/-20%): parent <= ~18k chars; `schema/` <= ~6k; `runtime/` <= ~4k;
  `runtime/scene_manager/` <= ~20k; `capabilities/` <= ~20k; a core edit in each directory loads the parent + that
  directory's file (+ matching rule stubs), and the worst-case files (`player.rs`, `camera.rs`) are recorded.
- `cargo test -p ironhold_core --test determinism_lint` passes and its failure message still names a section that
  exists.
- OpenCode touching a core file attaches the parent and the directory file; no `AGENTS.md` exists in a subfolder; its
  `instructions` does not glob `.claude/rules`.
- Docs: every developer doc is under `docs/dev/`, designer docs have not moved, no live file references an old
  developer-doc path, the CLI documentation is reachable from `docs/20` without entering `docs/dev/`, and
  `docs/README.md` tags every doc honestly.
