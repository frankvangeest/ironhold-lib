# Feature: GLM 5.3 flash pilot and the OpenCode default-driver decision

_Status: Ready — plan-review 2026-10-08 (two rounds: system-architect, ux-gamedesigner-reviewer); Frank confirmed the thresholds, the $2 script cap and the $3 OpenRouter key limit (set on the key)_
_Planned at: `1a4fa55` (2026-10-09); first drafted at `edc93f1` (2026-10-08), split out of the compat-probe plan after its plan-review_

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
  next agent in the role's chain (see "Fallback chains"); never use a `*-deep` agent unless the user has said yes to it.
- A roster table in `.opencode/README.md`: role, primary agent (free model A), twin `<role>-alt` (free model B on another
  provider, as the C tier already does on purpose), and the existing paid `-deep` twin (denied for delegation).
- **Keep the paid driver from leaking spend to other agents:** give `general` and every command that sets no model an explicit
  free model, so only the driver's own turns are paid. Add a check to `opencode_sync_check.py` that paid model ids appear
  only on allow-listed keys (the top-level `model` and `*-deep`).
- Subagent reports go back into the paid driver's context, so the prompt asks for short reports.
- The driver's own turns dominate the cost: a trivial GLM 5.3 flash turn is about $0.005 with a 31.8k-token machine-specific
  baseline, so the number of turns matters more than the model price. The pilot measures it.

## M365 Copilot through a local proxy (added 2026-10-08)
Frank has a proxy at `C:\ProgramData\m365-copilot-proxy` that serves M365 Copilot as an OpenAI-compatible endpoint at
`http://localhost:4141/v1` (start it with `pnpm run proxy 4141` in that folder; default host `127.0.0.1`, unauthenticated, so
never expose it). It costs nothing extra. The global OpenCode config (`~/.config/opencode/opencode.jsonc`, machine-local) already
defines an `m365` provider with `m365/gpt-5.5-think-deeper`, the proxy README's recommended tool-calling model (its own
benchmarks, not ours; the default `m365-copilot` auto tone is reported to confabulate and must not be used). Tool calls are
**emulated** by the proxy, so tool reliability must be measured, not assumed.

How it fits the routing design:
- **A delegate, never the driver.** The driver is the one model nothing can fall back for, so it must be always available
  (the paid model). A model that only works while a local process is running is a good *primary agent for a role* with a free
  OpenRouter twin as `-alt`, not the driver. Best fit: reviews (`system-architect`, `debug-detective`, `alignment-reviewer`),
  which are advisory only anyway.
- **Proxy down looks like a 429:** the delegation errors and the driver moves to the next agent in the role's chain (the free primary, then `-alt`). No extra mechanism.
- **Portability:** the repo's `.opencode/opencode.json` must stay valid on a machine without the proxy (the sync check
  validates model ids against `opencode models`). Decision for Frank: either (a) keep `m365/...` out of the repo config and put
  the agent overrides in the machine-local global config, or (b) add `m365` agents to the repo config with documented
  prerequisites and make `opencode_sync_check.py` treat an unavailable `m365` provider as informational, not as drift.
  Recommendation: (a) first, since the provider itself is already machine-local. **Decided 2026-10-08 (Frank): (a)**, machine-local overrides in the global OpenCode config; the repo config stays portable.
- **Preflight:** the pilot and `opencode_probe.py --live` check reachability of the proxy (`GET http://localhost:4141/v1/models`)
  before routing anything to `m365/...`, and say how to start it if it is down.
- **Account note (not a judgement):** the proxy signs in to the work Microsoft 365 account (credentials and a TOTP secret in
  local config files, an automated browser login) and sends repository content to Copilot. Worth confirming with IT that this
  use is acceptable; never put its credential files in the repo.
- **Pilot addition:** run the review task and the routing task against `m365/gpt-5.5-think-deeper` too, with the same
  pass criteria, and record empty/throttled responses (the proxy has its own degradation backoff).

## Plan-review outcome (2026-10-08)
Both reviewers agreed the plan "needs more design work"; nothing touches a crate, WASM or the designer docs. Frank's answers
and what they change (this section **overrides** the earlier sections where they differ):

**Frank's answers**
- **m365 stays machine-local (a)**, but the repo config cannot be overridden from the global file: verified 2026-10-08 with a
  temporary `XDG_CONFIG_HOME` holding `agent.system-architect.model = m365/...` (`opencode debug agent system-architect` still
  reported the repo's `opencode/nemotron-3-ultra-free`), so the project config wins (new fact **V17**, README table). (a) therefore
  means **extra agents with new names** (for example `system-architect-m365`) defined only in the global config; the repo roster and
  driver prompt mention them conditionally ("if the agent exists"). A `{file:}` prompt in the global config must be an absolute path,
  so those agents point at the primary checkout's prompt files or carry the prompt inline.
- **Budget: $10 a month in total, mostly for GLM 5.3 flash as the driver.** The pilot cap is **$2** (proposed; Frank to confirm),
  split about $1 GLM, $0.5 DeepSeek reference, rest reserve; the script aborts on the cap and the result table says which cells
  are missing. Runs go in this order: GLM, current free default, DeepSeek, m365, so an abort leaves the most useful cells. The cost
  from `step_finish` probably excludes child (task) sessions, so delegation runs are reconciled against OpenRouter key usage; the
  key's spend limit is the real backstop. Long-session compaction runs on the paid model and is part of the measured cost.
- **Reviews and architecture go to a strong reasoning model**, not to GLM: m365 `gpt-5.5-think-deeper` where the proxy runs, the
  `*-deep` DeepSeek agents otherwise. **GLM asks the user before using a paid model**: `permission.task` for `*-deep` changes
  from `deny` to `ask`. Checked 2026-10-09 (V20): headless `opencode run` auto-approves `ask`, so unattended runs must override `*-deep` to `deny`.
- **Both the primary agent and its `-alt` twin fail: stop and report.** The driver must not do the work itself.
- **Gemini counts as free** (no billing account is linked; if one is ever linked the allow-list entry must be removed).
- **IT confirmation is settled**: using the M365 proxy is fine. Kept anyway: explicit deny rules so no agent can read
  `C:\ProgramData\m365-copilot-proxy` (credential and TOTP files) with `cat`/`grep`/`find`, and the scoring script never logs
  request bodies and touches the proxy only through `GET /v1/models`.
- **`-alt` roles (decided later the same day, see "Follow-up decisions")**: twins for the roles that run on free models.

**Design changes from the review**
- **One free/paid classifier** in `tools/_opencode_common.py`: an explicit allow-list of free model ids (anything unlisted fails
  closed), `google/gemini-3.8-flash` listed as free, the paid keys limited to the top-level `model`, `build` (it inherits it) and
  `*-deep`. `opencode_sync_check.py` (repo JSON) and `opencode_probe.py --agents` (resolved config) both use it, so they cannot
  disagree. The probe's `--agent NAME` flag is dropped (the `--agents` assertion covers it).
- **"Different provider" for an `-alt` twin means a different upstream lab**, not a different OpenCode prefix (Nemotron through
  `opencode/` and through `openrouter/` is one upstream pool). The roster table states the lab explicitly and the checks compare it.
- **Driver prompt delivery is decided by a fact, not by taste**: verify with `opencode debug agent build` whether `agent.build.prompt`
  replaces the built-in prompt and which agents receive `instructions`, record it as **V18**, then pick. Slash commands
  (`/code-review`, `/plan-review`, ...) run their own orchestrator and do not see the driver prompt, so the roster and the retry
  rule are also added to those commands' text.
- **README guarantee rewritten, not appended**: paid = the default session (`build`); free = `plan`, every subagent, every command
  that sets its own model; paid only when asked for and confirmed = `*-deep`. The C-tier row "Default `model` (`build`/`general`)"
  is split because `general` gets its own free model; a letter for the new "cheap paid" tier is added to the tier table.
- **Routing-failure test mechanics**: failure is injected by editing the pilot worktree's own `.opencode/opencode.json` (the
  `OPENCODE_CONFIG` file loses to it), with a second failure mode of stopping the m365 proxy. Pass check: the `subagent_type` of the
  task parts in `opencode export <session>` shows the `-alt` twin and no `*-deep`. Empty or thin answers are explicitly untested.
- **Pilot worktrees** are created with `git worktree add --detach` at an `integration` commit that already has `.opencode/` (so the
  post-checkout hook adds nothing and the base stays pinned); fixtures stay in those throwaway worktrees and are never committed
  under `assets/projects/`.
- **Thresholds, pre-registered (Frank confirmed 2026-10-08):** aggregate over the 5 core tasks x 3 runs = 15 runs per model (`toolcall` and the routing task are judged by their own criteria, not in this denominator): pass
  rate at least 80% and not below the current free default's rate; cost per task at most $0.10 on average and $0.25 worst case;
  median wall time per task no worse than twice the free default's. Tie-break: the cheaper model. If DeepSeek beats GLM within the
  budget it is eligible to become the driver (Frank may veto).
- **If GLM fails the thresholds** the roster, `-alt` twins, free `general`, the classifier and the probe extension stay (they are
  worth having on their own); the driver goes back to Laguna, or tries `nex-n2.5-pro` (already named in the README).
- **Housekeeping**: GLM 5.3 flash's upstream tool-calling support is part of the known-limitations pass (`glm-5.2:free` is excluded
  for lacking it); update V7's link when this plan moves to `done/`; refresh `Planned at:` when it becomes Ready; give every `-alt`
  agent a description starting "Fallback for `<role>` ..." (the driver chooses by description); `docs/dev/claude_md_maintenance.md`
  gets one row ("rules only for the OpenCode driver" -> `.opencode/prompts/driver.md`) if the prompt lands in `instructions`.

## Follow-up decisions (Frank, 2026-10-08, after the answers above)
- **`-alt` twins for the roles on free models**: `system-architect`, `debug-detective`, `alignment-reviewer`, `wasm-perf-reviewer`,
  `integration-test-author`, `ron-gameplay-scripter`, `data-format-doc-writer`, `explore`. **No twins for the Gemini roles**
  (`ux-gamedesigner-reviewer`, `game-world-designer`): Gemini is kept for its creativity, and its free quota is the only limit.
- **m365 takes precedence over the free models where it works well.** It reasons deeply and costs nothing extra (part of the company
  M365 subscription). Roster order for a role on a machine with the proxy running: `<role>-m365` (machine-local, global config) >
  `<role>` (free primary) > `<role>-alt` (free twin on another upstream lab); `*-deep` stays paid and asks first. The driver prompt
  states this order and the preflight (`GET http://localhost:4141/v1/models`) decides whether the m365 step exists at all.
- **Deeper thinking for `system-architect` and `debug-detective`**: their order puts m365 first, then `-deep` (DeepSeek, asks first),
  then the free primary; the pilot's review task is scored on these two roles specifically.
- **m365 earns that precedence by passing a tool-calling test** (the proxy emulates tool calls, so this is the weak spot, not
  reasoning). A new task `toolcall` is part of the pilot for every delegate candidate, m365 first. Mechanical pass criteria, all read
  from `opencode export <session>` and the resulting files: (1) a sequence of at least 4 dependent calls (grep -> read -> edit ->
  read-back) finishes with the exact expected file content; (2) arguments are well-formed JSON every time (no call fails schema
  validation, counted per run); (3) a call that must be **skipped** (the tool is denied by permission) is not retried in a loop;
  (4) a read of a file that does not exist is reported, not invented; (5) two independent reads in one turn both come back. Threshold:
  at least 90% of calls well-formed over 15 runs and every criterion met in at least 13 of 15 runs; below that m365 stays a
  reviewer-only delegate of last resort (after the free primary) instead of first choice. The same task is run against GLM (as
  driver) and the free agents, so the numbers are comparable.

## Re-review resolutions (2026-10-08, after the second review)
Both reviewers: "Not ready, but close"; the points below are the editing pass. Where this section differs from an earlier one, it wins.

**One rule for `*-deep`.** `*-deep` agents are paid. They are **never part of an automatic chain**: the driver may only *offer* one
to the user (attended runs: `permission.task` is `ask`, the user answers) and a headless run keeps `deny`. So "no `*-deep` ran"
stays true for every automatic path, and "GLM asks before paid" is the same rule.

**Fallback chains** (one place; the driver prompt and the README roster are generated from this). The driver tries the next step
at most **twice** per delegation; when the chain is exhausted it **stops and reports** (never does the work itself).

| Role group | Proxy running | Proxy not running | Then |
|---|---|---|---|
| `system-architect`, `debug-detective` | `-m365` > free primary > `-alt` | free primary > `-alt` | stop and report; attended: offer the `-deep` agent |
| `alignment-reviewer`, `wasm-perf-reviewer` | `-m365` > free primary > `-alt` | free primary > `-alt` | stop and report |
| `integration-test-author`, `ron-gameplay-scripter`, `data-format-doc-writer`, `explore` | free primary > `-alt` | same | stop and report |
| `ux-gamedesigner-reviewer`, `game-world-designer` (Gemini, no twin) | Gemini | Gemini | stop and report |

(The chain is the same attended and headless; only the final "offer `-deep`" step differs, and it does not run headless.)

**Credential guard does not depend on `ask`.** Besides the bash deny patterns, `read` and `external_directory` get an explicit
`deny` for `C:\ProgramData\m365-copilot-proxy` (bash patterns are easy to bypass with `type`, `Get-Content`, `python -c`). The V18
task verifies it with `opencode debug agent build --tool read` on a file in that folder (expected: denied), and the probe's
`--agents` assertion checks the deny rule is present on every agent.

**`toolcall` run counts match the budget.** m365 and the free agents cost nothing, so they run `toolcall` **15 times** (the 90%
well-formed and 13-of-15 criteria stand). GLM and DeepSeek run it **5 times** each (criteria restated: 100% of calls well-formed
is not required, at least 90% is, and each criterion met in at least 4 of 5 runs). The task count is 7 (5 core, `toolcall`, routing).

**Classifier and parity.** The allow-list has a "machine-local, no extra cost" class containing exactly
`m365/gpt-5.5-think-deeper` (never `m365/m365-copilot`), so `--agents` does not fail closed on the global `-m365` agents;
`check_deep_twins_match` covers `-m365` prompt parity where those agents are visible. The global `-m365` agents' absolute
`{file:}` paths tie them to the primary checkout's prompt files; that is accepted and documented in the README.

**README roster legend.** One legend above the roster explains the four suffixes: none = free primary (repo config); `-alt` = free
twin on another upstream lab (repo config); `-m365` = machine-local, needs the proxy running, free of extra cost (global config
only, "agent not found" elsewhere); `-deep` = paid, asks first (repo config). Each roster row shows paid/free, asks-first and where
the agent is defined. `integration-test-author` (nex) and `ron-gameplay-scripter` (laguna) get twins from a lab different from both
their primary and the driver's fallback candidates.

**Numbers.** Thresholds are confirmed. The OpenRouter key's hard spend limit is proposed at **$3** on a pilot-only key (the
script's own cap is $2, so the script stops first); Frank to confirm the $2 and $3.

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
  the next agent in the chain and not touch a `*-deep` agent without a yes. Pass criterion: the final artifact is correct and no
  `*-deep` agent ran (the routing failure is injected on a role that is not `system-architect`/`debug-detective`, whose chains
  start with m365).
- **Models:** `glm-5.3-flash` (pinned), the current free default driver, the E-tier DeepSeek as the reference, and
  `m365/gpt-5.5-think-deeper` as a delegate (its runs cost nothing, so they sit outside the cap ordering).
- **Cost cap $2** (see Plan-review outcome), enforced in the script by summing `step_finish.part.cost` and aborting, plus a spend
  limit on the OpenRouter key.
- **Known-limitations pass:** run the mandatory SAFE/CAUTION/AVOID check from `opencode_free_models.md` section 5 on GLM
  before routing anything to it. Record the date and, if `opencode export` exposes it, the upstream provider (pinning an id
  is not a snapshot).
- **If adopted:** the model goes in the `.opencode/README.md` tier table as a new "cheap paid" tier (not in the free-models
  catalog), with the GLM driver (Decisions 1); run `python tools/opencode_sync_check.py`.

## Tasks
- [x] Frank's two decisions (2026-10-08, above)
- [x] Plan-review 2026-10-08 and Frank's answers (above); V17 verified
- [x] Facts checks done 2026-10-09, recorded as V18-V20 in `.opencode/README.md`: the driver prompt goes on `agent.build.prompt` (replacement of the built-in prompt unverified, so the pilot gets a with/without-driver-prompt arm for GLM); the proxy folder is guarded by an `external_directory` deny (a `read` deny with an absolute pattern does not match); **headless `opencode run` auto-approves `ask`**, so `*-deep: ask` would silently allow paid delegation in unattended runs: the pilot scripts and any unattended run must override it to `deny` (via `OPENCODE_CONFIG_CONTENT`, which wins over the project config), and the repo config keeps `deny` for `*-deep` until a human-attended mode is decided
- [x] Frank confirms the $2 cap and the $3 key limit (2026-10-09; the $3 limit is set on the OpenRouter key)
- [ ] Driver prompt, roster table (with the upstream lab per agent) and `-alt` twins for the eight roles on free models (Follow-up decisions)
- [x] Shared free/paid classifier in `tools/_opencode_common.py` (`model_class`, `paid_model_problems`; unknown ids fail closed, Gemini listed free, only `m365/gpt-5.5-think-deeper` machine-local) and the `opencode_sync_check.py` rule for paid ids (done 2026-10-09; proved by planting a paid id on `alignment-reviewer`: exit 1, then reverted)
- [ ] Explicit free models on `general` and the no-model commands (`new-project`, `rust-idioms`); `external_directory` deny rules for the m365 proxy folder (V19); `*-deep` stays `deny` in the repo config (V20)
- [ ] Extend the probe for agents (all model-free): an
  `--agents` assertion runs `opencode debug agent <name>` with no `--tool` for every configured agent and checks that `read`
  is not denied, that the model passes the shared classifier, and that an `-alt` twin is on a different upstream lab;
  extend `check_deep_twins_match` in `opencode_sync_check.py` so an `-alt` twin must share its base agent's `{file:}` prompt
- [ ] Task set (including `toolcall`), fixtures, scoring script with the cost cap and the pre-registered thresholds
- [ ] Known-limitations pass on GLM
- [ ] Run the pilot (3 runs per task and model), write the results into this plan
- [ ] Adopt or not per the thresholds; update `opencode.json`, the README tier table, `opencode_sync_check.py`

## Open questions
- Is the empty-response rate of the free default high enough (about 16% was reported for a free endpoint) to justify a paid
  default for unattended work?
- Which OpenCode mirrors of the Claude Code agents/commands should be routed to the cheap tier (Claude Code itself is never routed through OpenCode)? Per Frank: reviews and architecture go to the strong reasoning models, not GLM.

## Acceptance criteria
- Given the pilot ran, then the plan records success, wall time, tokens and cost per task per model with at least 3 runs
  each, and the decision follows the thresholds that were written down before the run.
- Given GLM is adopted as the driver, then the README states the changed guarantee (driver paid, delegates free, `*-deep` only
  after a yes), no agent other than the driver and the `*-deep` agents has a paid model (checked by `opencode_sync_check.py`),
  the model id is pinned, and the routing task passes (correct subagent, recovery through `-alt`, no `*-deep` run).
