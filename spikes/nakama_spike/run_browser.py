"""Drives the spike page in a visible Chromium (planning/features/nakama_backend_spike.md, S3b/S4b).

Serves this folder on http://localhost:8002 (not 8000/8001, other work uses those), opens index.html with the Nakama endpoint and
server key from the environment / key file, waits for the in-page report and prints it. Nothing secret is printed.

    NAKAMA_URL=http://<host>:7350 python run_browser.py
"""
import functools
import http.server
import os
import pathlib
import threading

from playwright.sync_api import sync_playwright

HERE = pathlib.Path(__file__).resolve().parent
PORT = 8002


def server_key() -> str:
    key = os.environ.get("NAKAMA_SERVER_KEY", "").strip()
    if key:
        return key
    path = pathlib.Path(os.environ.get("NAKAMA_SERVER_KEY_FILE", pathlib.Path.home() / ".nakama_spike_server_key"))
    return path.read_text().strip()


class Quiet(http.server.SimpleHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def end_headers(self):
        self.send_header("Cache-Control", "no-store")
        super().end_headers()


def main() -> None:
    base = os.environ.get("NAKAMA_URL", "http://localhost:7350")
    handler = functools.partial(Quiet, directory=str(HERE))
    httpd = http.server.ThreadingHTTPServer(("127.0.0.1", PORT), handler)
    threading.Thread(target=httpd.serve_forever, daemon=True).start()
    args = ["--disable-background-timer-throttling", "--disable-renderer-backgrounding"]
    with sync_playwright() as p:
        browser = p.chromium.launch(headless=False, args=args)
        page = browser.new_page()
        failures = []
        page.on("console", lambda m: failures.append(m.text) if m.type == "error" else None)
        page.goto(f"http://localhost:{PORT}/index.html?base={base}&key={server_key()}")
        page.wait_for_function("window.__report !== undefined", timeout=60000)
        print(page.evaluate("window.__report"))
        for f in failures:
            # console errors can carry the request URL; the socket URL has the session token, so strip query strings.
            print("console error:", f.split("?")[0][:200])
        browser.close()
    httpd.shutdown()


if __name__ == "__main__":
    main()
