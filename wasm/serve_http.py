#!/usr/bin/env python3
"""Static server for the wasm dev harness.

Sends the cross-origin-isolation headers the wasm build needs:
  Cross-Origin-Opener-Policy: same-origin
  Cross-Origin-Embedder-Policy: require-corp
Without them the page is not crossOriginIsolated, SharedArrayBuffer is
undefined and the gpui web platform degrades (wasm_thread cannot spawn its
background workers; Atomics.waitAsync waker loop is disabled).

Also pins the correct MIME type for .wasm (application/wasm) —
instantiateStreaming requires it — plus .webmanifest/.js/.png/.svg.

Precompressed serving: if a sibling `<file>.br` / `<file>.gz` (produced by
`node wasm/precompress.mjs`) exists and the client's Accept-Encoding allows
it, the compressed bytes are sent with `Content-Encoding` + `Vary:
Accept-Encoding` (Content-Type stays that of the ORIGINAL file). Without
siblings the raw file is served — rebuilds land live either way.

Service worker version injection: /sw.js is served with the build hash from
wasm/pkg/version.json substituted into its `var BUILD_VERSION = null;`
line, so every deploy changes the SW's bytes (the browser's update check
then drives install -> activate -> page toast -> reload). sw.js is never
served compressed and always no-store.

Everything stays `Cache-Control: no-store` (kept from run 1: live-reload
rebuilds must land without a server restart; the service worker provides
the offline cache, not HTTP caching).

Usage: serve_http.py [port]   (default 8642, binds 127.0.0.1)
"""

from __future__ import annotations

import http.server
import json
import os
import sys

DIR = os.path.dirname(os.path.abspath(__file__))

MIME_TYPES = {
    ".wasm": "application/wasm",
    ".webmanifest": "application/manifest+json",
    ".js": "text/javascript; charset=utf-8",
    ".mjs": "text/javascript; charset=utf-8",
    ".json": "application/json",
    ".png": "image/png",
    ".svg": "image/svg+xml",
    ".html": "text/html; charset=utf-8",
}

# The line serve_http.py replaces with the real version (see sw.js header).
SW_PLACEHOLDER = b"var BUILD_VERSION = null;"
SW_FILENAME = "sw.js"


class HarnessHandler(http.server.SimpleHTTPRequestHandler):
    # Keep-alive (the debug .wasm is ~370 MB; browsers appreciate it).
    protocol_version = "HTTP/1.1"

    def __init__(self, *args, **kwargs):
        super().__init__(*args, directory=DIR, **kwargs)

    def end_headers(self) -> None:
        self.send_header("Cross-Origin-Opener-Policy", "same-origin")
        self.send_header("Cross-Origin-Embedder-Policy", "require-corp")
        self.send_header("Cache-Control", "no-store")
        super().end_headers()

    def guess_type(self, path: str) -> str:
        # Strip a precompression sibling extension so `x.wasm.br` reports
        # the type of `x.wasm` (Content-Encoding carries the rest).
        for enc_ext in (".br", ".gz"):
            if path.endswith(enc_ext):
                path = path[: -len(enc_ext)]
                break
        ext = os.path.splitext(path)[1].lower()
        if ext in MIME_TYPES:
            return MIME_TYPES[ext]
        return super().guess_type(path)

    # ---- negotiation helpers -------------------------------------------

    def _accepts(self, encoding: str) -> bool:
        # RFC 9110 s12.4.2: an omitted q weight defaults to 1; q=0 means
        # "not acceptable". Duplicates keep the highest weight.
        header = self.headers.get("Accept-Encoding", "")
        best = 0.0
        for token in header.split(","):
            parts = token.strip().split(";")
            if parts[0].strip().lower() != encoding:
                continue
            q = 1.0
            for param in parts[1:]:
                param = param.strip()
                if param.lower().startswith("q="):
                    try:
                        q = float(param[2:])
                    except ValueError:
                        pass  # malformed weight: keep the default
            best = max(best, q)
        return best > 0.0

    def _serve_bytes(self, body: bytes, content_type: str) -> None:
        self.send_response(200)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        if self.command != "HEAD":
            self.wfile.write(body)

    def _serve_file(self, path: str, encoding: str | None, vary: bool) -> None:
        try:
            f = open(path, "rb")
        except OSError:
            self.send_error(404, "File not found")
            return
        with f:
            stats = os.fstat(f.fileno())
            self.send_response(200)
            self.send_header("Content-Type", self.guess_type(path))
            self.send_header("Content-Length", str(stats.st_size))
            self.send_header("Last-Modified", self.date_time_string(stats.st_mtime))
            if encoding:
                self.send_header("Content-Encoding", encoding)
            if vary:
                self.send_header("Vary", "Accept-Encoding")
            self.end_headers()
            if self.command != "HEAD":
                self.copyfile(f, self.wfile)

    def _swjs_bytes(self) -> bytes:
        with open(os.path.join(DIR, SW_FILENAME), "rb") as f:
            body = f.read()
        version = None
        try:
            with open(os.path.join(DIR, "pkg", "version.json")) as f:
                version = json.load(f).get("version")
        except (OSError, ValueError):
            pass
        if version and SW_PLACEHOLDER in body:
            injected = 'var BUILD_VERSION = "%s";' % version
            body = body.replace(SW_PLACEHOLDER, injected.encode(), 1)
        elif version:
            self.log_message("sw.js: placeholder line not found; serving unversioned")
        return body

    # ---- request handling ----------------------------------------------

    def send_head(self):  # noqa: N802 (http.server API)
        path = self.translate_path(self.path)

        if os.path.basename(path) == SW_FILENAME and os.path.isfile(path):
            # Versioned, never-compressed, no-store (end_headers adds the rest).
            self._serve_bytes(self._swjs_bytes(), MIME_TYPES[".js"])
            return None

        # Directory index ("/" etc.): route through the same precompression
        # negotiation as explicit files instead of the stock raw serve.
        if os.path.isdir(path):
            index = os.path.join(path, "index.html")
            if self.path.endswith("/") and os.path.isfile(index):
                path = index
            else:
                return super().send_head()

        if os.path.isfile(path):
            br = path + ".br"
            gz = path + ".gz"
            has_variants = os.path.isfile(br) or os.path.isfile(gz)
            if has_variants and self._accepts("br") and os.path.isfile(br):
                self._serve_file(br, "br", vary=True)
                return None
            if has_variants and self._accepts("gzip") and os.path.isfile(gz):
                self._serve_file(gz, "gzip", vary=True)
                return None
            self._serve_file(path, None, vary=has_variants)
            return None

        return super().send_head()


def main() -> int:
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 8642
    server = http.server.ThreadingHTTPServer(("127.0.0.1", port), HarnessHandler)
    print(
        f"serving {DIR} at http://127.0.0.1:{port}/ "
        "(COOP: same-origin, COEP: require-corp)",
        flush=True,
    )
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
