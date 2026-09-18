#!/usr/bin/env bash
# Build the web (wasm32-unknown-unknown) artifact into wasm/pkg/.
#
#   wasm/build.sh            debug build (fast, huge, fine for development)
#   wasm/build.sh --release  release build (slow: 30-60 min for the gpui graph;
#                            run when a smaller artifact is needed)
#
# Steps: (1) cargo +nightly build of the lib cdylib (NIGHTLY is mandatory:
# wasm_thread uses `#![feature(stdarch_wasm_atomic_wait)]`), (2) wasm-bindgen
# --target web post-processing into wasm/pkg/ next to index.html.
#
# The wasm-bindgen CLI version MUST match the crate's wasm-bindgen pin
# (=0.2.125 in Cargo.toml) or the post-process fails with schema errors.
# Acquire the pinned CLI from GitHub release assets:
#   https://github.com/wasm-bindgen/wasm-bindgen/releases/download/0.2.125/ \
#     wasm-bindgen-0.2.125-x86_64-unknown-linux-musl.tar.gz
# (the release asset is named `wasm-bindgen-…`, not `wasm-bindgen-cli-…`).
set -euo pipefail

cd "$(dirname "$0")/.."

PROFILE_DIR="debug"
PROFILE_FLAG=""
if [[ "${1:-}" == "--release" ]]; then
    PROFILE_DIR="release"
    PROFILE_FLAG="--release"
elif [[ -n "${1:-}" ]]; then
    echo "usage: $0 [--release]" >&2
    exit 2
fi

# Resolve cargo's target directory the way cargo itself does: the global
# ~/.cargo/config.toml redirects it to ~/.cache/cargo-target, so the artifact
# is NOT under ./target/. Honor an explicit CARGO_TARGET_DIR first.
if [[ -n "${CARGO_TARGET_DIR:-}" ]]; then
    TARGET_DIR="${CARGO_TARGET_DIR}"
else
    TARGET_DIR="$(python3 -c '
import json, subprocess
meta = json.loads(subprocess.check_output(
    ["cargo", "+nightly", "metadata", "--format-version", "1", "--no-deps"]))
print(meta["target_directory"])
')"
fi

echo "==> cargo +nightly build --target wasm32-unknown-unknown --lib ${PROFILE_FLAG}"
cargo +nightly build --target wasm32-unknown-unknown --lib ${PROFILE_FLAG}

WASM="${TARGET_DIR}/wasm32-unknown-unknown/${PROFILE_DIR}/gpui_starter.wasm"
if [[ ! -f "${WASM}" ]]; then
    echo "error: cdylib artifact not found at ${WASM}" >&2
    exit 1
fi

BINDGEN="${WASM_BINDGEN:-wasm-bindgen}"
if ! command -v "${BINDGEN}" >/dev/null 2>&1; then
    echo "error: wasm-bindgen CLI not found (need exactly 0.2.125)" >&2
    echo "  install: see the URL in this script's header comment" >&2
    exit 1
fi
VERSION="$("${BINDGEN}" --version | awk '{print $NF}')"
if [[ "${VERSION}" != "0.2.125" ]]; then
    echo "error: wasm-bindgen ${VERSION} found, but Cargo.toml pins =0.2.125" >&2
    echo "  (set WASM_BINDGEN=/path/to/wasm-bindgen to override)" >&2
    exit 1
fi

echo "==> wasm-bindgen ${VERSION} --target web -> wasm/pkg/"
rm -rf wasm/pkg
mkdir -p wasm/pkg
"${BINDGEN}" "${WASM}" \
    --target web \
    --out-dir wasm/pkg \
    --out-name gpui_starter

echo "==> done: wasm/pkg/gpui_starter.js + wasm/pkg/gpui_starter_bg.wasm"
echo "    serve with: just wasm-serve  (or bash wasm/serve.sh)"
