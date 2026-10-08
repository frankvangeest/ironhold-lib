---
name: opencode-glm-driver-plan-review
description: 2026-10-08 plan-review of planning/features/opencode_glm_driver_pilot.md (paid OpenCode driver + -alt twins + m365 proxy) - zero designer impact; operator-UX issues to re-check at implementation
metadata:
  type: project
---

Dev-tooling plan: paid top-level `model` (GLM 5.3 flash) as the OpenCode driver, free subagents with `<role>-alt` fallback twins, `-deep` stays paid and task-denied, m365 Copilot proxy as machine-local review delegate. Designer impact nil (touches only .opencode/, tools/, planning/); only risk is pilot fixtures landing in assets/projects.

Open issues raised (re-check at implementation review):
- Option (a) "m365 overrides in global config" likely cannot override repo agents: OpenCode merges global < project, so `.opencode/opencode.json`'s per-agent `model` wins. Needs distinct agent names or OPENCODE_CONFIG_CONTENT; verify with `opencode debug config`.
- `--agents` assertion "free unless -deep" will fail on `build` (inherits paid driver) unless build/top-level is allow-listed, same as the sync-check rule.
- "different provider" for -alt is undefined: gateway (opencode/ vs openrouter/) vs upstream vendor. R tier already runs the SAME Nemotron via two gateways; an upstream 429 hits both.
- No stated mechanism for attaching driver.md (build.prompt replaces build's system prompt; `instructions` reaches every agent). Retry-with-alt rule never reaches the /code-review etc. orchestrators, which delegate on their own free model.
- Paid-vs-free classification for the sync check undefined (Gemini is free only without billing; m365 is neither).
- README "Every agent's plain name is free / nothing can accidentally spend money" becomes false for `build` and plain `opencode run`; must be rewritten, not appended.

**Why:** OpenCode operator docs (.opencode/README.md) are the dev-side equivalent of designer docs; guarantees there are what people rely on to not spend money.
**How to apply:** for any OpenCode routing change, check config-merge precedence, the plain-name-is-free guarantee, and which orchestrators a prompt rule actually reaches. See [[opencode-probe-plan-review]].
