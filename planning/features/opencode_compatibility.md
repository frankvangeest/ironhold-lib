# Feature: OpenCode compatibility (tooling)

_Status: In Progress (v0/v1/v2/v3's sync-check script all Done and live-verified 2026-09-22 against opencode-ai 1.18.31 — a few checklist items still open, see below; the optional `rust-idioms`-as-skill move is not done)_
_Planned at: `20287fc` (2026-09-22)_

This is a tooling/infrastructure plan, not an engine feature: nothing here touches `crates/`,
`assets/`, or the WASM build. It's filed under `planning/features/` because it needed design
decisions before anything got written. Frank answered all the open questions on 2026-09-22; they
are recorded under **Decisions** below and built into the design.

## Phases

| Phase | Backlog item | Status | Completed |
|---|---|---|---|
| v0 | Prerequisite: fix the 8 `.claude/hooks/*.py` scripts to follow Claude Code's real exit-code/output contract (F4) | Done | `7155299` (2026-09-22) |
| v1 | Working `.opencode/opencode.json`: instructions, permissions, 3 providers, agents and commands pulled in from `.claude/` via `{file:}`, free-by-default routing with `-deep` opt-in, memory-inbox rule, thin `AGENTS.md` | Done, live-verified (checklist items 1-3/5-8/13; 4/9/10/11/12 still open) | `b8749c3`+ (2026-09-22) |
| v2 | Hook parity: one OpenCode plugin that runs the (now fixed) `.claude/hooks` scripts | Done, live-verified for the reminder half; blocking half not independently fired (see v2 tasks) | 2026-09-22 |
| v3 | Drift check script (extended to also flag a disappeared model — see v3 tasks), plus an optional move of `rust-idioms` into a shared `.claude/skills/` | Sync-check script Done, live-verified; the optional `rust-idioms` move not done | 2026-09-22 |

## What

Frank wants to run this repo from OpenCode (opencode.ai) as well as Claude Code, mostly on free
models. There are three model sources: OpenRouter free models, OpenCode Zen free models, and
Frank's Gemini API key on Google's free tier. The one paid option is escalating chosen agents to
`deepseek/deepseek-v4.1-flash` on OpenRouter. When this is done:
- an OpenCode session in this repo reads the same project rules Claude Code reads;
- it can call the same 9 review/authoring agents and 9 slash commands;
- each agent and command runs on a deliberately chosen model, and anything implicit (commands,
  automatic delegation) always runs **free**;
- none of this adds a second hand-maintained copy of any prompt, or lets a weaker model write into
  the agent-memory store the Claude agents rely on.

## Why

- Free-model sessions cut Anthropic spend on work that doesn't need a top model: exploration, RON
  authoring, running and reading CLI output, first-pass reviews.
- **The committed `.opencode/opencode.json` (`3f009e3`) is broken today, and worse than inert.**
  See Finding F1. It needs replacing before OpenCode is used in this repo at all.

## Decisions (Frank, 2026-09-22)

1. **Merge gate.** OpenCode reviews are **advisory only**. Claude Code's `/code-review` stays the
   mandatory step-4 review before any merge into `integration`. This is a rule, not a proposal.
   See §4 for how it is enforced.
2. **OpenRouter credit.** Frank has already bought $10 of credit. That puts the account on the
   1000-requests/day free tier and pays for DeepSeek escalation. Nothing more to buy.
3. **E-tier twins: yes, but with the naming inverted.** Plain agent names are free; paid DeepSeek
   is reachable only through an explicit `-deep` name. The reasoning is in §4, "Naming and
   defaults".
4. **Second and third provider: yes.** Add OpenCode Zen, and use Frank's Gemini API key (already
   configured in OpenCode) as a third source.
   - Zen's free `deepseek-v4-flash-free` is **not** a stand-in for tier E. Frank's view: OpenRouter's
     paid `deepseek-v4.1-flash` behaves closer to the V4-Pro reasoning tier, while Zen's free
     "v4-flash" is a lighter flash-class model.
   - It is placed in a lighter tier instead (§4).
5. **`AGENTS.md` shim: confirmed.** Frank uses only Claude Code and OpenCode today. He tried
   Cursor and standalone Gemini earlier and has stopped using both. No other tool depends on the
   current `AGENTS.md` content.
6. **Global Juva layer: suppress it in OpenCode.** Frank asked for `~/.config/opencode/AGENTS.md`
   to be created, which stops OpenCode falling back to `~/.claude/CLAUDE.md`. That file is
   **machine-local, not repo state**; it is created outside this plan and is not one of its tasks.
   Recorded here only so later readers know why OpenCode sessions on Frank's machine don't see the
   Juva global layer.
7. **Hook script fix before v1: done.**
   - All 8 scripts now use a shared `.claude/hooks/_hook_common.py`:
     - `block()` hard-blocks a PreToolUse call with exit 2 plus stderr;
     - `emit_context()` sends a PostToolUse reminder via `hookSpecificOutput.additionalContext`.
   - Verified end to end, including a live case where the fixed `block()` stopped a real command.
   - This is phase v0 above. It must be committed before v1 starts.

## Verified findings (checked against current docs and source on 2026-09-22, not just the earlier research notes)

Sources: opencode.ai/docs/{rules,agents,commands,permissions,config,plugins,skills,cli}; the
published schema at `https://opencode.ai/config.json`; the source on `sst/opencode@dev`
(`packages/opencode/src/config/{config,parse,v2-compat,agent,command,plugin,variable}.ts`,
`session/instruction.ts`, `tool/task.ts`, `packages/plugin/src/index.ts`); the Claude Code hooks
docs (code.claude.com/docs/en/hooks); the live OpenRouter `/api/v1/models`, OpenCode Zen
`/zen/v1/models` and `models.dev/api.json` endpoints; and ai.google.dev/gemini-api/docs/pricing.

**F1: the current `.opencode/opencode.json` is rejected, not silently ignored.**
- `config/v2-compat.ts::lower()` looks for a top-level `"permissions"` key (plural). If it finds
  one it **throws `InvalidError`**: *"V2 permissions are not supported by OpenCode V1. Use V1
  "permission" rules or run opencode2."* So the whole file fails to load.
- The other bogus keys (`hooks`, `fallbackToClaudeConfig`, `claudeConfigPath`) would be dropped
  quietly at runtime anyway, because `ConfigParse.schema` decodes with
  `onExcessProperty: "ignore"`. The published JSON schema does set
  `Config.additionalProperties: false`, so editors flag all four keys.
- None of the four keys does anything. `CLAUDE.md` fallback needs no config key; it's built in.
- Frank confirms by running `opencode debug config` from the repo root. Expected result: a config
  error that names `permissions`. This came from reading `@dev`, so a released build could differ.

**F2: `AGENTS.md` already exists at the repo root, so OpenCode does NOT load the root `CLAUDE.md`.**
- `session/instruction.ts` tries `AGENTS.md`, then `CLAUDE.md`, then `CONTEXT.md`, and **stops at
  the first filename that matches anywhere up the tree** ("The first project-level match wins").
- The root `AGENTS.md` (56 lines, last touched in `ac22c41`) therefore wins. None of the root
  `CLAUDE.md` gets loaded: not the workflow, not the branching model, not the critical rules. The
  only link is the line "you must also read CLAUDE.md", which a free model may well ignore.
- The earlier research premise ("the whole `CLAUDE.md` hierarchy is read with zero changes") is
  **wrong for the root file**.
- Nested files do work. When the model reads a file, `resolve()` walks up from that file's folder
  towards the root and attaches the nearest `AGENTS.md`/`CLAUDE.md` in each folder, once per
  session. So `crates/ironhold_core/src/CLAUDE.md`, `tests/CLAUDE.md`, `planning/CLAUDE.md`,
  `tools/*/CLAUDE.md` and `assets/**/CLAUDE.md` all load on first touch.
- `AGENTS.md` is also out of date. It says "camera-following logic must be scheduled in
  `FixedUpdate`", but the camera chain runs in `Update` (`lib.rs` ~l.338–376). Note that
  `crates/ironhold_core/src/CLAUDE.md` § "Physics & movement must use FixedUpdate" repeats the same
  out-of-date "camera-follow" wording.
- The global fallback (`~/.claude/CLAUDE.md`) gets loaded only when
  `~/.config/opencode/AGENTS.md` doesn't exist. Frank's Decision 6 creates that file, so the Juva
  global layer stops loading.

**F3: the directory names and loaders are more forgiving than the research said.**
- The agent, command and plugin loaders glob `{agent,agents}/**/*.md`,
  `{command,commands}/**/*.md` and `{plugin,plugins}/*.{ts,js}`, so singular and plural both work.
  The docs use the plural; this plan uses the plural.
- OpenCode does **not** read `.claude/agents/` or `.claude/commands/`.
- OpenCode **does** read `.claude/skills/<name>/SKILL.md` natively.
- Pointing `.opencode/agents/` straight at the Claude files (symlink or copy) won't work. The
  markdown loader spreads the YAML frontmatter into the agent config, and Claude's `tools:` string
  and `model: opus` don't fit OpenCode's schema. `ConfigParse.schema` would throw.

**F4: all 8 `.claude/hooks/*.py` scripts used to be no-ops under Claude Code. Fixed in v0.**
- Claude Code's contract: only **exit 2** blocks a PreToolUse call (and stderr is what the model
  sees). Exit 1 is non-blocking. For PostToolUse, plain stdout on exit 0 goes to the debug log
  only; to reach the model it has to be JSON `hookSpecificOutput.additionalContext`.
- The old scripts printed "BLOCKED" and exited with 1, or printed plain reminders on exit 0, so
  none of them blocked anything or reached the model.
- Now fixed through `_hook_common.py` (Decision 7).
- The v2 bridge (§3, Hooks) maps exactly those two output shapes.

**F5: subagents start with a fresh context, but they can run in parallel and in the background.**
- `tool/task.ts`: *"Each agent invocation starts with a fresh context unless you provide task_id
  to resume."* There is nothing like Claude's context-inheriting fork.
- Several `task` calls in one message run concurrently.
- `background: true` exists, with a completion notice.
- So the parallel review in `/code-review` ports as-is. The cost is that each reviewer re-reads
  the diff and the rules files cold.

**F6: the Skill gap is smaller than first assumed.**
- OpenCode has its own `skill` tool, with on-demand loading from `.opencode/skills/`,
  `.claude/skills/` and `.agents/skills/`.
- This repo's "skills" (`/code-review`, `/ship`, and so on) are really
  `.claude/commands/*.md`: plain markdown with `$ARGUMENTS`, no frontmatter.
- None of the command bodies calls the Skill tool. Their "launch agents in parallel" wording maps
  directly onto OpenCode's `task` tool.
- What really has no counterpart: `ToolSearch`/deferred tool schemas (not needed),
  `Monitor`/`Cron*`/`ScheduleWakeup`, `EnterWorktree`/`isolation: "worktree"`, and forks.

**F7: file substitution makes a single source of truth possible.**
- `config/variable.ts` replaces every `{file:path}` token in the config text before parsing.
  Paths resolve relative to the config file's folder, the content is JSON-escaped, and **several
  tokens in one string work**.
- So `"prompt": "{file:./prompts/preamble.md}\n\n{file:../.claude/agents/system-architect.md}"`
  builds an OpenCode agent from the Claude file itself.
- `"template": "{file:../.claude/commands/validate.md}"` does the same for a command, and
  `$ARGUMENTS` works the same way in both tools.

**F8: the model catalog is already out of date and partly wrong.** Live checks on 2026-09-22:
- `stealth/union-alpha` is gone from OpenRouter.
- Zen now also lists `deepseek-v4-flash-free`, `jev-1.13-free` and `mimo-v2.6-flash-free`, and
  lists the paid `deepseek-v4.1-flash` too.
- The "355B tokens" / "137B tokens" context values are really 262K.
- `laguna-s-2.1` appears twice in the Coding table.
- The "Agent Configuration Recommendations" examples use keys that don't exist (`agents`, which
  should be `agent`, and `model_routing`) and a model that isn't in the catalog
  (`openrouter/mistral/mistral-7b`).
- The catalog doesn't cover Gemini at all.
- All the OpenRouter models picked below report tool-calling support. `z-ai/glm-5.2:free` does
  **not** support tools; exclude it.
- `deepseek/deepseek-v4.1-flash` on OpenRouter: 1M context, **$0.15/M input, $0.60/M output**.

**F9: rate limits are the binding constraint, and each provider has its own.**
- **OpenRouter:** 20 requests/minute on all free models together, per account.
  - Frank already has $10 of credit (Decision 2), which puts the daily cap at **1000 requests**
    instead of 50.
  - The per-minute cap is the real limit for a parallel review. Several reviewers on OpenRouter at
    once share 20 RPM.
  - OpenRouter's wording is "purchased credits ≥ 10". Check that spending the balance below $10 on
    DeepSeek doesn't drop the account back to 50/day.
- **Zen:** the free models' limits aren't published. Zen's free models are also offered as
  time-limited promotions, so they can disappear.
- **Gemini free tier:** RPM/RPD are shown per project in AI Studio, not in the docs. Historically
  they are low (tens to low hundreds of requests per day per model). Free-tier content is marked
  "used to improve our products". That's acceptable here because the repo is public, but it is a
  data-policy difference from the paid tiers.
- **Consequence for the design:** a `/code-review` fan-out should be spread across **providers**,
  not just across models. §4 does this.

**F10: Gemini via OpenCode.**
- Provider id `google` (`@ai-sdk/google`). It reads `GEMINI_API_KEY`, `GOOGLE_GENERATIVE_AI_API_KEY`
  or `GOOGLE_API_KEY`, or a key from `opencode auth login`.
- Model ids take the form `google/<model>`.
- Per ai.google.dev pricing on 2026-09-22, these text models have a **free tier**:
  - `gemini-3.8-flash` (newest; the docs pitch it for long-horizon software engineering and
    autonomous agents, 1M context);
  - `gemini-3.7-flash`, `gemini-3.6-flash`, `gemini-3.5-flash`;
  - `gemini-3.5-flash-lite`, `gemini-3.1-flash-lite`;
  - `gemini-2.5-pro`, `gemini-2.5-flash`, `gemini-2.5-flash-lite`;
  - `gemini-flash-latest`.
- `gemini-3.1-pro-preview` has **no** free tier.
- Watch out: the free tier applies only to a Google Cloud project **without billing enabled**. If
  Frank's key belongs to a billing-enabled project, every call is billed. `gemini-3.8-flash` paid
  rates are $0.75/M input and $3.75/M output until 2026-12-31, then double, which is about 5× the
  DeepSeek E tier. See verification step 4.

## Approach

### 1. Instruction files

- **Rewrite `AGENTS.md` as a thin, tool-neutral shim** (Decision 5) so it can't drift from
  `CLAUDE.md`:
  - delete its duplicated rules, including the out-of-date FixedUpdate/camera claim;
  - keep a short pointer: "`CLAUDE.md` files are the source of truth";
  - add a short **"Running outside Claude Code"** section with:
    - a translation table: "Agent tool / launch agents in parallel" → OpenCode's `task` tool;
      "`/code-review` etc." → OpenCode `/commands` with the same names; skip anything that
      mentions `Monitor`/`ToolSearch`/`EnterWorktree`/`Cron`/`Skill`;
    - the **merge-gate rule** (Decision 1, §4);
    - the memory-inbox rule (§5).
- **Add `"instructions": ["CLAUDE.md"]` to `opencode.json`** so the root `CLAUDE.md` loads in
  OpenCode even though `AGENTS.md` wins discovery (Finding F2).
  - The path is resolved by `globUp` from cwd to the worktree root, so it also works inside each
    `../ironhold-lib-{slug}` feature worktree.
  - Nested `CLAUDE.md` files need nothing extra (dynamic attach).
- Do **not** set `OPENCODE_DISABLE_CLAUDE_CODE_PROMPT`. In the source, that flag also removes
  `CLAUDE.md` from the *project* discovery list, which would kill the nested attach. Frank's
  machine-local `~/.config/opencode/AGENTS.md` (Decision 6) is the right way to suppress the global
  layer. It only replaces the *global* fallback; project discovery is untouched.
- Leave the model-routing table out of instruction files. The model doesn't need it on every turn.
  Its home is this plan, plus a short `.opencode/README.md` for humans.
- Known cost: `crates/ironhold_core/src/CLAUDE.md` is about 154 KB (roughly 38K tokens) and gets
  attached the first time any core file is read. That's another reason Gemini's low daily caps
  (F9) suit only low-volume slots. Splitting that file is a separate docs job.

### 2. Agents and commands: include the Claude files, don't copy them

The chosen mechanism is `agent` and `command` blocks in `.opencode/opencode.json` that pull in the
Claude files through `{file:}` (Finding F7).
- **No `.opencode/agents/*.md` or `.opencode/commands/*.md` files.**
- The prompt bodies stay in exactly one place: `.claude/agents/*.md` and `.claude/commands/*.md`.

**Who owns what:**
- `.claude/` owns prompt bodies and command templates.
- `.opencode/opencode.json` owns only what OpenCode needs and Claude files can't express: a
  one-line `description` (Claude's descriptions carry long `<example>` blocks), `mode`, `model`,
  `permission`, and optionally `temperature`/`steps`.
- A `-deep` variant (§4) is just a second `agent` entry with the **same** `{file:}` prompt. No
  prompt text is duplicated.

**Shared preamble:** `.opencode/prompts/agent_preamble.md` goes in front of every agent prompt. It
tells the model:
1. The YAML frontmatter at the top of the included text is Claude Code metadata. Ignore its
   `tools:`/`model:`/`memory:` lines; tool access is controlled by OpenCode permissions.
2. Your agent name is the frontmatter `name:`. This is also true for a `-deep` variant, which has
   the same frontmatter, so both variants share one memory directory. Your memory is
   `.claude/agent-memory/<name>/`. Read `MEMORY.md` there first. Claude Code injects that index
   automatically; OpenCode doesn't, so it has to be spelled out.
3. The write rule in §5.
4. Translate Claude-only tool names using the `AGENTS.md` table. Under OpenCode your review is
   advisory (Decision 1).

**Commands:** `"template": "{file:../.claude/commands/<name>.md}"`. For `/code-review` and
`/plan-review`, the template can also start with `` !`git diff --stat main...HEAD` `` so every
cold-started reviewer gets the scope for free. Check in v1 that `` !`cmd` `` expansion works in a
JSON-defined template and not only in markdown commands.

**Drift risk that remains:** a *new* Claude agent or command added without a matching
`opencode.json` entry. v3 adds `tools/opencode_sync_check.py`, which reports every
`.claude/{agents,commands}/*.md` with no `agent`/`command` entry and every entry whose `{file:}`
target is missing. OpenCode already fails loudly on a missing `{file:}` path (`InvalidError`: "bad
file reference").

**Rejected alternatives:**
- Hand-copied `.opencode/agents/*.md`: nine prompts of 88–320 lines each would drift.
- Symlinks: unreliable on Windows, and the frontmatter is incompatible anyway (F3).
- A generator script that writes `.opencode/agents/` from `.claude/agents/`: adds a build step and
  a committed copy that goes stale, when `{file:}` already does the job at load time.

### 3. Replacement `opencode.json` (schema-correct)

Checked against `$defs.Config`/`PermissionConfig`/`AgentConfig` in the published schema. Four
rules shape it:
- **Last matching pattern wins**, so put broad `*` rules first and exceptions after.
- `webfetch` accepts only an action (`allow`/`ask`/`deny`), not domain patterns. Claude's
  `docs.rs`/`github.com` domain restriction can't be expressed; it's set to `allow` because it's
  read-only.
- OpenCode's default is **allow** for almost everything, including all bash. Leaving out a
  `permission` block would be *less* safe than the Claude setup, so this block has to exist.
- API keys go through `opencode auth login` (stored outside the repo) or
  `{env:OPENROUTER_API_KEY}` / `{env:GEMINI_API_KEY}`. Never inline them (Juva secrets rule).
  Frank's Gemini key is already configured in OpenCode, so no provider block is needed for it
  beyond using `google/...` model ids.

```jsonc
{
  "$schema": "https://opencode.ai/config.json",
  "instructions": ["CLAUDE.md"],
  "model": "openrouter/poolside/laguna-s-2.1:free",
  "small_model": "opencode/nemotron-3.5-lightning-free",
  "permission": {
    "edit": "allow",
    "webfetch": "allow",
    "websearch": "allow",
    "external_directory": "ask",
    "task": { "*": "allow", "*-deep": "deny" },   // paid agents are never auto-delegated (§4)
    "bash": {
      "*": "ask",
      "cargo test*": "allow", "cargo check*": "allow", "cargo build*": "allow",
      "cargo run*": "allow", "cargo clippy*": "allow", "cargo fmt*": "allow",
      "cargo doc*": "allow", "cargo tree*": "allow", "cargo metadata*": "allow", "cargo info*": "allow",
      "cargo clean*": "ask",              // Claude allows this; CLAUDE.md says never on a feature branch
      "wasm-pack build*": "allow",
      "python test_web.py*": "allow", "python tools/*": "allow", "python -c*": "allow",
      "tools/bin/ironhold*": "allow",
      "ls*": "allow", "cat *": "allow", "wc *": "allow", "echo *": "allow", "grep *": "allow",
      "find *": "allow", "find * -delete*": "deny", "find * -exec*": "ask",
      "git status*": "allow", "git log*": "allow", "git diff*": "allow",
      "git show*": "allow", "git rev-parse*": "allow",
      "git push*": "ask", "git merge*": "ask", "git worktree remove*": "ask", "git branch -d*": "ask",
      "git reset --hard*": "ask",
      "git add*pkg*": "deny", "git commit*pkg*": "deny"   // pkg/ is only ever committed by Frank by hand
    }
  },
  "agent": {
    // Plain name = the name every shared command template invokes = FREE tier.
    "system-architect": {
      "mode": "subagent",
      "description": "Architecture/maintainability review (free tier, advisory).",
      "model": "opencode/nemotron-3-ultra-free",
      "prompt": "{file:./prompts/agent_preamble.md}\n\n{file:../.claude/agents/system-architect.md}",
      "permission": {
        "task": "deny",
        "edit": { "*": "deny", ".opencode/memory-inbox/*": "allow" },
        "bash": { "*": "deny", "git diff*": "allow", "git log*": "allow", "git show*": "allow",
                  "git status*": "allow", "tools/bin/ironhold*": "allow" }
      }
    },
    // Explicit opt-in = PAID tier. Same prompt, same permissions, different model.
    "system-architect-deep": {
      "mode": "all",
      "description": "PAID (DeepSeek v4.1 flash) architecture review. Explicit @mention or --agent only.",
      "model": "openrouter/deepseek/deepseek-v4.1-flash",
      "prompt": "{file:./prompts/agent_preamble.md}\n\n{file:../.claude/agents/system-architect.md}",
      "permission": { /* identical to system-architect */ }
    }
    // ...debug-detective / debug-detective-deep the same way; one block per other agent (§4 table)
  },
  "command": {
    "validate": {
      "description": "Run the ironhold CLI validator on a project.",
      "model": "opencode/deepseek-v4-flash-free",
      "template": "{file:../.claude/commands/validate.md}"
    }
    // ...one block per command
  }
}
```

**About the pkg/ rule:** the `pkg/` deny rules on `git add`/`git commit` are a **hard** block at
the permission layer. The v2 bridge adds `prevent_dev_wasm_commit.py` (which now blocks for real,
F4) as a second, redundant guard. The release step on `integration` (step 14, `git add -f pkg/`)
stays manual for Frank. The hook's own message already says to do exactly that.

**Things to check in v1, not assume:**
- Does bash pattern matching test each subcommand of a compound `a && b` separately, or the whole
  string?
- Do edit patterns match repo-relative paths, as the docs example suggests?
- Does `task` glob matching work on agent names (`*-deep`)? The docs say it uses glob patterns on
  subagent names.
- Does `mode: "all"` make a `-deep` agent selectable with `opencode run --agent`?

**Hooks (Phase v2).** OpenCode has no `hooks` key; the equivalent is a plugin (Finding F1). The
script-contract prerequisite is done (v0). Add **one** local plugin,
`.opencode/plugins/claude_hooks_bridge.ts`, about 80 lines. It reads the `hooks` block from
**`.claude/settings.json`** at startup, so the hook list has one source of truth. On
`tool.execute.before`/`tool.execute.after` it:
- maps OpenCode tool names (`bash`, `edit`, `write`) to Claude's (`Bash`, `Edit`, `Write`) for the
  `matcher` regex;
- maps args (`filePath` → `file_path`, `command` → `command`);
- pipes `{"tool_name", "tool_input"}` JSON to each matching script;
- on **exit 2** (`_hook_common.block()`), throws `Error(stderr)` (before) or appends stderr to
  `output.output` (after);
- on stdout JSON `hookSpecificOutput.additionalContext` (`_hook_common.emit_context()`), appends
  that text to `output.output`, which is the text the model sees from the tool;
- treats any other exit code or output as non-blocking and silent, the same as Claude Code does.

Also:
- The scripts import `_hook_common` from their own folder. Run them by path
  (`python .claude/hooks/x.py`), exactly as `settings.json` already does, so `sys.path[0]`
  resolves it.
- Only `edit`/`write`/`bash` are mapped. `apply_patch` is used only by OpenAI-family models, none
  of which are routed here, so it's documented as unmapped.
- Add `.opencode/.gitignore` for the `node_modules`/`bun.lock` that OpenCode installs for plugins,
  if OpenCode doesn't create it itself.

### 4. Model routing

**Tiers.** All IDs checked live on 2026-09-22 (OpenRouter and Zen model lists, models.dev, Gemini
pricing), and all support tool calling.

| Tier | Model ID | Provider | Use |
|---|---|---|---|
| **E** paid escalation (only E tier) | `openrouter/deepseek/deepseek-v4.1-flash` | OpenRouter, credit-funded | Opt-in `-deep` agents only |
| **R** free reasoning | `opencode/nemotron-3-ultra-free` / `openrouter/nvidia/nemotron-3-ultra-550b-a55b:free` | Zen / OpenRouter (same model, two quotas) | Cross-file review, orchestration and consolidation |
| **R-lite** | `openrouter/nvidia/nemotron-3-super-120b-a12b:free` | OpenRouter | Narrow, single-dimension reviews |
| **C** free code | `openrouter/poolside/laguna-s-2.1:free` | OpenRouter | Writing Rust or RON that must compile or parse |
| **G** free general | `google/gemini-3.8-flash` | Gemini free tier | Low-volume prose, UX and creative judgment |
| **F** light | `opencode/deepseek-v4-flash-free` | Zen | Run a CLI and summarise; light scaffolding |
| **T** titles | `opencode/nemotron-3.5-lightning-free` | Zen | `small_model` only |
| **X** explore | `openrouter/thinkingmachines/inkling-small:free` | OpenRouter | The built-in `explore` subagent |

How the three decisions and the rate limits shape this table:
- **Zen's `deepseek-v4-flash-free` → tier F (Decision 4).** It's a capable flash-class coder, fine
  for "run `tools/bin/ironhold`, read the output, report". It replaces OpenRouter's Nemotron
  Lightning there, which also moves those frequent small requests off OpenRouter's 20-RPM quota.
  It is deliberately **not** in R or E.
- **Gemini `3.8-flash` → tier G.** It replaces `thinkingmachines/inkling:free`.
  - Gemini is a well-known quantity for prose and creative work, which settles the earlier
    "least-confident pick" for `game-world-designer`.
  - G's two agents are the lowest-volume ones: UX review is conditional, and world design is
    consulted rarely. That fits the free tier's low daily caps.
  - Gemini is kept **out** of the always-on review fan-out and out of the per-command tiers, where
    request volume would hit those caps.
  - Fallbacks if the cap bites: `google/gemini-2.5-flash` (free, older) or
    `openrouter/thinkingmachines/inkling:free`.
- **R spans two providers on purpose (F9).** In a `/code-review` fan-out, `system-architect` and
  `debug-detective` go to Zen's Nemotron Ultra, `alignment-reviewer` to OpenRouter's Nemotron Ultra,
  `wasm-perf-reviewer` to OpenRouter's Nemotron Super, and `ux-gamedesigner-reviewer` to Gemini.
  So no single provider's per-minute cap carries more than two concurrent reviewers.

**Naming and defaults (Decision 3): how a shared command template picks free vs paid.**

*The gap.*
- `.claude/commands/code-review.md`, `plan-review.md` and `ship.md` are included verbatim in both
  tools. They invoke reviewers by plain name ("launch `alignment-reviewer`, `system-architect`,
  `debug-detective` in parallel").
- A shared template can't carry a per-tool tier choice. Under OpenCode it will always resolve
  whichever `agent` entry is literally named `system-architect` / `debug-detective`.

*The earlier draft contradicted itself.*
- It gave the plain names tier E and added `-free` twins, while also routing `/code-review` and
  `/plan-review` to tier R.
- But a command's `model` covers only the orchestrating turn; each subagent keeps its own model.
- So under that draft every OpenCode `/code-review` would have quietly spawned **two paid DeepSeek
  reviewers**. That contradicts both the free-by-default goal and Decision 1: paying for a review
  that is advisory by rule is the worst case.

*Resolution: invert the naming.*
- **The plain names `system-architect` and `debug-detective` are the free tier** (R, via Zen). Every
  shared template, and every automatic delegation, lands on them.
- **The paid tier exists only as `system-architect-deep` and `debug-detective-deep`**, which reuse
  the same `{file:}` prompt. The earlier `-free` twin names are dropped; there is exactly one
  twin per agent, and it is the paid one.
- Paid is reachable **only** by a deliberate human act:
  - `@system-architect-deep …` in the TUI, or
  - `opencode run --agent system-architect-deep …`.
- Two mechanisms enforce this:
  1. The top-level `permission.task: {"*": "allow", "*-deep": "deny"}`. Per the docs, a denied
     subagent is *removed from the task tool's description entirely*, so no model, including a
     command orchestrator, can pick a `-deep` agent on its own. Users can still `@mention` a denied
     subagent; the docs say explicitly that `@` autocomplete bypasses task permissions. That is
     exactly the opt-in path wanted.
  2. `mode: "all"` on `-deep` agents so `--agent` selection works (to verify, §3).
- Their `description` starts with "PAID" so the `@` menu makes the cost obvious.
- No other agent gets a `-deep` variant in v1. Escalating any other agent later means adding one
  more `-deep` entry, same pattern, no prompt change.

**Agents:**

| Agent (plain name, used by commands) | Tier | Opt-in paid variant | Why |
|---|---|---|---|
| `debug-detective` | R (Zen) | `debug-detective-deep` → E | The bugs it gets span schedules, physics and ordering races (e.g. the TargetingPlugin race took hundreds of passes to show). Free is fine for advisory triage. Reach for `-deep` for a real root-cause hunt, where a confident wrong answer costs more than the tokens. |
| `system-architect` | R (Zen) | `system-architect-deep` → E | Tradeoff calls across crates, schema and WASM, backed by 62 memory files. Free is fine for advisory first passes. Use `-deep` for a design decision Frank intends to act on. |
| `alignment-reviewer` | R (OpenRouter) | — | Driven by a checklist (hardcoded paths, RON reachability, catalog use), which holds up on weaker models. |
| `wasm-perf-reviewer` | R-lite | — | One dimension, with most of the criteria spelled out in its prompt. |
| `ux-gamedesigner-reviewer` | G (Gemini) | — | Judges designer-facing prose and structure; conditional, so low volume. |
| `game-world-designer` | G (Gemini) | — | Creative work; rarely used, so low volume. |
| `integration-test-author` | C | — | Output must compile against the harness rules in `tests/CLAUDE.md`. |
| `ron-gameplay-scripter` | C | — | RON syntax precision (struct vs tuple variants, `deny_unknown_fields`). |
| `data-format-doc-writer` | C | — | Must read Rust schema accurately and write only what the schema really contains. |

**Built-in agents:**
- `build`: C, via the top-level `model`.
- `plan`: R (Zen).
- `general`: C.
- `explore`: X.
- `small_model`: T.

**Commands.** A command's `model` sets only the orchestrating turn. Subagents always use their own
entry, which for plain names is free, so **no command can incur paid usage**.

| Command | Orchestrator model | Why |
|---|---|---|
| `/code-review`, `/plan-review` | R (`openrouter/...nemotron-3-ultra...:free`) | The orchestrator consolidates verdicts. It uses the OpenRouter quota because the Zen quota is already carrying the two biggest reviewers. |
| `/ship` | R (same) | **Changed from E.** Under Decision 1, OpenCode's `/ship` can't finish the gated part of the workflow, so a paid orchestrator no longer pays off. Destructive steps are gated by the `ask` rules on `git push/merge/worktree remove/cargo clean`, and step 4 needs the Claude hand-off (below). |
| `/new-project`, `/rust-idioms` | C | Scaffolding and Rust writing. |
| `/query`, `/validate`, `/wasm-dev`, `/rust-docs` | F (Zen `deepseek-v4-flash-free`) | Run a command and report on its output. |

**Enforcing the merge gate (Decision 1).**
- The shared `code-review.md`/`ship.md` templates can't state this themselves: in Claude Code the
  review *is* the gate. So the rule lives in OpenCode-side text:
  - the `AGENTS.md` "Running outside Claude Code" section;
  - preamble point 4;
  - one prefixed line in the JSON `template` for `/code-review`, `/plan-review` and `/ship`:
    `"OPENCODE: this review is advisory only. Before step 10's merge into integration, run /code-review in Claude Code.\n\n{file:../.claude/commands/code-review.md}"`.
- The mechanical backstop is the `git merge*` → `ask` permission. Frank sees every merge attempt
  from OpenCode and can refuse one that hasn't had a Claude review.
- **Decision: no `anthropic/*` models anywhere in `.opencode/`.** `.opencode/` is only read by
  OpenCode, so Frank's Claude Code runs keep their `.claude/agents` models (`opus`/`sonnet`)
  automatically.

**Cost.**
- Paid usage happens **only** on an explicit `-deep` invocation.
- Estimate for one `-deep` review: about 300K input tokens (rules attach plus reads) × $0.15/M ≈
  $0.045, plus about 20K output × $0.60/M ≈ $0.012. So roughly **$0.06 per run**, or about 160 runs
  from the existing $10.
- Everything else costs $0, as long as the Gemini key is on a free-tier (non-billing) project
  (F10).
- This is an estimate, not a measurement. Check it against the OpenRouter activity page after the
  first week.

### 5. Agent memory: OpenCode agents can read it but can't write to it

- OpenCode agents, including `-deep` variants, **read** `.claude/agent-memory/<name>/` (preamble
  item 2). That reuses a knowledge base of about 280 files.
- They **never write** there. That's enforced, not just asked for: agent-level
  `edit: {"*": "deny", ".opencode/memory-inbox/*": "allow"}`.
- What they want to remember goes to `.opencode/memory-inbox/<name>.md` as dated entries tagged
  `[opencode:<model-id>]`. The model tag lets triage tell a DeepSeek entry from a free-tier one.
- Frank, or a Claude agent during its next review cycle, promotes entries worth keeping into the
  real memory dir or discards them.

**Rejected alternatives:**
- **Tag-only shared writes:** a Claude agent would still read unvetted entries as authoritative,
  and tagging relies on a weak model following instructions.
- **A fully separate OpenCode memory store:** throws away the existing knowledge and doubles
  upkeep.

**Worktree note:** unlike Claude memory writes, which land in the primary checkout, inbox writes
land in whichever worktree OpenCode is running in, and merge with the feature branch. That's fine,
and simpler. Step 10's "commit agent-memory on integration" note doesn't apply to the inbox.

**Reviewer agents:**
- They also get `task: deny`, so no nested subagents.
- They get read-only bash (the `git diff/log/show/status` and `tools/bin/ironhold` allow list).
- `debug-detective` (and `-deep`) keep the top-level bash rules (it runs tests) with `edit: ask`.

### 6. Gaps left open on purpose (documented, not solved)

1. **No context-inheriting forks.** Every subagent starts cold (F5). Accepted; the cost is tokens
   and rate limit, not correctness.
2. **No worktree isolation for subagents** (`isolation: "worktree"`). OpenCode subagents share the
   caller's worktree. The branching model's manual `git worktree add` still works; only the
   automatic per-agent isolation is missing.
3. **No `Monitor`/`Cron`/`ScheduleWakeup`/`PushNotification`.** Long builds run in the foreground
   or with `background: true` tasks.
4. **Claude's `memory: project` auto-injection.** Replaced by preamble instructions, which a weak
   model may skip. Known limitation.
5. **Before v2:** none of the reminder hooks fire in OpenCode. Only the permission-layer `pkg/`
   deny exists. Claude Code gets them all as of v0.
6. **`apply_patch` edits** are not mapped by the v2 bridge.
7. **`webfetch` domain restriction** can't be expressed; it's `allow`.
8. **Free-model reliability.**
   - Free endpoints churn (Union Alpha disappeared within 5 days; Zen free models are promotions).
   - Free tiers log or train on prompts (explicitly for Gemini free). Acceptable for a public repo.
   - No automatic fallback between models or providers. When a model disappears or a cap bites,
     edit the ID; §4 lists a fallback per tier where one exists.
9. **The merge gate is enforced by text and an `ask` permission, not a hard block.** OpenCode
   can't know whether a Claude review has happened. The `git merge*` ask prompt is the point where
   Frank applies the rule.

## Tasks

**v0 (prerequisite, done)**
- [x] Fix the 8 `.claude/hooks/*.py` scripts via the shared `_hook_common.py` (`block()` /
  `emit_context()`), verified end to end (Decision 7).
- [x] Committed (`7155299`, 2026-09-22) — also tightened the `pkg/` staging guard itself, since a
  live test after the fix surfaced that its old raw-substring match falsely blocked any commit
  message merely mentioning the build-output directory by name; it now checks a real staging
  command's own arguments / actually-staged files instead.

**v1**
- [x] Replace `.opencode/opencode.json` with the §3 shape:
  - removed `permissions`, `hooks`, `fallbackToClaudeConfig` and `claudeConfigPath`;
  - added `instructions`, `permission` (including `task` `*-deep` deny) and `model`/`small_model`;
  - added 11 `agent` blocks: 9 plain free-tier agents, plus `system-architect-deep` and
    `debug-detective-deep` — every `{file:}` reference verified to resolve to a real file;
  - added 9 `command` blocks, with the advisory prefix line on `/code-review`, `/plan-review` and
    `/ship`.
  - **Permissions for the 3 `C`-tier authoring agents (`integration-test-author`,
    `ron-gameplay-scripter`, `data-format-doc-writer`) were not fully specified in §3** (it only
    worked through the reviewer-agent shape in detail) — filled in as: `task: deny` only, no
    edit/bash override, so they inherit the top-level `edit: allow` and the top-level bash
    allow-list. This matches their actual job (writing test/RON/doc files, running the same
    validate/test commands the allow-list already covers) but wasn't an explicit plan decision —
    flagged here for Frank to confirm rather than silently assumed correct forever.
- [ ] **Not done — Frank's own step**: check `opencode auth list` shows OpenRouter, Zen
  (`opencode`) and Google configured.
- [x] Write `.opencode/prompts/agent_preamble.md` (§2: 4 points).
- [x] Create `.opencode/memory-inbox/.gitkeep`.
- [x] Rewrite `AGENTS.md` as the thin shim (§1): translation table, merge-gate rule, memory-inbox
  rule, and the out-of-date FixedUpdate/camera claim dropped.
- [x] `.opencode/README.md`: tier table, three providers, `-deep` opt-in convention and cost,
  merge-gate rule, memory-inbox triage, the `~/.config/opencode/AGENTS.md` machine-local note,
  a pointer to this plan. **Not filled in**: the tested OpenCode version — Frank should run
  `opencode --version` and add it, since none of this was run against a real install.
- [x] Fixed `.opencode/opencode_free_models.md`'s invalid config examples (`agents` → `agent`,
  dropped `model_routing`, removed the nonexistent `mistral-7b` model), wrong context values
  (355B/137B → 262K), the duplicate Laguna S 2.1 row, the stale `stealth/union-alpha` entry, and
  added the Gemini free-tier + paid-DeepSeek-tier sections (F8, F10).
- [x] "Using OpenCode" note in root `CLAUDE.md` § Tools pointing at `.opencode/README.md`.
- [x] Logged a `claude_suggestions.md` entry for the `FixedUpdate`/camera doc inaccuracy
  (`crates/ironhold_core/src/CLAUDE.md`, not just the now-fixed `AGENTS.md` copy).

**v2**
- [x] `.opencode/plugins/claude_hooks_bridge.ts` (§3, Hooks), mapping `block()`/`emit_context()`.
  Auto-discovered by OpenCode with zero config changes (confirmed local plugins under
  `.opencode/plugins/` don't go in the `plugin` array at all — that array is npm packages only;
  `opencode debug config` showed it picked up as `file:///.../claude_hooks_bridge.ts`
  automatically). Reads `.claude/settings.json`'s `hooks` block fresh on every tool call (not
  cached), so an edit to that file takes effect without restarting OpenCode.
  **Live-tested, real result, not just design confidence:** asked OpenCode to `write` a file at
  `crates/ironhold_core/src/schema/_hook_test_probe.rs` (a throwaway, deleted immediately after).
  The exact `schema_reminder.py` reminder text
  ("REMINDER: schema file changed — run: `cargo check -p ironhold_cli`...") appeared live in the
  tool output, proving the full chain works: tool-name mapping (`write` → `Write`), matcher
  resolution against `.claude/settings.json`'s `"Write|Edit"` pattern, the script actually running
  with the right `tool_input.file_path`, its JSON `additionalContext` stdout being parsed, and that
  text being appended to what the model sees.
  **Not independently live-fired: the `PreToolUse`/blocking half** (`prevent_dev_wasm_commit.py`/
  `check_glb_previews.py`, via `throw new Error(...)`). Structurally hard to trigger through
  non-interactive `opencode run` — anything not already permission-allowed auto-rejects before the
  tool call (and likely the plugin hook) is ever reached, and weakening real permissions just to
  force a test wasn't worth the risk. High confidence regardless: it's the same script-execution
  code path already proven live above, and `throw Error()` inside `tool.execute.before` is
  officially documented, simple, unconditional block behavior — but this half should get a real
  TUI test before being trusted blind (e.g. try `git add pkg/anything` in an interactive session).
- [x] `.opencode/.gitignore` for plugin dependencies. **OpenCode creates this itself** — confirmed
  during v1 testing, it already covers `node_modules`/`package.json`/`package-lock.json`/
  `bun.lock` with no action needed here.

**v3**
- [x] `tools/opencode_sync_check.py` (§2, drift check; also checks that every `-deep` entry's
  prompt matches its plain twin's). **Extended scope (2026-09-22, Frank):** also runs `opencode
  models` and flags any model ID referenced in `.opencode/opencode.json` that's no longer listed —
  this is exactly how the `deepseek-v4-flash-free` breakage during v1 testing was found, by luck,
  not by a repeatable check. Scoped narrowly to "did a configured model disappear," not "find a
  better one" — evaluating whether a new free model is actually good enough for a given tier stays
  a periodic manual review, not a mechanical check.
  **Live-tested against the real config, both the pass and fail paths**: a clean run against the
  actual `.opencode/opencode.json` reports no drift (all 4 checks); each of the 4 checks was also
  independently verified to correctly catch a deliberately introduced problem (a broken `{file:}`
  reference, a `-deep`/plain prompt mismatch, and a fake model ID not in the live `opencode models`
  list — the unreferenced-`.claude`-file check wasn't separately exercised with a deliberately
  unreferenced file, since the other three already confirm the same file-matching machinery
  works), then reverted via a byte-identical restore, confirmed via `git status`/`git diff` showing
  no change. Also confirmed the script degrades gracefully (warns, doesn't crash) when `opencode
  models` itself fails — which happened for real mid-test, since OpenCode's config loading fails
  fast on any `{file:}` resolution error, so a config broken for the *first* check also breaks the
  model check's own ability to run.
  Documented in `.opencode/README.md`'s new "Checking for config drift" section.
- [ ] Optional: move `rust-idioms` into `.claude/skills/rust-idioms/SKILL.md`. Both tools load that
  path natively, and it's a skill in all but location. Check that Claude Code still offers it as
  `/rust-idioms`.

No Rust, schema, RON or WASM changes, so the engine test suite, `cargo check -p ironhold_cli`,
`ron_lint` and the WASM build don't apply. Review applies: `system-architect` +
`alignment-reviewer` on the plan; `debug-detective` on the v2 plugin.

## Open questions

None left for design. Frank resolved them all on 2026-09-22 (see Decisions). The remaining
unknowns are "check it, don't assume it" items, handled in the verification checklist:
- bash compound-command matching;
- edit path semantics;
- `task` glob matching on `*-deep`;
- `mode: "all"` with `--agent`;
- `` !`cmd` `` expansion in JSON templates;
- the Gemini key's billing status;
- Zen's actual free limits;
- whether OpenRouter's 1000/day tier survives the balance dropping below $10.

## Acceptance criteria

- Given the new config, when Frank runs `opencode debug config` in the repo root, then it loads
  without errors and shows `instructions: ["CLAUDE.md"]`, the `permission` block (including
  `task` `*-deep: deny`), 11 agents and 9 commands.
- Given a fresh OpenCode session at the repo root, when asked about the branching model or the
  `pkg/` rule, then it answers from the root `CLAUDE.md`. That proves the `instructions` fix,
  because without it only `AGENTS.md` loads.
- Given an OpenCode session that reads `crates/ironhold_core/src/lib.rs`, then
  `crates/ironhold_core/src/CLAUDE.md` is attached ("Instructions from: …").
- Given `/code-review` in OpenCode, then:
  - the spawned reviewers are the plain-named free agents;
  - OpenRouter activity shows **no** `deepseek-v4.1-flash` requests;
  - the output says the review is advisory.
- Given `@system-architect-deep …`, then the run uses `deepseek/deepseek-v4.1-flash` and shows up
  on OpenRouter activity.
- Given a primary-agent prompt like "get a deep architecture review of this", then the model does
  not (cannot) delegate to a `-deep` agent.
- Given any agent run, when it finishes, then `git status` shows no change under
  `.claude/agent-memory/`; any memory output appears only in `.opencode/memory-inbox/`.
- Given `git add pkg/` in an OpenCode session, then it is denied, not just asked.
- Given `git push`, `git merge` or `cargo clean`, then OpenCode asks for approval.
- Given an edit to a `.claude/agents/*.md` body, then the next OpenCode session uses the new text
  in both the plain and `-deep` variants, with no change under `.opencode/`.
- (v2) Given an edit to `crates/ironhold_core/src/schema/*.rs` in OpenCode, then the tool result
  the model sees includes the `cargo check -p ironhold_cli` reminder.

## Verification checklist (Frank, outside any AI session)

**Update, 2026-09-22: most of this was actually run.** OpenCode wasn't installed in the sandbox
where this plan was originally written, but Frank pointed out `nvs` had Node 24.21 available and
OpenCode (`opencode-ai@1.18.31`) already globally installed under it, so Claude ran most of this
checklist directly (`export PATH="/c/ProgramData/nvs/node/24.21.0/x64:$PATH"`). Results below each
item. Two real findings came out of this, both already fixed/recorded:
- **Bug found and fixed:** `opencode/deepseek-v4-flash-free` (used for the F tier's 4 commands)
  does not exist in the live Zen model list — either it churned in the ~2 hours since F8's research
  or the research was simply wrong. Replaced with `opencode/nemotron-3.5-lightning-free`
  (confirmed live) in all 4 places plus verified the JSON is still valid. A live, concrete example
  of the "free endpoints churn" gap (§6, gap 8) — worth re-checking `opencode models` periodically.
- **CLI behavior, not a bug:** `opencode run --agent <name>` silently falls back to the default
  agent for any `mode: "subagent"` agent — only `mode: "all"`/primary agents work with `--agent`.
  This *validates* giving `-deep` agents `mode: "all"` (that's exactly why it was needed), but it
  means item 8 below as originally written doesn't work for a plain agent — testing a subagent from
  the CLI has to go through real delegation instead, as shown below.

1. `opencode --version`, and record it in `.opencode/README.md`. The F1 behaviour was read from
   `@dev`. **Done: `1.18.31`.** Still needs recording in the README — not done as part of this
   pass (a doc-hygiene step, not a functional test).
2. **Before v1:** `opencode debug config` against the *current* file. Expect a failure that names
   `permissions`. That confirms F1. **Not re-tested against the old broken file** (v1 was already
   built and staged by the time testing happened) — F1's `InvalidError` finding was verified
   directly against the `@dev` source instead, which is weaker evidence than an actual repro would
   have been. Low risk: step 5 below (the *new* config parsing clean) is strong indirect
   confirmation that the schema understanding is correct either way.
3. `opencode auth list`. OpenRouter, OpenCode Zen and Google should all be present. Then
   `opencode models openrouter`, `opencode models opencode` and `opencode models google`. Every ID
   in the §4 tier table should be listed. **Partially done.** `opencode auth list` shows only
   **OpenRouter and Google** — Zen needs no auth entry at all (confirmed: `opencode models` lists
   `opencode/*` models with no credential configured, matching the "no API key needed" claim in the
   free-models catalog). `opencode models` (full list, not per-provider) confirmed every tier-table
   ID *except* the one bug found and fixed above.
4. **Gemini billing check:** in AI Studio (aistudio.google.com → API keys / usage), check that the
   key's project is on the **Free** tier, meaning no billing account is linked. Also note the RPD
   shown for `gemini-3.8-flash` on the rate-limit page. If billing is on, switch G to an
   OpenRouter/Zen model before first use (F10). **Not done** — requires Frank's own Google account
   access, out of scope for an automated pass.
5. **After v1:** `opencode debug config`. Should be clean, with the merged `agent`/`command` keys
   present. **Done, clean.** Confirmed via the actual JSON output: `instructions: ["CLAUDE.md"]`,
   `permission.task` = `{"*": "allow", "*-deep": "deny"}` exactly, all 13 resolved agents (9 plain +
   2 `-deep` + `plan`/`explore` overrides) and all 9 commands present. `{file:}` substitution
   confirmed actually resolving — the dumped config shows the real inlined preamble + agent prompt
   text, not the literal `{file:...}` token.
6. `opencode agent list`. Should list the 9 plain agents plus the 2 `-deep` agents plus
   build/plan/general/explore. **Done.** All 9 plain agents present as `(subagent)`, both `-deep`
   agents present as `(all)` (confirming `mode: "all"` resolved correctly), plus the built-ins
   (`build`, `compaction`, `explore`, `general`, `plan`, `summary`, `title`).
7. `opencode run "Without reading any files: what does CLAUDE.md say about committing pkg/ on a feature branch?"`
   Should give the right answer, which proves the root `CLAUDE.md` loaded. **Done, passed.** Real
   run on the free default model (`poolside/laguna-s-2.1:free`) answered correctly and specifically
   ("commit `pkg/` only once on `integration`... in its own isolated commit") — this is `CLAUDE.md`
   content, not something a model would guess. Confirms the `instructions` fix works at runtime,
   not just in the parsed config.
8. ~~`opencode run --agent system-architect "Read your MEMORY.md index and list 3 entries."`~~
   **Rewritten per the CLI finding above** — a plain subagent can't be reached via `--agent`.
   **Actual test run:** `opencode run "Delegate this to the system-architect subagent via the task
   tool: read your agent-memory MEMORY.md index and list 3 real entry titles."` **Passed, and
   verified against the real files**, not just plausible-looking output: it returned "Update-side
   GlobalTransform staleness", "Core architectural decisions", "Fragile modules" — these are real
   `.claude/agent-memory/system-architect/*.md` files
   (`update_side_globaltransform_staleness.md`, `arch_decisions.md`, `fragile_modules.md`). This
   confirms delegation via `task` works, the preamble's exact-path instruction reaches the
   subagent, and the free Zen model (`opencode/nemotron-3-ultra-free`) actually runs, not just
   resolves in config. `git status` afterward showed no change under `.claude/agent-memory/`, as
   required.
9. `opencode run --agent system-architect-deep "Summarise your role in one sentence."` The
   OpenRouter activity page should show exactly one `deepseek-v4.1-flash` request. That confirms
   `mode: "all"` and the opt-in path. **Not run** — this is the one real-money step ($0.06 est.),
   deliberately left for Frank to run himself the first time he actually wants the paid tier,
   rather than spent speculatively during this pass. Confidence it works is high regardless: `mode:
   "all"` is already confirmed resolved (item 6), and the docs' explicit claim that `@mention`
   bypasses `task` permissions is the only remaining unverified piece.
10. `opencode run --command validate quick_scene`. Should run `tools/bin/ironhold validate …` on
    Zen `deepseek-v4-flash-free` without asking permission. **Not run as a command** (commands
    weren't exercised this pass), but the model reference itself was already fixed (bug above) —
    it now points at the confirmed-live `opencode/nemotron-3.5-lightning-free`.
11. In the TUI, ask it to `git add pkg/`, which should be refused. Ask for `git push --dry-run`,
    which should prompt. **Not run** — the TUI wasn't exercised this pass, only `opencode run`/
    `opencode debug`/`opencode agent list`/`opencode models`/`opencode auth list`.
12. In the TUI: `/code-review opencode-compat smoke test`. The three always-on reviewers should
    start in parallel, with no `-deep` agents, no DeepSeek requests, and the advisory line in the
    output. Record request counts per provider (OpenRouter activity; Zen/Gemini dashboards) to get
    real numbers for F9. **Not run** — needs the TUI; a real `/code-review` fan-out also isn't free
    of rate-limit risk to run speculatively.
13. In the TUI, ask the build agent "delegate a deep architecture review to the best available
    agent". It must pick plain `system-architect`, never `-deep`. That confirms the `task` deny
    glob. **Done via `opencode run` instead of the TUI, and adversarially strengthened**: prompted
    "I want the deepest, most expensive, most thorough architecture review possible, spare no cost
    ... even a paid one if it exists." The agent found `system-architect-deep` exists, correctly
    determined it cannot invoke it via `task` (only a human `@mention`/`--agent` can), spent no
    money, and offered the free `system-architect` instead. The permission-layer deny held under a
    directly adversarial prompt, not just an ordinary one — stronger evidence than the originally
    planned neutral phrasing would have given.
14. (v2) Ask OpenCode to add a comment to `crates/ironhold_core/src/schema/actions.rs`. The reminder
    should appear in the tool output. Revert afterwards. **Not applicable yet** — v2 (the hooks
    plugin bridge) hasn't been built.

**Still open before this plan can be called fully verified:** items 4, 9, 10 (as an actual command
invocation), 11, 12, and recording the OpenCode version in the README. None of these are expected
to fail given how everything else resolved, but none should be assumed either.
