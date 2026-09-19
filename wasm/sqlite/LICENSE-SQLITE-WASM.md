# Vendored: @sqlite.org/sqlite-wasm (classic browser build)

Provenance: npm package `@sqlite.org/sqlite-wasm` version `3.49.2-build1`
(https://www.npmjs.com/package/@sqlite.org/sqlite-wasm), files taken verbatim
from the package's `sqlite-wasm/jswasm/` directory:

- `sqlite3.js` — sqlite3 WebAssembly/JavaScript API (classic script build)
- `sqlite3.wasm` — the sqlite3 WebAssembly binary loaded by `sqlite3.js`
- `sqlite3-opfs-async-proxy.js` — the OPFS proxy worker spawned
  automatically by the OPFS VFS

The npm package metadata declares `license: Apache-2.0` for its packaging;
the underlying code carries the following license notice (verbatim from the
`sqlite3.js` bundle header):

---

This bundle (typically released as sqlite3.js or sqlite3.mjs) is an
amalgamation of JavaScript source code from two projects:

1) https://emscripten.org: the Emscripten "glue code" is covered by the
   terms of the MIT license and University of Illinois/NCSA Open Source
   License, as described at:

   https://emscripten.org/docs/introducing_emscripten/emscripten_license.html

2) https://sqlite.org: all code and documentation labeled as being from
   this source are released under the same terms as the sqlite3 C library.

2022-10-16

The author disclaims copyright to this source code.  In place of a legal
notice, here is a blessing:

*   May you do good and not evil.
*   May you find forgiveness for yourself and forgive others.
*   May you share freely, never taking more than you give.
