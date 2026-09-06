---
name: windows-case-sensitivity-probing
description: How to reproduce case-sensitive-filesystem behavior on this Windows box (fsutil per-directory flag) plus the four path forms that defeat any read_dir-based case check
metadata:
  type: project
---

This is a Windows-only dev box, so any code that reasons about filesystem case (validate.rs's
`path_case_mismatch`, asset_checker.py, anything comparing an authored path to disk) cannot be
tested against a case-sensitive filesystem by default. It can be, per directory:

```
powershell.exe -NoProfile -Command "fsutil file setCaseSensitiveInfo '<abs\dir>' enable"
```

No admin needed here (verified 2026-09-05). After enabling, that one directory can hold both
`main.scene.ron` and `Main.scene.ron` simultaneously — which is exactly the state that breaks
`read_dir(...).find(|e| e.file_name().eq_ignore_ascii_case(c))` lookups: `find` returns whichever
entry `read_dir` yields first, not the exact match, so an *already-correct* authored path gets
reported as wrong. Use this to test the false-positive side of any case-comparison code.

Four authored path forms that make a component-by-component `read_dir` walk silently bail (each
verified against the real CLI binary, all returned exit 0 with a genuinely wrong-cased path):
- `./scenes/Main.scene.ron` — `read_dir` never yields `.`
- `logic/../scenes/Main.scene.ron` — never yields `..`
- `scenes//Main.scene.ron` — the empty component matches nothing
- `scenes/CAFÉ.scene.ron` vs on-disk `café.scene.ron` — NTFS folds non-ASCII case, but
  `eq_ignore_ascii_case` does not

**Why:** these are the standard blind spots of the read_dir-walk approach, and none of them are
visible from a Windows-only test run.

**How to apply:** when reviewing or writing filesystem-case logic, probe all five of these
(four above + the fsutil dual-entry case) before accepting a "verified inert / verified correct"
claim. Also remember cargo is cheap on this machine now (267 GB free as of 2026-09-05, an
`ironhold_cli` rebuild is ~25 s) — the disk-scarcity warnings in CLAUDE.md are from the old laptop,
so just build and run the real binary instead of reasoning from source.
Related: [[stale-cli-binary-as-prefix-oracle]].
