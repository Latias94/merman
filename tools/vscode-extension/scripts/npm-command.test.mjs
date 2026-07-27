import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { npmCommand } from "../../../scripts/npm-command.mjs";

describe("npm command selection", () => {
  it("runs the lifecycle npm CLI through the active Node executable", () => {
    assert.deepEqual(
      npmCommand(["exec", "--", "license-tool"], {
        platform: "win32",
        execPath: "C:\\node\\node.exe",
        env: {
          npm_execpath: "C:\\node\\node_modules\\npm\\bin\\npm-cli.js",
        },
      }),
      {
        command: "C:\\node\\node.exe",
        args: [
          "C:\\node\\node_modules\\npm\\bin\\npm-cli.js",
          "exec",
          "--",
          "license-tool",
        ],
      },
    );
  });

  it("uses the Windows command interpreter outside an npm lifecycle", () => {
    assert.deepEqual(
      npmCommand(["pack", "--dry-run"], {
        platform: "win32",
        execPath: "C:\\node\\node.exe",
        env: { ComSpec: "C:\\Windows\\System32\\cmd.exe" },
      }),
      {
        command: "C:\\Windows\\System32\\cmd.exe",
        args: ["/d", "/s", "/c", "npm.cmd", "pack", "--dry-run"],
      },
    );
  });

  it("runs npm directly on non-Windows hosts without lifecycle metadata", () => {
    assert.deepEqual(
      npmCommand(["pack"], {
        platform: "linux",
        execPath: "/usr/bin/node",
        env: {},
      }),
      {
        command: "npm",
        args: ["pack"],
      },
    );
  });
});
