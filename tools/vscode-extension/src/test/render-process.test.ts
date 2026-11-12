import * as assert from "node:assert/strict";
import * as fs from "node:fs";
import * as os from "node:os";
import * as path from "node:path";
import { spawn, type ChildProcessWithoutNullStreams } from "node:child_process";
import { EventEmitter, getEventListeners } from "node:events";
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
  it("rejects without spawning when the request is already aborted", async () => {
    const abortController = new AbortController();
    let spawned = false;
    abortController.abort();

    await assert.rejects(
      runRenderProcess({
        invocation: {
          command: "merman-cli",
          args: [],
          source: "explicit",
          label: "test cli",
        },
        source: "flowchart TD\nA --> B\n",
        signal: abortController.signal,
        spawnProcess: () => {
          spawned = true;
          throw new Error("spawn should not be called");
        },
      }),
      /superseded/,
    );
    assert.equal(spawned, false);
  });

  it("hides Windows console windows for spawned render processes", async () => {
    let spawnOptions: import("node:child_process").SpawnOptionsWithoutStdio | undefined;
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
        callback();
      },
      final(callback) {
        queueMicrotask(() => child.emit("close", 0, null));
        callback();
      },
    });
    child.stdout = new Readable({ read() {} });
    child.stderr = new Readable({ read() {} });
    child.exitCode = null;
    child.signalCode = null;
    child.kill = () => true;

    await runRenderProcess({
      invocation: {
        command: "merman-cli",
        args: [],
        source: "explicit",
        label: "test cli",
      },
      source: "flowchart TD\nA --> B\n",
      spawnProcess: (_command, _args, options) => {
        spawnOptions = options;
        return child as unknown as import("node:child_process").ChildProcessWithoutNullStreams;
      },
    });

    assert.equal(spawnOptions?.windowsHide, true);
    assert.equal(spawnOptions?.detached, process.platform !== "win32");
  });

  it("uses the process-tree terminator for graceful and forced termination", async () => {
    const abortController = new AbortController();
    const signals: NodeJS.Signals[] = [];
    const child = new EventEmitter() as EventEmitter & {
      stdin: Writable;
      stdout: Readable;
      stderr: Readable;
      exitCode: number | null;
      signalCode: NodeJS.Signals | null;
      pid: number;
      kill: () => boolean;
    };
    child.stdin = new Writable({
      write(_chunk, _encoding, callback) {
        callback();
      },
    });
    child.stdout = new Readable({ read() {} });
    child.stderr = new Readable({ read() {} });
    child.exitCode = null;
    child.signalCode = null;
    child.pid = 12345;
    child.kill = () => true;

    const render = runRenderProcess({
      invocation: {
        command: "merman-cli",
        args: [],
        source: "explicit",
        label: "test cli",
      },
      source: "flowchart TD\nA --> B\n",
      signal: abortController.signal,
      killGraceMs: 1,
      spawnProcess: () =>
        child as unknown as import("node:child_process").ChildProcessWithoutNullStreams,
      terminateProcessTree: (_child, signal) => {
        signals.push(signal);
      },
    });
    abortController.abort();
    await waitUntil(() => signals.length === 2);
    child.emit("close", null, "SIGKILL");

    await assert.rejects(render, /superseded/);
    assert.deepEqual(signals, ["SIGTERM", "SIGKILL"]);
  });

  it("escalates after the root exits while descendant pipes remain open", async () => {
    const abortController = new AbortController();
    const signals: NodeJS.Signals[] = [];
    const child = Object.assign(new EventEmitter(), {
      stdin: new Writable({ write(_chunk, _encoding, callback) { callback(); } }),
      stdout: new Readable({ read() {} }),
      stderr: new Readable({ read() {} }),
      exitCode: null as number | null,
      signalCode: null,
      kill: () => true,
    });
    const render = runRenderProcess({
      invocation: { command: "merman-cli", args: [], source: "explicit", label: "test cli" },
      source: "flowchart TD\nA --> B\n",
      signal: abortController.signal,
      killGraceMs: 5,
      spawnProcess: () => child as unknown as import("node:child_process").ChildProcessWithoutNullStreams,
      terminateProcessTree: (_child, signal) => {
        signals.push(signal);
        if (signal === "SIGTERM") {
          child.exitCode = 0;
          child.emit("exit", 0, null);
        }
      },
    });
    const rejected = assert.rejects(render, /superseded/);
    abortController.abort();
    try {
      await waitUntil(() => signals.length === 2);
      assert.deepEqual(signals, ["SIGTERM", "SIGKILL"]);
    } finally {
      child.emit("close", 0, null);
      await rejected;
    }
  });

  for (const termination of ["abort", "timeout", "output-limit"] as const) {
    it(`bounds ${termination} settlement and releases pipes when forced termination never closes the transport`, async () => {
      const abortController = new AbortController();
      const signals: NodeJS.Signals[] = [];
      let unreferenced = false;
      let settled = false;
      const child = Object.assign(new EventEmitter(), {
        stdin: new Writable({ write(_chunk, _encoding, callback) { callback(); } }),
        stdout: new Readable({ read() {} }),
        stderr: new Readable({ read() {} }),
        exitCode: 0,
        signalCode: null,
        kill: () => true,
        unref: () => { unreferenced = true; },
      });
      const render = runRenderProcess({
        invocation: { command: "merman-cli", args: [], source: "explicit", label: "test cli" },
        source: "flowchart TD\nA --> B\n",
        signal: abortController.signal,
        timeoutMs: termination === "timeout" ? 5 : 30000,
        killGraceMs: 5,
        killDrainMs: 5,
        maxStdoutBytes: 1,
        spawnProcess: () => child as unknown as ChildProcessWithoutNullStreams,
        terminateProcessTree: (_child, signal) => { signals.push(signal); },
      });
      const errorPattern = termination === "timeout" ? /timed out/
        : termination === "output-limit" ? /output exceeded/ : /superseded/;
      const rejected = assert.rejects(render, errorPattern).then(() => { settled = true; });
      if (termination === "abort") {
        abortController.abort();
      } else if (termination === "output-limit") {
        child.stdout.emit("data", Buffer.from("xx"));
      }
      try {
        await waitUntil(() => settled);
        assert.deepEqual(signals, ["SIGTERM", "SIGKILL"]);
        assert.equal(child.stdin.destroyed, true);
        assert.equal(child.stdout.destroyed, true);
        assert.equal(child.stderr.destroyed, true);
        assert.equal(unreferenced, true);
        assert.deepEqual(getEventListeners(abortController.signal, "abort"), []);
      } finally {
        child.emit("close", 0, null);
        await rejected;
      }
    });
  }

  it("does not signal the tree after close even when an abort arrives later", async () => {
    const abortController = new AbortController();
    const signals: NodeJS.Signals[] = [];
    const child = Object.assign(new EventEmitter(), {
      stdin: new Writable({ write(_chunk, _encoding, callback) { callback(); } }),
      stdout: new Readable({ read() {} }),
      stderr: new Readable({ read() {} }),
      exitCode: null,
      signalCode: null,
      kill: () => true,
    });
    const render = runRenderProcess({
      invocation: { command: "merman-cli", args: [], source: "explicit", label: "test cli" },
      source: "flowchart TD\nA --> B\n",
      signal: abortController.signal,
      killGraceMs: 5,
      maxStdoutBytes: 1,
      spawnProcess: () => child as unknown as ChildProcessWithoutNullStreams,
      terminateProcessTree: (_child, signal) => { signals.push(signal); },
    });
    child.emit("close", 0, null);
    await render;
    abortController.abort();
    child.stdout.emit("data", Buffer.from("xx"));
    await new Promise((resolve) => setTimeout(resolve, 20));
    assert.deepEqual(signals, []);
  });

  it("does not retry a missing POSIX group or signal its exited root PID", {
    skip: process.platform === "win32",
  }, async (context) => {
    const groupSignals: Array<{ pid: number; signal: NodeJS.Signals | number | undefined }> = [];
    const rootSignals: NodeJS.Signals[] = [];
    context.mock.method(process, "kill", (pid: number, signal?: NodeJS.Signals | number) => {
      groupSignals.push({ pid, signal });
      throw Object.assign(new Error("process group is gone"), { code: "ESRCH" });
    });
    const child = Object.assign(new EventEmitter(), {
      stdin: new Writable({ write(_chunk, _encoding, callback) { callback(); } }),
      stdout: new Readable({ read() {} }),
      stderr: new Readable({ read() {} }),
      pid: 12345,
      exitCode: 0,
      signalCode: null,
      kill: (signal: NodeJS.Signals) => { rootSignals.push(signal); return true; },
      unref() {},
    });
    const abortController = new AbortController();
    const render = runRenderProcess({
      invocation: { command: "merman-cli", args: [], source: "explicit", label: "test cli" },
      source: "flowchart TD\nA --> B\n",
      signal: abortController.signal,
      killGraceMs: 5,
      killDrainMs: 5,
      spawnProcess: () => child as unknown as ChildProcessWithoutNullStreams,
    });
    const rejected = assert.rejects(render, /superseded/);
    abortController.abort();
    await rejected;
    assert.deepEqual(groupSignals, [{ pid: -12345, signal: "SIGTERM" }]);
    assert.deepEqual(rootSignals, []);
  });

  it("still force kills a live root when its POSIX group is missing", {
    skip: process.platform === "win32",
  }, async (context) => {
    context.mock.method(process, "kill", () => {
      throw Object.assign(new Error("process group is gone"), { code: "ESRCH" });
    });
    const rootSignals: NodeJS.Signals[] = [];
    const child = Object.assign(new EventEmitter(), {
      stdin: new Writable({ write(_chunk, _encoding, callback) { callback(); } }),
      stdout: new Readable({ read() {} }),
      stderr: new Readable({ read() {} }),
      pid: 12345,
      exitCode: null,
      signalCode: null,
      kill: (signal: NodeJS.Signals) => { rootSignals.push(signal); return true; },
      unref() {},
    });
    const abortController = new AbortController();
    const render = runRenderProcess({
      invocation: { command: "merman-cli", args: [], source: "explicit", label: "test cli" },
      source: "flowchart TD\nA --> B\n",
      signal: abortController.signal,
      killGraceMs: 5,
      killDrainMs: 5,
      spawnProcess: () => child as unknown as ChildProcessWithoutNullStreams,
    });
    const rejected = assert.rejects(render, /superseded/);
    abortController.abort();
    await rejected;
    assert.deepEqual(rootSignals, ["SIGTERM", "SIGKILL"]);
  });

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

  it("force kills a render process that ignores graceful timeout termination", {
    skip: process.platform === "win32",
  }, async () => {
    const cliSource = [
      "process.on('SIGTERM', () => {});",
      "process.stdin.resume();",
      "setInterval(() => {}, 1000);",
      "process.stdout.write('ready');",
    ].join("\n");
    const child = spawn(process.execPath, ["-e", cliSource], {
      detached: true,
      stdio: "pipe",
      windowsHide: true,
    });
    let ready = false;
    let closed = false;
    let spawnError: Error | undefined;
    child.stdout.once("data", () => { ready = true; });
    child.once("close", () => { closed = true; });
    child.once("error", (error) => { spawnError = error; });

    try {
      await waitUntil(() => {
        assert.ifError(spawnError);
        return ready;
      });
      await assert.rejects(
        runRenderProcess({
          invocation: {
            command: process.execPath,
            args: ["-e", cliSource],
            source: "explicit",
            label: "test cli",
          },
          source: "flowchart TD\nA --> B\n",
          timeoutMs: 500,
          killGraceMs: 20,
          spawnProcess: () => child,
        }),
        /timed out/,
      );
      await waitUntil(() => closed);
      assert.equal(child.signalCode, "SIGKILL");
    } finally {
      if (!closed) {
        child.kill("SIGKILL");
      }
      await waitUntil(() => closed);
    }
  });

  it("reclaims a real POSIX descendant that holds pipes after its root exits", {
    skip: process.platform === "win32",
  }, async () => {
    const tempDir = tempDirPath();
    const readyPath = path.join(tempDir, "descendant-ready");
    const termPath = path.join(tempDir, "descendant-term");
    const cleanupPath = path.join(tempDir, "descendant-cleanup");
    const descendantSource = [
      "const fs = require('node:fs');",
      `process.on('SIGTERM', () => fs.writeFileSync(${JSON.stringify(termPath)}, 'ignored'));`,
      `fs.writeFileSync(${JSON.stringify(readyPath)}, String(process.pid));`,
      `setInterval(() => { if (fs.existsSync(${JSON.stringify(cleanupPath)})) process.exit(0); }, 10);`,
    ].join("\n");
    const rootSource = [
      "const { spawn } = require('node:child_process');",
      "process.on('SIGTERM', () => process.exit(0));",
      `spawn(process.execPath, ['-e', ${JSON.stringify(descendantSource)}], { stdio: 'inherit' });`,
      "setInterval(() => {}, 1000);",
    ].join("\n");
    const abortController = new AbortController();
    let root: ChildProcessWithoutNullStreams | undefined;
    let transportClosed = false;
    let rootExited = false;
    let settled = false;
    let descendantPid: number | undefined;
    const render = runRenderProcess({
      invocation: { command: process.execPath, args: ["-e", rootSource], source: "explicit", label: "test tree" },
      source: "flowchart TD\nA --> B\n",
      signal: abortController.signal,
      killGraceMs: 100,
      spawnProcess: (command, args, options) => {
        root = spawn(command, [...args], options);
        root.on("exit", () => { rootExited = true; });
        root.on("close", () => { transportClosed = true; });
        return root;
      },
    });
    const rejected = assert.rejects(render, /superseded/).then(() => { settled = true; });
    try {
      await waitUntil(() => fs.existsSync(readyPath));
      descendantPid = Number(fs.readFileSync(readyPath, "utf8"));
      assert.ok(Number.isSafeInteger(descendantPid) && descendantPid > 1);
      abortController.abort();
      await waitUntil(() => rootExited);
      assert.equal(root?.exitCode, 0);
      assert.equal(transportClosed, false);
      assert.equal(process.kill(descendantPid, 0), true);
      await waitUntil(() => settled);
      assert.equal(transportClosed, true);
      assert.equal(fs.readFileSync(termPath, "utf8"), "ignored");
      await waitUntil(() => processHasExited(descendantPid!));
    } finally {
      abortController.abort();
      fs.writeFileSync(cleanupPath, "cleanup");
      await rejected;
      if (descendantPid !== undefined) {
        await waitUntil(() => processHasExited(descendantPid!));
      }
    }
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
        setImmediate(() => child.emit("close", 0, null));
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

  it("prefers renderer stderr over an early stdin pipe error", async () => {
    const child = new EventEmitter() as EventEmitter & {
      stdin: Writable;
      stdout: Readable;
      stderr: Readable;
      exitCode: number | null;
      signalCode: NodeJS.Signals | null;
      kill: () => boolean;
    };
    child.stdout = new Readable({ read() {} });
    child.stderr = new Readable({ read() {} });
    child.stdin = new Writable({
      write(_chunk, _encoding, callback) {
        callback(new Error("render stdin closed"));
        setTimeout(() => {
          child.stderr.emit("data", Buffer.from("real renderer failure"));
          child.emit("close", 1, null);
        }, 75);
      },
    });
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
      /real renderer failure/,
    );
  });
});

function tempDirPath(): string {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "merman-vscode-renderer-"));
  tempDirs.push(dir);
  return dir;
}

function processHasExited(pid: number): boolean {
  try {
    process.kill(pid, 0);
    return false;
  } catch (error) {
    assert.equal((error as NodeJS.ErrnoException).code, "ESRCH");
    return true;
  }
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
