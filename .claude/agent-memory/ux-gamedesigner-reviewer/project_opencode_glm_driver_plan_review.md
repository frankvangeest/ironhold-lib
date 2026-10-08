---
name: opencode-glm-driver-plan-review
description: 2026-10-08 plan-review + re-review of planning/features/opencode_glm_driver_pilot.md (paid OpenCode driver, -alt/-m365/-deep twins) - zero designer impact; open contradictions in the -deep rule and per-role fallback order
metadata:
  type: project
---

Dev-tooling plan: paid top-level `model` (GLM 5.3 flash) as the OpenCode driver, free subagents with `<role>-alt` twins (different upstream LAB), `<role>-m365` agents defined only in the machine-local global config (V17: project config beats global, so overrides are impossible), `-deep` paid with task permission `ask`. Designer impact nil.

First review (all 10 findings) resolved or scheduled as of the re-review: V17, shared free/paid classifier in tools/_opencode_common.py (build allow-listed), upstream-lab definition, V18 task for prompt delivery + slash-command text, README guarantee rewrite, $2 cap, V7 link, status line.

Re-review (same day) open issues - check these at implementation:
- Three wordings of the -deep rule disagree: driver prompt "never unless asked by name", outcome "GLM asks before paid", follow-up order puts -deep 2nd for system-architect/debug-detective; acceptance + routing task still say "no *-deep ran".
- Per-role chain is up to 4 steps (m365 > deep > primary > alt) but driver rule is "retry once"; headless keeps deny so the chain differs attended vs unattended; m365 section still says proxy-down falls to -alt.
- Gemini roles have no twin: retry rule has no target.
- probe --agents on resolved config will see global `-m365` agents whose model is in no free allow-list -> fails closed on Frank's machine.
- README roster listing `-m365` agents that don't exist on other machines needs a "machine-local, needs proxy" marker.
- Stale: Approach "Models" omits m365, "agent from decision 1", task "roles Frank picks"; thresholds/$2 still "Frank to confirm" with no task line.

**Why:** .opencode/README.md is the operator doc people rely on to not spend money; contradictory spend rules there are the dev-side equivalent of a designer footgun.
**How to apply:** for any OpenCode routing change, check config-merge precedence, the plain-name-is-free guarantee, one canonical statement of when paid agents run, and which orchestrators a prompt rule reaches. See [[opencode-probe-plan-review]].
