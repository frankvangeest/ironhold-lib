#!/usr/bin/env python3
"""Runner and report for the OpenCode GLM pilot (planning/features/done/opencode_glm_driver_pilot.md).

It spends money, so nothing runs without `--go`. Without it the script only prints the plan (arms, tasks, run counts, a cost
estimate). Safety rules from the plan, all enforced here:
  - every run uses a throwaway **detached worktree** reset to one pinned base commit (never the primary checkout), sequentially;
  - **unattended permissions**: `OPENCODE_CONFIG_CONTENT` sets bash to deny-by-default (the repo's allow-list stays) and keeps
    `*-deep` denied, because headless `opencode run` auto-approves `ask` (fact V20);
  - a **script cap** (default $2) on the summed `step_finish` cost of the paid arms; it stops before the next run. The OpenRouter key's
    own spend limit is the real backstop, and OpenCode's cost figure probably leaves out sub-agent sessions, so reconcile the totals
    against the OpenRouter activity page afterwards. The m365 arm costs nothing and sits outside the cap;
  - `opencode run` gets `--dir` and `PWD` set to the throwaway worktree (it ignores the process cwd, fact V21), and uses the real
    OpenCode data dir, because a temporary one hides the OpenRouter credentials (fact V22); each run's sessions are deleted by id afterwards;
  - the M365 proxy is only touched through `GET /v1/models` (preflight); request bodies are never logged by this script.

Usage:
    python tools/opencode_pilot.py                      # print the plan and the cost estimate, run nothing
    python tools/opencode_pilot.py --go --arms glm      # run one arm (after `nvs use 24.21`)
    python tools/opencode_pilot.py --report DIR         # evaluate DIR/results.jsonl against the pre-registered thresholds
    python tools/opencode_pilot.py --rescore DIR        # re-score DIR's saved events with the current scorers (results_rescored.jsonl)
    python tools/opencode_pilot.py --selftest           # fixture tests, no OpenCode, no money
    python tools/opencode_pilot.py --verify-tasks       # scorers against a real throwaway worktree (git only, no model, no money)
Exit codes: 0 ok, 1 a check failed (selftest, or thresholds not met in --report), 2 tool error.
"""

from __future__ import annotations

import argparse
import datetime
import json
import os
import re
import shutil
import statistics
import subprocess
import sys
import tempfile
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from _opencode_common import find_opencode, model_class, not_found_message, opencode_env, opencode_version  # noqa: E402
import opencode_pilot_tasks as T  # noqa: E402

TOOL = "opencode_pilot"
REPO_ROOT = Path(__file__).resolve().parent.parent
PROXY_URL = "http://localhost:4141/v1/models"
# The pilot measured GLM, so a re-run must start from the config as it was BEFORE GLM and the driver prompt were adopted: the repo config now has
# `agent.build.prompt` and OPENCODE_CONFIG_CONTENT can only add to it. `--base` can override, but never with a config that has the driver prompt
# unless every selected arm is meant to use it (checked in main).
PRE_ADOPTION_BASE = "5db115f24c"
INVALID_MODEL = "openrouter/invalid-lab/does-not-exist:free"

# arm id -> model, whether the driver prompt is wired into `build`, whether it counts against the cap, runs per task group
ARMS = {
    "glm":        {"model": "openrouter/z-ai/glm-5.3-flash", "driver": False, "paid": True, "core": 3, "toolcall": 5, "routing": 3},
    "glm_driver": {"model": "openrouter/z-ai/glm-5.3-flash", "driver": True, "paid": True, "core": 0, "toolcall": 5, "routing": 0},
    "free":       {"model": "openrouter/poolside/laguna-s-2.1:free", "driver": False, "paid": False, "core": 3, "toolcall": 15, "routing": 3},
    # opt-in extra reference (all counts 0 by default; use --arms free_alt with --core-runs/--toolcall-runs/--routing-runs): another free lab
    "free_alt":   {"model": "opencode/nemotron-3-ultra-free", "driver": False, "paid": False, "core": 0, "toolcall": 0, "routing": 0},
    "deepseek":   {"model": "openrouter/deepseek/deepseek-v4.1-flash", "driver": False, "paid": True, "core": 3, "toolcall": 5, "routing": 0},
    "m365":       {"model": "m365/gpt-5.5-think-deeper", "driver": False, "paid": False, "core": 3, "toolcall": 15, "routing": 3},
    # opt-in m365 candidates that became available later (2026-10-09, listed in the global OpenCode config): counts 0 by default
    "m365_gpt56": {"model": "m365/gpt-5.6-think-deeper", "driver": False, "paid": False, "core": 0, "toolcall": 0, "routing": 0},
    "m365_claude": {"model": "m365/claude-sonnet-think-deeper", "driver": False, "paid": False, "core": 0, "toolcall": 0, "routing": 0},
    # screening candidates (2026-10-09), ranked by the proxy README's own tool-calling notes; run with --toolcall-runs 5 first
    "m365_gpt6":    {"model": "m365/gpt-6-think-deeper", "driver": False, "paid": False, "core": 0, "toolcall": 0, "routing": 0},
    "m365_opus":    {"model": "m365/claude-opus-5", "driver": False, "paid": False, "core": 0, "toolcall": 0, "routing": 0},
    "m365_sonnet":  {"model": "m365/claude-sonnet", "driver": False, "paid": False, "core": 0, "toolcall": 0, "routing": 0},
    "m365_gpt55":   {"model": "m365/gpt-5.5", "driver": False, "paid": False, "core": 0, "toolcall": 0, "routing": 0},
    "m365_gpt55q":  {"model": "m365/gpt-5.5-quick", "driver": False, "paid": False, "core": 0, "toolcall": 0, "routing": 0},
}
for _arm in ARMS.values():  # one source of truth for free vs paid (tools/_opencode_common.py)
    _arm["paid"] = model_class(_arm["model"]) == "paid"
ARM_ORDER = ["glm", "glm_driver", "free", "free_alt", "deepseek", "m365", "m365_gpt56", "m365_claude", "m365_gpt6", "m365_opus", "m365_sonnet", "m365_gpt55", "m365_gpt55q"]
DEFAULT_ARMS = [a for a in ARM_ORDER if any(ARMS[a][k] for k in ("core", "toolcall", "routing"))]
# rough cost per run in USD, for the plan printout only (GLM: ~$0.005 per trivial turn at a 31.8k-token baseline; DeepSeek ~$0.06 per review run)
EST_COST = {"glm": 0.03, "glm_driver": 0.03, "deepseek": 0.06, "free": 0.0, "free_alt": 0.0, "m365": 0.0, "m365_gpt56": 0.0, "m365_claude": 0.0, "m365_gpt6": 0.0, "m365_opus": 0.0, "m365_sonnet": 0.0, "m365_gpt55": 0.0, "m365_gpt55q": 0.0}


class PilotError(Exception):
    pass


# ----------------------------------------------------------------------------------------------------------------- plan
def plan_runs(arm_ids, task_ids, core_runs=None, toolcall_runs=None, routing_runs=None) -> list[tuple[str, str, int]]:
    """(arm, task, run index) in execution order: arms in ARM_ORDER, tasks in TASKS order, runs 1..n."""
    runs = []
    for arm_id in ARM_ORDER:
        if arm_id not in arm_ids:
            continue
        arm = ARMS[arm_id]
        for task in T.TASKS:
            if task.id not in task_ids:
                continue
            group = "core" if task.core else task.id
            n = {"core": core_runs, "toolcall": toolcall_runs, "routing": routing_runs}[group]
            n = arm[group] if n is None else n
            runs.extend((arm_id, task.id, i) for i in range(1, n + 1))
    return runs


def estimate(runs) -> dict:
    by_arm: dict = {}
    for arm_id, _, _ in runs:
        by_arm.setdefault(arm_id, [0, 0.0])
        by_arm[arm_id][0] += 1
        by_arm[arm_id][1] += EST_COST[arm_id]
    return by_arm


# ----------------------------------------------------------------------------------------------------------------- config
def harden(node):
    """Rewrite every `ask` leaf of a permission tree to `deny` (a headless run auto-approves `ask`, fact V20), keeping the key order
    (OPENCODE_CONFIG_CONTENT merges in place, and the last matching rule wins)."""
    if isinstance(node, dict):
        return {k: harden(v) for k, v in node.items()}
    return "deny" if node == "ask" else node


def build_config(task: T.Task, arm: dict, driver_prompt_path: Path, repo_permission: dict = None) -> dict:
    """OPENCODE_CONFIG_CONTENT for one run (it wins over the project config and merges into it).

    Deny-by-default really means every `ask` of the repo's own permission block becomes `deny` (`git push*`, `cargo clean*`,
    `git worktree remove*`, external directories, ...), and cargo and wasm-pack are denied too: the shared target dir must never see two
    cargo runs, and the pilot's tasks need none."""
    base_perm = harden(repo_permission or {})
    bash = dict(base_perm.get("bash") or {})
    bash["*"] = "deny"
    for key in list(bash):
        if key.startswith(("cargo", "wasm-pack")):
            bash[key] = "deny"
    for pattern in task.bash_deny:
        bash[pattern] = "deny"
    perm = {k: v for k, v in base_perm.items() if k not in ("bash", "task")}
    ext = perm.get("external_directory")
    perm["external_directory"] = dict(ext, **{"*": "deny"}) if isinstance(ext, dict) else "deny"
    cfg: dict = {"permission": {**perm, "bash": bash, "task": {"*": "allow", "*-deep": "deny"}}}
    if arm["driver"] or task.needs_driver_prompt:
        cfg["agent"] = {"build": {"prompt": "{file:%s}" % driver_prompt_path.as_posix()}}  # absolute: relative file refs resolve against the cwd here
    if task.break_agent:
        cfg.setdefault("agent", {})[task.break_agent] = {"model": INVALID_MODEL}
    return cfg


# ----------------------------------------------------------------------------------------------------------------- worktree
def run_git(args, cwd=None) -> str:
    proc = subprocess.run(["git", *args], cwd=cwd or REPO_ROOT, capture_output=True, text=True, encoding="utf-8")
    if proc.returncode != 0:
        raise PilotError(f"git {' '.join(args)} failed: {proc.stderr.strip()[:300]}")
    return proc.stdout.strip()


def create_worktree(path: Path, base: str) -> None:
    if path.exists():
        raise PilotError(f"{path} already exists; remove it (git worktree remove) or pass another --worktree")
    run_git(["worktree", "add", "--detach", str(path), base])


def remove_worktree(path: Path) -> None:
    gone = subprocess.run(["git", "worktree", "remove", "--force", str(path)], cwd=REPO_ROOT, capture_output=True, text=True)
    subprocess.run(["git", "worktree", "prune"], cwd=REPO_ROOT, capture_output=True, text=True)
    if gone.returncode != 0 and path.exists():
        print(f"warning: could not remove {path} ({gone.stderr.strip()[:200]}); remove it by hand before the next run", file=sys.stderr)


def reset_worktree(path: Path, base: str) -> None:
    run_git(["reset", "--hard", base], cwd=path)
    run_git(["clean", "-fd"], cwd=path)


def primary_root() -> Path:
    """The primary checkout (the gitignored tools/bin cache lives there, not in a feature worktree)."""
    return Path(run_git(["rev-parse", "--path-format=absolute", "--git-common-dir"])).parent


def copy_cli_binary(path: Path) -> None:
    """The ironhold binary is gitignored (tools/bin/); the `ron_edit` task needs it."""
    name = "ironhold.exe" if sys.platform == "win32" else "ironhold"
    src = primary_root() / "tools" / "bin" / name
    if src.is_file():
        (path / "tools" / "bin").mkdir(parents=True, exist_ok=True)
        dst = path / "tools" / "bin" / name
        if not (dst.is_file() and dst.stat().st_size == src.stat().st_size):  # `git clean` keeps ignored files; a just-run binary can still be locked
            shutil.copy2(src, dst)


# ----------------------------------------------------------------------------------------------------------------- one run
def run_one(binary: str, wt: Path, base: str, arm_id: str, task: T.Task, run: int, variant, out: Path, keep_sessions: bool = False) -> dict:
    arm = ARMS[arm_id]
    reset_worktree(wt, base)
    copy_cli_binary(wt)
    ctx = T.make_ctx(task.id, run)
    task.setup(wt, ctx)
    # The real OpenCode data dir is used on purpose: a temporary XDG_DATA_HOME hides auth.json, so every `openrouter/*` model would
    # fail (fact V22). The sessions this run creates are deleted afterwards by their own ids (unless --keep-sessions).
    env = opencode_env(wt)  # opencode takes its working directory from PWD / --dir, not from the process cwd (fact V21)
    repo_cfg = json.loads(T.read_text(wt / ".opencode" / "opencode.json") or "{}")
    driver_md = wt / ".opencode" / "prompts" / "driver.md"
    if (arm["driver"] or task.needs_driver_prompt) and not driver_md.is_file():
        raise PilotError(f"{driver_md} does not exist at the base commit; use a base that has the driver prompt for driver arms and the routing task")
    env["OPENCODE_CONFIG_CONTENT"] = json.dumps(build_config(task, arm, driver_md, repo_cfg.get("permission")))
    cmd = [binary, "run", "--format", "json", "--dir", str(wt), "-m", arm["model"], "--title", f"opencode-pilot {arm_id} {task.id} {run}"]
    if variant:
        cmd += ["--variant", variant]
    cmd.append(task.prompt(ctx))
    started = time.time()
    status, raw = "ok", ""
    flags = subprocess.CREATE_NEW_PROCESS_GROUP if sys.platform == "win32" else 0
    child = subprocess.Popen(cmd, cwd=wt, env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, encoding="utf-8",
                             stdin=subprocess.DEVNULL, creationflags=flags)
    try:
        raw, _ = child.communicate(timeout=task.timeout)
        if child.returncode != 0:
            status = f"exit {child.returncode}"
    except subprocess.TimeoutExpired:
        status = "timeout"
        if sys.platform == "win32":  # kill the whole tree: bash/node children would otherwise hold the pipe and lock worktree files
            subprocess.run(["taskkill", "/T", "/F", "/PID", str(child.pid)], capture_output=True)
        else:
            child.kill()
        try:
            raw, _ = child.communicate(timeout=30)
        except subprocess.TimeoutExpired:
            raw = ""
    wall = time.time() - started
    ev = T.parse_events(raw)
    if not keep_sessions:
        for sid in ev["sessions"]:
            gone = subprocess.run([binary, "session", "delete", sid], cwd=wt, env=env, capture_output=True, text=True, encoding="utf-8")
            if gone.returncode != 0:
                print(f"warning: could not delete session {sid}; sub-agent sessions are never deleted, look for 'opencode-pilot' titles", file=sys.stderr)
    try:
        criteria = task.score(wt, ev, ctx)
    except Exception as exc:  # a scorer bug must not kill a paid sweep: record it and carry on
        criteria = {"scorer_error": False}
        status += f" scorer:{type(exc).__name__}"
    (out / "events").mkdir(exist_ok=True)
    (out / "events" / f"{arm_id}_{task.id}_{run}.jsonl").write_text(raw, encoding="utf-8")
    return {"arm": arm_id, "task": task.id, "run": run, "model": arm["model"], "status": status, "pass": all(criteria.values()) and status.split()[0] == "ok",
            "criteria": criteria, "wall_s": round(wall, 1), "cost": round(ev["cost"], 5), "tokens_in": ev["tokens_in"], "calls": len(ev["calls"]),
            "wellformed_ratio": round(T.wellformed_ratio(ev), 3), "malformed_calls": sum(1 for c in ev["calls"] if T.is_malformed(c)),
            "total_calls": len(ev["calls"]), "errors": [e[:300] for e in ev["errors"][:3]], "driver_prompt": arm["driver"] or task.needs_driver_prompt, "base": base,
            "at": datetime.datetime.now(datetime.timezone.utc).isoformat(timespec="seconds")}


# ----------------------------------------------------------------------------------------------------------------- report
def summarise(rows: list[dict]) -> dict:
    """Per arm: core pass rate, cost per core task (mean, worst), median wall, toolcall criteria and well-formed share, routing pass rate."""
    arms: dict = {}
    for r in rows:
        a = arms.setdefault(r["arm"], {"core": [], "toolcall": [], "routing": []})
        a["core" if r["task"] in T.CORE_IDS else r["task"]].append(r)
    out = {}
    for arm_id, groups in arms.items():
        core = groups["core"]
        tc = groups["toolcall"]
        crit: dict = {}
        for r in tc:
            for k, v in r["criteria"].items():
                crit.setdefault(k, []).append(bool(v))
        calls = sum(r["total_calls"] for r in tc)
        bad = sum(r["malformed_calls"] for r in tc)
        out[arm_id] = {
            "core_runs": len(core), "core_pass_rate": (sum(r["pass"] for r in core) / len(core)) if core else None,
            "cost_mean": (sum(r["cost"] for r in core) / len(core)) if core else None, "cost_worst": max((r["cost"] for r in core), default=None),
            "wall_median": statistics.median(r["wall_s"] for r in core) if core else None,
            "toolcall_runs": len(tc), "toolcall_wellformed_share": (1 - bad / calls) if calls else None,
            "toolcall_criteria_rate": {k: sum(v) / len(v) for k, v in crit.items()},
            "routing_runs": len(groups["routing"]), "routing_pass_rate": (sum(r["pass"] for r in groups["routing"]) / len(groups["routing"])) if groups["routing"] else None,
            "total_cost": sum(r["cost"] for r in rows if r["arm"] == arm_id),
        }
    return out


def evaluate(summary: dict, candidate: str = "glm", reference: str = "free") -> list[tuple[str, bool, str]]:
    """The thresholds pre-registered in the plan (Frank confirmed 2026-10-08): pass >= 80% and >= the free default's, mean cost <= $0.10,
    worst <= $0.25, median wall <= 2x the free default's; toolcall: >= 90% of calls well-formed and every criterion in >= 13 of 15 runs
    (>= 4 of 5 for a 5-run arm, i.e. a rate of at least 0.8 when 5 runs, 13/15 when 15)."""
    c, ref = summary.get(candidate), summary.get(reference)
    checks = []
    if not c or c["core_pass_rate"] is None:
        return [("core tasks ran", False, f"no core runs for {candidate}")]
    checks.append(("pass rate >= 80%", c["core_pass_rate"] >= 0.8, f"{c['core_pass_rate']:.0%}"))
    if not ref or ref["core_pass_rate"] is None:
        checks.append(("pass rate >= the free default's", False, f"reference arm '{reference}' has no core runs, so the comparison cannot be made"))
    if ref and ref["core_pass_rate"] is not None:
        checks.append(("pass rate >= the free default's", c["core_pass_rate"] >= ref["core_pass_rate"], f"{c['core_pass_rate']:.0%} vs {ref['core_pass_rate']:.0%}"))
        if ref["wall_median"]:
            checks.append(("median wall <= 2x the free default's", c["wall_median"] <= 2 * ref["wall_median"], f"{c['wall_median']:.0f}s vs {ref['wall_median']:.0f}s"))
    checks.append(("mean cost per task <= $0.10", c["cost_mean"] <= 0.10, f"${c['cost_mean']:.3f}"))
    checks.append(("worst cost per task <= $0.25", c["cost_worst"] <= 0.25, f"${c['cost_worst']:.3f}"))
    if c["toolcall_runs"]:
        floor = 13 / 15 if c["toolcall_runs"] >= 15 else 0.8
        checks.append(("toolcall: >= 90% of calls well-formed", (c["toolcall_wellformed_share"] or 0) >= 0.9, f"{(c['toolcall_wellformed_share'] or 0):.0%}"))
        for name, rate in sorted(c["toolcall_criteria_rate"].items()):
            checks.append((f"toolcall: {name} in enough runs", rate >= floor, f"{rate:.0%}"))
    return checks


def print_report(rows: list[dict]) -> int:
    summary = summarise(rows)
    print(f"{TOOL} report: {len(rows)} run(s), total reported cost ${sum(r['cost'] for r in rows):.3f} (reconcile against the OpenRouter activity page)")
    for arm_id, s in summary.items():
        print(f"- {arm_id}: core {s['core_runs']} runs, pass {s['core_pass_rate'] if s['core_pass_rate'] is None else format(s['core_pass_rate'], '.0%')}, "
              f"toolcall {s['toolcall_runs']} runs, routing {s['routing_runs']} runs, cost ${s['total_cost']:.3f}")
    failed = 0
    for name, ok, detail in evaluate(summary):
        print(f"  {'PASS' if ok else 'FAIL'} {name}: {detail}")
        failed += 0 if ok else 1
    return 1 if failed else 0


def rescore(directory: Path) -> list[dict]:
    """Recompute the event-based criteria whose scorers were tightened after the first run (2026-10-09 review) from the saved events,
    keeping every other stored criterion; writes results_rescored.jsonl and returns the rows."""
    rows = read_rows(directory)
    out = []
    for r in rows:
        events = directory / "events" / f"{r['arm']}_{r['task']}_{r['run']}.jsonl"
        raw = events.read_text(encoding="utf-8") if events.is_file() else ""
        ev = T.parse_events(raw)
        ctx = T.make_ctx(r["task"], r["run"])
        crit = dict(r["criteria"])
        if r["task"] == "toolcall":
            crit.update(T.toolcall_criteria(ev, ctx, None))
        elif r["task"] == "ron_edit" and "reported_the_result" in crit:
            crit["reported_the_result"] = bool(re.search(r"\b(passed|passes|exit code 0|no errors|succeeded|is valid|validated)\b", ev["text"], re.I)) \
                and not re.search(r"\b(failed|fails|errors? found|invalid)\b", ev["text"], re.I)
        elif r["task"] == "routing":
            crit.update(T.TASK_BY_ID["routing"].score(Path("."), ev, ctx))
        out.append({**r, "criteria": crit, "pass": all(crit.values()) and r["status"].split()[0] == "ok", "rescored": True})
    (directory / "results_rescored.jsonl").write_text("\n".join(json.dumps(x) for x in out) + "\n", encoding="utf-8")
    return out


def read_rows(directory: Path) -> list[dict]:
    path = directory / "results.jsonl"
    if not path.is_file():
        raise PilotError(f"{path} does not exist")
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line.strip()]


# ----------------------------------------------------------------------------------------------------------------- main
def m365_preflight() -> None:
    import urllib.request
    try:
        with urllib.request.urlopen(PROXY_URL, timeout=5) as resp:
            resp.read(1)
    except Exception as exc:
        raise PilotError(f"the M365 proxy is not reachable at {PROXY_URL} ({exc}); start it (C:\\ProgramData\\m365-copilot-proxy) or leave the m365 arm out") from exc


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--go", action="store_true", help="really run (spends money on the paid arms); without it only the plan is printed")
    ap.add_argument("--arms", default=",".join(DEFAULT_ARMS), help="comma list of: " + ", ".join(ARM_ORDER))
    ap.add_argument("--tasks", default=",".join(t.id for t in T.TASKS), help="comma list of: " + ", ".join(t.id for t in T.TASKS))
    ap.add_argument("--core-runs", type=int, help="runs per core task (default per arm: 3)")
    ap.add_argument("--toolcall-runs", type=int, help="runs of the toolcall task (default per arm: 15 free/m365, 5 paid)")
    ap.add_argument("--routing-runs", type=int, help="runs of the routing task (default per arm: 3)")
    ap.add_argument("--cap", type=float, default=2.0, help="stop starting runs once the paid arms' summed cost reaches this (USD, default 2.0)")
    ap.add_argument("--out", default=None, help="result folder (default: a timestamped folder next to the worktree)")
    ap.add_argument("--worktree", default=None, help="throwaway worktree path (default: ../ironhold-pilot-wt)")
    ap.add_argument("--base", default=None, help="base commit every run starts from (default: the pre-adoption commit %s, so arms without the driver prompt really have none)" % PRE_ADOPTION_BASE)
    ap.add_argument("--variant", default=None, help="OpenCode --variant (reasoning effort) passed to every run; recorded as unset when omitted")
    ap.add_argument("--keep-sessions", action="store_true", help="keep the OpenCode sessions the runs create (default: delete them by id after each run; the events are saved either way)")
    ap.add_argument("--pause", type=float, default=3.0, help="seconds between runs (free models allow 20 requests per minute)")
    ap.add_argument("--report", metavar="DIR", help="evaluate DIR/results.jsonl against the pre-registered thresholds and exit")
    ap.add_argument("--verify-tasks", action="store_true", help="model-free: check each core task's scorer against an untouched fixture (must fail) and a scripted correct outcome (must pass)")
    ap.add_argument("--rescore", metavar="DIR", help="re-score DIR's saved events with the current (tightened) scorers, write results_rescored.jsonl and report on it")
    ap.add_argument("--selftest", action="store_true", help="run the fixture tests (no OpenCode, no money)")
    args = ap.parse_args(argv)

    if args.selftest:
        return selftest()
    try:
        if args.report:
            return print_report(read_rows(Path(args.report)))
        if args.rescore:
            return print_report(rescore(Path(args.rescore)))
        if args.verify_tasks:
            return verify_tasks(run_git(["rev-parse", args.base or "HEAD"]),
                                Path(args.worktree).resolve() if args.worktree else REPO_ROOT.parent / "ironhold-pilot-verify")
        arm_ids = [a for a in args.arms.split(",") if a]
        task_ids = [t for t in args.tasks.split(",") if t]
        for a in arm_ids:
            if a not in ARMS:
                raise PilotError(f"unknown arm {a}")
        for t in task_ids:
            if t not in T.TASK_BY_ID:
                raise PilotError(f"unknown task {t}")
        runs = plan_runs(arm_ids, task_ids, args.core_runs, args.toolcall_runs, args.routing_runs)
        if not runs:
            raise PilotError("0 runs planned: opt-in arms (free_alt, m365_*) have run counts of 0 by default; pass --core-runs, --toolcall-runs and/or --routing-runs")
        est = estimate(runs)
        print(f"{TOOL}: {len(runs)} run(s) planned; estimated cost per arm (rough, the cap is what counts):")
        for arm_id, (n, cost) in est.items():
            print(f"  {arm_id}: {n} runs, about ${cost:.2f} ({ARMS[arm_id]['model']})")
        paid_est = sum(c for a, (_, c) in est.items() if ARMS[a]["paid"])
        print(f"  paid arms total about ${paid_est:.2f}; script cap ${args.cap:.2f}; remember the OpenRouter key limit is the real backstop")
        if not args.go:
            print("Nothing was run (no --go).")
            return 0

        binary = find_opencode()
        if binary is None:
            print(not_found_message(TOOL, static_hint=False))
            return 2
        if any(a.startswith("m365") for a in arm_ids):
            m365_preflight()
        base = run_git(["rev-parse", args.base or PRE_ADOPTION_BASE])
        base_cfg = json.loads(run_git(["show", f"{base}:.opencode/opencode.json"]))
        if "prompt" in base_cfg.get("agent", {}).get("build", {}) and any(not ARMS[a]["driver"] for a in arm_ids):
            raise PilotError(f"base {base[:10]} already wires agent.build.prompt, so the arms without the driver prompt would get it too; "
                             f"use a base before the adoption (default {PRE_ADOPTION_BASE}) or select only driver arms")
        wt = Path(args.worktree).resolve() if args.worktree else (REPO_ROOT.parent / "ironhold-pilot-wt")
        out = Path(args.out) if args.out else REPO_ROOT.parent / f"ironhold-pilot-results-{datetime.datetime.now().strftime('%Y%m%d-%H%M%S')}"
        out.mkdir(parents=True, exist_ok=True)
        print(f"opencode {opencode_version(binary)}; base {base[:10]}; worktree {wt}; results {out}")
        create_worktree(wt, base)
        spent, aborted = 0.0, None
        try:
            for arm_id, task_id, run in runs:
                arm = ARMS[arm_id]
                if arm["paid"] and spent >= args.cap:
                    # the cap stops every further PAID run; unpaid arms (free models, m365) cost nothing and carry on
                    aborted = aborted or f"cap ${args.cap:.2f} reached at {arm_id}/{task_id}/{run}; the remaining paid runs were not started"
                    continue
                result = run_one(binary, wt, base, arm_id, T.TASK_BY_ID[task_id], run, args.variant, out, args.keep_sessions)
                spent += result["cost"] if arm["paid"] else 0.0
                with open(out / "results.jsonl", "a", encoding="utf-8") as fh:
                    fh.write(json.dumps(result) + "\n")
                print(f"{'PASS' if result['pass'] else 'FAIL'} {arm_id} {task_id} #{run}  {result['wall_s']}s  ${result['cost']:.4f}  (paid arms so far ${spent:.3f})")
                time.sleep(args.pause)
        finally:
            remove_worktree(wt)
        if aborted:
            print("ABORTED PARTIALLY:", aborted)
        return print_report(read_rows(out))
    except PilotError as exc:
        print(f"{TOOL}: {exc}")
        return 2


# ----------------------------------------------------------------------------------------------------------------- verify-tasks
def scripted_solutions(wt: Path) -> dict:
    """A hand-made correct outcome for each core task: (callable applying it to the worktree, text a model would have replied)."""
    def folder(wt):
        p = wt / "planning" / "backlog.md"
        text = p.read_text(encoding="utf-8")
        head = run_git(["rev-parse", "--short", "HEAD"], cwd=wt)
        entry = (f"- [ ] **Pause menu opens twice in co-op** _(found at `{head}` 2026-10-09)_ — two players pressing pause in the same frame open it twice; "
                 f"Reproduce: local_coop_demo, both players press pause on the same frame; wrong: two menus, expected: one.")
        marker = "## Bugs" + chr(10) + chr(10)
        p.write_text(text.replace(marker, marker + entry + chr(10), 1), encoding="utf-8")

    def ron(wt):
        p = wt / "assets" / "projects" / "primitive_world" / "primitive_world.project.ron"
        p.write_text(p.read_text(encoding="utf-8").replace("intensity: 350.0", "intensity: 500.0", 1), encoding="utf-8")

    def refactor(wt):
        for f in (wt / "tools").rglob("*.py"):
            if "__pycache__" in f.parts or f.name.startswith("opencode_pilot"):
                continue
            t = f.read_text(encoding="utf-8")
            if "is_free_model" in t:
                f.write_text(t.replace("is_free_model", "costs_nothing"), encoding="utf-8")

    return {"folder_rule": (folder, ""), "ron_edit": (ron, "The validator passed."), "docs_pointer": (lambda wt: None, "docs/dev/animation-pipeline.md"),
            "refactor_py": (refactor, "The selftest passed."), "planted_review": (lambda wt: None, "average divides by len(values) + 1 (off-by-one).")}


def verify_tasks(base: str, path: Path) -> int:
    """Model-free: for every core task the untouched fixture must FAIL and a scripted correct outcome must PASS (proves the scorers)."""
    create_worktree(path, base)
    bad = 0
    try:
        for task in T.TASKS:
            if not task.core:
                continue
            reset_worktree(path, base)
            copy_cli_binary(path)
            ctx = T.make_ctx(task.id, 1)
            task.setup(path, ctx)
            before = task.score(path, T.parse_events(""), ctx)
            apply, reply = scripted_solutions(path)[task.id]
            apply(path)
            after = task.score(path, T.parse_events(json.dumps({"type": "text", "part": {"text": reply}})), ctx)
            ok = (not all(before.values())) and all(after.values())
            bad += 0 if ok else 1
            print(f"{'OK  ' if ok else 'FAIL'} {task.id}: untouched -> {'fails' if not all(before.values()) else 'PASSES (wrong)'}, scripted solution -> "
                  f"{'passes' if all(after.values()) else 'FAILS: ' + str({k: v for k, v in after.items() if not v})}")
    finally:
        remove_worktree(path)
    return 1 if bad else 0


# ----------------------------------------------------------------------------------------------------------------- selftest
def selftest() -> int:
    ok = 0

    def check(cond, label):
        nonlocal ok
        if not cond:
            print(f"selftest FAILED: {label}")
            raise SystemExit(1)
        ok += 1

    def ev_line(kind, part=None, **extra):
        return json.dumps({"type": kind, "sessionID": "ses_1", "part": part or {}, **extra})

    def call(tool, inp, status="completed", error=None, output=""):
        return ev_line("tool_use", {"tool": tool, "state": {"status": status, "input": inp, "error": error, "output": output}})

    # parse_events / well-formedness
    raw = "\n".join([call("read", {"filePath": "a"}), call("bash", {"command": "x"}, "error", "The user has specified a rule which prevents you"),
                     call("write", {}, "error", "Invalid input: expected string, received undefined"),
                     ev_line("step_finish", {"tokens": {"input": 31000}, "cost": 0.004}), ev_line("text", {"text": "DONE"}), "noise"])
    ev = T.parse_events(raw)
    check(len(ev["calls"]) == 3 and ev["text"] == "DONE" and ev["tokens_in"] == 31000 and abs(ev["cost"] - 0.004) < 1e-9, "events parse")
    check([T.is_malformed(c) for c in ev["calls"]] == [False, False, True], "only argument errors count as malformed")
    check(abs(T.wellformed_ratio(ev) - 2 / 3) < 1e-9, "well-formed ratio")

    # plan_runs and the config builder
    runs = plan_runs(["glm"], ["toolcall", "routing", "docs_pointer"])
    check([r[1] for r in runs].count("toolcall") == 5 and [r[1] for r in runs].count("routing") == 3 and [r[1] for r in runs].count("docs_pointer") == 3, "run counts per arm")
    check(plan_runs(["free"], ["toolcall"], toolcall_runs=2) == [("free", "toolcall", 1), ("free", "toolcall", 2)], "run count override")
    repo_perm = {"edit": "allow", "external_directory": {"*": "ask", "C:/p/**": "deny"}, "task": {"*": "allow", "*-deep": "ask"},
                 "bash": {"*": "ask", "cargo test*": "allow", "wasm-pack build*": "allow", "git push*": "ask", "ls*": "allow", "git add*pkg*": "deny"}}
    cfg = build_config(T.TASK_BY_ID["toolcall"], ARMS["glm"], Path("C:/x/driver.md"), repo_perm)
    b = cfg["permission"]["bash"]
    check(b["*"] == "deny" and b["git push*"] == "deny" and b["cargo test*"] == "deny" and b["wasm-pack build*"] == "deny" and b["ls*"] == "allow"
          and b["echo FORBIDDEN*"] == "deny" and list(b)[-1] == "echo FORBIDDEN*", "unattended bash: every ask and every cargo/wasm-pack entry denied, the allow-list kept")
    check(cfg["permission"]["external_directory"] == {"*": "deny", "C:/p/**": "deny"} and cfg["permission"]["task"]["*-deep"] == "deny" and "agent" not in cfg
          and cfg["permission"]["edit"] == "allow", "unattended external directories and -deep denied, the rest kept")
    check(harden({"a": {"b": "ask", "c": "allow"}, "d": "ask"}) == {"a": {"b": "deny", "c": "allow"}, "d": "deny"}, "harden rewrites every ask")
    cfg = build_config(T.TASK_BY_ID["routing"], ARMS["glm"], Path("C:/x/driver.md"))
    check(cfg["agent"]["build"]["prompt"] == "{file:C:/x/driver.md}" and cfg["agent"]["alignment-reviewer"]["model"] == INVALID_MODEL, "routing config wires the driver prompt and breaks the primary")
    check(build_config(T.TASK_BY_ID["docs_pointer"], ARMS["glm_driver"], Path("d"))["agent"]["build"]["prompt"] == "{file:d}", "an arm can wire the driver prompt")

    # scorers on a synthetic worktree
    with tempfile.TemporaryDirectory() as td:
        wt = Path(td)
        ctx = T.make_ctx("toolcall", 1)
        T.TASK_BY_ID["toolcall"].setup(wt, ctx)
        n, v = ctx["n"], ctx["value"]
        (wt / "pilot" / "chain" / "result.txt").write_text(f"{2 * v}\n", encoding="utf-8")
        good = T.parse_events("\n".join([
            call("grep", {"pattern": f"MARK-{n}"}), call("read", {"filePath": "pilot/chain/beta.txt"}),
            call("write", {"filePath": "pilot/chain/result.txt", "content": str(2 * v)}), call("read", {"filePath": "pilot/chain/result.txt"}),
            call("bash", {"command": f"echo FORBIDDEN-{n}"}, "error", "The user has specified a rule which prevents you"),
            call("read", {"filePath": "pilot/chain/missing.txt"}, "error", "File not found"),
            call("read", {"filePath": "pilot/chain/x1.txt"}), call("read", {"filePath": "pilot/chain/x2.txt"}),
            ev_line("text", {"text": f"1 done. 2 the command was refused. 3 missing.txt does not exist. 4 X1-{n} and X2-{n}."})]))
        score = T.TASK_BY_ID["toolcall"].score(wt, good, ctx)
        check(all(score.values()) and len(score) == 5, f"a perfect toolcall run passes all five criteria {score}")
        looped = T.parse_events("\n".join([call("bash", {"command": f"echo FORBIDDEN-{n}"}, "error", "rule which prevents you")] * 4 + [ev_line("text", {"text": "refused"})]))
        check(T.TASK_BY_ID["toolcall"].score(wt, looped, ctx)["denied_call_not_looped"] is False, "a looped denied call fails the criterion")
        invented = T.parse_events(call("read", {"filePath": "pilot/chain/missing.txt"}, "error", "File not found") + "\n" + ev_line("text", {"text": "missing.txt contains hello world"}))
        check(T.TASK_BY_ID["toolcall"].score(wt, invented, ctx)["missing_file_reported_not_invented"] is False, "an invented file content fails (the file's own name does not count as 'missing')")
        done = T.parse_events(call("bash", {"command": f"echo FORBIDDEN-{n}"}, "completed", "", f"FORBIDDEN-{n}") + "\n" + ev_line("text", {"text": "I could not find the file"}))
        check(T.TASK_BY_ID["toolcall"].score(wt, done, ctx)["denied_call_not_looped"] is False, "a denied command that actually ran, or no attempt, fails")
        check(T.TASK_BY_ID["ron_edit"].score(wt, T.parse_events(ev_line("text", {"text": "validation failed: invalid value"})), ctx)["reported_the_result"] is False, "'invalid' is not a pass report")

        T.TASK_BY_ID["planted_review"].setup(wt, ctx)
        check(T.TASK_BY_ID["planted_review"].score(wt, T.parse_events(ev_line("text", {"text": "average divides by len(values) + 1"})), ctx)["finds_the_planted_bug"]
              and not T.TASK_BY_ID["planted_review"].score(wt, T.parse_events(ev_line("text", {"text": "looks fine"})), ctx)["finds_the_planted_bug"], "planted review scorer")
        check(T.TASK_BY_ID["docs_pointer"].score(wt, T.parse_events(ev_line("text", {"text": "docs/dev/animation-pipeline.md"})), ctx) == {"names_the_right_file": True, "answer_is_short": True},
              "docs pointer scorer")
        r = T.TASK_BY_ID["routing"].score(wt, T.parse_events("\n".join([call("task", {"subagent_type": "alignment-reviewer"}, "error", "model not found"),
                                                                       call("task", {"subagent_type": "alignment-reviewer-alt"}), ev_line("text", {"text": "the bounds are swapped"})])), ctx)
        check(all(r.values()), f"routing scorer accepts recovery through the alt twin {r}")
        r = T.TASK_BY_ID["routing"].score(wt, T.parse_events("\n".join([call("task", {"subagent_type": "alignment-reviewer-alt"}), ev_line("text", {"text": "the bounds handling is correct"})])), ctx)
        check(r["final_answer_correct"] is False, "a vague 'bounds' answer is not the planted bug")
        r = T.TASK_BY_ID["routing"].score(wt, T.parse_events("\n".join([call("task", {"subagent_type": "alignment-reviewer-deep"}), ev_line("text", {"text": "swapped"})])), ctx)
        check(r["no_deep_agent_ran"] is False and r["recovered_through_the_alt_twin"] is False, "a -deep run fails the routing task")

    # thresholds
    def row(arm, task, ok, cost=0.02, wall=30.0, crit=None, total=10, bad=0):
        return {"arm": arm, "task": task, "pass": ok, "cost": cost, "wall_s": wall, "criteria": crit or {}, "total_calls": total, "malformed_calls": bad}
    rows = ([row("glm", t, True) for t in T.CORE_IDS for _ in range(3)] + [row("free", t, i % 5 != 0, 0.0, 40.0) for t in T.CORE_IDS for i in range(3)]
            + [row("glm", "toolcall", True, crit={"chain_of_dependent_calls": True}) for _ in range(5)])
    res = evaluate(summarise(rows))
    check(all(ok for _, ok, _ in res), f"a good candidate passes every threshold {res}")
    check(any(not ok and "reference arm" in detail for _, ok, detail in evaluate(summarise([row("glm", t, True) for t in T.CORE_IDS]))), "a missing reference arm is a FAIL row")
    bad_rows = [row("glm", t, False, 0.3) for t in T.CORE_IDS for _ in range(3)] + [row("free", t, True, 0.0, 10.0) for t in T.CORE_IDS for _ in range(3)]
    res = evaluate(summarise(bad_rows))
    check(not any(ok for name, ok, _ in res if name.startswith(("pass", "mean", "worst", "median"))), "a bad candidate fails the thresholds")
    print(f"selftest ok ({ok} checks)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
