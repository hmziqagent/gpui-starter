#!/usr/bin/env python3
"""Static server for the wasm dev harness.

Sends the cross-origin-isolation headers the wasm build needs:
  Cross-Origin-Opener-Policy: same-origin
  Cross-Origin-Embedder-Policy: require-corp
Without them the page is not crossOriginIsolated, SharedArrayBuffer is
undefined and the gpui web platform degrades (wasm_thread cannot spawn its
background workers; Atomics.waitAsync waker loop is disabled).

Also pins the correct MIME type for .wasm (application/wasm) —
instantiateStreaming requires it.

Usage: serve_http.py [port]   (default 8642, binds 127.0.0.1)
"""

from __future__ import annotations

import http.server
import os
import sys

DIR = os.path.dirname(os.path.abspath(__file__))


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
        if path.endswith(".wasm"):
            return "application/wasm"
        return super().guess_type(path)


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
