"""Task set, fixtures and scorers for the OpenCode GLM pilot (planning/features/opencode_glm_driver_pilot.md).

Pure and model-free: everything here can be tested with fake events (`python tools/opencode_pilot.py --selftest`). The runner
(`tools/opencode_pilot.py`) creates a throwaway worktree, calls `setup`, runs `opencode run`, parses the JSON events with
`parse_events` and asks `score` for a dict of named criteria; a run passes when every criterion is true.

Seven tasks: five core (`folder_rule`, `ron_edit`, `docs_pointer`, `refactor_py`, `planted_review`), `toolcall` (five
tool-calling criteria in one session) and `routing` (the driver must recover through an `-alt` twin). `refactor_py` replaces
the plan's cargo-check refactor on purpose: a cargo build per run would mean a 10-25 minute rebuild on a disk-constrained machine
and must never overlap another cargo run (a Rust variant can be added later if Frank wants one).
"""

from __future__ import annotations

import json
import random
import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Callable, Optional

# --------------------------------------------------------------------------------------------------------------- events
MALFORMED_RE = re.compile(
    r"invalid (arguments|input|parameters|tool)|validation|schema|expected (string|number|object|array|boolean)|unknown tool|"
    r"malformed|missing (required )?(parameter|argument)|required property", re.I)
NOT_MALFORMED_RE = re.compile(r"rule which prevents|permission|ENOENT|no such file|not found|does not exist|doesn't exist|File not found", re.I)


def parse_events(text: str) -> dict:
    """Condense `opencode run --format json` output: tool calls, reply text, tokens, cost, errors, session ids."""
    ev = {"calls": [], "text": "", "tokens_in": 0, "cost": 0.0, "errors": [], "sessions": set(), "steps": 0}
    texts = []
    for line in text.splitlines():
        line = line.strip()
        if not line.startswith("{"):
            continue
        try:
            e = json.loads(line)
        except json.JSONDecodeError:
            continue
        if e.get("sessionID"):
            ev["sessions"].add(e["sessionID"])
        part = e.get("part") if isinstance(e.get("part"), dict) else {}
        kind = e.get("type")
        if kind == "tool_use":
            state = part.get("state") if isinstance(part.get("state"), dict) else {}
            ev["calls"].append({"tool": part.get("tool"), "status": state.get("status"), "input": state.get("input") or {},
                                "error": str(state.get("error") or ""), "output": str(state.get("output") or "")})
        elif kind == "text":
            texts.append(str(part.get("text") or ""))
        elif kind == "step_finish":
            ev["steps"] += 1
            tokens = part.get("tokens") or {}
            ev["tokens_in"] = max(ev["tokens_in"], int(tokens.get("input") or 0))
            ev["cost"] += float(part.get("cost") or 0.0)
        elif kind == "error":
            ev["errors"].append(str(e.get("error") or e))
    ev["text"] = "\n".join(texts)
    return ev


def is_malformed(call: dict) -> bool:
    """A tool call that failed because its arguments were wrong (not because a rule or the filesystem said no)."""
    err = call.get("error") or ""
    return bool(err) and bool(MALFORMED_RE.search(err)) and not NOT_MALFORMED_RE.search(err)


def wellformed_ratio(ev: dict) -> float:
    calls = ev["calls"]
    return 1.0 if not calls else sum(1 for c in calls if not is_malformed(c)) / len(calls)


def calls_of(ev: dict, *tools: str) -> list:
    return [c for c in ev["calls"] if c.get("tool") in tools]


def read_text(path: Path) -> str:
    try:
        return path.read_text(encoding="utf-8")
    except OSError:
        return ""


def write_file(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def git(worktree: Path, *args: str) -> subprocess.CompletedProcess:
    return subprocess.run(["git", *args], cwd=worktree, capture_output=True, text=True, encoding="utf-8")


# ------------------------------------------------------------------------------------------------------------------ tasks
@dataclass
class Task:
    id: str
    core: bool
    prompt: Callable[[dict], str]
    setup: Callable[[Path, dict], None]
    score: Callable[[Path, dict, dict], dict]
    timeout: int = 600
    needs_driver_prompt: bool = False
    bash_deny: tuple = ()          # bash patterns denied for this task (the `toolcall` denied-call check)
    break_agent: Optional[str] = None  # `routing`: the primary whose model is made invalid so the driver must use its twin


def make_ctx(task_id: str, run: int) -> dict:
    """Per-run random values, reproducible from (task, run) so a re-run starts from the same fixture."""
    rng = random.Random(f"{task_id}-{run}")
    return {"n": rng.randint(1000, 9999), "value": rng.randint(11, 97)}


# 1. folder_rule: the nearest CLAUDE.md (planning/CLAUDE.md) says how a bug entry must look
def _folder_setup(wt: Path, ctx: dict) -> None:
    ctx["before"] = read_text(wt / "planning" / "backlog.md").split("\n")


def _folder_score(wt: Path, ev: dict, ctx: dict) -> dict:
    text = read_text(wt / "planning" / "backlog.md")
    lines = text.split("\n")
    try:
        start = lines.index("## Bugs")
    except ValueError:
        return {"entry_in_bugs_section": False}
    end = next((i for i in range(start + 1, len(lines)) if lines[i].startswith("## ")), len(lines))
    new = [ln for ln in lines[start:end] if ln not in set(ctx.get("before", [])) and "pause" in ln.lower()]
    entry = new[0] if new else ""
    m = re.search(r"found at `([0-9a-f]{7,40})`", entry)
    real_hash = bool(m) and git(wt, "cat-file", "-e", m.group(1) + "^{commit}").returncode == 0
    return {"entry_in_bugs_section": bool(entry), "has_found_at_hash": bool(m), "hash_is_a_real_commit": real_hash,
            "has_reproduce_steps": "reproduce" in entry.lower()}


# 2. ron_edit: edit a RON value, then the project validator must still pass
def _ron_score(wt: Path, ev: dict, ctx: dict) -> dict:
    ron = read_text(wt / "assets" / "projects" / "primitive_world" / "primitive_world.project.ron")
    block = re.search(r"global_environment:\s*\((.*?)\n    \),", ron, re.S)
    edited = bool(block) and "intensity: 500.0" in block.group(1) and "intensity: 350.0" not in ron
    binary = wt / "tools" / "bin" / ("ironhold.exe" if sys.platform == "win32" else "ironhold")
    validates = False
    if binary.is_file():
        proc = subprocess.run([str(binary), "validate", "assets/projects/primitive_world"], cwd=wt, capture_output=True, text=True, timeout=120)
        validates = proc.returncode == 0
    reported = bool(re.search(r"pass|valid|success|no errors|ok\b|exit code 0", ev["text"], re.I))
    return {"value_edited": edited, "validator_passes_on_result": validates, "reported_the_result": reported}


# 3. docs_pointer: find where a moved section lives now
def _docs_score(wt: Path, ev: dict, ctx: dict) -> dict:
    answer = ev["text"].strip()
    return {"names_the_right_file": "docs/dev/animation-pipeline.md" in answer.replace("\\", "/"), "answer_is_short": len(answer) <= 400}


# 4. refactor_py: rename a function across files; the selftest must still pass
def _refactor_score(wt: Path, ev: dict, ctx: dict) -> dict:
    tools = wt / "tools"
    leftovers = 0
    for f in tools.rglob("*.py"):
        if "__pycache__" in f.parts:
            continue
        leftovers += read_text(f).count("is_free_model")
    defs = sum(read_text(f).count("def costs_nothing") for f in tools.rglob("*.py") if "__pycache__" not in f.parts)
    changed = [ln for ln in git(wt, "diff", "--name-only").stdout.split() if ln.endswith(".py")]
    proc = subprocess.run([sys.executable, "tools/opencode_probe.py", "--selftest"], cwd=wt, capture_output=True, text=True, timeout=300)
    return {"no_old_name_left": leftovers == 0, "defined_exactly_once": defs == 1, "at_least_two_files_changed": len(changed) >= 2,
            "selftest_passes": proc.returncode == 0}


# 5. planted_review: a review must find the planted bug
PLANTED_REVIEW = '''"""Small statistics helpers (pilot fixture)."""


def average(values):
    total = 0
    for v in values:
        total += v
    return total / (len(values) + 1)


def maximum(values):
    best = values[0]
    for v in values[1:]:
        if v > best:
            best = v
    return best
'''


def _review_setup(wt: Path, ctx: dict) -> None:
    write_file(wt / "tools" / "_pilot_review.py", PLANTED_REVIEW)


def _review_score(wt: Path, ev: dict, ctx: dict) -> dict:
    found = bool(re.search(r"\+\s*1|off[- ]by[- ]one|wrong (divisor|denominator)|incorrect (divisor|denominator)|divides by (one )?(more|too)", ev["text"], re.I))
    return {"finds_the_planted_bug": found}


# 6. toolcall: five tool-calling criteria in one session
def _toolcall_setup(wt: Path, ctx: dict) -> None:
    d = wt / "pilot" / "chain"
    d.mkdir(parents=True, exist_ok=True)
    n, v = ctx["n"], ctx["value"]
    (d / "alpha.txt").write_text("nothing here\nvalue: 1\n", encoding="utf-8")
    (d / "beta.txt").write_text(f"marker: MARK-{n}\nvalue: {v}\n", encoding="utf-8")
    (d / "gamma.txt").write_text("also nothing\nvalue: 2\n", encoding="utf-8")
    (d / "x1.txt").write_text(f"X1-{n}\nsecond line\n", encoding="utf-8")
    (d / "x2.txt").write_text(f"X2-{n}\nsecond line\n", encoding="utf-8")


def _toolcall_prompt(ctx: dict) -> str:
    n = ctx["n"]
    return (f"Do these five things in order, then finish with one message that lists the five results.\n"
            f"1. Search the folder pilot/chain for the text MARK-{n}, read the file that contains it, double its `value:` number, write only the doubled "
            f"number to pilot/chain/result.txt, then read result.txt back.\n"
            f"2. Run the shell command `echo FORBIDDEN-{n}`. If it is refused, do not try another way to run it; say that it was refused and continue.\n"
            f"3. Read pilot/chain/missing.txt and tell me its content. If the file does not exist, say so.\n"
            f"4. Read pilot/chain/x1.txt and pilot/chain/x2.txt and give me the first line of each.\n"
            f"5. Finish.")


REFUSED_RE = re.compile(r"refus|denied|prevent|not allowed|blocked|couldn't|could not|unable|permission", re.I)
MISSING_RE = re.compile(r"not exist|no such|not found|missing|cannot be found|can't be found|doesn't exist|does not exist|isn't there|nonexistent", re.I)


def _toolcall_score(wt: Path, ev: dict, ctx: dict) -> dict:
    n, v = ctx["n"], ctx["value"]
    calls = ev["calls"]
    result = read_text(wt / "pilot" / "chain" / "result.txt").strip()

    def idx(pred, start=0):
        return next((i for i in range(start, len(calls)) if pred(calls[i])), -1)

    i_read_beta = idx(lambda c: c["tool"] == "read" and "beta.txt" in json.dumps(c["input"]))
    i_write = idx(lambda c: c["tool"] in ("write", "edit") and "result.txt" in json.dumps(c["input"]), max(i_read_beta, 0))
    i_back = idx(lambda c: c["tool"] == "read" and "result.txt" in json.dumps(c["input"]), max(i_write, 0))
    chain = len(calls) >= 4 and 0 <= i_read_beta < i_write < i_back and result == str(2 * v)
    forbidden = [c for c in calls_of(ev, "bash") if f"FORBIDDEN-{n}" in json.dumps(c["input"])]
    missing_read = [c for c in calls_of(ev, "read") if "missing.txt" in json.dumps(c["input"])]
    xreads = [c for c in calls_of(ev, "read") if re.search(r"x[12]\.txt", json.dumps(c["input"]))]
    text = ev["text"]
    return {
        "chain_of_dependent_calls": chain,
        "all_arguments_wellformed": not any(is_malformed(c) for c in calls),
        "denied_call_not_looped": len(forbidden) <= 2 and bool(REFUSED_RE.search(text)),
        "missing_file_reported_not_invented": bool(missing_read) and bool(MISSING_RE.search(text)),
        "two_independent_reads_both_returned": len(xreads) >= 2 and f"X1-{n}" in text and f"X2-{n}" in text,
    }


# 7. routing: the primary of a role is broken; the driver must reach the review through the -alt twin, never a -deep agent
ROUTING_FIXTURE = '''"""Number helpers (pilot fixture)."""


def clamp(x, lo, hi):
    """Return x limited to the range lo..hi."""
    return max(hi, min(x, lo))
'''


def _routing_setup(wt: Path, ctx: dict) -> None:
    write_file(wt / "tools" / "_pilot_routing.py", ROUTING_FIXTURE)


def _routing_score(wt: Path, ev: dict, ctx: dict) -> dict:
    types = [str(c["input"].get("subagent_type", "")) for c in calls_of(ev, "task")]
    found = bool(re.search(r"swap|revers|wrong order|max\(hi|min\(x, lo\)|bounds|inverted|lo and hi", ev["text"], re.I))
    return {"delegated_to_a_subagent": bool(types), "recovered_through_the_alt_twin": "alignment-reviewer-alt" in types,
            "no_deep_agent_ran": not any(t.endswith("-deep") for t in types), "final_answer_correct": found}


TASKS = [
    Task("folder_rule", True,
         lambda ctx: "Log a new bug in the project backlog: when two players press the pause button in the same frame in local co-op, the pause menu opens twice. Add it the way this repository's planning rules require.",
         _folder_setup, _folder_score),
    Task("ron_edit", True,
         lambda ctx: "In assets/projects/primitive_world/primitive_world.project.ron set the global environment intensity to 500.0, then check the project with the ironhold validator (tools/bin/ironhold validate assets/projects/primitive_world) and tell me whether it passed.",
         lambda wt, ctx: None, _ron_score),
    Task("docs_pointer", True,
         lambda ctx: "Where is the long-form note about the animation resolver/playback pipeline now documented? Reply with the file path only.",
         lambda wt, ctx: None, _docs_score, timeout=300),
    Task("refactor_py", True,
         lambda ctx: "Rename the Python function `is_free_model` to `costs_nothing` everywhere under tools/ (the definition and every use, including selftests). Keep the behaviour identical, then run `python tools/opencode_probe.py --selftest` and report the result.",
         lambda wt, ctx: None, _refactor_score, timeout=900),
    Task("planted_review", True,
         lambda ctx: "Review tools/_pilot_review.py. List the real bugs you find, one line each, most serious first.",
         _review_setup, _review_score, timeout=300),
    Task("toolcall", False, _toolcall_prompt, _toolcall_setup, _toolcall_score, timeout=600, bash_deny=("echo FORBIDDEN*",)),
    Task("routing", False,
         lambda ctx: "Use the alignment-reviewer subagent to review tools/_pilot_routing.py and tell me in one sentence what bug it finds.",
         _routing_setup, _routing_score, timeout=600, needs_driver_prompt=True, break_agent="alignment-reviewer"),
]
TASK_BY_ID = {t.id: t for t in TASKS}
CORE_IDS = [t.id for t in TASKS if t.core]
