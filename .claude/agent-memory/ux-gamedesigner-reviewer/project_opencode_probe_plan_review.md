---
name: opencode-probe-plan-review
description: 2026-10-08 plan-review of planning/features/opencode_compat_probe.md (dev tooling) - designer-safety verdict and the open UX issues to re-check at implementation
metadata:
  type: project
---

Dev-tooling plan (probe script + `.opencode/README.md` facts table + `docs/dev/claude_md_maintenance.md`). Designer impact is nil IF the new doc lands only as a row in docs/README.md's developer half of the table (Audience "Developer (tooling)"), never in the designer opener paragraph.

Issues raised at plan time (re-check at implementation review):
- Default `--model` fallback list included paid `glm-5.3-flash` -> a plain probe run could spend money, contradicting the "plain agent names never spend money" rule in .opencode docs. Asked for free-only default + explicit `--allow-paid`.
- nvs recipe inconsistent (`nvs use 24.21` vs hand-off `nvs use 24`); nvs use from a Python subprocess cannot fix PATH, so the missing-opencode message must tell the user to run it in their own shell then re-run.
- Unexplained jargon for outsiders: F-tier/E-tier, F1-F10, `b:N` anchors, Safety=Y, `governs` globs, 31.8k baseline attributed to a "first GLM run" that predates v2.
- Plan never links the guide from docs/dev/60_contributing.md (the dev hub docs/README sends contributors to).

**Why:** dev-tooling plans still touch docs/README.md, the shared designer/dev index ([[docs-audience-split]]).
**How to apply:** for any dev-only doc, verify it enters docs/README.md only via the dev rows, and check paid-model defaults in any OpenCode tooling.
