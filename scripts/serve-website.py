#!/usr/bin/env python3
"""Serve website/ locally and reload open pages whenever a file in it changes.

    scripts/serve-website.py [PORT]      # default 8765

Pages get a small EventSource snippet injected; nothing in website/ changes.
"""
import http.server
import os
import socketserver
import sys
import time

ROOT = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "website")
PORT = int(sys.argv[1]) if len(sys.argv) > 1 else 8765
SNIPPET = b"<script>new EventSource('/__reload').onmessage = () => location.reload();</script>"


def newest_mtime():
    newest = 0.0
    for folder, dirs, files in os.walk(ROOT):
        dirs[:] = [d for d in dirs if not d.startswith(".")]
        for name in files:
            try:
                newest = max(newest, os.stat(os.path.join(folder, name)).st_mtime)
            except OSError:
                pass
    return newest


class Handler(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, directory=ROOT, **kwargs)

    def end_headers(self):
        self.send_header("Cache-Control", "no-store")
        super().end_headers()

    def log_message(self, *args):
        pass

    def do_GET(self):
        if self.path == "/__reload":
            return self.watch()
        path = self.translate_path(self.path)
        if os.path.isdir(path):
            path = os.path.join(path, "index.html")
        if path.endswith(".html") and os.path.isfile(path):
            with open(path, "rb") as f:
                body = f.read().replace(b"</body>", SNIPPET + b"</body>")
            self.send_response(200)
            self.send_header("Content-Type", "text/html; charset=utf-8")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            return
        super().do_GET()

    def watch(self):
        """Server-sent events: one message when anything under website/ changes."""
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.end_headers()
        seen, ticks = newest_mtime(), 0
        try:
            while True:
                time.sleep(0.3)
                if newest_mtime() != seen:
                    time.sleep(0.2)  # let a burst of saves finish
                    self.wfile.write(b"data: reload\n\n")
                    self.wfile.flush()
                    return
                ticks += 1
                if ticks % 50 == 0:  # keep the connection alive
                    self.wfile.write(b": ping\n\n")
                    self.wfile.flush()
        except (BrokenPipeError, ConnectionResetError):
            pass


class Server(socketserver.ThreadingMixIn, http.server.HTTPServer):
    daemon_threads = True
    allow_reuse_address = True


if __name__ == "__main__":
    print(f"http://127.0.0.1:{PORT}/  (reloads when website/ changes)", flush=True)
    Server(("127.0.0.1", PORT), Handler).serve_forever()
