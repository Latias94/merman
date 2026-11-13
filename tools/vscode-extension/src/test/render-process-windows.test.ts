import * as assert from "node:assert/strict";
import type * as cp from "node:child_process";
import { EventEmitter } from "node:events";
import Module from "node:module";
import { Readable, Writable } from "node:stream";
import { describe, it } from "node:test";

function fakeChild() {
  return Object.assign(new EventEmitter(), {
    stdin: new Writable({ write(_chunk, _encoding, callback) { callback(); } }),
    stdout: new Readable({ read() {} }),
    stderr: new Readable({ read() {} }),
    pid: 12345,
    exitCode: null as number | null,
    signalCode: null as NodeJS.Signals | null,
    signals: [] as NodeJS.Signals[],
    kill(signal: NodeJS.Signals = "SIGTERM") { this.signals.push(signal); return true; },
    unref() {},
  });
}

describe("Windows render tree termination", () => {
  it("starts one forced taskkill while the root is live and lets it finish after render close", async (context) => {
    context.mock.property(process, "platform", "win32");
    const root = fakeChild();
    const killer = fakeChild();
    const spawns: Array<{ command: string; args: readonly string[]; options: cp.SpawnOptions }> = [];
    const { runRenderProcess } = loadRenderProcess((command, args, options) => {
      spawns.push({ command, args, options });
      return killer as unknown as cp.ChildProcess;
    });
    const abortController = new AbortController();
    const render = runRenderProcess({
      invocation: { command: "merman-cli", args: [], source: "explicit", label: "test cli" },
      source: "flowchart TD\nA --> B\n",
      signal: abortController.signal,
      killGraceMs: 5,
      killDrainMs: 50,
      spawnProcess: () => root as unknown as cp.ChildProcessWithoutNullStreams,
    });
    const rejected = assert.rejects(render, /superseded/);
    abortController.abort();
    root.exitCode = 0;
    root.emit("exit", 0, null);
    try {
      await new Promise((resolve) => setTimeout(resolve, 15));
      assert.deepEqual(spawns, [{
        command: "taskkill",
        args: ["/PID", "12345", "/T", "/F"],
        options: { stdio: "ignore", windowsHide: true },
      }]);
      root.emit("close", 0, null);
      await rejected;
      assert.deepEqual(killer.signals, []);
      assert.deepEqual(root.signals, []);
    } finally {
      root.emit("close", 0, null);
      killer.exitCode = 0;
      killer.emit("close", 0, null);
      await rejected;
    }
  });

  it("does not launch taskkill against an already exited root PID", async (context) => {
    context.mock.property(process, "platform", "win32");
    const root = fakeChild();
    root.exitCode = 0;
    let taskkillCalls = 0;
    const { runRenderProcess } = loadRenderProcess(() => {
      taskkillCalls += 1;
      return fakeChild() as unknown as cp.ChildProcess;
    });
    const abortController = new AbortController();
    const render = runRenderProcess({
      invocation: { command: "merman-cli", args: [], source: "explicit", label: "test cli" },
      source: "flowchart TD\nA --> B\n",
      signal: abortController.signal,
      killGraceMs: 5,
      killDrainMs: 5,
      spawnProcess: () => root as unknown as cp.ChildProcessWithoutNullStreams,
    });
    const rejected = assert.rejects(render, /superseded/);
    abortController.abort();
    await rejected;
    assert.equal(taskkillCalls, 0);
    assert.deepEqual(root.signals, []);
  });

  it("bounds a hung taskkill independently after render close without signaling the root again", async (context) => {
    context.mock.property(process, "platform", "win32");
    const root = fakeChild();
    const killer = fakeChild();
    const { runRenderProcess } = loadRenderProcess(() => killer as unknown as cp.ChildProcess);
    const abortController = new AbortController();
    const render = runRenderProcess({
      invocation: { command: "merman-cli", args: [], source: "explicit", label: "test cli" },
      source: "flowchart TD\nA --> B\n",
      signal: abortController.signal,
      killGraceMs: 5,
      killDrainMs: 5,
      spawnProcess: () => root as unknown as cp.ChildProcessWithoutNullStreams,
    });
    const rejected = assert.rejects(render, /superseded/);
    abortController.abort();
    root.emit("close", 0, null);
    await rejected;
    await new Promise((resolve) => setTimeout(resolve, 25));
    assert.deepEqual(killer.signals, ["SIGKILL"]);
    assert.deepEqual(root.signals, []);
    killer.emit("error", new Error("late taskkill failure"));
    assert.deepEqual(root.signals, []);
    killer.exitCode = 1;
    killer.emit("close", 1, null);
  });

  it("falls back to the live root when taskkill fails", async (context) => {
    context.mock.property(process, "platform", "win32");
    const root = fakeChild();
    const killer = fakeChild();
    const { runRenderProcess } = loadRenderProcess(() => killer as unknown as cp.ChildProcess);
    const abortController = new AbortController();
    const render = runRenderProcess({
      invocation: { command: "merman-cli", args: [], source: "explicit", label: "test cli" },
      source: "flowchart TD\nA --> B\n",
      signal: abortController.signal,
      killGraceMs: 5,
      killDrainMs: 5,
      spawnProcess: () => root as unknown as cp.ChildProcessWithoutNullStreams,
    });
    const rejected = assert.rejects(render, /superseded/);
    abortController.abort();
    killer.emit("error", new Error("taskkill unavailable"));
    assert.deepEqual(root.signals, ["SIGKILL"]);
    root.exitCode = 0;
    root.emit("close", 0, null);
    killer.exitCode = 1;
    killer.emit("close", 1, null);
    await rejected;
  });
});

function loadRenderProcess(
  spawn: (command: string, args: readonly string[], options: cp.SpawnOptions) => cp.ChildProcess,
): typeof import("../render-process.js") {
  type LoadModule = (this: unknown, request: string, parent: unknown, isMain: boolean) => unknown;
  const moduleWithLoad = Module as typeof Module & { _load: LoadModule };
  const originalLoad = moduleWithLoad._load;
  moduleWithLoad._load = function patchedLoad(
    this: unknown,
    request: string,
    parent: unknown,
    isMain: boolean,
  ): unknown {
    if (request === "node:child_process") {
      return { spawn };
    }
    return originalLoad.call(this, request, parent, isMain);
  };
  try {
    delete require.cache[require.resolve("../render-process.js")];
    return require("../render-process.js") as typeof import("../render-process.js");
  } finally {
    moduleWithLoad._load = originalLoad;
  }
}
