---
name: restriction-lift-doc-sweep
description: When a bug fix removes a "don't combine X with Y" designer restriction, sweep every place the restriction was written down - the docs/20 field row is the one most often missed
metadata:
  type: feedback
---

A bug fix that lifts a designer restriction is only designer-complete once every copy of the old
warning is gone. Found on feature/fix_trigger_zone_mass (2026-10-23): the fix rewrote the
scene_manager CLAUDE.md block and docs/30's corpse walkthrough, and the backlog Done entry said
"notes rewritten", yet the docs/20 `PrefabDef.trigger_zone` row still said
"**Do not put this on a prefab with a Dynamic rigid body** ... (a known engine bug)".

**Why:** caveats migrate. The trigger_zone caveat began in src/CLAUDE.md, moved during the CLAUDE.md
split, and was later copied into the docs/20 field row. The fix author only followed the backlog's
own pointers, so they missed the newest copy.

**How to apply:** grep the whole worktree (excluding pkg/) for the restriction's wording, not just
the files the backlog names. Check: the docs/20 field row; docs/30; folder CLAUDE.md files;
docs/dev/moved-sections-index.md titles; example-project RON comments; *open* planning plans that
propose a validate error for the old restriction. Treat a stale designer-doc instruction as
must-fix before merge. Leave done/ plans and investigations/ alone, since they are history.

Related: [[dev-doc-split-designer-leak-audit]], [[lootable-corpse-pattern]].
