import assert from "node:assert/strict";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  renameSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import os from "node:os";
import path from "node:path";
import { describe, it } from "node:test";

import { replaceSurfacesDir } from "./build-surface-packages.mjs";

describe("surface package generation", () => {
  it("restores existing surfaces when final rename fails", () => {
    const root = mkdtempSync(path.join(os.tmpdir(), "merman-web-surfaces-"));
    try {
      const srcDir = path.join(root, "src");
      const surfacesDir = path.join(srcDir, "surfaces");
      const tempSurfacesDir = path.join(srcDir, ".surfaces-temp");
      const backupSurfacesDir = path.join(srcDir, ".surfaces-backup");
      mkdirSync(surfacesDir, { recursive: true });
      mkdirSync(tempSurfacesDir, { recursive: true });
      writeFileSync(path.join(surfacesDir, "core.ts"), "current");
      writeFileSync(path.join(tempSurfacesDir, "core.ts"), "generated");

      const fsOps = {
        existsSync,
        rmSync,
        renameSync(source, target) {
          if (source === tempSurfacesDir && target === surfacesDir) {
            throw new Error("simulated final rename failure");
          }
          renameSync(source, target);
        },
      };

      assert.throws(
        () =>
          replaceSurfacesDir({
            surfacesDir,
            tempSurfacesDir,
            backupSurfacesDir,
            fsOps,
          }),
        /simulated final rename failure/,
      );
      assert.equal(readFileSync(path.join(surfacesDir, "core.ts"), "utf8"), "current");
      assert.equal(existsSync(backupSurfacesDir), false);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });

  it("restores stale backup from a previous process before replacing surfaces", () => {
    const root = mkdtempSync(path.join(os.tmpdir(), "merman-web-surfaces-"));
    try {
      const srcDir = path.join(root, "src");
      const surfacesDir = path.join(srcDir, "surfaces");
      const tempSurfacesDir = path.join(srcDir, ".surfaces-temp");
      const backupSurfacesDir = path.join(srcDir, ".surfaces-backup-current-process");
      const staleBackupSurfacesDir = path.join(srcDir, ".surfaces-backup-previous-process");
      mkdirSync(tempSurfacesDir, { recursive: true });
      mkdirSync(staleBackupSurfacesDir, { recursive: true });
      writeFileSync(path.join(tempSurfacesDir, "core.ts"), "generated");
      writeFileSync(path.join(staleBackupSurfacesDir, "core.ts"), "backup");

      const fsOps = {
        existsSync,
        readdirSync,
        rmSync,
        renameSync(source, target) {
          if (source === tempSurfacesDir && target === surfacesDir) {
            throw new Error("simulated final rename failure");
          }
          renameSync(source, target);
        },
        statSync,
      };

      assert.throws(
        () =>
          replaceSurfacesDir({
            surfacesDir,
            tempSurfacesDir,
            backupSurfacesDir,
            fsOps,
          }),
        /simulated final rename failure/,
      );
      assert.equal(readFileSync(path.join(surfacesDir, "core.ts"), "utf8"), "backup");
      assert.equal(existsSync(backupSurfacesDir), false);
      assert.equal(existsSync(staleBackupSurfacesDir), false);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });
});
