# Feature: Split `crates/ironhold_core/src/CLAUDE.md` by directory and topic

_Status: Draft — section map done (`planning/investigations/core_claude_md_split_map.md`); Frank resolved the mechanism, directory-file, condensing and docs-audience decisions on 2026-10-07 (below); plan-review still pending_
_Planned at: `b24078b` (2026-10-07)_

## What

`crates/ironhold_core/src/CLAUDE.md` is 160,039 chars (~40k est. tokens) and loads whenever *any* file under
`crates/ironhold_core/src/` is touched — editing one line of `schema/` costs the same as editing the physics code.
Split it so a typical core edit loads ~5-10k tokens instead: a lean parent for rules that hold crate-wide, a short
`CLAUDE.md` per directory for directory-wide rules, and on-demand topic references that load only when their files
are touched. No rule may be lost, and no existing citation may silently dangle. This is a documentation refactor:
no engine behaviour changes.

## Why it is not a mechanical split

The headings do not match the content (map §1): the 52k "Entity FSM" section is mostly per-player targeting,
action bars and animation; "Physics & FixedUpdate" is mostly one 20k `player.rs` jump/ground-detection saga;
the 48k "Despawning" section holds the player-construction inventory and ~34k of local co-op/split-screen/gamepad
notes. So content was classified block by block (map §2: ~100 blocks, five read-only slice agents, line ranges
spot-checked). About 70k chars are reference material about *one subsystem* each; ~11-12k chars are derivable or
history and can be cut; the rest is directory-wide rules.

## Decisions (resolved with Frank, 2026-10-07)

1. **Mechanism: tool-neutral content + a thin Claude-Code layer.** OpenCode's documentation (read 2026-10-07, not
   tested here) says it auto-discovers `AGENTS.md`, `CLAUDE.md` (fallback, per folder), `~/.claude/CLAUDE.md` and
   `~/.claude/skills/` (user-global only); its `instructions` field accepts globs but loads **every** match, and it has
   **no path-scoped rules**. So `.claude/rules` is a Claude-Code-only mechanism, and pointing OpenCode's
   `instructions` at it would load all ~70k into every session. Therefore:
   - the **content** of each topic reference lives in a tool-neutral file, `docs/dev/<topic>.md` (readable by humans,
     Claude Code and OpenCode);
   - **Claude Code** gets a tiny `.claude/rules/<topic>.md` stub: `paths:` frontmatter naming the exact source files +
     an `@import`/pointer to the `docs/dev` file, so it auto-loads at **file** granularity (important: `capabilities/`
     has 33 files);
   - **OpenCode and humans** get a one-line pointer in the directory `CLAUDE.md` (OpenCode loads those per folder on
     first touch, then reads the file on demand);
   - skills are used only for a topic with no clean file mapping (none identified so far); `docs/` pages for
     designer-facing material.
   Rule of thumb unchanged: anything whose violation is silent (`Safety = Y` in the map) stays in an always-loaded
   file that covers every file that could violate it; only explanation/history/reference moves to on-demand loading.
2. **Directory files:** `capabilities/CLAUDE.md`, `runtime/scene_manager/CLAUDE.md`, `schema/CLAUDE.md`; **no**
   `runtime/CLAUDE.md` (every interpreter/executor/loader file is in `runtime/scene_manager/`). Rules that cross
   directories, or constrain src-root files (`lib.rs`, `det_math.rs`, `inspector.rs`, `utils.rs`; no directory file
   loads for them), stay in the **parent**.
3. **Condense moved incident narrative** (30-40% of moved text is "this was fixed / used to"): keep the current-state
   rule plus a test/plan pointer; history lives in `planning/` and git.
4. **Docs are split by audience, with minimal churn.**
   - *Library users* (designers/game authors using the prebuilt WASM + RON) keep their current paths — `00_overview`,
     `05_art_style`, `20_data_formats`, `25_custom_shaders`, `30_runtime_events_and_logic` — so the ~330 files citing
     them stay valid. `STATUS.md` (mixed) stays.
   - *Core developers* get `docs/dev/`: `10_architecture`, `40_determinism_and_networking`, `50_roadmap_and_milestones`,
     `60_contributing`, `70_profiling`, `browser_tests` (~61 inbound references, rewritten by script; a 3-line redirect
     stub is left at each old path so historical citations in done plans and agent memory stay resolvable).
   - `docs/dev/` is also the home for the topic references moved out of the core `CLAUDE.md` (decision 1).
   - Designer-relevant fragments found in the map (WASM first-use stalls, the `ParticleBudget` footgun, SFX authoring)
     go to the designer side (check `docs/20` first; some may already be there).
   - `docs/README.md` indexes every doc with an audience tag; the root `README.md` table is split into "Using the
     library" and "Developing the library"; the root `CLAUDE.md` "Updating documentation" list and the
     `ux-gamedesigner-reviewer` agent scope (designer docs) are updated; `system-architect` covers `docs/dev/`.
   - **Out of scope here, logged as separate backlog items:** splitting `docs/20_data_formats.md` (354k chars, ~88k est.
     tokens, 24 top-level sections mixing designer reference with developer-ish parts such as "Schema evolution";
     anchors like `#max_frame_delta_secs` are cited) and auditing `docs/30_runtime_events_and_logic.md` for the same
     audience mix.

## Mitigations for the caveats (the part that makes this safe)

| # | Caveat | Mitigation |
|---|---|---|
| R1 | A rule is lost in the move | **Rule-conservation audit** (Phase 0 script, run before every commit): extract from the snapshot of the old file (`git show b24078b:...`) every bold-lead phrase and every sentence containing must/never/do not/always/only; assert each appears in exactly one new file or is listed in an explicit CUT table with the reason. Output is pasted into the commit message. The reviewer pass (R7) double-checks the CUT list. |
| R2 | A live citation dangles | **Citation contract** (map §4): headings cited by live code (`determinism_lint.rs` incl. a failing-test message, `docs/40`, `camera.rs`, `nameplate.rs`) keep their exact text in the parent; comments citing moved sections (`animation_resolver.rs`, `targeting.rs`, `validate.rs`) are updated in the same commit as the move; hooks `action_docs_reminder.py` and `capability_registration_reminder.py` are rewritten (the former points at a list that is being cut). A grep for `ironhold_core/src/CLAUDE.md` after each phase must show only intended hits. |
| R3 | Historical citations (done plans, agent memory, ~150 lines) dangle | They are records and are not edited. The parent carries a **moved-sections index** (old heading -> new location, ~1.2k chars) so they stay resolvable. |
| R4 | `capabilities/CLAUDE.md` is still too big (~45k raw) | Condense (drop incident narrative; keep current-state rule + test/plan pointer) and push topical blocks to path-scoped rules until the file is ~15-20k chars. Budget enforced by the audit script. If it still exceeds the budget, split by subdirectory (`capabilities/` can gain subfolders) — an explicit follow-up, not a blocker. |
| R5 | Lazy loading misses a rule (nested files load by file path; a session reasoning about a topic without opening a file never sees it) | Safety=Y rules stay always-loaded (above); every on-demand topic gets a **one-line pointer** in its directory file ("before editing player ground detection read `.claude/rules/player-ground-detection.md`") so it is discoverable by name and by OpenCode. |
| R6 | "see above/below" breaks across files; stale pointers | The audit greps every new file for `above`/`below`/`see`/`line \d+`/`four-site` and each hit is rewritten as a named pointer. Known cases are listed in map §5 (e.g. "four sites" is now five; "line 560"). |
| R7 | Reviewers cannot see what vanished | Per phase: `debug-detective` runs the rule-loss audit adversarially against the CUT list; `system-architect` checks that no `Safety=Y` rule ended up where it does not load for every file it governs; `ux-gamedesigner-reviewer` checks moved designer-facing text (`docs/`). |
| R8 | OpenCode (free models, token caps) loads differently | Per OpenCode's docs (not tested): nested `AGENTS.md`/`CLAUDE.md` are attached per folder on first touch, so directory files work and cut its cost too; `.claude/rules` and path-scoped rules are **not** supported, so topic content lives in `docs/dev/` and every topic gets a pointer line in its directory `CLAUDE.md` (decision 1). **Do not add `AGENTS.md` files in subfolders** (an `AGENTS.md` wins over `CLAUDE.md` in that folder and would shadow ours) and **do not point OpenCode's `instructions` at `.claude/rules/*.md`** (it loads every match, always). The Phase-0 spike tests that OpenCode follows the pointer. |
| R9 | The change is big and hard to roll back | One commit per destination (parent, schema, scene_manager, capabilities, each topic), each followed by the audit; base commit `b24078b` recorded; the old file is never rewritten in place until every block has a destination. Everything is in git. |
| R10 | Known drift gets copied into the new files | Fix while moving: FixedUpdate camera-follow wording (the camera chain runs in `Update`), the `audio_volume_var_system` note filed under "Audio file authoring", "four sites" -> five, `opencode_compatibility.md`'s "~154 KB" note. |

## Approach (phases)

**Phase A — docs audience split (first: small, scripted, low risk).** `git mv` the six developer docs into
`docs/dev/`; a script rewrites live inbound references (README, root `CLAUDE.md` "Updating documentation" list, agent
prompts, hooks, code comments, `docs/*`, live planning files) and leaves a 3-line redirect stub at each old path;
add `docs/README.md` (audience-tagged index), split the root `README.md` table, update the reviewer scopes. Grep
afterwards for the old paths: only the stubs and historical records may still contain them. No `CLAUDE.md` content
moves in this phase.

**Phase 0 — spike and tooling (no content moves).**
- Verify, in Claude Code: a throwaway `.claude/rules/_spike.md` with `paths:` for one file attaches when that file
  is touched and not for another; an `@import` inside the rules file works; check via the loaded-instructions list
  or `/context`. Verify, in OpenCode (if cheap): touching a core file attaches the directory `CLAUDE.md`, and the
  agent follows the pointer to `docs/dev/`. Delete the spike. Record the results here.
- Write the rule-conservation/citation audit script (`tools/claude_md_audit.py`, header comment only) and run it on
  the *unchanged* file to prove the extraction works (it must report 0 missing against itself).
- Freeze the base: record `b24078b` and the block table (map §2) as the source of truth.

**Phase 1 — parent + citation stubs.** Slim the parent to the PARENT rows (~14-17k chars), keep cited headings
verbatim, add the moved-sections index. Fix the FixedUpdate wording. Audit.

**Phase 2 — `schema/` and `runtime/scene_manager/` files.** Move/condense the SCH and SM rows; add the
`{self}`-targets cut and rewrite the hook (R2). Audit.

**Phase 3 — `capabilities/` file and topic references**, one commit per topic (content in `docs/dev/<topic>.md`, plus
the thin `.claude/rules/<topic>.md` stub and the directory pointer): ground detection (the 20k saga; hard rules stay in
`capabilities/CLAUDE.md`), split-screen/cameras/widgets (~25k), action-bar input (~10k), gamepad routing (~9.5k),
player spawn sites (~6.5k), lootable corpse (~4k), and the WASM-stalls fragment on the designer side. Audit after each.

**Phase 4 — references.** Update live comments/hooks/docs from map §4, `tests/CLAUDE.md` dedupe (the `SpawnRegistry`
fixture rule already exists there), `AGENTS.md`, `ship-feature.md`. Grep for dangling references.

**Phase 5 — verify and measure.** Run the audit end-to-end; `cargo test -p ironhold_core --test determinism_lint`
(its failure message cites a heading); confirm which files attach when touching one file per directory (Claude Code
and, if cheap, OpenCode); record before/after chars and est. tokens per directory. No WASM build or `pkg/` work is
involved (documentation and comment-only changes), so this follows the lightweight path of the code workflow.

## Tasks
- [ ] Plan-review this file (system-architect + ux-gamedesigner-reviewer)
- [ ] Phase A: `docs/dev/` move, scripted reference rewrite + redirect stubs, `docs/README.md`, README tables, reviewer scopes
- [ ] Phase 0: spike (path-scoped rules, `@import` in a rules file, OpenCode pointer), audit script, freeze the base
- [ ] Phase 1: parent + citation stubs + moved-sections index
- [ ] Phase 2: `schema/CLAUDE.md`, `runtime/scene_manager/CLAUDE.md`
- [ ] Phase 3: `capabilities/CLAUDE.md` + topic references (`docs/dev/` content + rule stub + pointer, one commit each)
- [ ] Phase 4: live citations, hooks, `tests/CLAUDE.md`, `AGENTS.md`, command files
- [ ] Phase 5: audit, determinism_lint, load verification, before/after measurement
- [ ] Reviews per phase (R7) and a final full-file reviewer pass
- [ ] Backlog: items for splitting `docs/20_data_formats.md` and auditing `docs/30_runtime_events_and_logic.md` (logged 2026-10-07)

## Open questions (remaining; recommendations in brackets)
1. **`capabilities/` budget:** target ~15-20k chars, with a subdirectory split as a follow-up if exceeded? [Yes.]
2. **Parent size:** ~14-17k chars is itself ~4k tokens on every core edit. Trim the determinism paragraph (3.8k)
   and the pipeline block further, or accept? [Trim during Phase 1 where the audit allows.]
3. **Redirect stubs at the six old doc paths:** keep permanently, or delete after one release? [Keep; they are 3 lines
   each and keep ~150 historical citations resolvable.]

## Acceptance criteria
- Given any block of the old file, then the audit finds it in exactly one new file or in the explicit CUT table, and
  every `Safety=Y` rule loads for every source file it governs.
- Given a live citation (map §4), then it resolves to text that exists at the cited heading/location.
- Given a core edit in each of `schema/`, `runtime/scene_manager/`, `capabilities/`, then the loaded memory is the
  parent plus that directory's file (plus matching path-scoped rule stubs), measured, and the totals meet the budgets
  (parent <= ~17k chars; `schema/` <= ~6k; `runtime/scene_manager/` <= ~20k; `capabilities/` <= ~20k).
- Given `cargo test -p ironhold_core --test determinism_lint`, then it passes and its failure message still names a
  section that exists.
- Given OpenCode touching a core file, then it attaches the parent and the directory file; no `AGENTS.md` was added
  in a subfolder, and `instructions` does not glob `.claude/rules`.
- Given the docs split, then every developer doc is under `docs/dev/`, the designer docs have not moved, no live file
  references an old developer-doc path, and `docs/README.md` tags every doc with its audience.
