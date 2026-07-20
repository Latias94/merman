import assert from "node:assert/strict";
import {
  mkdirSync,
  mkdtempSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import os from "node:os";
import path from "node:path";
import { describe, it } from "node:test";

import { buildLaunchArgs, runSmoke } from "./smoke-vsix.mjs";

describe("packaged VSIX smoke", () => {
  it("keeps VS Code state inside short transaction-owned paths", () => {
    assert.deepEqual(
      buildLaunchArgs({
        fixturePath: "/workspace/fixture",
        tempRoot: "/tmp/mvs-123",
      }),
      [
        "/workspace/fixture",
        "--user-data-dir=/tmp/mvs-123/u",
        "--extensions-dir=/tmp/mvs-123/e",
      ],
    );
  });

  it("cleans the temporary extraction root when VSIX parsing fails", async () => {
    const root = mkdtempSync(path.join(os.tmpdir(), "merman-vsix-smoke-test-"));
    try {
      const tempDir = path.join(root, "tmp");
      mkdirSync(tempDir);
      const vsixPath = path.join(root, "broken.vsix");
      writeFileSync(vsixPath, "not a zip");

      await assert.rejects(
        () =>
          runSmoke({
            argv: ["--vsix", vsixPath],
            cwd: root,
            tempDir,
            testRunner: async () => {
              throw new Error("test runner should not be invoked");
            },
          }),
        /Invalid VSIX/,
      );

      assert.deepEqual(
        readdirSync(tempDir).filter((entry) => entry.startsWith("merman-vsix-smoke-")),
        [],
      );
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });
});
