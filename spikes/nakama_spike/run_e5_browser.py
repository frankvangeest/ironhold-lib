"""E5 from browser pages (planning/features/nakama_backend_spike.md): calibration C3 and C4 and the browser grid cells.

    NAKAMA_URL=http://<host>:7350 python run_e5_browser.py c3 [secs] [load_hz] [probe_hz] [payload]   browser <-> browser
    NAKAMA_URL=http://<host>:7350 python run_e5_browser.py c4 [secs] [load_hz] [probe_hz] [payload]   native <-> browser

Serves this folder on http://localhost:8002, drives visible Chromium windows launched with the background-throttling switches (a
hidden or occluded tab throttles timers and would be measured as "Nakama is slow"), prints round-trip statistics. Each page is its
own browser context, so each gets its own window; keep both visible and do not touch them during a run. Nothing secret is printed.
"""
import functools
import http.server
import json
import math
import os
import pathlib
import subprocess
import sys
import threading

from playwright.sync_api import sync_playwright

from run_browser import HERE, PORT, Quiet, server_key

NATIVE_EXE = pathlib.Path(os.environ.get("CARGO_TARGET_DIR", "")) / "release" / "nakama_spike.exe"
BASE = os.environ.get("NAKAMA_URL", "http://localhost:7350")


def percentile(sorted_vals, p):
    rank = min(max(math.ceil(p * len(sorted_vals)), 1), len(sorted_vals))
    return sorted_vals[rank - 1]


def summarize(label, result):
    if result.get("error") and not result.get("rtts"):
        print(f"  {label}: ERROR {result['error']}")
        return
    r = sorted(result["rtts"])
    n = len(r)
    p99 = f"{percentile(r, 0.99):.2f}" if n >= 2000 else f"n/a (n={n} < 2000)"
    print(
        f"  {label}: n={n} min {r[0]:.2f} p50 {percentile(r, 0.5):.2f} mean {sum(r) / n:.2f} p95 {percentile(r, 0.95):.2f} "
        f"p99 {p99} max {r[-1]:.2f} ms | longest receive gap {result['max_gap_ms']:.0f} ms | send lateness mean "
        f"{result['late_mean_ms']:.2f} max {result['late_max_ms']:.1f} ms | probes sent {result['sent_probes']} load sent "
        f"{result['sent_load']} | error: {result['error']}"
    )


def relay_url(match, peers, load, probe, size, secs):
    return (
        f"http://localhost:{PORT}/index.html?mode=relay&base={BASE}&key={server_key()}&match={match}"
        f"&peers={peers}&load={load}&probe={probe}&size={size}&secs={secs}"
    )


def run_cell(browser, pages, load, probe, size, secs):
    """One native host plus `pages` browser pages (each its own window). Returns (page results, host lines)."""
    env = dict(os.environ, E5_EXTERNAL_PEERS=str(pages), NAKAMA_URL=BASE)
    host = subprocess.Popen(
        [str(NATIVE_EXE), "e5", str(pages + 1), str(load), str(probe), str(size), str(secs)],
        stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, env=env,
    )
    match = None
    for line in host.stdout:
        if line.startswith("MATCH_ID="):
            match = line.strip().split("=", 1)[1]
            break
    if not match:
        raise SystemExit("the native host did not print a match id")
    ctxs = [browser.new_context() for _ in range(pages)]
    ps = [c.new_page() for c in ctxs]
    for pg in ps:
        pg.goto(relay_url(match, pages, load, probe, size, secs))
    timeout = (secs + 120) * 1000
    results = []
    for pg in ps:
        pg.wait_for_function("window.__relay !== undefined", timeout=timeout)
        results.append(json.loads(pg.evaluate("window.__relay")))
    rest, _ = host.communicate(timeout=180)
    for c in ctxs:
        c.close()
    return results, [l for l in rest.splitlines() if "RTT ms" in l or "disconnects" in l or "longest gap" in l]


def grid(browser, secs, reps, size, probe):
    """Browser grid: 1 native host + 1 or 3 pages (2 or 4 clients), load 20 and 30 Hz, `reps` repeats."""
    print(f"browser grid: {size} B, probes {probe} Hz, {secs} s per run, {reps} repeats")
    for pages in (1, 3):
        for hz in (20, 30):
            for rep in range(1, reps + 1):
                print(f"cell {pages + 1} clients / {hz} Hz / rep {rep}")
                results, host_lines = run_cell(browser, pages, hz, probe, size, secs)
                pooled = {
                    "rtts": [x for r in results for x in r.get("rtts", [])],
                    "max_gap_ms": max(r.get("max_gap_ms", 0) for r in results),
                    "late_mean_ms": sum(r.get("late_mean_ms", 0) for r in results) / len(results),
                    "late_max_ms": max(r.get("late_max_ms", 0) for r in results),
                    "sent_probes": sum(r.get("sent_probes", 0) for r in results),
                    "sent_load": sum(r.get("sent_load", 0) for r in results),
                    "error": "; ".join(str(r["error"]) for r in results if r.get("error")) or None,
                }
                if pooled["rtts"]:
                    summarize("pages pooled", pooled)
                else:
                    print("  no samples:", pooled["error"])
                for l in host_lines:
                    print("  native host:" + l)


def main() -> None:
    mode = sys.argv[1] if len(sys.argv) > 1 else "c3"
    if mode == "grid":
        # python run_e5_browser.py grid [secs=65] [reps=3] [size=400] [probe_hz=10]
        a = sys.argv[2:]
        httpd = http.server.ThreadingHTTPServer(("127.0.0.1", PORT), functools.partial(Quiet, directory=str(HERE)))
        threading.Thread(target=httpd.serve_forever, daemon=True).start()
        flags = [
            "--disable-background-timer-throttling",
            "--disable-renderer-backgrounding",
            "--disable-backgrounding-occluded-windows",
            "--disable-features=CalculateNativeWinOcclusion",
        ]
        with sync_playwright() as p:
            browser = p.chromium.launch(headless=False, args=flags)
            grid(browser, int(a[0]) if a else 65, int(a[1]) if len(a) > 1 else 3, int(a[2]) if len(a) > 2 else 400,
                 float(a[3]) if len(a) > 3 else 10)
            browser.close()
        httpd.shutdown()
        return
    secs = int(sys.argv[2]) if len(sys.argv) > 2 else 90
    load = float(sys.argv[3]) if len(sys.argv) > 3 else 0
    probe = float(sys.argv[4]) if len(sys.argv) > 4 else 30
    size = int(sys.argv[5]) if len(sys.argv) > 5 else 64
    print(f"{mode}: load {load} Hz, probes {probe} Hz, payload {size} B, {secs} s (warm-up 5 s discarded)")

    httpd = http.server.ThreadingHTTPServer(("127.0.0.1", PORT), functools.partial(Quiet, directory=str(HERE)))
    threading.Thread(target=httpd.serve_forever, daemon=True).start()
    flags = [
        "--disable-background-timer-throttling",
        "--disable-renderer-backgrounding",
        "--disable-backgrounding-occluded-windows",
        "--disable-features=CalculateNativeWinOcclusion",
    ]
    native = None
    with sync_playwright() as p:
        browser = p.chromium.launch(headless=False, args=flags)
        done = f"window.__relay !== undefined"
        timeout = (secs + 90) * 1000
        if mode == "c3":
            a = browser.new_context().new_page()
            a.goto(relay_url("", 1, load, probe, size, secs))
            a.wait_for_function("window.__matchId !== undefined", timeout=30000)
            match = a.evaluate("window.__matchId")
            b = browser.new_context().new_page()
            b.goto(relay_url(match, 1, load, probe, size, secs))
            a.wait_for_function(done, timeout=timeout)
            b.wait_for_function(done, timeout=timeout)
            summarize("browser A (creator)", json.loads(a.evaluate("window.__relay")))
            summarize("browser B (joiner) ", json.loads(b.evaluate("window.__relay")))
        elif mode == "c4":
            env = dict(os.environ, E5_EXTERNAL_PEERS="1", NAKAMA_URL=BASE)
            native = subprocess.Popen(
                [str(NATIVE_EXE), "e5", "2", str(load), str(probe), str(size), str(secs)],
                stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, env=env,
            )
            match = None
            for line in native.stdout:
                if line.startswith("MATCH_ID="):
                    match = line.strip().split("=", 1)[1]
                    break
            if not match:
                raise SystemExit("the native host did not print a match id")
            page = browser.new_context().new_page()
            page.goto(relay_url(match, 1, load, probe, size, secs))
            page.wait_for_function(done, timeout=timeout)
            summarize("browser (joiner)", json.loads(page.evaluate("window.__relay")))
            rest, _ = native.communicate(timeout=120)
            for line in rest.splitlines():
                if "RTT ms" in line or "disconnects" in line or "longest gap" in line:
                    print("  native host:" + line)
        else:
            raise SystemExit("mode must be c3 or c4")
        browser.close()
    httpd.shutdown()


if __name__ == "__main__":
    main()
