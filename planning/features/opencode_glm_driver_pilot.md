# Feature: GLM 5.3 flash pilot and the OpenCode default-driver decision

_Status: Draft — Frank's decisions recorded 2026-10-08, routing design added; becomes Ready when `opencode_compat_probe` is implemented (its probe is a precondition)_
_Planned at: `edc93f1` (2026-10-08); split out of the compat-probe plan after its plan-review_

## What
Measure whether `openrouter/z-ai/glm-5.3-flash` can be the cheap, capable driver for OpenCode work in this repo, and decide
how (or whether) to adopt it. Developer tooling only; nothing changes for game authors.

## Why
The free models are unreliable (a `:free` model returned 429 during the split work, and about one in six free-endpoint
responses can come back empty), while the only paid model is the E-tier DeepSeek behind the explicit `*-deep` agents.
GLM 5.3 flash looked cheap and fast in one smoke run on 2026-10-08 (17 s, about $0.005 for a trivial read task, correct
instruction attachment), but one run is not a capability test.

## Facts established during plan-review (OpenCode 1.18.33)
- The **default driver** is the top-level `model` in `.opencode/opencode.json`, currently `openrouter/poolside/laguna-s-2.1:free`
  (`opencode debug config`); `build` has no model of its own. The `general` subagent and the commands that set no model
  (for example `new-project`, `rust-idioms`) inherit it.
- Therefore making the top-level `model` a paid model silently makes all of those paid, which breaks the README's
  "nothing can accidentally spend money" guarantee for plain agent names.
- Available models: `openrouter/z-ai/glm-5.3-flash` (pinned id) and `openrouter/~z-ai/glm-flash-latest` (alias that can
  change behaviour silently; `opencode_sync_check.py` only catches disappearance).

## Decisions (Frank, 2026-10-08)
1. **A cheap paid model is the main driver** (the top-level `model`), because the free models have had many outages. Free
   models are used for the agentic work the driver delegates, and several agents with different models can cover the
   same role as fallbacks. This **supersedes** the earlier recommendation of a named opt-in agent. The guarantee therefore
   changes and must be stated in `.opencode/README.md`: *the driver is paid; everything it delegates to is free unless a
   `*-deep` agent is asked for by name.* See "Routing design" for how that stays true.
2. **OpenRouter credit threshold: lifetime purchases, not the balance.** The 1000 free requests per day need at least $10 of
   credit *purchased in total*, so spending the balance down to zero keeps the cap (F9's own wording is "purchased credits
   >= 10"; third-party sources agree: benchlm.ai, ask-coreai.com; official OpenRouter docs not yet checked, so confirm there
   when convenient). Two practical limits remain: free models also have a 20 requests/minute cap, and **a negative balance
   returns HTTP 402 even on free models**, so keep a positive balance (and set a spend limit on the key).

## Routing design (the driver, the agents and their fallbacks)
**What OpenCode can and cannot do (verified from this repo's notes, `.opencode/README.md` and `opencode_compatibility.md`
section 6):**
- A model is bound to an **agent** in the config (`model` is a single string per agent). The driver cannot name a model id.
- The driver delegates with the `task` tool and chooses **which subagent**, guided by each agent's `description` and limited
  by `permission.task` (today `{ "*": "allow", "*-deep": "deny" }`). Choosing the agent *is* choosing the model.
- There is **no automatic fallback** between models or providers. A fallback therefore has to be a *second agent for the
  same role on a different model/provider*, and the driver has to be told when to use it.
- A failed delegation (a 429, an error) is visible to the driver; an empty or poor answer is only visible if the driver checks.

**Design:**
- A short driver prompt (for example `.opencode/prompts/driver.md`) with three rules: delegate review, test-writing and
  documentation-lookup work to the named subagents; if a subagent errors or returns nothing usable, retry **once** with its
  `-alt` twin; never use a `*-deep` agent unless the user asks for it by name.
- A roster table in `.opencode/README.md`: role, primary agent (free model A), twin `<role>-alt` (free model B on another
  provider, as the C tier already does on purpose), and the existing paid `-deep` twin (denied for delegation).
- **Keep the paid driver from leaking spend to other agents:** give `general` and every command that sets no model an explicit
  free model, so only the driver's own turns are paid. Add a check to `opencode_sync_check.py` that paid model ids appear
  only on allow-listed keys (the top-level `model` and `*-deep`).
- Subagent reports go back into the paid driver's context, so the prompt asks for short reports.
- The driver's own turns dominate the cost: a trivial GLM 5.3 flash turn is about $0.005 with a 31.8k-token machine-specific
  baseline, so the number of turns matters more than the model price. The pilot measures it.

## Approach (when unblocked)
- **Tasks** (4-5, identical prompt per model, each with a mechanical pass criterion): follow a folder rule (a change that
  must respect a "never"); edit a RON file then `validate` passes; answer a question that needs a `docs/dev` pointer; a
  two-file refactor that passes `cargo check`; a review task with a **planted bug** whose pass criterion is a keyword match.
- **Design for a trustworthy result:** at least **3 runs per task and model** (n=1 is noise); every run starts from a fresh
  worktree reset to a pinned base commit; runs are **sequential** (the "never two cargo at once" rule and the disk limit);
  a dedicated permission profile, never `--auto`; set `--variant` (reasoning effort) explicitly; report model time separately
  from cargo time; record `tokens` and `cost` from each `step_finish`.
- **Pre-registered thresholds** (written before running): for example GLM passes at least 80% of runs and at least the
  current default's rate, with cost per task under a stated amount. "At least as many as the default" over 5 tasks is mostly noise.
- **A routing task** (the one that matters for decision 1): given a short brief, the driver must pick the right subagent,
  and when the primary agent is made to fail (a throwaway config with an invalid model id for it) it must recover through
  the `-alt` twin and not touch a `*-deep` agent. Pass criterion: the final artifact is correct and no `*-deep` agent ran.
- **Models:** `glm-5.3-flash` (pinned), the current free default driver, and the E-tier DeepSeek as the reference.
- **Cost cap about $1**, enforced in the script by summing `step_finish.part.cost` and aborting, plus a spend limit on the
  OpenRouter key.
- **Known-limitations pass:** run the mandatory SAFE/CAUTION/AVOID check from `opencode_free_models.md` section 5 on GLM
  before routing anything to it. Record the date and, if `opencode export` exposes it, the upstream provider (pinning an id
  is not a snapshot).
- **If adopted:** the model goes in the `.opencode/README.md` tier table as a new "cheap paid" tier (not in the free-models
  catalog), with the agent from decision 1; run `python tools/opencode_sync_check.py`.

## Tasks
- [x] Frank's two decisions (2026-10-08, above)
- [ ] Driver prompt, roster table and `-alt` twins (a free model on a different provider for each role that needs a fallback)
- [ ] Explicit free models on `general` and the no-model commands; `opencode_sync_check.py` rule for paid ids on allow-listed keys
- [ ] Wait for `opencode_compat_probe` (the probe checks the new agent still attaches the right instruction files)
- [ ] Task set, fixtures, scoring script with the cost cap and the pre-registered thresholds
- [ ] Known-limitations pass on GLM
- [ ] Run the pilot (3 runs per task and model), write the results into this plan
- [ ] Adopt or not per the thresholds; update `opencode.json`, the README tier table, `opencode_sync_check.py`

## Open questions
- Is the empty-response rate of the free default high enough (about 16% was reported for a free endpoint) to justify a paid
  default for unattended work?
- Which Claude Code agents/commands should ever be routed to the cheap tier (only the default build work, or also reviews)?

## Acceptance criteria
- Given the pilot ran, then the plan records success, wall time, tokens and cost per task per model with at least 3 runs
  each, and the decision follows the thresholds that were written down before the run.
- Given GLM is adopted as the driver, then the README states the changed guarantee (driver paid, delegates free unless `*-deep`
  is named), no agent other than the driver and the `*-deep` agents has a paid model (checked by `opencode_sync_check.py`),
  the model id is pinned, and the routing task passes (correct subagent, recovery through `-alt`, no `*-deep` run).
