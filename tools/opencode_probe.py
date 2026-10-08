#!/usr/bin/env python3
"""
OpenCode load-behaviour probe: which instruction files does OpenCode attach when it reads a file here?

Why it exists: whether OpenCode picks up a folder's CLAUDE.md (and not an AGENTS.md stub, not .claude/rules) was assumed
once and turned out wrong (an `AGENTS.md` containing `@CLAUDE.md` is attached as literal text and shadows the real
file; found 2026-10-08, OpenCode 1.18.33). `tools/opencode_sync_check.py` checks configuration drift; this checks what
OpenCode actually loads. Facts and how each is re-verified: the "Verified compatibility facts" table in
.opencode/README.md. What to account for when editing CLAUDE.md files: docs/dev/claude_md_maintenance.md.

Default mode is MODEL-FREE: it runs OpenCode's own read tool through `opencode debug agent build --tool read` (no model,
no tokens, no auth) with temporary XDG data/state dirs so no session lands in your OpenCode history, and compares what
was attached with what the tree says should be attached. For each probe file the expected set is, from the file's own
folder up to (not including) the repo root, the first of AGENTS.md > CLAUDE.md > CONTEXT.md of each folder. The root is
excluded because its files arrive at session start (AGENTS.md by discovery, CLAUDE.md through `instructions` in
.opencode/opencode.json). Negative probe files (Cargo.toml, a docs/dev page, .claude/rules/*.md ...) must attach nothing.

Usage (run on this Windows machine after `nvs use 24.21` in the SAME terminal, see below):
    python tools/opencode_probe.py                   # model-free probe of every folder with a CLAUDE.md
    python tools/opencode_probe.py --static          # only the checks that need no OpenCode at all
    python tools/opencode_probe.py --only crates/ironhold_core/src/lib.rs   # one file, e.g. after an edit
    python tools/opencode_probe.py --dir ../ironhold-lib-some-feature       # probe another worktree
    python tools/opencode_probe.py --json            # machine-readable report
    python tools/opencode_probe.py --live            # the ONLY mode that calls a model (see below)
    python tools/opencode_probe.py --selftest        # built-in fixture tests, no OpenCode needed

--live runs one tiny `opencode run` ("read 3 lines of FILE, reply DONE") to report OpenCode's fixed token baseline
(tokens.input of the first step; MACHINE-SPECIFIC: your global skills and ~/.config/opencode/AGENTS.md are in it) and the
skill count. Free models only unless --allow-paid; an m365/... model needs the local proxy running (checked first). A run
with no `read` tool event is an error to retry, never "attached nothing". Its sessions are titled
"opencode-probe <utc>" and deleted afterwards. --max-cost aborts when the summed cost is exceeded.

Exit codes (same as `ironhold validate`): 0 every check passed, 1 a check failed, 2 tool error (OpenCode missing, bad
arguments, the debug interface did not answer).

Finding OpenCode: OPENCODE_BIN, then PATH. nvs cannot be activated from inside Python (it would only change a child's
PATH), so when it is missing the message tells you to run `nvs use 24.21` first.

Caveat: `opencode debug ...` is not a documented contract and may change between OpenCode versions. The probe warns when
`opencode --version` differs from the version recorded in .opencode/README.md; --live is the fallback.
"""

import argparse
import datetime
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import urllib.error
import urllib.request
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import opencode_sync_check as sync  # noqa: E402
from _opencode_common import find_opencode, not_found_message, opencode_version  # noqa: E402

TOOL = "opencode_probe"
INSTR_NAMES = ("AGENTS.md", "CLAUDE.md", "CONTEXT.md")
TEXT_EXTS = {".rs", ".py", ".wgsl", ".ron", ".toml", ".md", ".json", ".html", ".js"}
VERIFIED_RE = re.compile(r"<!--\s*opencode-verified-version:\s*([0-9][^\s>]*)\s*-->")
BLOCK_RE = re.compile(r"Instructions from: ([^\n]+)\n(.*?)(?=\n\nInstructions from: |\n</system-reminder>|\Z)", re.S)
COMMENT_RE = re.compile(r"<!--.*?-->", re.S)
M365_URL = os.environ.get("M365_PROXY_URL", "http://localhost:4141/v1/models")


class ProbeError(Exception):
    """A tool-level failure (exit code 2)."""


# ----------------------------------------------------------------------------------------------- paths and expectation
def norm(path) -> str:
    return os.path.normcase(os.path.normpath(str(path)))


def instruction_file_in(folder: Path):
    for name in INSTR_NAMES:
        candidate = folder / name
        if candidate.is_file():
            return candidate
    return None


def relposix(root: Path, path) -> str:
    """`path` relative to `root`, forward slashes, original case (display form)."""
    return os.path.relpath(str(path), str(root)).replace("\\", "/")


def same(a: str, b: str) -> bool:
    return os.path.normcase(a) == os.path.normcase(b)


def expected_attach(root: Path, rel_file: str) -> list[str]:
    """Repo-relative instruction files OpenCode should attach for `rel_file`, nearest first (root excluded)."""
    root_n = norm(root)
    folder = (root / rel_file).parent
    found = []
    while norm(folder) != root_n and norm(folder).startswith(root_n):
        chosen = instruction_file_in(folder)
        if chosen is not None:
            found.append(relposix(root, chosen))
        folder = folder.parent
    return found


def tracked_files(root: Path) -> list[str]:
    out = subprocess.run(["git", "ls-files", "-z"], cwd=root, capture_output=True, text=True, encoding="utf-8")
    if out.returncode != 0:
        raise ProbeError(f"{root} is not a git checkout (git ls-files failed); pass --dir <repo root>")
    return [f for f in out.stdout.split("\0") if f]


def choose_probe_files(tracked: list[str]) -> tuple[list[str], list[str]]:
    """(positives, negatives): one text file per folder that owns a CLAUDE.md (root excluded), plus files that attach nothing."""
    tracked_set = set(tracked)
    folders = sorted({str(Path(f).parent.as_posix()) for f in tracked if Path(f).name == "CLAUDE.md" and Path(f).parent != Path(".")})
    positives: list[str] = []
    for folder in folders:
        def is_text(f: str) -> bool:
            return Path(f).suffix in TEXT_EXTS and Path(f).name not in INSTR_NAMES
        direct = sorted(f for f in tracked if Path(f).parent.as_posix() == folder and is_text(f))
        below = sorted(f for f in tracked if f.startswith(folder + "/") and is_text(f))
        pick = (direct or below or [None])[0]
        if pick:
            positives.append(pick)
    if "crates/ironhold_core/src/lib.rs" in tracked_set:
        positives.append("crates/ironhold_core/src/lib.rs")  # the file that must see ONLY its own folder's CLAUDE.md
    negatives = [f for f in ("Cargo.toml", "crates/ironhold_web/src/lib.rs") if f in tracked_set]
    negatives += sorted(f for f in tracked if f.startswith("docs/dev/") and f.endswith(".md"))[:1]
    negatives += sorted(f for f in tracked if f.startswith(".claude/rules/") and f.endswith(".md"))[:1]
    seen: set[str] = set()
    return [f for f in positives if not (f in seen or seen.add(f))], negatives


# --------------------------------------------------------------------------------------------- the model-free debug read
def isolated_env() -> tuple[dict, str]:
    """Environment whose OpenCode data/state live in a temp dir, so debug sessions never reach the real history."""
    tmp = tempfile.mkdtemp(prefix="opencode_probe_xdg_")
    env = os.environ.copy()
    env["XDG_DATA_HOME"] = tmp
    env["XDG_STATE_HOME"] = tmp
    return env, tmp


def parse_json_text(text: str) -> dict:
    text = text.lstrip("\ufeff")
    start = text.find("{")
    if start < 0:
        raise ProbeError("OpenCode printed no JSON")
    try:
        return json.loads(text[start:])
    except json.JSONDecodeError as exc:
        raise ProbeError(f"could not parse OpenCode's JSON output: {exc}") from exc


def attachments(data: dict) -> tuple[list[str], list[tuple[str, str]]]:
    """(metadata.loaded, [(path, body)] parsed from the `Instructions from:` blocks of the tool output), absolute paths as printed."""
    result = data.get("result") or {}
    loaded = list((result.get("metadata") or {}).get("loaded", []))
    blocks = [(m.group(1).strip(), m.group(2)) for m in BLOCK_RE.finditer(result.get("output", ""))]
    return loaded, blocks


def debug_read(binary: str, root: Path, rel: str, env: dict, pure: bool, timeout: int = 90) -> dict:
    params = "{filePath:'%s',limit:2}" % rel  # JS-style literal, single quotes, repo-relative forward slashes
    cmd = [binary, "debug", "agent", "build", "--tool", "read", "--params", params] + (["--pure"] if pure else [])
    try:
        proc = subprocess.run(cmd, cwd=root, env=env, capture_output=True, text=True, encoding="utf-8", timeout=timeout)
    except subprocess.TimeoutExpired as exc:
        raise ProbeError(f"`opencode debug agent` timed out after {timeout}s on {rel}") from exc
    except OSError as exc:
        raise ProbeError(f"could not run {binary}: {exc}") from exc
    if proc.returncode != 0:
        raise ProbeError(f"`opencode debug agent` exited {proc.returncode} on {rel}: {(proc.stderr or proc.stdout).strip()[:300]}")
    return parse_json_text(proc.stdout)


def probe_one(binary, root: Path, rel: str, env: dict, pure: bool) -> dict:
    data = debug_read(binary, root, rel, env, pure)
    loaded_abs, blocks = attachments(data)
    loaded = [relposix(root, p) for p in loaded_abs]
    block_paths = [relposix(root, p) for p, _ in blocks]
    expected = expected_attach(root, rel)
    missing = [e for e in expected if not any(same(e, l) for l in loaded)]
    unexpected = [l for l in loaded if not any(same(l, e) for e in expected)]
    warn = []
    if sorted(map(os.path.normcase, loaded)) != sorted(map(os.path.normcase, block_paths)):
        warn.append("metadata.loaded and the Instructions-from text disagree")
    size = sum(len(body.encode("utf-8")) for _, body in blocks)
    comment_bytes = sum(len(m.group(0).encode("utf-8")) for _, body in blocks for m in COMMENT_RE.finditer(body))
    return {"file": rel, "expected": expected, "attached": loaded, "missing": missing, "unexpected": unexpected,
            "status": "PASS" if not missing and not unexpected else "FAIL", "bytes": size, "comment_bytes": comment_bytes, "warn": warn}


# ------------------------------------------------------------------------------------------------------- static checks
def run_static(root: Path) -> list[tuple[str, list[str]]]:
    config_path = root / ".opencode" / "opencode.json"
    config = json.loads(config_path.read_text(encoding="utf-8")) if config_path.is_file() else {}
    return [
        ("Subfolder AGENTS.md / CONTEXT.md", sync.check_no_subfolder_instruction_files(root)),
        ("`instructions` matching .claude/rules", sync.check_instructions_not_matching_rules(config, root)),
    ]


def verified_version(root: Path):
    readme = root / ".opencode" / "README.md"
    if not readme.is_file():
        return None
    match = VERIFIED_RE.search(readme.read_text(encoding="utf-8"))
    return match.group(1) if match else None


# ------------------------------------------------------------------------------------------------------------- --live
def is_free_model(model_id: str) -> bool:
    return model_id.endswith(":free") or model_id.endswith("-free") or model_id.startswith("m365/")


def parse_run_events(text: str) -> dict:
    """Summarise `opencode run --format json` output: read events, per-step tokens, summed cost, errors, reply text."""
    summary = {"reads": 0, "attached": [], "steps": [], "cost": 0.0, "errors": [], "reply": ""}
    for line in text.splitlines():
        line = line.strip()
        if not line.startswith("{"):
            continue
        try:
            event = json.loads(line)
        except json.JSONDecodeError:
            continue
        kind, part = event.get("type"), event.get("part") or {}
        if kind == "tool_use" and part.get("tool") == "read":
            summary["reads"] += 1
            out = (part.get("state") or {}).get("output", "")
            summary["attached"] += [m.group(1).strip() for m in BLOCK_RE.finditer(out)]
        elif kind == "step_finish":
            tokens = part.get("tokens") or {}
            summary["steps"].append(int(tokens.get("input", 0)))
            summary["cost"] += float(part.get("cost") or 0.0)
        elif kind == "error":
            summary["errors"].append(json.dumps(event.get("error", event))[:200])
        elif kind == "text":
            summary["reply"] += part.get("text", "")
    return summary


def m365_preflight() -> None:
    try:
        urllib.request.urlopen(M365_URL, timeout=3).read(64)
    except (urllib.error.URLError, OSError) as exc:
        raise ProbeError(
            f"the m365 proxy is not reachable at {M365_URL} ({exc}). Start it: in C:\\ProgramData\\m365-copilot-proxy run "
            f"`pnpm run proxy 4141`, then re-run."
        ) from exc


def session_ids(binary: str, root: Path) -> set[str]:
    listing = subprocess.run([binary, "session", "list"], cwd=root, capture_output=True, text=True, encoding="utf-8")
    return set(re.findall(r"^(ses_\S+)", listing.stdout, re.M))


def delete_probe_sessions(binary: str, root: Path, title_prefix: str) -> int:
    listing = subprocess.run([binary, "session", "list"], cwd=root, capture_output=True, text=True, encoding="utf-8")
    ids = re.findall(r"^(ses_\S+)\s+" + re.escape(title_prefix), listing.stdout, re.M)
    for sid in ids:
        subprocess.run([binary, "session", "delete", sid], cwd=root, capture_output=True, text=True, encoding="utf-8")
    return len(ids)


def run_live(binary: str, root: Path, config: dict, args, sample_file: str) -> dict:
    candidates = [args.model] if args.model else [m for m in (config.get("small_model"), config.get("model")) if m]
    if not args.allow_paid:
        paid = [m for m in candidates if not is_free_model(m)]
        if args.model and paid:
            raise ProbeError(f"{paid[0]} is not a free model; pass --allow-paid to use it")
        candidates = [m for m in candidates if is_free_model(m)]
    if not candidates:
        raise ProbeError("no free model to try; pass --model <id> (and --allow-paid for a paid one)")
    title_prefix = "opencode-probe"
    title = f"{title_prefix} {datetime.datetime.now(datetime.timezone.utc).strftime('%Y%m%dT%H%M%SZ')}"
    prompt = f"Read the first 3 lines of {sample_file} using the read tool, then reply DONE and nothing else."
    print(f"--live: about {len(candidates)} model call(s) at most (about 20 s each); sessions titled '{title}' are deleted afterwards.")
    spent, last_error = 0.0, "no attempt"
    try:
        for model in candidates:
            if model.startswith("m365/"):
                m365_preflight()
            for attempt in (1, 2):
                cmd = [binary, "run", "--format", "json", "-m", model, "--title", title, prompt]
                if args.pure:
                    cmd.insert(2, "--pure")
                try:
                    proc = subprocess.run(cmd, cwd=root, capture_output=True, text=True, encoding="utf-8", timeout=240)
                except subprocess.TimeoutExpired:
                    last_error = f"{model}: timed out"
                    continue
                summary = parse_run_events(proc.stdout)
                spent += summary["cost"]
                if args.max_cost is not None and spent > args.max_cost:
                    raise ProbeError(f"--max-cost {args.max_cost} exceeded (spent {spent:.4f})")
                if summary["errors"] or summary["reads"] == 0 or not summary["steps"]:
                    last_error = f"{model}: " + (summary["errors"][0] if summary["errors"] else "no `read` tool event (empty response)")
                    continue  # an empty or failed run is an error to retry, never "attached nothing"
                skills = None
                skill_run = subprocess.run([binary, "debug", "skill"], cwd=root, capture_output=True, text=True, encoding="utf-8")
                try:
                    parsed = json.loads(skill_run.stdout[skill_run.stdout.find("["):]) if "[" in skill_run.stdout else None
                    skills = len(parsed) if isinstance(parsed, list) else None
                except (json.JSONDecodeError, ValueError):
                    skills = None
                return {"model": model, "baseline_input_tokens": summary["steps"][0], "cost": round(spent, 6), "skills": skills,
                        "attached": summary["attached"]}
        raise ProbeError(f"all free models failed (last: {last_error}). Re-run with --static for the no-model checks, or "
                         f"--allow-paid / --model <id> to try another model.")
    finally:
        removed = delete_probe_sessions(binary, root, title_prefix)
        if removed:
            print(f"--live: deleted {removed} probe session(s).")


# --------------------------------------------------------------------------------------------------------- reporting
def kb(n: int) -> str:
    return f"{n / 1024:.1f} KB"


def print_report(report: dict, root: Path) -> None:
    head = f"{TOOL}: OpenCode {report['version'] or '?'} (last verified {report['verified'] or 'unrecorded'}), mode {report['mode']}, {report['date']}"
    print(head)
    if report["version_warning"]:
        print("WARNING:", report["version_warning"])
    for r in report["results"]:
        names = ", ".join(r["attached"]) or "(nothing)"
        line = f"{r['status']} {r['file']}  attached: {names}  ({kb(r['bytes'])})"
        if r["missing"]:
            line += "  missing: " + ", ".join(r["missing"])
        if r["unexpected"]:
            line += "  unexpected: " + ", ".join(r["unexpected"])
        print(line)
        for w in r["warn"]:
            print("  warning:", w)
    for w in report["warnings"]:
        print("WARNING:", w)
    for title, problems in report["static"]:
        print(f"{title}: " + ("OK" if not problems else f"{len(problems)} problem(s)"))
        for p in problems:
            print("  -", p)
    if report["results"]:
        print(f"HTML comments inside the attached files (informational, they reach the model): {report['comment_bytes']} bytes in total")
    if report["live"]:
        live = report["live"]
        print(f"--live ({live['model']}): baseline {live['baseline_input_tokens']} input tokens (machine-specific), "
              f"{live['skills'] if live['skills'] is not None else '?'} skills, cost {live['cost']}")
    print(f"{report['failed']} check(s) failed.")


def build_parser() -> argparse.ArgumentParser:
    ap = argparse.ArgumentParser(description="Check which instruction files OpenCode attaches (see the module docstring).",
                                 epilog="Exit codes: 0 all checks passed, 1 a check failed, 2 tool error.")
    ap.add_argument("--static", action="store_true", help="only the checks that need no OpenCode (subfolder AGENTS.md, instructions globs)")
    ap.add_argument("--live", action="store_true", help="also run one model call for the token baseline (free models unless --allow-paid)")
    ap.add_argument("--only", metavar="PATH", help="probe a single repo-relative file")
    ap.add_argument("--dir", metavar="DIR", help="repo/worktree to probe (default: the checkout this script lives in)")
    ap.add_argument("--json", action="store_true", help="machine-readable output")
    ap.add_argument("--pure", action="store_true", help="run OpenCode without plugins (compare the attach set with and without)")
    ap.add_argument("--model", help="--live: model id, e.g. opencode/nemotron-3.5-lightning-free or m365/gpt-5.5-think-deeper")
    ap.add_argument("--allow-paid", action="store_true", help="--live: allow a paid model")
    ap.add_argument("--max-cost", type=float, default=None, help="--live: abort when the summed cost exceeds this (USD)")
    ap.add_argument("--selftest", action="store_true", help="run the built-in fixture tests (no OpenCode needed)")
    return ap


def main(argv=None) -> int:
    args = build_parser().parse_args(argv)
    if args.selftest:
        return selftest()
    root = Path(args.dir).resolve() if args.dir else Path(__file__).resolve().parent.parent
    if not (root / ".opencode" / "opencode.json").is_file():
        print(f"{TOOL}: {root} does not look like this repo (no .opencode/opencode.json); run from the repo root or pass --dir.")
        return 2
    config = json.loads((root / ".opencode" / "opencode.json").read_text(encoding="utf-8"))
    static = run_static(root)
    report = {"version": None, "verified": verified_version(root), "version_warning": None, "mode": "static" if args.static else "model-free",
              "date": datetime.date.today().isoformat(), "results": [], "static": static, "live": None, "comment_bytes": 0, "failed": 0, "warnings": []}
    failed = sum(1 for _, problems in static if problems)

    if not args.static:
        binary = find_opencode()
        if binary is None:
            print(not_found_message(TOOL))
            return 2
        report["version"] = opencode_version(binary)
        if report["verified"] and report["version"] and report["version"] != report["verified"]:
            report["version_warning"] = (f"OpenCode is {report['version']} but the facts table was last verified on {report['verified']}; "
                                         f"re-check the rows in .opencode/README.md")
        try:
            tracked = tracked_files(root)
            positives, negatives = choose_probe_files(tracked)
            files = [args.only] if args.only else positives + negatives
            env, tmp = isolated_env()
            sessions_before = session_ids(binary, root)
            try:
                for f in files:
                    report["results"].append(probe_one(binary, root, f, env, args.pure))
            finally:
                shutil.rmtree(tmp, ignore_errors=True)
            report["comment_bytes"] = sum(r["comment_bytes"] for r in report["results"])
            new_sessions = session_ids(binary, root) - sessions_before
            if new_sessions:
                report["warnings"].append(f"{len(new_sessions)} new session(s) appeared in your real OpenCode history during the probe "
                                          f"(another OpenCode run in parallel, or XDG isolation stopped working; fact V15)")
            failed += sum(1 for r in report["results"] if r["status"] == "FAIL")
            if args.live:
                sample = next((p for p in positives if "capabilities/" in p), positives[0] if positives else "Cargo.toml")
                report["live"] = run_live(binary, root, config, args, sample)
        except ProbeError as exc:
            print(f"{TOOL}: {exc}")
            return 2
    report["failed"] = failed
    if args.json:
        print(json.dumps(report, indent=1))
    else:
        print_report(report, root)
    return 1 if failed else 0


# --------------------------------------------------------------------------------------------------------------- selftest
def selftest() -> int:
    ok = 0

    def check(cond, label):
        nonlocal ok
        if not cond:
            print("SELFTEST FAIL:", label)
            raise SystemExit(1)
        ok += 1

    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        for rel_path, text in {
            "CLAUDE.md": "root", "a/CLAUDE.md": "a", "a/b/CLAUDE.md": "ab", "a/b/file.rs": "", "a/x.rs": "", "c/file.rs": "",
            "d/AGENTS.md": "@CLAUDE.md", "d/CLAUDE.md": "d", "d/z.rs": "", "e/CONTEXT.md": "e", "e/y.rs": "",
            ".claude/rules/r.md": "stub", ".opencode/opencode.json": json.dumps({"instructions": ["CLAUDE.md"]}),
        }.items():
            target = root / rel_path
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text(text, encoding="utf-8")
        check(expected_attach(root, "a/b/file.rs") == ["a/b/CLAUDE.md", "a/CLAUDE.md"], "nearest-first ancestors")
        check(expected_attach(root, "a/x.rs") == ["a/CLAUDE.md"], "file directly in a folder")
        check(expected_attach(root, "c/file.rs") == [], "folder without an instruction file attaches nothing")
        check(expected_attach(root, "d/z.rs") == ["d/AGENTS.md"], "AGENTS.md wins over CLAUDE.md (OpenCode shadowing)")
        check(expected_attach(root, "e/y.rs") == ["e/CONTEXT.md"], "CONTEXT.md is recognised")
        check(expected_attach(root, "CLAUDE.md") == [], "root files are never attached")
        problems = sync.check_no_subfolder_instruction_files(root)
        check(len(problems) == 2 and any("d/AGENTS.md" in p for p in problems) and any("e/CONTEXT.md" in p for p in problems), "static scan finds the stubs")
        check(not sync.check_instructions_not_matching_rules({"instructions": ["CLAUDE.md"]}, root), "plain instructions entry is fine")
        check(len(sync.check_instructions_not_matching_rules({"instructions": [".claude/rules/*.md"]}, root)) == 1, "rules glob is flagged")
        pos, neg = choose_probe_files(["CLAUDE.md", "a/CLAUDE.md", "a/x.rs", "Cargo.toml", "docs/dev/p.md", ".claude/rules/r.md"])
        check(pos == ["a/x.rs"] and neg == ["Cargo.toml", "docs/dev/p.md", ".claude/rules/r.md"], "probe file selection")

    block = "<path>x</path>\n\n<system-reminder>\nInstructions from: C:\\r\\a\\CLAUDE.md\nbody <!-- b:1 -->\n\n\nInstructions from: C:\\r\\CLAUDE.md\nparent\n\n</system-reminder>"
    data = {"tool": "read", "result": {"output": block, "metadata": {"loaded": ["C:\\r\\a\\CLAUDE.md", "C:\\r\\CLAUDE.md"]}}}
    loaded, blocks = attachments(parse_json_text("noise line\n\ufeff" + json.dumps(data)))
    check(len(loaded) == 2 and [p for p, _ in blocks] == loaded, "debug JSON parse and cross-check")
    check(sum(len(m.group(0)) for _, b in blocks for m in COMMENT_RE.finditer(b)) == len("<!-- b:1 -->"), "HTML comment bytes counted")
    events = "\n".join(json.dumps(e) for e in [
        {"type": "tool_use", "part": {"tool": "read", "state": {"output": "Instructions from: C:\\r\\a\\CLAUDE.md\nx"}}},
        {"type": "step_finish", "part": {"tokens": {"input": 31849}, "cost": 0.004}},
        {"type": "step_finish", "part": {"tokens": {"input": 31900}, "cost": 0.001}},
        {"type": "text", "part": {"text": "DONE"}}])
    summary = parse_run_events(events)
    check(summary["reads"] == 1 and summary["steps"][0] == 31849 and abs(summary["cost"] - 0.005) < 1e-9 and summary["reply"] == "DONE", "run events parse")
    check(parse_run_events(json.dumps({"type": "error", "error": {"name": "APIError"}}))["errors"], "errors are surfaced")
    check(parse_run_events("")["reads"] == 0, "no read event is detectable")
    check(is_free_model("openrouter/x/y:free") and is_free_model("opencode/n-free") and is_free_model("m365/gpt-5.5-think-deeper")
          and not is_free_model("openrouter/z-ai/glm-5.3-flash"), "free-model rule")
    check(VERIFIED_RE.search("x <!-- opencode-verified-version: 1.18.33 -->").group(1) == "1.18.33", "verified-version marker")
    env, tmp = isolated_env()
    check(env["XDG_DATA_HOME"] == tmp and os.path.isdir(tmp), "isolated env")
    shutil.rmtree(tmp, ignore_errors=True)
    msg = not_found_message(TOOL)
    check(same("A/B.md", "a/b.md") == (os.name == "nt") or os.name != "nt", "case-insensitive compare only on Windows")
    check("nvs use 24.21" in msg and "--static" in msg and "same terminal" in msg, "not-found message")
    print(f"selftest ok ({ok} checks)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
