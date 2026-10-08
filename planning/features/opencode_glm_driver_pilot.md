# Feature: GLM 5.3 flash pilot and the OpenCode default-driver decision

_Status: Draft — blocked on two decisions from Frank (below) and on `planning/features/opencode_compat_probe.md`_
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

## Decisions needed from Frank before this plan can be Ready
1. **Paid default, or a named opt-in agent?** Recommended: do **not** change the top-level `model`. Add GLM as a named
   primary agent (for example `build-cheap`, `mode: primary`, `permission.task` denying it like the `*-deep` agents) so
   money is only spent when asked for by name, and add a check to `opencode_sync_check.py` that paid model ids appear
   only on allow-listed keys. If a paid default is wanted instead, the guarantee change must be stated in `.opencode/README.md`.
2. **OpenRouter credit threshold.** Finding F9 left unverified whether having less than $10 of credit drops OpenRouter's
   free daily cap from 1000 requests to 50. If true, spending any credit on GLM could break all free routing. Is it OK to
   check this first (it needs a look at the account's credit and limits page, not a spend)?

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
- **Models:** `glm-5.3-flash` (pinned), the current free default driver, and the E-tier DeepSeek as the reference.
- **Cost cap about $1**, enforced in the script by summing `step_finish.part.cost` and aborting, plus a spend limit on the
  OpenRouter key.
- **Known-limitations pass:** run the mandatory SAFE/CAUTION/AVOID check from `opencode_free_models.md` section 5 on GLM
  before routing anything to it. Record the date and, if `opencode export` exposes it, the upstream provider (pinning an id
  is not a snapshot).
- **If adopted:** the model goes in the `.opencode/README.md` tier table as a new "cheap paid" tier (not in the free-models
  catalog), with the agent from decision 1; run `python tools/opencode_sync_check.py`.

## Tasks
- [ ] Frank's two decisions
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
- Given GLM is adopted, then plain agent names still cannot spend money (named opt-in agent, or an explicit documented
  guarantee change), the model id is pinned, and `opencode_sync_check.py` passes.
