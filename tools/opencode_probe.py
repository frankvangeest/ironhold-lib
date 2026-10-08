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
IMAGE_EXTS = {".png", ".jpg", ".jpeg", ".webp"}
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
    try:
        return os.path.relpath(str(path), str(root)).replace("\\", "/")
    except ValueError:  # a different drive on Windows
        return str(path).replace("\\", "/")


def same(a: str, b: str) -> bool:
    return os.path.normcase(a) == os.path.normcase(b)


def expected_attach(root: Path, rel_file: str) -> list[str]:
    """Repo-relative instruction files OpenCode should attach for `rel_file`, nearest first (root excluded)."""
    root_n = norm(root)
    folder = (root / rel_file).parent
    found = []
    # The string-prefix guard mirrors how OpenCode decides a file is "inside the project" (fact V14): a sibling worktree whose
    # folder name starts with the primary checkout's name counts as inside it. Change both together or neither.
    while norm(folder) != root_n and norm(folder).startswith(root_n):
        chosen = instruction_file_in(folder)
        if chosen is not None:
            found.append(relposix(root, chosen))
        folder = folder.parent
    return found


def tracked_files(root: Path) -> list[str]:
    try:
        out = subprocess.run(["git", "ls-files", "-z"], cwd=root, capture_output=True, text=True, encoding="utf-8")
    except OSError as exc:
        raise ProbeError(f"could not run git ({exc}); the probe needs git to list the tracked files") from exc
    if out.returncode != 0:
        raise ProbeError(f"{root} is not a git checkout (git ls-files failed); pass --dir <repo root>")
    return [f for f in out.stdout.split("\0") if f]


def choose_probe_files(tracked: list[str]) -> tuple[list[str], list[str], list[str]]:
    """(positives, negatives, unprobed folders).

    One readable file per folder that owns a CLAUDE.md (root excluded), preferring text, then a PNG/JPEG/WebP image; files that
    attach nothing as negatives. OpenCode's read tool refuses other binaries ("Cannot read binary file", checked for .glb and
    .avif), so a folder holding only those cannot be probed through a read and is reported, not silently skipped.
    """
    tracked_set = set(tracked)
    folders = sorted({str(Path(f).parent.as_posix()) for f in tracked if Path(f).name == "CLAUDE.md" and Path(f).parent != Path(".")})
    positives: list[str] = []
    unprobed: list[str] = []
    for folder in folders:
        def usable(exts):
            return lambda f: Path(f).suffix.lower() in exts and Path(f).name not in INSTR_NAMES
        picks = []
        for exts in (TEXT_EXTS, IMAGE_EXTS):
            ok = usable(exts)
            picks = sorted(f for f in tracked if Path(f).parent.as_posix() == folder and ok(f)) or sorted(
                f for f in tracked if f.startswith(folder + "/") and ok(f))
            if picks:
                break
        if picks:
            positives.append(picks[0])
        else:
            unprobed.append(folder)
    if "crates/ironhold_core/src/lib.rs" in tracked_set:
        positives.append("crates/ironhold_core/src/lib.rs")  # the file that must see ONLY its own folder's CLAUDE.md
    negatives = [f for f in ("Cargo.toml", "crates/ironhold_web/src/lib.rs") if f in tracked_set]
    negatives += sorted(f for f in tracked if f.startswith("docs/dev/") and f.endswith(".md"))[:1]
    negatives += sorted(f for f in tracked if f.startswith(".claude/rules/") and f.endswith(".md"))[:1]
    seen: set[str] = set()
    return [f for f in positives if not (f in seen or seen.add(f))], negatives, unprobed


# --------------------------------------------------------------------------------------------- the model-free debug read
def normalize_only(root: Path, value: str) -> str:
    """`--only` accepts backslashes (PowerShell tab completion), `./` prefixes and absolute paths inside the repo."""
    p = Path(value)
    if p.is_absolute():
        try:
            return relposix(root, p)
        except ValueError as exc:  # another drive on Windows
            raise ProbeError(f"--only {value} is not inside {root}") from exc
    cleaned = value.replace("\\", "/")
    while cleaned.startswith("./"):
        cleaned = cleaned[2:]
    return cleaned


def parse_paths(text: str) -> dict:
    """`opencode debug paths` prints `name   path` lines."""
    out = {}
    for line in text.splitlines():
        parts = line.strip().split(None, 1)
        if len(parts) == 2:
            out[parts[0]] = parts[1].strip()
    return out


def check_isolation(binary: str, root: Path, env: dict, tmp: str) -> None:
    """Positive proof that the XDG override is honoured (fact V15): `debug paths` must show data and state under `tmp`."""
    proc = subprocess.run([binary, "debug", "paths"], cwd=root, env=env, capture_output=True, text=True, encoding="utf-8")
    paths = parse_paths(proc.stdout)
    base = os.path.normcase(os.path.normpath(tmp))
    bad = [k for k in ("data", "state") if not os.path.normcase(os.path.normpath(paths.get(k, ""))).startswith(base)]
    if proc.returncode != 0 or bad:
        raise ProbeError("OpenCode did not honour XDG_DATA_HOME/XDG_STATE_HOME (`opencode debug paths` shows "
                         f"{', '.join(bad) or 'no data'} outside {tmp}); a probe run could add sessions to your real history (fact V15)")


def isolated_env() -> tuple[dict, str]:
    """Environment whose OpenCode data/state live in a temp dir, so debug sessions never reach the real history."""
    tmp = tempfile.mkdtemp(prefix="opencode_probe_xdg_")
    env = os.environ.copy()
    env["XDG_DATA_HOME"] = tmp
    env["XDG_STATE_HOME"] = tmp
    return env, tmp


def parse_json_text(text: str) -> dict:
    """The first JSON object in `text`; tolerates log lines before or after it (a plugin could print some)."""
    text = text.lstrip("\ufeff")
    decoder = json.JSONDecoder()
    start = text.find("{")
    last_error = None
    while start >= 0:
        try:
            value, _ = decoder.raw_decode(text[start:])
            if isinstance(value, dict):
                return value
        except json.JSONDecodeError as exc:
            last_error = exc
        start = text.find("{", start + 1)
    if last_error is None:
        raise ProbeError("OpenCode printed no JSON")
    raise ProbeError(f"could not parse OpenCode's JSON output: {last_error}")


def attachments(data: dict) -> tuple[list[str], list[tuple[str, str]]]:
    """(metadata.loaded, [(path, body)] parsed from the `Instructions from:` blocks of the tool output), absolute paths as printed."""
    result = data.get("result") or {}
    meta = result.get("metadata")
    if not isinstance(meta, dict) or "loaded" not in meta:
        # `loaded` is [] (present) even when nothing attaches, so a missing key means OpenCode's debug interface changed.
        raise ProbeError("debug interface changed: no result.metadata.loaded in the read output (fact V6 in .opencode/README.md); "
                         "re-verify the facts table, or use --live")
    loaded = list(meta["loaded"])
    blocks = [(m.group(1).strip(), m.group(2)) for m in BLOCK_RE.finditer(result.get("output", ""))]
    return loaded, blocks


def debug_read(binary: str, root: Path, rel: str, env: dict, pure: bool, timeout: int = 90) -> dict:
    # The real opencode.exe takes plain JSON through the argument list (handles quotes and spaces in a path). The npm `.cmd`/`.ps1`
    # shims re-parse their arguments (double quotes mangled, backslashes eaten), so through a shim use a JS literal in single quotes.
    if Path(binary).suffix.lower() in (".cmd", ".bat", ".ps1"):
        if "'" in rel:
            raise ProbeError(f"{rel} contains a single quote and OpenCode is only reachable through its .cmd shim; "
                             "set OPENCODE_BIN to opencode.exe (fact V9)")
        params = "{filePath:'%s',limit:2}" % rel
    else:
        params = json.dumps({"filePath": rel, "limit": 2})
    cmd = [binary, "debug", "agent", "build", "--tool", "read", "--params", params] + (["--pure"] if pure else [])
    try:
        proc = subprocess.run(cmd, cwd=root, env=env, capture_output=True, text=True, encoding="utf-8", timeout=timeout)
    except subprocess.TimeoutExpired as exc:
        raise ProbeError(f"`opencode debug agent` timed out after {timeout}s on {rel}") from exc
    except OSError as exc:
        raise ProbeError(f"could not run {binary}: {exc}") from exc
    if proc.returncode != 0:
        detail = re.sub(r"\x1b\[[0-9;]*m", "", (proc.stderr or proc.stdout)).strip()[:300]  # strip ANSI colour codes
        raise ProbeError(f"`opencode debug agent` exited {proc.returncode} on {rel}: {detail}")
    return parse_json_text(proc.stdout)


def probe_one(binary, root: Path, rel: str, env: dict, pure: bool) -> dict:
    data = debug_read(binary, root, rel, env, pure)
    loaded_abs, blocks = attachments(data)
    loaded = [relposix(root, p) for p in loaded_abs]
    expected = expected_attach(root, rel)
    missing = [e for e in expected if not any(same(e, l) for l in loaded)]
    unexpected = [l for l in loaded if not any(same(l, e) for e in expected)]
    warn = []
    # Every path in metadata.loaded must have its own `Instructions from:` block. Extra headers are fine: a file body may quote
    # OpenCode's own format, which must not be mistaken for an interface change.
    blocks = [(p, body) for p, body in blocks if any(same(relposix(root, p), l) for l in loaded)]
    without_block = [l for l in loaded if not any(same(relposix(root, p), l) for p, _ in blocks)]
    if without_block:
        raise ProbeError(f"debug interface changed: {', '.join(without_block)} is in metadata.loaded but has no "
                         f"'Instructions from:' text for {rel} (fact V6)")
    size = sum(len(body.encode("utf-8")) for _, body in blocks)
    comment_bytes = sum(len(m.group(0).encode("utf-8")) for _, body in blocks for m in COMMENT_RE.finditer(body))
    return {"file": rel, "expected": expected, "attached": loaded, "missing": missing, "unexpected": unexpected,
            "status": "PASS" if not missing and not unexpected else "FAIL", "bytes": size, "comment_bytes": comment_bytes, "warn": warn}


# ------------------------------------------------------------------------------------------------------- static checks
def run_static(root: Path, config: dict) -> list[tuple[str, list[str]]]:
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
    # m365/... goes through the local M365 Copilot proxy on the organisation's licence: no per-token cost (decision (a), 2026-10-08).
    # Unknown ids fail safe: an id without a `:free`/`-free` suffix counts as paid.
    return model_id.endswith(":free") or model_id.endswith("-free") or model_id.startswith("m365/")


def parse_run_events(text: str) -> dict:
    """Summarise `opencode run --format json` output: read events, per-step tokens, summed cost, errors, reply text."""
    summary = {"reads": 0, "attached": [], "steps": [], "cost": 0.0, "errors": [], "reply": "", "sessions": set()}
    for line in text.splitlines():
        line = line.strip()
        if not line.startswith("{"):
            continue
        try:
            event = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(event.get("sessionID"), str):
            summary["sessions"].add(event["sessionID"])
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


def delete_sessions(binary: str, root: Path, ids) -> int:
    """Delete exactly these session ids (the ones this run created, taken from its own `sessionID` events), never by title:
    the session list is shared by every worktree of the repo, so a title match could hit someone else's session."""
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
    title = f"opencode-probe {datetime.datetime.now(datetime.timezone.utc).strftime('%Y%m%dT%H%M%SZ')}"
    prompt = f"Read the first 3 lines of {sample_file} using the read tool, then reply DONE and nothing else."
    print(f"--live: about {len(candidates)} model call(s) at most (about 20 s each); the session(s) it creates ('{title}') are deleted afterwards.", file=sys.stderr)
    spent, last_error = 0.0, "no attempt"
    created: set[str] = set()
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
                created |= summary["sessions"]
                spent += summary["cost"]
                # The cap is checked after a call has already been paid for, so one call can overshoot it by its own cost.
                if args.max_cost is not None and spent > args.max_cost:
                    raise ProbeError(f"--max-cost {args.max_cost} exceeded (spent {spent:.4f})")
                if proc.returncode != 0 or summary["errors"] or summary["reads"] == 0 or not summary["steps"]:
                    last_error = f"{model}: " + (summary["errors"][0] if summary["errors"] else
                                                 f"opencode run exited {proc.returncode}" if proc.returncode != 0 else
                                                 "no `read` tool event (empty response)")
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
        removed = delete_sessions(binary, root, created)
        if removed:
            print(f"--live: deleted {removed} probe session(s).", file=sys.stderr)
        elif not created:
            print("--live: no session id was reported, so nothing was deleted; look for sessions titled "
                  f"'{title}' in `opencode session list`.", file=sys.stderr)


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
    parser = build_parser()
    args = parser.parse_args(argv)
    if args.selftest:
        return selftest()
    if args.live and args.static:
        parser.error("--live calls a model and --static never touches OpenCode; use one or the other")
    root = Path(args.dir).resolve() if args.dir else Path(__file__).resolve().parent.parent
    config_path = root / ".opencode" / "opencode.json"
    if not config_path.is_file():
        print(f"{TOOL}: {root} does not look like this repo (no .opencode/opencode.json); run from the repo root or pass --dir.")
        return 2
    try:
        config = json.loads(config_path.read_text(encoding="utf-8"))
    except (json.JSONDecodeError, OSError) as exc:
        print(f"{TOOL}: cannot read {config_path}: {exc}")
        return 2
    static = run_static(root, config)
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
            positives, negatives, unprobed = choose_probe_files(tracked)
            only = normalize_only(root, args.only) if args.only else None
            if only and not (root / only).is_file():
                raise ProbeError(f"--only expects a file, and {only} is not one in {root} (a directory would pass vacuously)")
            files = [only] if only else positives + negatives
            if unprobed and not only:
                report["warnings"].append("not probed (no file OpenCode's read tool can open, only binaries): " + ", ".join(unprobed))
            env, tmp = isolated_env()
            try:
                check_isolation(binary, root, env, tmp)
            except ProbeError:
                shutil.rmtree(tmp, ignore_errors=True)
                raise
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
                sample = only or next((p for p in positives if "capabilities/" in p), positives[0] if positives else "Cargo.toml")
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
        check(parse_json_text("log before\n{\"a\": 1}\nlog after {not json")["a"] == 1, "JSON tolerant of noise before and after")
        check(not sync.check_instructions_not_matching_rules({"instructions": ["CLAUDE.md"]}, root), "plain instructions entry is fine")
        check(len(sync.check_instructions_not_matching_rules({"instructions": [".claude/rules/*.md"]}, root)) == 1, "rules glob is flagged")
        pos, neg, unprobed = choose_probe_files(["CLAUDE.md", "a/CLAUDE.md", "a/x.rs", "Cargo.toml", "docs/dev/p.md", ".claude/rules/r.md",
                                                  "m/CLAUDE.md", "m/x.glb", "i/CLAUDE.md", "i/p.png"])
        check(pos == ["a/x.rs", "i/p.png"] and unprobed == ["m"], "text first, image fallback, binary-only folder reported")
        check(neg == ["Cargo.toml", "docs/dev/p.md", ".claude/rules/r.md"], "negative probe files")
        # case-insensitive shadowing names, and build output skipped only at the top level
        (root / "low").mkdir()
        (root / "low" / "agents.md").write_text("@CLAUDE.md", encoding="utf-8")
        (root / "low" / "CLAUDE.md").write_text("x", encoding="utf-8")
        (root / "target").mkdir()
        (root / "target" / "AGENTS.md").write_text("top-level build output, skipped", encoding="utf-8")
        (root / "x" / "target").mkdir(parents=True)
        (root / "x" / "target" / "AGENTS.md").write_text("nested, a real problem", encoding="utf-8")
        found = sync.check_no_subfolder_instruction_files(root)
        check(any("low/agents.md" in p for p in found), "a lowercase agents.md is flagged")
        check(not any(p.startswith("target/") for p in found) and any(p.startswith("x/target/") for p in found), "target/ skipped only at the top level")
        check(not sync.glob_matches("*.md", ".claude/rules/x.md") and sync.glob_matches("**/*.md", ".claude/rules/x.md")
              and sync.glob_matches("{CLAUDE.md,.claude/rules/*.md}", ".claude/rules/x.md")
              and sync.glob_matches(".claude/rules/**/*.md", ".claude/rules/sub/x.md") and not sync.glob_matches("CLAUDE.md", "x/CLAUDE.md"), "glob semantics")
        check(len(sync.check_instructions_not_matching_rules({"instructions": ["*.md"]}, root)) == 0, "a root-only *.md does not match rules")
        check(len(sync.check_instructions_not_matching_rules({"instructions": ["{CLAUDE.md,.claude/rules/*.md}"]}, root)) == 1, "braces are expanded")
        abs_entry = (root / ".claude" / "rules" / "r.md").as_posix()
        check(len(sync.check_instructions_not_matching_rules({"instructions": [abs_entry]}, root)) == 1, "an absolute path to a rule is flagged")
        # a file body that quotes OpenCode's own header must not be mistaken for an interface change; a loaded path with no text is
        saved = globals()["debug_read"]
        try:
            quoted = ("<path>x</path>\n\n<system-reminder>\nInstructions from: " + str(root / "a" / "CLAUDE.md") + "\nbody\n\nInstructions from: X\nquoted\n"
                      "</system-reminder>")
            globals()["debug_read"] = lambda *a, **k: {"result": {"output": quoted, "metadata": {"loaded": [str(root / "a" / "CLAUDE.md")]}}}
            check(probe_one(None, root, "a/x.rs", {}, False)["status"] == "PASS", "extra 'Instructions from:' text in a body is not an error")
            globals()["debug_read"] = lambda *a, **k: {"result": {"output": "nothing here", "metadata": {"loaded": [str(root / "a" / "CLAUDE.md")]}}}
            try:
                probe_one(None, root, "a/x.rs", {}, False)
                check(False, "a loaded path without text must be an error")
            except ProbeError:
                check(True, "loaded path without text raises")
        finally:
            globals()["debug_read"] = saved

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
    check(parse_run_events(json.dumps({"type": "step_start", "sessionID": "ses_abc", "part": {}}))["sessions"] == {"ses_abc"}, "session ids are collected from events")
    check(parse_run_events("")["reads"] == 0, "no read event is detectable")
    check(is_free_model("openrouter/x/y:free") and is_free_model("opencode/n-free") and is_free_model("m365/gpt-5.5-think-deeper")
          and not is_free_model("openrouter/z-ai/glm-5.3-flash"), "free-model rule")
    try:
        attachments({"result": {"output": "x", "metadata": {}}})
        check(False, "a missing metadata.loaded key must be an error")
    except ProbeError:
        check(True, "missing metadata.loaded raises")
    check(attachments({"result": {"output": "x", "metadata": {"loaded": []}}}) == ([], []), "an empty loaded list is fine")
    check(normalize_only(Path("."), "crates\\a\\b.rs") == "crates/a/b.rs" and normalize_only(Path("."), "./x/y.rs") == "x/y.rs", "--only normalisation")
    check(parse_paths("data       C:\\t\\opencode\nstate      C:\\t\\opencode\n")["data"] == "C:\\t\\opencode", "debug paths parse")
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
