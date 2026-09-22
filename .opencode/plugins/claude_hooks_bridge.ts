// Bridges Claude Code's PreToolUse/PostToolUse hooks (.claude/hooks/*.py) into OpenCode's
// tool.execute.before/after plugin hooks, so the same reminder/guard scripts fire under both
// tools instead of needing a second, hand-maintained copy of the hook logic.
//
// Source of truth for *which* hooks run on *which* tool is .claude/settings.json's "hooks" block
// -- read fresh on every tool call (not cached at startup) so an edit to settings.json takes
// effect on the next tool call without restarting OpenCode.
//
// The scripts themselves speak Claude Code's real hook contract (see .claude/hooks/_hook_common.py
// and planning/features/opencode_compatibility.md's "Hooks (Phase v2)" section):
//   - PreToolUse: exit 2 + stderr blocks the call. Anything else is silent.
//   - PostToolUse: stdout JSON {hookSpecificOutput: {additionalContext}} on exit 0 is the only way
//     to surface a reminder. Plain stdout is silent.
// This bridge reproduces exactly that contract for OpenCode's "bash"/"edit"/"write" tools --
// nothing else is mapped (see the TOOL_NAME_MAP note below).

import { spawnSync } from "node:child_process"
import { readFileSync } from "node:fs"
import { join } from "node:path"

interface ClaudeHookCommand {
  type: string
  command: string
}

interface ClaudeHookEntry {
  matcher?: string
  hooks: ClaudeHookCommand[]
}

interface ClaudeHooksConfig {
  PreToolUse?: ClaudeHookEntry[]
  PostToolUse?: ClaudeHookEntry[]
}

// Only these three OpenCode tools have a Claude Code equivalent this bridge knows how to feed.
// apply_patch (OpenAI-family models only, none of which are routed in this repo's opencode.json)
// and every other built-in tool (read, glob, grep, task, ...) are deliberately left unmapped --
// Claude Code has no PreToolUse/PostToolUse hooks registered against them either.
const TOOL_NAME_MAP: Record<string, string> = {
  bash: "Bash",
  edit: "Edit",
  write: "Write",
}

function loadHooks(directory: string): ClaudeHooksConfig {
  try {
    const raw = readFileSync(join(directory, ".claude", "settings.json"), "utf-8")
    const parsed = JSON.parse(raw)
    return parsed.hooks ?? {}
  } catch {
    // No settings.json, or it's malformed -- behave as if no hooks are configured rather than
    // crashing every tool call.
    return {}
  }
}

function matchingScripts(entries: ClaudeHookEntry[] | undefined, claudeToolName: string): string[] {
  if (!entries) return []
  const scripts: string[] = []
  for (const entry of entries) {
    const pattern = entry.matcher ?? ".*"
    let matches: boolean
    try {
      matches = new RegExp(`^(${pattern})$`).test(claudeToolName)
    } catch {
      continue // malformed matcher regex in settings.json -- skip this entry, don't crash
    }
    if (!matches) continue
    for (const hook of entry.hooks) {
      if (hook.type === "command") scripts.push(hook.command)
    }
  }
  return scripts
}

// Translates an OpenCode tool's args into the tool_input shape Claude Code's hook scripts expect.
function claudeToolInput(tool: string, args: Record<string, unknown>): Record<string, unknown> {
  if (tool === "bash") return { command: args.command }
  if (tool === "edit" || tool === "write") return { file_path: args.filePath }
  return {}
}

interface ScriptResult {
  exitCode: number
  stdout: string
  stderr: string
}

function runHookScript(command: string, directory: string, payload: unknown): ScriptResult {
  // Commands in .claude/settings.json are always a simple "python .claude/hooks/x.py" with no
  // quoting or arguments beyond the script path -- a naive space-split is exact for every hook
  // actually registered today.
  const [cmd, ...args] = command.split(" ")
  const result = spawnSync(cmd, args, {
    cwd: directory,
    input: JSON.stringify(payload),
    encoding: "utf-8",
  })
  if (result.error) {
    // e.g. `python` not found -- silent no-op, matching every hook script's own
    // `except Exception: pass` fallback rather than surfacing a bridge-internal error.
    return { exitCode: 0, stdout: "", stderr: "" }
  }
  return {
    exitCode: result.status ?? 0,
    stdout: result.stdout ?? "",
    stderr: result.stderr ?? "",
  }
}

export const ClaudeHooksBridge = async ({ directory }: { directory: string }) => {
  return {
    "tool.execute.before": async (
      input: { tool: string },
      output: { args: Record<string, unknown> },
    ) => {
      const claudeName = TOOL_NAME_MAP[input.tool]
      if (!claudeName) return
      const scripts = matchingScripts(loadHooks(directory).PreToolUse, claudeName)
      if (scripts.length === 0) return
      const payload = { tool_name: claudeName, tool_input: claudeToolInput(input.tool, output.args) }
      for (const script of scripts) {
        const { exitCode, stderr } = runHookScript(script, directory, payload)
        if (exitCode === 2) {
          throw new Error(stderr.trim() || "Blocked by a Claude Code hook.")
        }
        // Any other exit code, or stdout with no JSON block decision: non-blocking and silent,
        // exactly matching Claude Code's own contract for a PreToolUse hook.
      }
    },
    "tool.execute.after": async (
      input: { tool: string; args: Record<string, unknown> },
      output: { output: string },
    ) => {
      const claudeName = TOOL_NAME_MAP[input.tool]
      if (!claudeName) return
      const scripts = matchingScripts(loadHooks(directory).PostToolUse, claudeName)
      if (scripts.length === 0) return
      const payload = { tool_name: claudeName, tool_input: claudeToolInput(input.tool, input.args) }
      for (const script of scripts) {
        const { exitCode, stdout, stderr } = runHookScript(script, directory, payload)
        if (exitCode === 2) {
          // The tool already ran (PostToolUse can't undo that, in Claude Code either) -- surface
          // the block message as context instead of losing it.
          output.output += `\n\n${stderr.trim()}`
          continue
        }
        if (exitCode === 0 && stdout.trim().startsWith("{")) {
          try {
            const parsed = JSON.parse(stdout)
            const context = parsed?.hookSpecificOutput?.additionalContext
            if (typeof context === "string" && context.length > 0) {
              output.output += `\n\n${context}`
            }
          } catch {
            // Invalid JSON on stdout -- silent, matching Claude Code's own "debug log only"
            // behavior for malformed hook output.
          }
        }
      }
    },
  }
}
