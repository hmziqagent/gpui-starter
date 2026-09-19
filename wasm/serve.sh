#!/usr/bin/env bash
# Serve the wasm harness at http://127.0.0.1:8642/ with COOP/COEP headers
# (required for SharedArrayBuffer / crossOriginIsolated). Run wasm/build.sh
# first, or use `just wasm-serve` which does both.
#
# Stop: Ctrl-C (foreground) or `kill "$(cat wasm/serve.pid)"` (background).
# The PID is written to wasm/serve.pid; a previous instance is stopped first,
# so re-running never leaves orphan servers behind.
set -euo pipefail

DIR="$(cd "$(dirname "$0")" && pwd)"
PORT="${1:-8642}"
PID_FILE="${DIR}/serve.pid"

if [[ -f "${PID_FILE}" ]]; then
    OLD_PID="$(cat "${PID_FILE}")"
    if kill -0 "${OLD_PID}" 2>/dev/null; then
        echo "==> stopping previous server (pid ${OLD_PID})"
        kill "${OLD_PID}" 2>/dev/null || true
        for _ in 1 2 3 4 5 6 7 8 9 10; do
            kill -0 "${OLD_PID}" 2>/dev/null || break
            sleep 0.2
        done
    fi
    rm -f "${PID_FILE}"
fi

python3 "${DIR}/serve_http.py" "${PORT}" &
PID=$!
echo "${PID}" > "${PID_FILE}"
trap 'kill "${PID}" 2>/dev/null || true; rm -f "${PID_FILE}"' INT TERM EXIT

echo "==> http://127.0.0.1:${PORT}/  (pid ${PID}; stop: kill \$(cat wasm/serve.pid))"
wait "${PID}"
