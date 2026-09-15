import http.server
import mimetypes

mimetypes.add_type("application/wasm", ".wasm")

PORT = 8000

class NoCacheHandler(http.server.SimpleHTTPRequestHandler):
    # HTTP/1.1 (with SimpleHTTPRequestHandler's own Content-Length on every response) enables
    # keep-alive, so a scene's many concurrent asset fetches reuse connections instead of each
    # opening a fresh one -- see ThreadingServer below for why this matters together.
    protocol_version = "HTTP/1.1"

    def end_headers(self):
        self.send_header("Cache-Control", "no-cache, no-store, must-revalidate")
        self.send_header("Pragma", "no-cache")
        self.send_header("Expires", "0")
        # Required for cross-origin isolation — enables SharedArrayBuffer and
        # allows wgpu to select the WebGPU backend instead of falling back to WebGL2.
        self.send_header("Cross-Origin-Opener-Policy", "same-origin")
        self.send_header("Cross-Origin-Embedder-Policy", "require-corp")
        super().end_headers()

    def log_message(self, format, *args):
        # Suppress per-request noise; print only errors
        if args and str(args[1]) not in ("200"):
            super().log_message(format, *args)

class ThreadingServer(http.server.ThreadingHTTPServer):
    # A Bevy WASM scene load fires many concurrent fetch()es at once (models, textures, audio,
    # preloaded scenes) -- easily more than a single-threaded server's default 5-deep listen()
    # backlog, which Windows responds to by RSTing the excess SYNs (ERR_CONNECTION_REFUSED in the
    # browser) rather than queuing them like Linux does. ThreadingHTTPServer (one thread per
    # request, daemon_threads=True, and — unlike plain socketserver.TCPServer —
    # allow_reuse_address=1 by default) plus a much deeper backlog closes this off entirely.
    request_queue_size = 64

with ThreadingServer(("", PORT), NoCacheHandler) as httpd:
    print(f"Serving at http://localhost:{PORT}  (no-cache)")
    httpd.serve_forever()
