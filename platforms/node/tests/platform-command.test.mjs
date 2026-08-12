import assert from "node:assert/strict";
import test from "node:test";

import {
  assertSuccessfulNpmSpawn,
  npmCommand,
} from "../../../scripts/npm-command.mjs";

test("npm subprocesses resolve through the shared Windows adapter", () => {
  assert.deepEqual(
    npmCommand(["pack", "--dry-run"], {
      platform: "win32",
      execPath: String.raw`C:\node\node.exe`,
      env: { ComSpec: String.raw`C:\Windows\System32\cmd.exe` },
    }),
    {
      command: String.raw`C:\Windows\System32\cmd.exe`,
      args: ["/d", "/s", "/c", "npm.cmd", "pack", "--dry-run"],
    },
  );
});

test("spawn failures preserve creation, signal, null-status, and exit diagnostics", () => {
  const creationError = Object.assign(new Error("spawn npm ENOENT"), { code: "ENOENT" });
  assert.throws(
    () => assertSuccessfulNpmSpawn({ error: creationError, status: null, signal: null }, "npm pack"),
    /npm pack could not start: spawn npm ENOENT/,
  );
  assert.throws(
    () => assertSuccessfulNpmSpawn({ status: null, signal: "SIGTERM" }, "npm install"),
    /npm install was terminated by signal SIGTERM/,
  );
  assert.throws(
    () => assertSuccessfulNpmSpawn({ status: null, signal: null }, "npm audit"),
    /npm audit ended without an exit status/,
  );
  assert.throws(
    () => assertSuccessfulNpmSpawn(
      { status: 17, signal: null, stderr: "registry rejected request" },
      "npm publish",
    ),
    /npm publish exited with status 17: registry rejected request/,
  );

  const success = { status: 0, signal: null, stdout: "ok" };
  assert.equal(assertSuccessfulNpmSpawn(success, "npm pack"), success);
});
