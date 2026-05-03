# Planning folder

Everything about what to build, fix, or investigate lives here. Nothing work-related goes in `docs/`.

## Folder layout

```
planning/
  backlog.md               ← canonical priority queue (features + bugs)
  claude_suggestions.md    ← Claude's observed improvement candidates for Frank to review
  features/                ← design specs for non-trivial features
    _template.md
    {name}.md
  investigations/          ← debug journals for bugs that need investigation before fixing
    {name}.md
```

---

## backlog.md — the single source of truth

Items flow: **Icebox → Queued → Active → Done**

- Move to **Active** when work starts; to **Done** when merged.
- Do not duplicate items into GitHub issues or anywhere else.
- Sections: `Active`, `Queued` (milestones + icebox), `Bugs`.

### Adding a bug

One line in the `## Bugs` section is enough for most bugs:

```
- [ ] **short title** — reproduction, suspected cause, candidate fix.
```

If the bug needs investigation before it can be fixed, add the backlog entry _and_ create an investigation file:

```
- [ ] **short title** — see `planning/investigations/{name}.md`.
```

---

## features/ — design specs

Create `planning/features/{name}.md` (copy `_template.md`) when a feature needs design discussion before coding: new schema fields, new event/action types, cross-capability changes, or anything where the approach is unclear. Skip the file for simple, self-contained additions.

Always fill in the `Planned at` metadata at the top of a new feature file:

```
Planned at: <short commit hash> (<YYYY-MM-DD>)
```

Run `git rev-parse --short HEAD` to get the hash. This creates a stable reference — use `git log <hash>..HEAD` later to see what changed between design and implementation.

---

## investigations/ — debug journals

Create `planning/investigations/{name}.md` for bugs that require exploration before a fix is clear: reproducing the issue, reading relevant code, forming hypotheses, logging findings.

Use free-form markdown. Useful sections: **Symptoms**, **Files read**, **Hypotheses**, **Findings**, **Root cause**, **Next steps**.

Keep the matching backlog entry as the authoritative status marker — the investigation file is the detail behind it.

---

## claude_suggestions.md — improvement candidates

While implementing features, if you notice something worth revisiting later — a pattern that could be improved, a latent bug, a follow-up optimisation — add a brief entry here. Only add things with a concrete technical basis observed during the current work, not general speculation.

Format:

```
- **Title** _(observed at `<hash>` <YYYY-MM-DD>)_
  What (one sentence) + Why (one sentence, concrete basis).
```

Frank reviews these periodically and promotes good ones to the backlog.
