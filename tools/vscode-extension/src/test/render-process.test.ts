import * as assert from "node:assert/strict";
import * as fs from "node:fs";
import * as os from "node:os";
import * as path from "node:path";
import { EventEmitter } from "node:events";
import { Readable, Writable } from "node:stream";
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
        command: process.execPath,
        args: [cliPath],
        source: "explicit",
        label: "test cli",
      },
      source: "flowchart TD\nA --> B\n",
      signal: abortController.signal,
    });
    await waitUntil(() => fs.existsSync(readyPath));
    abortController.abort();

    await assert.rejects(render, /superseded/);
    if (process.platform !== "win32") {
      assert.equal(fs.readFileSync(markerPath, "utf8"), "aborted");
    }
  });

  it("force kills a render process that ignores graceful timeout termination", async () => {
    if (process.platform === "win32") {
      return;
    }

    const tempDir = tempDirPath();
    const cliPath = path.join(tempDir, "merman-cli");
    const readyPath = path.join(tempDir, "ready");
    const termPath = path.join(tempDir, "term");
    fs.writeFileSync(
      cliPath,
      [
        "#!/usr/bin/env node",
        "const fs = require('node:fs');",
        `const ready = ${JSON.stringify(readyPath)};`,
        `const term = ${JSON.stringify(termPath)};`,
        "process.on('SIGTERM', () => { fs.writeFileSync(term, 'ignored'); });",
        "fs.writeFileSync(ready, 'ready');",
        "process.stdin.resume();",
        "setInterval(() => {}, 1000);",
      ].join("\n"),
    );
    fs.chmodSync(cliPath, 0o755);

    const render = runRenderProcess({
      invocation: {
        command: process.execPath,
        args: [cliPath],
        source: "explicit",
        label: "test cli",
      },
      source: "flowchart TD\nA --> B\n",
      timeoutMs: 500,
      killGraceMs: 20,
    });
    const rejected = assert.rejects(render, /timed out/);
    await waitUntil(() => fs.existsSync(readyPath));

    await rejected;
    assert.equal(fs.readFileSync(termPath, "utf8"), "ignored");
  });

  it("terminates renders that exceed the stdout size limit", async () => {
    const tempDir = tempDirPath();
    const cliPath = path.join(tempDir, "merman-cli");
    fs.writeFileSync(
      cliPath,
      [
        "#!/usr/bin/env node",
        "process.stdout.write('x'.repeat(64));",
      ].join("\n"),
    );
    fs.chmodSync(cliPath, 0o755);

    await assert.rejects(
      runRenderProcess({
        invocation: {
          command: process.execPath,
          args: [cliPath],
          source: "explicit",
          label: "test cli",
        },
        source: "flowchart TD\nA --> B\n",
        maxStdoutBytes: 16,
      }),
      /output exceeded the size limit/,
    );
  });

  it("terminates renders that exceed the stderr size limit", async () => {
    const tempDir = tempDirPath();
    const cliPath = path.join(tempDir, "merman-cli");
    fs.writeFileSync(
      cliPath,
      [
        "#!/usr/bin/env node",
        "process.stderr.write('x'.repeat(64));",
        "setTimeout(() => process.exit(1), 20);",
      ].join("\n"),
    );
    fs.chmodSync(cliPath, 0o755);

    await assert.rejects(
      runRenderProcess({
        invocation: {
          command: process.execPath,
          args: [cliPath],
          source: "explicit",
          label: "test cli",
        },
        source: "flowchart TD\nA --> B\n",
        maxStderrBytes: 16,
      }),
      /output exceeded the size limit/,
    );
  });

  it("rejects stdin pipe errors through the render promise", async () => {
    const child = new EventEmitter() as EventEmitter & {
      stdin: Writable;
      stdout: Readable;
      stderr: Readable;
      exitCode: number | null;
      signalCode: NodeJS.Signals | null;
      kill: () => boolean;
    };
    child.stdin = new Writable({
      write(_chunk, _encoding, callback) {
        callback(new Error("render stdin closed"));
      },
    });
    child.stdout = new Readable({ read() {} });
    child.stderr = new Readable({ read() {} });
    child.exitCode = null;
    child.signalCode = null;
    child.kill = () => true;

    await assert.rejects(
      runRenderProcess({
        invocation: {
          command: "merman-cli",
          args: [],
          source: "explicit",
          label: "test cli",
        },
        source: "flowchart TD\nA --> B\n",
        spawnProcess: () =>
          child as unknown as import("node:child_process").ChildProcessWithoutNullStreams,
      }),
      /render stdin closed/,
    );
  });
});

function tempDirPath(): string {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "merman-vscode-renderer-"));
  tempDirs.push(dir);
  return dir;
}

async function waitUntil(predicate: () => boolean): Promise<void> {
  for (let attempt = 0; attempt < 200; attempt += 1) {
    if (predicate()) {
      return;
    }
    await new Promise((resolve) => setTimeout(resolve, 10));
  }
  assert.fail("Condition was not met");
}
