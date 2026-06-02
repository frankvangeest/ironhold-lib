import http.server
import socketserver

PORT = 8000

class NoCacheHandler(http.server.SimpleHTTPRequestHandler):
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

with socketserver.TCPServer(("", PORT), NoCacheHandler) as httpd:
    httpd.allow_reuse_address = True
    print(f"Serving at http://localhost:{PORT}  (no-cache)")
    httpd.serve_forever()
