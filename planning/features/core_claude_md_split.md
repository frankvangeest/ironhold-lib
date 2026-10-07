# Feature: Split `crates/ironhold_core/src/CLAUDE.md` by directory and topic

_Status: Draft — section map done (`planning/investigations/core_claude_md_split_map.md`); not plan-reviewed yet_
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

## Decisions to make before coding (see "Open questions"; recommendations given)

**Loading mechanism for the ~70k of topic references.** Three options:
1. **Path-scoped rules** — `.claude/rules/<topic>.md` with `paths:` frontmatter naming the exact source files
   (e.g. `capabilities/player.rs`, `capabilities/animation_resolver.rs`). Loads deterministically when a matching
   file is touched, at **file** granularity (finer than a directory — important because `capabilities/` has 33
   files), and does not depend on the model choosing to invoke anything. **Preferred, subject to the Phase-0 spike.**
2. **Skills** — description-matched, loaded when the model decides the topic applies. Good for topics with no clean
   file mapping, but a session that never invokes it never sees the rule — so hard rules must never live only here.
3. **`docs/` pages** — visible to humans, loaded only if someone follows a pointer. Right for designer-relevant
   material (e.g. the WASM first-use-stall page).

Rule of thumb: **anything whose violation is silent (`Safety = Y` in the map) stays in an always-loaded file that
covers every file that could violate it**; only explanatory history/reference moves to on-demand loading.

**Directory files.** Create `capabilities/CLAUDE.md`, `runtime/scene_manager/CLAUDE.md`, `schema/CLAUDE.md`. No
`runtime/CLAUDE.md`: every interpreter/executor/loader file lives in `runtime/scene_manager/` and nothing
belongs only to `runtime/` (gamepad input in `runtime/input.rs` is covered by the gamepad topic rule + the parent).
Rules that cross directories, or that constrain `lib.rs`/`det_math.rs`/`inspector.rs`/`utils.rs` (src root — no
directory file loads for them), stay in the **parent**.

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
| R8 | OpenCode (free models, token caps) loads differently | Verified in `opencode_compatibility.md`: OpenCode attaches the nearest `AGENTS.md`/`CLAUDE.md` per folder on first touch, so directory files work and cut its cost too. **Do not add `AGENTS.md` files in subfolders** (an `AGENTS.md` wins over `CLAUDE.md` in that folder and would shadow ours). Path-scoped `.claude/rules` support in OpenCode is **unverified** — hence the pointers in R5. |
| R9 | The change is big and hard to roll back | One commit per destination (parent, schema, scene_manager, capabilities, each topic), each followed by the audit; base commit `b24078b` recorded; the old file is never rewritten in place until every block has a destination. Everything is in git. |
| R10 | Known drift gets copied into the new files | Fix while moving: FixedUpdate camera-follow wording (the camera chain runs in `Update`), the `audio_volume_var_system` note filed under "Audio file authoring", "four sites" -> five, `opencode_compatibility.md`'s "~154 KB" note. |

## Approach (phases)

**Phase 0 — spike and tooling (no content moves)**
- Verify path-scoped rules: create a throwaway `.claude/rules/_spike.md` with `paths:` for one file, touch that file in
  a fresh session and confirm it attaches (and does not attach for another file); check the same session's loaded
  instruction list/`/context`. Check whether OpenCode attaches it. Delete the spike. Record the result in this file.
- Write the rule-conservation/citation audit script (`tools/claude_md_audit.py`, header comment only) and run it on
  the *unchanged* file to prove the extraction works (it must report 0 missing against itself).
- Freeze the base: record `b24078b` and the block table (map §2) as the source of truth.

**Phase 1 — parent + citation stubs.** Slim the parent to the PARENT rows (~14-17k chars), keep cited headings
verbatim, add the moved-sections index. Fix the FixedUpdate wording. Audit.

**Phase 2 — `schema/` and `runtime/scene_manager/` files.** Move/condense the SCH and SM rows; add the
`{self}`-targets cut and rewrite the hook (R2). Audit.

**Phase 3 — `capabilities/` file and topic rules**, one commit per topic: ground detection (the 20k saga; hard rules
stay), split-screen/cameras/widgets (~25k), action-bar input (~10k), gamepad routing (~9.5k), player spawn sites
(~6.5k), lootable corpse (~4k), plus the `docs/` WASM-stalls page. Audit after each.

**Phase 4 — references.** Update live comments/hooks/docs from map §4, `tests/CLAUDE.md` dedupe (the
`SpawnRegistry` fixture rule already exists there), `AGENTS.md`, `ship-feature.md`. Grep for dangling references.

**Phase 5 — verify and measure.** Run the audit end-to-end; `cargo test -p ironhold_core --test determinism_lint`
(its failure message cites a heading); confirm which files attach when touching one file per directory (Claude Code
and, if cheap, OpenCode); record before/after chars and est. tokens per directory. No WASM build or `pkg/` work is
involved (documentation and comment-only changes), so this follows the lightweight path of the code workflow.

## Tasks
- [ ] Plan-review this file (system-architect + ux-gamedesigner-reviewer) and resolve the open questions
- [ ] Phase 0: path-scoped-rules spike (Claude Code + OpenCode), audit script, freeze the base
- [ ] Phase 1: parent + citation stubs + moved-sections index
- [ ] Phase 2: `schema/CLAUDE.md`, `runtime/scene_manager/CLAUDE.md`
- [ ] Phase 3: `capabilities/CLAUDE.md` + topic rules (one commit each)
- [ ] Phase 4: live citations, hooks, `tests/CLAUDE.md`, `AGENTS.md`, command files
- [ ] Phase 5: audit, determinism_lint, load verification, before/after measurement
- [ ] Reviews per phase (R7) and a final full-file reviewer pass

## Open questions (recommendations in brackets)
1. **Mechanism for topic references:** path-scoped `.claude/rules` (file granularity, deterministic) vs skills
   (model-invoked) vs `docs/` pages. [Path-scoped rules if the spike passes; skills only for topics with no file
   mapping; `docs/` for designer-facing material.]
2. **Directory files:** `capabilities/`, `runtime/scene_manager/`, `schema/` — and no `runtime/CLAUDE.md`? [Yes.]
3. **How hard to condense moved incident narrative** (30-40% of moved text is "this was fixed / used to"):
   [keep the current-state rule + a test/plan pointer; history lives in `planning/` and git.]
4. **`capabilities/` budget:** target ~15-20k chars, subdirectory split as a follow-up if exceeded? [Yes.]
5. **Where should designer-relevant moved text live** (WASM first-use stalls, `ParticleBudget` footgun, SFX
   authoring)? [`docs/`; check `docs/20` first — some of it may already be there.]
6. **Parent size:** ~14-17k chars is itself ~4k tokens on every core edit. Trim the determinism paragraph (3.8k) and
   the pipeline block further, or accept? [Trim during Phase 1 where the audit allows.]

## Acceptance criteria
- Given any block of the old file, then the audit finds it in exactly one new file or in the explicit CUT table, and
  every `Safety=Y` rule loads for every source file it governs.
- Given a live citation (map §4), then it resolves to text that exists at the cited heading/location.
- Given a core edit in each of `schema/`, `runtime/scene_manager/`, `capabilities/`, then the loaded memory is the
  parent plus that directory's file (plus matching path-scoped rules), measured, and the totals meet the budgets
  (parent <= ~17k chars; `schema/` <= ~6k; `runtime/scene_manager/` <= ~20k; `capabilities/` <= ~20k).
- Given `cargo test -p ironhold_core --test determinism_lint`, then it passes and its failure message still names a
  section that exists.
- Given OpenCode touching a core file, then it attaches the parent and the directory file; no `AGENTS.md` was added
  in a subfolder.
