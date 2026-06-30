import * as assert from "node:assert/strict";
import * as fs from "node:fs";
import * as os from "node:os";
import * as path from "node:path";
import { afterEach, describe, it } from "node:test";

import { runRenderProcess } from "../render-process.js";

const tempDirs: string[] = [];

afterEach(() => {
  for (const dir of tempDirs.splice(0)) {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});

describe("render process lifecycle", () => {
  it("terminates an in-flight render when the abort signal fires", async () => {
    const tempDir = tempDirPath();
    const cliPath = path.join(tempDir, "merman-cli");
    const markerPath = path.join(tempDir, "aborted");
    const readyPath = path.join(tempDir, "ready");
    fs.writeFileSync(
      cliPath,
      [
        "#!/usr/bin/env node",
        "const fs = require('node:fs');",
        `const marker = ${JSON.stringify(markerPath)};`,
        `const ready = ${JSON.stringify(readyPath)};`,
        "process.on('SIGTERM', () => { fs.writeFileSync(marker, 'aborted'); process.exit(0); });",
        "fs.writeFileSync(ready, 'ready');",
        "process.stdin.resume();",
        "setTimeout(() => process.exit(0), 10000);",
      ].join("\n"),
    );
    fs.chmodSync(cliPath, 0o755);

    const abortController = new AbortController();
    const render = runRenderProcess({
      invocation: {
        command: cliPath,
        args: [],
        source: "explicit",
        label: "test cli",
      },
      source: "flowchart TD\nA --> B\n",
      signal: abortController.signal,
    });
    await waitUntil(() => fs.existsSync(readyPath));
    abortController.abort();

    await assert.rejects(render, /superseded/);
    assert.equal(fs.readFileSync(markerPath, "utf8"), "aborted");
  });
});

function tempDirPath(): string {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "merman-vscode-renderer-"));
  tempDirs.push(dir);
  return dir;
}

async function waitUntil(predicate: () => boolean): Promise<void> {
  for (let attempt = 0; attempt < 50; attempt += 1) {
    if (predicate()) {
      return;
    }
    await new Promise((resolve) => setTimeout(resolve, 10));
  }
  assert.fail("Condition was not met");
}
