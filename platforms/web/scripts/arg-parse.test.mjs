import assert from "node:assert/strict";
import {
  mkdirSync,
  mkdtempSync,
  rmSync,
  symlinkSync,
} from "node:fs";
import os from "node:os";
import path from "node:path";
import { describe, it } from "node:test";
import { fileURLToPath } from "node:url";

import {
  ArgParseError,
  assertKnownArgs,
  parseArgValue,
  resolvePackageSubdir,
} from "./arg-parse.mjs";

const packageRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

describe("web script argument parsing", () => {
  it("parses split and equals values", () => {
    assert.equal(parseArgValue(["--preset", "browser-core"], "--preset"), "browser-core");
    assert.equal(parseArgValue(["--preset=browser-core"], "--preset"), "browser-core");
    assert.equal(parseArgValue([], "--preset"), null);
  });

  it("rejects missing or empty values", () => {
    assert.throws(() => parseArgValue(["--preset"], "--preset"), ArgParseError);
    assert.throws(() => parseArgValue(["--preset", "--out-dir-rel"], "--preset"), ArgParseError);
    assert.throws(() => parseArgValue(["--preset="], "--preset"), ArgParseError);
  });

  it("rejects unknown arguments", () => {
    assert.doesNotThrow(() =>
      assertKnownArgs(["--preset", "browser-core"], { valueArgs: ["--preset"] }),
    );
    assert.throws(
      () => assertKnownArgs(["--preset", "browser-core", "--extra"], { valueArgs: ["--preset"] }),
      ArgParseError,
    );
  });

  it("resolves pkg and pkg child directories", () => {
    assert.deepEqual(resolvePackageSubdir(packageRoot, "pkg", "--out-dir-rel"), {
      absolute: path.join(packageRoot, "pkg"),
      relative: "pkg",
    });
    assert.deepEqual(resolvePackageSubdir(packageRoot, "pkg/core", "--out-dir-rel"), {
      absolute: path.join(packageRoot, "pkg", "core"),
      relative: path.join("pkg", "core"),
    });
  });

  it("allows first-build package output directories that do not exist yet", () => {
    const root = mkdtempSync(path.join(os.tmpdir(), "merman-web-arg-parse-"));
    try {
      assert.deepEqual(resolvePackageSubdir(root, "pkg/core", "--out-dir-rel"), {
        absolute: path.join(root, "pkg", "core"),
        relative: path.join("pkg", "core"),
      });
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });

  it("rejects package output directories that pass through pkg symlinks or junctions", (t) => {
    const root = mkdtempSync(path.join(os.tmpdir(), "merman-web-arg-parse-"));
    const external = mkdtempSync(path.join(os.tmpdir(), "merman-web-arg-parse-target-"));
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
        () => resolvePackageSubdir(root, "pkg/core", "--out-dir-rel"),
        /symlink or junction/,
      );
    } finally {
      rmSync(root, { recursive: true, force: true });
      rmSync(external, { recursive: true, force: true });
    }
  });

  it("rejects package output child directories that are symlinks or junctions", (t) => {
    const root = mkdtempSync(path.join(os.tmpdir(), "merman-web-arg-parse-"));
    const external = mkdtempSync(path.join(os.tmpdir(), "merman-web-arg-parse-target-"));
    try {
      mkdirSync(path.join(root, "pkg"));
      try {
        symlinkSync(
          external,
          path.join(root, "pkg", "core"),
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
        () => resolvePackageSubdir(root, "pkg/core/snippets", "--out-dir-rel"),
        /symlink or junction/,
      );
    } finally {
      rmSync(root, { recursive: true, force: true });
      rmSync(external, { recursive: true, force: true });
    }
  });

  it("rejects dangling package output symlinks or junctions", (t) => {
    const root = mkdtempSync(path.join(os.tmpdir(), "merman-web-arg-parse-"));
    try {
      const missingTarget = path.join(root, "missing-target");
      try {
        symlinkSync(
          missingTarget,
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
        () => resolvePackageSubdir(root, "pkg/core", "--out-dir-rel"),
        /symlink or junction/,
      );
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });

  it("rejects package output directories outside pkg", () => {
    const invalid = [
      "",
      ".",
      "..",
      "../..",
      "dist",
      "pkg2",
      "pkg/..",
      "pkg/../pkg/core",
      "pkg/../../..",
      path.join(packageRoot, "pkg", "core"),
    ];
    if (process.platform === "win32") {
      invalid.push("C:\\tmp\\merman-pkg", "\\\\server\\share\\merman-pkg");
    } else {
      invalid.push("/tmp/merman-pkg");
    }

    for (const relativeDir of invalid) {
      assert.throws(
        () => resolvePackageSubdir(packageRoot, relativeDir, "--out-dir-rel"),
        ArgParseError,
        relativeDir,
      );
    }
  });
});
