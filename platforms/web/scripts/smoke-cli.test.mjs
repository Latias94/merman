import assert from "node:assert/strict";
import {
  mkdtempSync,
  rmSync,
  symlinkSync,
} from "node:fs";
import os from "node:os";
import path from "node:path";
import { describe, it } from "node:test";

import { ArgParseError } from "./arg-parse.mjs";
import { parseSmokeCli } from "./smoke-cli.mjs";

describe("web smoke CLI path guards", () => {
  it("normalizes package subdirectories before deriving default WASM paths", () => {
    const root = mkdtempSync(path.join(os.tmpdir(), "merman-web-smoke-cli-"));
    try {
      assert.deepEqual(parseSmokeCli(["--pkg-dir-rel", path.join("pkg", "core")], root), {
        entrySubpath: ".",
        pkgDirRel: "pkg/core",
        wasmModuleSubpath: "./pkg/core/merman_wasm.js",
        wasmBinaryRel: "pkg/core/merman_wasm_bg.wasm",
        manifestRel: "pkg/core/merman_wasm_preset.json",
      });
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });

  it("rejects smoke package directories that pass through pkg symlinks or junctions", (t) => {
    const root = mkdtempSync(path.join(os.tmpdir(), "merman-web-smoke-cli-"));
    const external = mkdtempSync(path.join(os.tmpdir(), "merman-web-smoke-cli-target-"));
    try {
      try {
        symlinkSync(
          external,
          path.join(root, "pkg"),
          process.platform === "win32" ? "junction" : "dir",
        );
      } catch (error) {
        if (
          error &&
          typeof error === "object" &&
          "code" in error &&
          (error.code === "EPERM" || error.code === "EACCES")
        ) {
          t.skip("symlink or junction creation is not permitted on this host");
          return;
        }
        throw error;
      }

      assert.throws(
        () => parseSmokeCli(["--pkg-dir-rel", "pkg/core"], root),
        ArgParseError,
      );
    } finally {
      rmSync(root, { recursive: true, force: true });
      rmSync(external, { recursive: true, force: true });
    }
  });
});
