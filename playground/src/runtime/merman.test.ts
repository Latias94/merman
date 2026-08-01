import assert from "node:assert/strict";
import test from "node:test";

import {
  createMermanRuntime,
  installMermanDocumentLifecycle,
  MermanRuntimeError,
  type MermanDomainFacade,
  type MermanDocumentLifecycleTarget,
  type MermanRequestCache,
  type MermanRuntime,
  type MermanRuntimeDependencies,
  type MermanSession,
} from "./merman-core.ts";
import { configuredMermanOperationInput } from "./merman-operation-input.ts";

interface Deferred<T> {
  promise: Promise<T>;
  reject(reason?: unknown): void;
  resolve(value: T): void;
}

function deferred<T>(): Deferred<T> {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((nextResolve, nextReject) => {
    resolve = nextResolve;
    reject = nextReject;
  });
  return { promise, reject, resolve };
}

function facade(version = "test"): MermanDomainFacade {
  return { packageVersion: version } as MermanDomainFacade;
}

function session(
  value = facade(),
  onDispose: () => void = () => undefined,
): MermanSession {
  return { dispose: onDispose, facade: value };
}

function readyResponse(): Response {
  return new Response(new Uint8Array([0, 97, 115, 109]), {
    headers: { "content-type": "application/wasm" },
    status: 200,
  });
}

function dependencies(
  overrides: Partial<MermanRuntimeDependencies> = {},
): MermanRuntimeDependencies {
  return {
    createSession: () => session(),
    fetchWasm: async () => readyResponse(),
    initialize: async () => undefined,
    isInitialized: () => false,
    isRetryableInitializationError: (error) =>
      error instanceof WebAssembly.CompileError ||
      error instanceof WebAssembly.LinkError,
    loadModule: async () => ({ module: true }),
    ...overrides,
  };
}

test("freezes one configured input for detection, parse, layout, and render", () => {
  const input = configuredMermanOperationInput(
    "flowchart TD\n  A --> B\n",
    "forest",
    '{"layout":"elk"}',
    { diagramFont: "arial", pipeline: "resvg-safe" },
  );

  assert.equal(Object.isFrozen(input), true);
  assert.match(
    input.source,
    /%%\{init: \{"layout":"elk","theme":"forest"\}\}%%/,
  );
  assert.match(input.source, /flowchart TD/);
  assert.deepEqual(input.bindingOptions, {
    site_config: {
      fontFamily: "Arial, Helvetica, sans-serif",
      themeVariables: { fontFamily: "Arial, Helvetica, sans-serif" },
    },
    svg: { pipeline: "resvg-safe" },
  });
});

test("coalesces callers and starts module import and WASM fetch together", async () => {
  const moduleResult = deferred<unknown>();
  const wasmResult = deferred<Response>();
  const calls: string[] = [];
  const value = facade("coalesced");
  const runtime = createMermanRuntime(
    dependencies({
      createSession: () => session(value),
      fetchWasm: () => {
        calls.push("fetch");
        return wasmResult.promise;
      },
      initialize: async () => {
        calls.push("initialize");
      },
      loadModule: () => {
        calls.push("import");
        return moduleResult.promise;
      },
    }),
  );

  const callers = Array.from({ length: 6 }, () => runtime.ensureReady());
  const first = callers[0];
  assert.deepEqual(calls, ["import", "fetch"]);
  assert.ok(callers.every((caller) => caller === first));
  const loading = runtime.store.getState();
  assert.equal(loading.status, "loading");
  assert.equal(loading.status === "loading" ? loading.stage : null, "acquire");

  moduleResult.resolve({ module: true });
  wasmResult.resolve(readyResponse());
  assert.equal(await first, value);
  assert.equal(runtime.store.getState().status, "ready");
  assert.deepEqual(calls, ["import", "fetch", "initialize"]);
});

test("retries a compile failure with one reload response", async () => {
  const fetchModes: MermanRequestCache[] = [];
  let initializeCalls = 0;
  const runtime = createMermanRuntime(
    dependencies({
      fetchWasm: async ({ cache }) => {
        fetchModes.push(cache);
        return readyResponse();
      },
      initialize: async () => {
        initializeCalls += 1;
        if (initializeCalls === 1) {
          throw new WebAssembly.CompileError("bad cached bytes");
        }
      },
    }),
  );

  await runtime.ensureReady();
  assert.deepEqual(fetchModes, ["default", "reload"]);
  assert.equal(initializeCalls, 2);
  assert.equal(runtime.store.getState().status, "ready");
});

test("retries a LinkError with one reload response", async () => {
  const fetchModes: MermanRequestCache[] = [];
  let initializeCalls = 0;
  const runtime = createMermanRuntime(
    dependencies({
      fetchWasm: async ({ cache }) => {
        fetchModes.push(cache);
        return readyResponse();
      },
      initialize: async () => {
        initializeCalls += 1;
        if (initializeCalls === 1) {
          throw new WebAssembly.LinkError("stale import table");
        }
      },
    }),
  );

  await runtime.ensureReady();
  assert.deepEqual(fetchModes, ["default", "reload"]);
  assert.equal(initializeCalls, 2);
  assert.equal(runtime.store.getState().status, "ready");
});

test("keeps the second compile failure as a staged error", async () => {
  const fetchModes: MermanRequestCache[] = [];
  const runtime = createMermanRuntime(
    dependencies({
      fetchWasm: async ({ cache }) => {
        fetchModes.push(cache);
        return readyResponse();
      },
      initialize: async () => {
        throw new WebAssembly.CompileError("still invalid");
      },
    }),
  );

  await assert.rejects(runtime.ensureReady(), /still invalid/);
  assert.deepEqual(fetchModes, ["default", "reload"]);
  const state = runtime.store.getState();
  assert.equal(state.status, "error");
  assert.equal(state.error.stage, "initialize");
  assert.equal(state.error.recovery, "retry");
});

test("marks dynamic import failure as reload-required", async () => {
  const runtime = createMermanRuntime(
    dependencies({
      loadModule: async () => {
        throw new Error("chunk unavailable");
      },
    }),
  );

  await assert.rejects(runtime.ensureReady(), /chunk unavailable/);
  const state = runtime.store.getState();
  assert.equal(state.status, "error");
  assert.equal(state.error.stage, "module-import");
  assert.equal(state.error.recovery, "reload");
  await assert.rejects(runtime.retry(), /chunk unavailable/);
});

test("preserves structured binding failure details", async () => {
  const runtime = createMermanRuntime(
    dependencies({
      loadModule: async () => {
        throw {
          version: 1,
          ok: false,
          code: 9,
          code_name: "MERMAN_INTERNAL_ERROR",
          message: "Runtime initialization failed.",
        };
      },
    }),
  );

  const firstFailure = await runtime.ensureReady().catch((error) => error);
  const state = runtime.store.getState();
  assert.equal(state.status, "error");
  if (state.status !== "error") return;
  assert.equal(state.error.message, "Runtime initialization failed.");
  assert.match(state.error.detail ?? "", /MERMAN_INTERNAL_ERROR/);
  assert.doesNotMatch(state.error.message, /\[object Object\]/);

  const repeatedFailure = await runtime.ensureReady().catch((error) => error);
  for (const failure of [firstFailure, repeatedFailure]) {
    assert.ok(failure instanceof MermanRuntimeError);
    assert.equal(failure.stage, "module-import");
    assert.equal(failure.recovery, "reload");
    assert.equal(failure.message, "Runtime initialization failed.");
    assert.match(failure.detail ?? "", /MERMAN_INTERNAL_ERROR/);
  }
});

test("aborts the sibling WASM fetch when module import fails", async () => {
  let fetchAborted = false;
  const runtime = createMermanRuntime(
    dependencies({
      fetchWasm: ({ signal }) =>
        new Promise<Response>((_resolve, reject) => {
          signal.addEventListener("abort", () => {
            fetchAborted = true;
            reject(new DOMException("aborted", "AbortError"));
          });
        }),
      loadModule: async () => {
        throw new Error("chunk unavailable");
      },
    }),
  );

  await assert.rejects(runtime.ensureReady(), /chunk unavailable/);
  assert.equal(fetchAborted, true);
  const state = runtime.store.getState();
  assert.equal(state.status, "error");
  assert.equal(state.error.stage, "module-import");
});

test("rejects invalid responses before initialization", async () => {
  let initializeCalls = 0;
  const runtime = createMermanRuntime(
    dependencies({
      fetchWasm: async () =>
        new Response("not wasm", {
          headers: { "content-type": "text/plain" },
          status: 200,
        }),
      initialize: async () => {
        initializeCalls += 1;
      },
    }),
  );

  await assert.rejects(runtime.ensureReady(), /application\/wasm/);
  assert.equal(initializeCalls, 0);
  const state = runtime.store.getState();
  assert.equal(state.status, "error");
  assert.equal(state.error.stage, "response-validation");
});

test("classifies an HTTP failure before initialization", async () => {
  const runtime = createMermanRuntime(
    dependencies({
      fetchWasm: async () => new Response(null, { status: 404 }),
    }),
  );

  await assert.rejects(runtime.ensureReady(), /HTTP 404/);
  const state = runtime.store.getState();
  assert.equal(state.status, "error");
  assert.equal(state.error.stage, "response-validation");
  assert.equal(state.error.recovery, "retry");
});

test("discards late completion and disposes its unpublished session", async () => {
  const initialization = deferred<void>();
  let sessionDisposals = 0;
  const runtime = createMermanRuntime(
    dependencies({
      createSession: () => session(facade(), () => (sessionDisposals += 1)),
      initialize: () => initialization.promise,
    }),
  );

  const pending = runtime.ensureReady();
  runtime.dispose();
  initialization.resolve();
  await assert.rejects(pending, /superseded/i);
  assert.equal(sessionDisposals, 1);
  assert.equal(runtime.store.getState().status, "idle");
});

test("replaces discriminated states without retaining disposed payloads", async () => {
  const runtime = createMermanRuntime(dependencies());

  await runtime.ensureReady();
  assert.deepEqual(Object.keys(runtime.store.getState()).sort(), [
    "facade",
    "status",
    "suspended",
  ]);

  runtime.dispose();
  const idle = runtime.store.getState();
  assert.deepEqual(Object.keys(idle).sort(), ["status", "suspended"]);
  assert.equal("facade" in idle, false);
  assert.equal("stage" in idle, false);
  assert.equal("error" in idle, false);
});

test("retry preserves an already ready session", async () => {
  const value = facade("healthy");
  let sessionDisposals = 0;
  const runtime = createMermanRuntime(
    dependencies({
      createSession: () => session(value, () => (sessionDisposals += 1)),
    }),
  );

  assert.equal(await runtime.ensureReady(), value);
  assert.equal(await runtime.retry(), value);
  assert.equal(runtime.store.getState().status, "ready");
  assert.equal(sessionDisposals, 0);
});

test("preserves ready session through BFCache and disposes on final exit", async () => {
  let sessionDisposals = 0;
  const runtime = createMermanRuntime(
    dependencies({
      createSession: () => session(facade(), () => (sessionDisposals += 1)),
    }),
  );
  const target = new FakeLifecycleTarget();
  const remove = installMermanDocumentLifecycle(runtime, target);

  await runtime.ensureReady();
  target.window.emit("pagehide", { persisted: true });
  assert.equal(runtime.store.getState().status, "ready");
  assert.equal(runtime.store.getState().suspended, true);
  assert.equal(sessionDisposals, 0);

  target.window.emit("pageshow", { persisted: true });
  await Promise.resolve();
  assert.equal(runtime.store.getState().status, "ready");
  assert.equal(runtime.store.getState().suspended, false);

  target.window.emit("pagehide", { persisted: false });
  assert.equal(runtime.store.getState().status, "idle");
  assert.equal(sessionDisposals, 1);
  assert.equal(target.window.registeredTypes().includes("unload"), false);

  remove();
  assert.equal(target.window.listenerCount(), 0);
  assert.equal(target.document.listenerCount(), 0);
});

test("coalesces overlapping document lifecycle signals", async () => {
  const runtime = createMermanRuntime(dependencies());
  const target = new FakeLifecycleTarget();
  let destroys = 0;
  let resumes = 0;
  let suspends = 0;
  const remove = installMermanDocumentLifecycle(runtime, target, {
    onDestroy: () => (destroys += 1),
    onResume: () => (resumes += 1),
    onSuspend: () => (suspends += 1),
  });

  await runtime.ensureReady();
  target.window.emit("pagehide", { persisted: true });
  target.document.emit("freeze");
  assert.equal(suspends, 1);

  target.window.emit("pageshow", { persisted: true });
  target.document.emit("resume");
  await Promise.resolve();
  assert.equal(resumes, 1);

  target.window.emit("pagehide", { persisted: false });
  target.window.emit("pagehide", { persisted: false });
  assert.equal(destroys, 1);
  remove();
});

test("publishes lifecycle resume only after the runtime is ready", async () => {
  const resumed = deferred<MermanDomainFacade>();
  let resumeCallbacks = 0;
  const runtime = {
    dispose() {},
    ensureReady: () => resumed.promise,
    resume: () => resumed.promise,
    retry: () => resumed.promise,
    store: {} as MermanRuntime["store"],
    suspend() {},
  } satisfies MermanRuntime;
  const target = new FakeLifecycleTarget();
  const remove = installMermanDocumentLifecycle(runtime, target, {
    onResume: () => (resumeCallbacks += 1),
  });

  target.window.emit("pagehide", { persisted: true });
  target.window.emit("pageshow", { persisted: true });
  assert.equal(resumeCallbacks, 0);
  resumed.resolve(facade());
  await Promise.resolve();
  assert.equal(resumeCallbacks, 1);
  remove();
});

class FakeEventTarget {
  private readonly listeners = new Map<string, Set<(event: Event) => void>>();

  addEventListener(type: string, listener: (event: Event) => void): void {
    const listeners = this.listeners.get(type) ?? new Set();
    listeners.add(listener);
    this.listeners.set(type, listeners);
  }

  emit(type: string, properties: Record<string, unknown> = {}): void {
    const event = Object.assign(new Event(type), properties);
    for (const listener of this.listeners.get(type) ?? []) {
      listener(event);
    }
  }

  listenerCount(): number {
    return [...this.listeners.values()].reduce((count, listeners) => {
      return count + listeners.size;
    }, 0);
  }

  registeredTypes(): string[] {
    return [...this.listeners.keys()];
  }

  removeEventListener(type: string, listener: (event: Event) => void): void {
    this.listeners.get(type)?.delete(listener);
  }
}

class FakeLifecycleTarget implements MermanDocumentLifecycleTarget {
  readonly document = Object.assign(new FakeEventTarget(), {
    visibilityState: "visible",
  });
  readonly window = new FakeEventTarget();
}
