#!/usr/bin/env node
/* Precompress the wasm harness artifacts for serving with Content-Encoding.
 *
 * Run AFTER `wasm/build.sh [--release]` (wired into `just wasm-release`):
 *   node wasm/precompress.mjs          (bun works too — zero dependencies)
 *   FAST=1 node wasm/precompress.mjs   quicker levels (gz6 / brotli5)
 *
 * Produces, next to each heavy artifact, both compressed siblings:
 *   wasm/pkg/gpui_starter_bg.wasm  -> .gz + .br
 *   wasm/pkg/gpui_starter.js       -> .gz + .br
 *   wasm/index.html                -> .gz + .br
 *   wasm/sqlite/{worker,sqlite3,sqlite3-opfs-async-proxy}.js/.wasm
 * (sw.js is deliberately NOT compressed: serve_http.py injects the build
 * version into its text on every request.)
 *
 * Also writes wasm/pkg/version.json — a sha256 digest over the served files
 * (index.html, manifest, icons, pkg JS + wasm, vendored sqlite engine) —
 * overwriting the null-version placeholder build.sh left there. serve_http.py
 * injects that version into sw.js so a changed build changes the SW's bytes,
 * which is what drives the browser update cycle and the "reload" toast.
 */
import { createHash } from "node:crypto";
import { createReadStream, createWriteStream } from "node:fs";
import { stat, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { pipeline } from "node:stream/promises";
import zlib from "node:zlib";

const ROOT = path.dirname(fileURLToPath(import.meta.url));
const FAST = process.env.FAST === "1";
const GZIP_LEVEL = FAST ? 6 : 9;
const BROTLI_QUALITY = FAST ? 5 : 11;

/** Files that get .gz/.br siblings (server picks by Accept-Encoding). */
const COMPRESS = [
  "pkg/gpui_starter_bg.wasm",
  "pkg/gpui_starter.js",
  "index.html",
  // Vendored sqlite-wasm engine + storage worker (area D): the raw
  // engine is ~855 KB; on the wire brotli cuts it to ~250 KB.
  "sqlite/worker.js",
  "sqlite/sqlite3.js",
  "sqlite/sqlite3.wasm",
  "sqlite/sqlite3-opfs-async-proxy.js",
];

/** Files whose content defines the build version (hash input). */
const VERSION_INPUTS = [
  "index.html",
  "manifest.webmanifest",
  "icons/icon.svg",
  "icons/icon-192.png",
  "icons/icon-512.png",
  "icons/icon-512-maskable.png",
  "pkg/gpui_starter.js",
  "pkg/gpui_starter_bg.wasm",
  // The sqlite engine/worker are served (runtime-cached by the SW), so
  // an engine upgrade must bump the version or deploys stop
  // invalidating the cache (area D interface note).
  "sqlite/worker.js",
  "sqlite/sqlite3.js",
  "sqlite/sqlite3.wasm",
  "sqlite/sqlite3-opfs-async-proxy.js",
];

const fmt = (n) => (n / 1024 / 1024).toFixed(2).padStart(8) + " MB";

async function compressTo(inPath, outPath, makeStream) {
  await pipeline(
    createReadStream(inPath),
    makeStream(),
    createWriteStream(outPath),
  );
}

async function main() {
  const sizes = {};
  for (const rel of COMPRESS) {
    const abs = path.join(ROOT, rel);
    const info = await stat(abs).catch(() => null);
    if (!info) {
      console.error(`precompress: missing ${rel} — run wasm/build.sh first`);
      process.exit(1);
    }
    const t0 = Date.now();
    await compressTo(abs, abs + ".gz", () => zlib.createGzip({ level: GZIP_LEVEL }));
    await compressTo(abs, abs + ".br", () =>
      zlib.createBrotliCompress({
        params: { [zlib.constants.BROTLI_PARAM_QUALITY]: BROTLI_QUALITY },
      }),
    );
    const [gz, br] = await Promise.all([
      stat(abs + ".gz").then((s) => s.size),
      stat(abs + ".br").then((s) => s.size),
    ]);
    sizes[rel] = { raw: info.size, gz, br };
    console.log(
      `${rel}: raw ${fmt(info.size)}  gz ${fmt(gz)}  br ${fmt(br)}  ` +
        `(${(((Date.now() - t0) / 1000) | 0)}s)`,
    );
  }

  const hash = createHash("sha256");
  const hashedSizes = {};
  for (const rel of VERSION_INPUTS) {
    const abs = path.join(ROOT, rel);
    const info = await stat(abs).catch(() => null);
    if (!info) {
      console.error(`precompress: version input ${rel} missing — skipping version.json`);
      process.exit(1);
    }
    hashedSizes[rel] = info.size;
    hash.update(rel + "\0");
    await pipeline(
      createReadStream(abs),
      async function* (source) {
        for await (const chunk of source) hash.update(chunk);
      },
    );
  }
  const version = hash.digest("hex").slice(0, 16);
  await writeFile(
    path.join(ROOT, "pkg/version.json"),
    JSON.stringify({ version, generated: new Date().toISOString(), sizes: hashedSizes }, null, 2) + "\n",
  );
  console.log(`version.json: ${version} (${VERSION_INPUTS.length} files hashed)`);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
