# Editing CLAUDE.md files, `.claude/rules` stubs and AGENTS.md

For anyone who edits an instruction file in this repo and wants both Claude Code and OpenCode to keep loading the right
rules. The short version is the checklist; the rest is the reasoning behind it. (Designers: nothing here concerns you;
see [docs/README.md](../README.md).)

## 1. Before you edit: checklist

Each item ends with how it is checked. "Human" means no tool can judge it.

- [ ] The rule lives in the folder file whose subtree contains **every file it governs**. Files directly in
  `crates/ironhold_core/src/` (`lib.rs`, `det_math.rs`, `inspector.rs`, `utils.rs`) see only the parent file.
  *Check: `python tools/opencode_probe.py` (OpenCode) and `/context` after touching a file (Claude Code).*
- [ ] No `AGENTS.md` (or `CONTEXT.md`) below the repo root. *Check: `python tools/opencode_probe.py --static`, or
  `python tools/opencode_sync_check.py --skip-models`.*
- [ ] A `.claude/rules/*.md` stub is **pointer-only**, has the exact frontmatter key `paths:`, every glob matches a real file,
  and it contains no `@import`. *Check: `python tools/claude_md_audit.py full` (keys, globs, imports).*
- [ ] A rule that must hold (the audit calls it Safety=Y) is in a file that loads for the files it governs, not only in a
  `docs/dev` topic page. *Check: human review; the audit only checks consistency with the `governs` globs someone wrote.*
- [ ] Headings that code or tests cite are unchanged word for word (for example "Deterministic iteration order on gameplay
  paths", cited by `tests/determinism_lint.rs`). *Check: `python tools/claude_md_audit.py full`, `cargo test -p ironhold_core
  --test determinism_lint`.*
- [ ] Size budgets hold: `capabilities/CLAUDE.md` at most 20,000 characters, the crate parent at most 18,000.
  *Check: `python tools/claude_md_audit.py full`.*
- [ ] You added nothing to the root `CLAUDE.md` unless it is needed in **every** session (it is about 14.8k tokens and loads
  every time). *Check: human; `/context` at session start.*
- [ ] Review agents were given the worktree path and told to read the folder `CLAUDE.md` files themselves (Claude Code only,
  see section 4). *Check: human step.*

## 2. Where does my rule go?

| The rule applies to | Put it in | Claude Code | OpenCode |
|---|---|---|---|
| everything under `crates/ironhold_core/src/`, including `lib.rs` | `crates/ironhold_core/src/CLAUDE.md` (parent, 18k budget) | loads for any file in the subtree | attached for any file in the subtree (V1) |
| one folder (`capabilities/`, `runtime/`, `runtime/scene_manager/`, `schema/`, `assets/`, `tests/`, a `tools/<x>/`) | that folder's `CLAUDE.md` | loads when a file in the subtree is read or edited (and the ancestors' files); **not** for files in the parent folder | same (V1) |
| one ordering edge or one function in `lib.rs` | a `// load-bearing:` comment next to the code, plus one line in the parent if it crosses folders | the comment is in the file you read | same |
| long history, derivations, incident narrative | `docs/dev/<topic>.md`, a pointer line in the folder file, optionally a pointer-only `.claude/rules/<topic>.md` stub | the stub loads when a matching path is read or written | the folder file's pointer line only (V5) |
| how a designer uses a feature | `docs/20_data_formats.md` or `docs/30_runtime_events_and_logic.md` | n/a | n/a |
| every session | the root `CLAUDE.md`, only if it truly is | always | through `instructions` (V3) |

## 3. Never do

- **Add an `AGENTS.md` stub (`@CLAUDE.md`) beside a `CLAUDE.md`.** OpenCode attaches an `AGENTS.md` instead of the `CLAUDE.md`
  in the same folder and does not expand `@file` in it, so the model sees the literal text `@CLAUDE.md` and the folder's rules
  are silently lost (V2). Seventeen were added once and removed.
- **Use `@import` in a path-scoped `.claude/rules` stub.** The imported file loads eagerly at session start, which defeats the
  scoping.
- **Mistype the `paths:` key or leave it out.** A rule without a valid `paths:` key loads on every session.
- **Park a must/never rule only in a topic page.** A topic page loads only when someone reads it. Nine such rules were found
  in the final review of the split and moved back into folder files.
- **Rely on a HTML comment being stripped.** Claude Code strips block comments from loaded files, OpenCode does not (V4); the
  audit anchors (`<!-- b:N -->`) therefore cost a little under OpenCode.

## 4. Claude Code and OpenCode differ here

| Behaviour | Claude Code (verified in the split's spike) | OpenCode 1.18.33 (verified by the probe) |
|---|---|---|
| Folder `CLAUDE.md` | loads lazily for files in its subtree | attached lazily on the first read of a file in the subtree (V1) |
| `.claude/rules/*.md` with `paths:` | loads on Read/Edit/Write of a matching path | never read (V5) |
| `@import` in `CLAUDE.md` | expanded | not applicable to subfolder files |
| `AGENTS.md` stub | not used | shadows the `CLAUDE.md`, `@file` not expanded (V2) |
| HTML comments in instruction files | stripped | reach the model (V4) |
| Reading a sibling worktree's file from the primary checkout | no folder file or rule loads (so tell review agents to read them) | the worktree's folder files **and its root `AGENTS.md`** load (V14) |
| Subagents | get the folder files when they read files in the subtree | n/a (agents carry their own prompt) |

## 5. How to verify

```
# Claude Code: open a fresh session in the checkout, then
/context                                  # at start: only the root file; after touching a core file: the ancestors' files

# OpenCode (this machine: `nvs use 24.21` first, in the same terminal)
python tools/opencode_probe.py            # model-free, about 40 s: attached files vs the tree, per folder
python tools/opencode_probe.py --only crates/ironhold_core/src/lib.rs
python tools/opencode_probe.py --static   # no OpenCode needed
python tools/opencode_sync_check.py       # configuration drift (+ the static AGENTS.md check)

# The CLAUDE.md split's own audit (budgets, anchors, stub frontmatter, forbidden phrases)
python tools/claude_md_audit.py full
```

Every OpenCode fact the repo depends on, with its re-verify command, is in the "Verified compatibility facts" table in
[`.opencode/README.md`](../../.opencode/README.md).

## 6. Why: incidents this guide comes from

- **The 17 stubs (2026-10-08).** The core CLAUDE.md split assumed an `AGENTS.md` containing `@CLAUDE.md` would forward to the
  real file for OpenCode. A live run showed it does not; the stubs were deleted and the probe was written so that this kind of
  assumption is checked, not guessed. See `planning/features/done/core_claude_md_split.md` (Phase 5) and `planning/features/opencode_compat_probe.md`.
- **Rules that loaded nowhere useful.** The review of the split found nine must-level rules that sat only in topic pages or
  in a folder that does not load for the code they govern; the audit's Safety flags had been set per block and a block marked
  "not safety" can still contain a "must".

## 7. Glossary

- **`b:N` anchor:** an HTML comment of the form `<!-- b:N -->` (N a number) before a block, used by `tools/claude_md_audit.py` to prove each block of
  the old crate `CLAUDE.md` landed in exactly one place. `docs/dev/moved-sections-index.md` maps old sections to new homes.
- **Safety=Y:** a block whose violation fails silently or breaks determinism, physics, WASM or data, so it must load for every
  file that could violate it.
- **`governs`:** the list of file globs a Safety=Y block protects, written in `planning/investigations/core_claude_md_split_blocks.json`.
- **`load-bearing:` comment:** `// load-bearing: <why>` next to a system-ordering edge in `lib.rs`; never remove or reorder
  the edge under one.
- **`/context`:** the Claude Code command that lists what is loaded into the session and its size.
- **Probe:** `tools/opencode_probe.py`, which asks OpenCode itself which instruction files it attaches for a file.
