import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";
import test from "node:test";
import vm from "node:vm";
import ts from "typescript";

const root = path.resolve(import.meta.dirname, "..");
const TEST_WORKER_PROTOCOL = 3;
const TEST_TRANSPORT_API_VERSION = 3;
const TEST_LEGEND_DIGEST = "sha256:test-generated-token-descriptor";
const TEST_LEGEND = Object.freeze({
  tokenTypes: Object.freeze(["string", "namespace"]),
  tokenModifiers: Object.freeze(["payload", "entity"]),
});
const TEST_LANGUAGE_IDENTITY = Object.freeze({
  legend: TEST_LEGEND,
  legendDigest: TEST_LEGEND_DIGEST,
});

function loadTypeScriptModule(relativePath, options = {}) {
  const cache = new Map();
  return load(path.join(root, relativePath));

  function load(sourcePath) {
    const normalizedPath = path.normalize(sourcePath);
    if (cache.has(normalizedPath)) return cache.get(normalizedPath).exports;
    const source = readFileSync(sourcePath, "utf8");
    const { outputText } = ts.transpileModule(source, {
      compilerOptions: {
        esModuleInterop: true,
        module: ts.ModuleKind.CommonJS,
        target: ts.ScriptTarget.ES2020,
      },
      fileName: sourcePath,
    });
    const module = { exports: {} };
    cache.set(normalizedPath, module);
    const context = {
      AbortController,
      DOMException,
      Error,
      Map,
      Promise,
      Set,
      Uint32Array,
      clearTimeout,
      console,
      ...options.globals,
      module,
      exports: module.exports,
      require(specifier) {
        if (options.externalModules?.[specifier]) {
          return options.externalModules[specifier];
        }
        if (specifier.startsWith(".")) {
          const resolved = path.resolve(path.dirname(sourcePath), specifier);
          return load(path.extname(resolved) ? resolved : `${resolved}.ts`);
        }
        throw new Error(
          `unexpected runtime import while testing ${relativePath}: ${specifier}`,
        );
      },
      setTimeout,
    };
    vm.runInNewContext(outputText, context, { filename: sourcePath });
    return module.exports;
  }
}

test("Monaco publishes planner-packed semantic tokens without transport projection", async () => {
  const { registerMermaidLanguage } = loadTypeScriptModule(
    "src/lib/mermaid-language.ts",
  );
  let semanticProvider;
  const packed = new Uint32Array([0, 0, 4, 1, 2]);
  const client = fakeLanguageClient(({ query }) => {
    assert.equal(query.kind, "semanticTokens");
    return packed;
  });

  const registration = registerMermaidLanguage(
    fakeMonaco({
      semantic: (provider) => {
        semanticProvider = provider;
      },
    }),
    client,
    TEST_LANGUAGE_IDENTITY,
  );

  assert.deepEqual(semanticProvider.getLegend(), TEST_LEGEND);
  const result = await semanticProvider.provideDocumentSemanticTokens(
    fakeModel("flowchart TD", 7),
    undefined,
    uncancelledToken(),
  );

  assert.equal(result.data, packed);
  registration.dispose();
  assert.equal(client.disposed, true);
});

test("worker owns one native editor session across document updates and queries", async () => {
  const scope = new FakeWorkerScope();
  const calls = [];
  let initCalls = 0;
  let initOptions;
  const session = {
    version: 1,
    uri: "file:///merman/playground.mmd",
    update(source, version) {
      calls.push(["update", source, version]);
      this.version = version;
    },
    diagnostics() {
      calls.push(["diagnostics"]);
      return { version: 1, diagnostics: [] };
    },
    diagramDetection() {
      return { status: "unavailable" };
    },
    codeActions() {
      return [];
    },
    completions(position) {
      calls.push(["completions", position]);
      return { isIncomplete: false, items: [] };
    },
    hover() {
      return null;
    },
    documentSymbols() {
      return [];
    },
    workspaceSymbols() {
      return [];
    },
    definition() {
      return null;
    },
    references() {
      return [];
    },
    prepareRename() {
      return null;
    },
    rename(_position, newName) {
      if (newName === "bad name") {
        throw {
          version: 1,
          ok: false,
          code: 1,
          code_name: "MERMAN_INVALID_ARGUMENT",
          kind: "generic",
          capability_id: null,
          message: "Rename target must be a valid Mermaid identifier.",
        };
      }
      return null;
    },
    semanticTokens() {
      calls.push(["semanticTokens"]);
      return new Uint32Array([0, 0, 4, 0, 0]);
    },
    dispose() {
      calls.push(["dispose"]);
    },
  };
  const editorApi = {
    SEMANTIC_TOKEN_DESCRIPTOR_DIGEST: TEST_LEGEND_DIGEST,
    SEMANTIC_TOKEN_MODIFIER_LSP_NAMES: TEST_LEGEND.tokenModifiers,
    SEMANTIC_TOKEN_TYPE_LSP_NAMES: TEST_LEGEND.tokenTypes,
    transportApiVersion: () => TEST_TRANSPORT_API_VERSION,
    runtimeCatalog: () => ({
      schema_version: 1,
      transport_api_version: TEST_TRANSPORT_API_VERSION,
      package_version: "test",
      capabilities: {
        capability_ids: ["analysis", "editor"],
        output_ids: [],
        operation_ids: ["analysis-json", "semantic-json"],
        system_adapter_ids: [],
        text_measurement: null,
      },
      registry: {
        diagram_family_count: 35,
      },
      resources: {
        schema_version: 1,
        general_binding_default_profile: "interactive",
        cli_default_profile: "trusted-native",
        limits: [],
        profiles: [],
      },
    }),
    async initMerman(options) {
      initCalls += 1;
      initOptions = options;
    },
    editorSemanticTokenDescriptor: () => ({ digest: TEST_LEGEND_DIGEST }),
    createEditorSession(source, version, uri) {
      calls.push(["create", source, version, uri]);
      session.version = version;
      session.uri = uri;
      return session;
    },
  };

  loadTypeScriptModule("src/editor/merman-language.worker.ts", {
    externalModules: { "@mermanjs/web": editorApi },
    globals: { self: scope },
  });

  await scope.request({
    protocol: TEST_WORKER_PROTOCOL,
    type: "initialize",
    requestId: 1,
  });
  assert.equal(initCalls, 1);
  assert.equal(initOptions, undefined);
  await scope.request({
    protocol: TEST_WORKER_PROTOCOL,
    type: "didOpen",
    requestId: 2,
    document: snapshot(1, "flowchart TD\nA-->B"),
  });
  await scope.request({
    protocol: TEST_WORKER_PROTOCOL,
    type: "didChange",
    requestId: 3,
    document: snapshot(2, "flowchart TD\nA-->C"),
  });
  const diagnostics = await scope.request({
    protocol: TEST_WORKER_PROTOCOL,
    type: "query",
    requestId: 4,
    uri: session.uri,
    version: session.version,
    legendDigest: TEST_LEGEND_DIGEST,
    query: { kind: "diagnostics" },
  });
  const tokens = await scope.request({
    protocol: TEST_WORKER_PROTOCOL,
    type: "query",
    requestId: 5,
    uri: session.uri,
    version: session.version,
    legendDigest: TEST_LEGEND_DIGEST,
    query: { kind: "semanticTokens" },
  });

  assert.deepEqual(calls.slice(0, 3), [
    ["create", "flowchart TD\nA-->B", 1, "file:///merman/playground.mmd"],
    ["update", "flowchart TD\nA-->C", 2],
    ["diagnostics"],
  ]);
  assert.deepEqual(diagnostics.result, { version: 1, diagnostics: [] });
  assert.deepEqual([...tokens.result], [0, 0, 4, 0, 0]);

  const failedRename = await scope.request({
    protocol: TEST_WORKER_PROTOCOL,
    type: "query",
    requestId: 6,
    uri: session.uri,
    version: session.version,
    legendDigest: TEST_LEGEND_DIGEST,
    query: {
      kind: "rename",
      position: { line: 1, character: 0 },
      newName: "bad name",
    },
  });
  assert.equal(failedRename.type, "error");
  assert.equal(failedRename.code, "OPERATION_REJECTED");
  assert.equal(failedRename.nativeCode, "MERMAN_INVALID_ARGUMENT");
  assert.match(failedRename.message, /valid Mermaid identifier/i);
  assert.match(failedRename.detail, /MERMAN_INVALID_ARGUMENT/);

  const diagnosticsAfterFailure = await scope.request({
    protocol: TEST_WORKER_PROTOCOL,
    type: "query",
    requestId: 7,
    uri: session.uri,
    version: session.version,
    legendDigest: TEST_LEGEND_DIGEST,
    query: { kind: "diagnostics" },
  });
  assert.equal(diagnosticsAfterFailure.type, "queryResult");
  assert.deepEqual(diagnosticsAfterFailure.result, {
    version: 1,
    diagnostics: [],
  });

  scope.dispatch({ protocol: TEST_WORKER_PROTOCOL, type: "dispose" });
  await Promise.resolve();
  assert.equal(calls.at(-1)[0], "dispose");
  assert.equal(scope.closed, true);
});

test("managed document changes explicitly refresh semantic tokens after didChange", async () => {
  const { registerMermaidLanguage } = loadTypeScriptModule(
    "src/lib/mermaid-language.ts",
  );
  let semanticProvider;
  let changes = 0;
  const client = {
    async openDocument() {},
    async changeDocument() {
      changes += 1;
    },
    async query(_snapshot, query) {
      return query.kind === "diagnostics" ? { diagnostics: [] } : [];
    },
    dispose() {},
  };
  const model = reactiveModel("flowchart TD", 1);
  const registration = registerMermaidLanguage(
    fakeMonaco({
      semantic: (provider) => {
        semanticProvider = provider;
      },
    }),
    client,
    TEST_LANGUAGE_IDENTITY,
  );
  let refreshes = 0;
  const refreshListener = semanticProvider.onDidChange(() => {
    refreshes += 1;
  });
  const binding = await registration.bindModel(model);
  assert.equal(refreshes, 1);

  model.change("flowchart TD\nA-->B");
  await Promise.resolve();

  assert.equal(changes, 1);
  assert.equal(refreshes, 2);
  refreshListener.dispose();
  binding.dispose();
  registration.dispose();
});

test("rename rejects a workspace edit targeting an unmanaged URI", async () => {
  const { registerMermaidLanguage } = loadTypeScriptModule(
    "src/lib/mermaid-language.ts",
  );
  let renameProvider;
  const client = fakeLanguageClient(({ query }) => {
    if (query.kind !== "rename") {
      throw new Error(`unexpected query: ${query.kind}`);
    }
    return {
      changes: {
        "file:///elsewhere.mmd": [
          {
            range: {
              start: { line: 0, character: 0 },
              end: { line: 0, character: 1 },
            },
            newText: "B",
          },
        ],
      },
    };
  });
  const model = fakeModel("flowchart TD\nA", 3);
  let requestRejection;

  const registration = registerMermaidLanguage(
    fakeMonaco({
      rename: (provider) => {
        renameProvider = provider;
      },
    }),
    client,
    TEST_LANGUAGE_IDENTITY,
    {
      onRequestRejected: (rejection) => {
        requestRejection = rejection;
      },
    },
  );
  const result = await renameProvider.provideRenameEdits(
    model,
    { lineNumber: 2, column: 1 },
    "B",
    uncancelledToken(),
  );

  assert.equal(result.edits.length, 0);
  assert.match(result.rejectReason, /current document/i);
  assert.equal(requestRejection.detail, null);
  assert.equal(
    requestRejection.message,
    "Rename is limited to the current document; received an edit for file:///elsewhere.mmd.",
  );
  assert.equal(requestRejection.nativeCode, null);
  assert.equal(requestRejection.operation, "rename");
  registration.dispose();
});

test("worker client discards a response after the managed document version changes", async () => {
  const { createMermanLanguageWorkerClient, StaleLanguageSnapshotError } =
    loadTypeScriptModule("src/editor/worker-client.ts");
  const worker = new FakeWorker();
  const client = createMermanLanguageWorkerClient(worker, TEST_LEGEND_DIGEST);
  await initializeClient(worker, client);
  await acknowledgeDocument(worker, client.openDocument(snapshot(1, "flowchart TD")));

  const query = client.query(snapshot(1, "flowchart TD"), {
    kind: "diagnostics",
  });
  const queryMessage = await worker.takeEventually("query");
  const change = client.changeDocument(snapshot(2, "flowchart TD\nA-->B"));
  worker.respond(queryMessage, { diagnostics: [] });

  await assert.rejects(
    query,
    (error) => error instanceof StaleLanguageSnapshotError,
  );
  await acknowledgeDocument(worker, change);
  client.dispose();
});

test("worker cancellation suppresses publication without claiming to interrupt synchronous WASM", async () => {
  const { createMermanLanguageWorkerClient } = loadTypeScriptModule(
    "src/editor/worker-client.ts",
  );
  const worker = new FakeWorker();
  const client = createMermanLanguageWorkerClient(worker, TEST_LEGEND_DIGEST);
  await initializeClient(worker, client);
  await acknowledgeDocument(worker, client.openDocument(snapshot(1, "flowchart TD")));
  const cancellation = cancellableToken();

  const query = client.query(
    snapshot(1, "flowchart TD"),
    { kind: "diagnostics" },
    cancellation.token,
  );
  const queryMessage = await worker.takeEventually("query");
  cancellation.cancel();

  await assert.rejects(query, (error) => error?.name === "AbortError");
  worker.respond(queryMessage, { diagnostics: [] });
  assert.equal(worker.messages.some((message) => message.type === "cancel"), false);
  assert.equal(worker.terminated, false);
  client.dispose();
});

test("worker client discards a result carrying an obsolete legend digest", async () => {
  const { createMermanLanguageWorkerClient, StaleLanguageSnapshotError } =
    loadTypeScriptModule("src/editor/worker-client.ts");
  const worker = new FakeWorker();
  const client = createMermanLanguageWorkerClient(worker, TEST_LEGEND_DIGEST);
  await initializeClient(worker, client);
  await acknowledgeDocument(worker, client.openDocument(snapshot(1, "flowchart TD")));

  const query = client.query(snapshot(1, "flowchart TD"), {
    kind: "semanticTokens",
  });
  const queryMessage = await worker.takeEventually("query");
  worker.respond(queryMessage, new Uint32Array([0, 0, 4, 0, 0]), {
    legendDigest: "sha256:obsolete",
  });

  await assert.rejects(
    query,
    (error) => error instanceof StaleLanguageSnapshotError,
  );
  client.dispose();
});

test("worker client does not send a query canceled while awaiting didChange", async () => {
  const { createMermanLanguageWorkerClient } = loadTypeScriptModule(
    "src/editor/worker-client.ts",
  );
  const worker = new FakeWorker();
  const client = createMermanLanguageWorkerClient(worker, TEST_LEGEND_DIGEST);
  await initializeClient(worker, client);
  await acknowledgeDocument(worker, client.openDocument(snapshot(1, "flowchart TD")));
  const next = snapshot(2, "flowchart TD\nA-->B");
  const change = client.changeDocument(next);
  const changeMessage = await worker.takeEventually("didChange");
  const cancellation = cancellableToken();
  const query = client.query(next, { kind: "diagnostics" }, cancellation.token);

  cancellation.cancel();
  worker.respond(changeMessage, null);
  await change;

  await assert.rejects(query, (error) => error?.name === "AbortError");
  assert.equal(worker.messages.some((message) => message.type === "query"), false);
  client.dispose();
});

test("worker client keeps operation rejection local and preserves native detail", async () => {
  const { createMermanLanguageWorkerClient, EditorWorkerProtocolError } =
    loadTypeScriptModule("src/editor/worker-client.ts");
  const worker = new FakeWorker();
  const client = createMermanLanguageWorkerClient(worker, TEST_LEGEND_DIGEST);
  const document = snapshot(1, "flowchart TD\nA-->B");
  await initializeClient(worker, client);
  await acknowledgeDocument(worker, client.openDocument(document));

  const rename = client.query(document, {
    kind: "rename",
    position: { line: 1, character: 0 },
    newName: "bad name",
  });
  const renameRequest = await worker.takeEventually("query");
  const nativeDetail = JSON.stringify({
    code_name: "MERMAN_INVALID_ARGUMENT",
    message: "Rename target must be a valid Mermaid identifier.",
  });
  worker.fail(
    renameRequest,
    "OPERATION_REJECTED",
    "Rename target must be a valid Mermaid identifier.",
    nativeDetail,
    "MERMAN_INVALID_ARGUMENT",
  );

  await assert.rejects(rename, (error) => {
    assert.ok(error instanceof EditorWorkerProtocolError);
    assert.equal(error.code, "OPERATION_REJECTED");
    assert.equal(error.detail, nativeDetail);
    assert.equal(error.nativeCode, "MERMAN_INVALID_ARGUMENT");
    return true;
  });
  assert.equal(worker.terminated, false);

  const diagnostics = client.query(document, { kind: "diagnostics" });
  const diagnosticsRequest = await worker.takeEventually("query");
  worker.respond(diagnosticsRequest, { version: 1, diagnostics: [] });
  assert.deepEqual(await diagnostics, { version: 1, diagnostics: [] });
  assert.equal(worker.terminated, false);
  client.dispose();
});

test("worker client exposes QUERY_FAILED without corrupting later queries", async () => {
  const { createMermanLanguageWorkerClient, EditorWorkerProtocolError } =
    loadTypeScriptModule("src/editor/worker-client.ts");
  const worker = new FakeWorker();
  const client = createMermanLanguageWorkerClient(worker, TEST_LEGEND_DIGEST);
  const document = snapshot(1, "flowchart TD\nA-->B");
  await initializeClient(worker, client);
  await acknowledgeDocument(worker, client.openDocument(document));

  const failed = client.query(document, {
    kind: "hover",
    position: { line: 1, character: 0 },
  });
  const failedRequest = await worker.takeEventually("query");
  worker.fail(
    failedRequest,
    "QUERY_FAILED",
    "Native editor query failed.",
    '{"code_name":"MERMAN_INTERNAL_ERROR"}',
    "MERMAN_INTERNAL_ERROR",
  );
  await assert.rejects(failed, (error) => {
    assert.ok(error instanceof EditorWorkerProtocolError);
    assert.equal(error.code, "QUERY_FAILED");
    assert.equal(error.nativeCode, "MERMAN_INTERNAL_ERROR");
    return true;
  });
  assert.equal(worker.terminated, false);

  const diagnostics = client.query(document, { kind: "diagnostics" });
  const diagnosticsRequest = await worker.takeEventually("query");
  worker.respond(diagnosticsRequest, { version: 1, diagnostics: [] });
  assert.deepEqual(await diagnostics, { version: 1, diagnostics: [] });
  client.dispose();
});

test("worker client poisons every pending request on a malformed dedicated response", async () => {
  const { createMermanLanguageWorkerClient } = loadTypeScriptModule(
    "src/editor/worker-client.ts",
  );
  const worker = new FakeWorker();
  const client = createMermanLanguageWorkerClient(worker, TEST_LEGEND_DIGEST);
  await initializeClient(worker, client);
  await acknowledgeDocument(worker, client.openDocument(snapshot(1, "flowchart TD")));
  const query = client.query(snapshot(1, "flowchart TD"), {
    kind: "diagnostics",
  });
  await worker.takeEventually("query");

  worker.emit("message", {
    data: {
      protocol: TEST_WORKER_PROTOCOL,
      type: "result",
      requestId: "not-a-request-id",
    },
  });

  const outcome = await Promise.race([
    query.then(
      () => ({ status: "resolved" }),
      (error) => ({ status: "rejected", error }),
    ),
    new Promise((resolve) =>
      setTimeout(() => resolve({ status: "timeout" }), 50),
    ),
  ]);
  assert.equal(outcome.status, "rejected");
  assert.match(outcome.error.message, /malformed editor worker response/i);
  client.dispose();
});

test("document synchronization failure poisons the inconsistent worker session", async () => {
  const { createMermanLanguageWorkerClient } = loadTypeScriptModule(
    "src/editor/worker-client.ts",
  );
  const worker = new FakeWorker();
  const client = createMermanLanguageWorkerClient(worker, TEST_LEGEND_DIGEST);
  await initializeClient(worker, client);
  await acknowledgeDocument(worker, client.openDocument(snapshot(1, "flowchart TD")));

  const change = client.changeDocument(snapshot(2, "flowchart TD\nA-->B"));
  const request = await worker.takeEventually("didChange");
  worker.fail(request, "INVALID_STATE", "synchronization rejected");

  await assert.rejects(change, /synchronization rejected/);
  assert.equal(worker.terminated, true);
  await assert.rejects(
    client.query(snapshot(2, "flowchart TD\nA-->B"), { kind: "diagnostics" }),
    /synchronization rejected/,
  );
  client.dispose();
});

test("a synchronous postMessage failure removes pending state and poisons the session", async () => {
  const { createMermanLanguageWorkerClient } = loadTypeScriptModule(
    "src/editor/worker-client.ts",
  );
  const worker = new FakeWorker();
  const client = createMermanLanguageWorkerClient(worker, TEST_LEGEND_DIGEST);
  await initializeClient(worker, client);
  await acknowledgeDocument(worker, client.openDocument(snapshot(1, "flowchart TD")));
  worker.throwOnType = "query";

  await assert.rejects(
    client.query(snapshot(1, "flowchart TD"), { kind: "diagnostics" }),
    /postMessage rejected/,
  );
  assert.equal(worker.terminated, true);
  await assert.rejects(
    client.query(snapshot(1, "flowchart TD"), { kind: "diagnostics" }),
    /postMessage rejected/,
  );
  client.dispose();
});

function fakeLanguageClient(runQuery) {
  return {
    disposed: false,
    async openDocument() {},
    async changeDocument() {},
    async query(_snapshot, query, _token) {
      return runQuery({ query });
    },
    dispose() {
      this.disposed = true;
    },
  };
}

function snapshot(version, source) {
  return { uri: "file:///merman/playground.mmd", version, source };
}

function fakeModel(value, version) {
  return {
    uri: { toString: () => "file:///merman/playground.mmd" },
    getValue: () => value,
    getVersionId: () => version,
    getLineCount: () => value.split("\n").length,
    getLineContent: (lineNumber) => value.split("\n")[lineNumber - 1] ?? "",
    getLineMaxColumn: (lineNumber) =>
      (value.split("\n")[lineNumber - 1] ?? "").length + 1,
    isDisposed: () => false,
    onDidChangeContent: () => ({ dispose() {} }),
  };
}

function reactiveModel(initialValue, initialVersion) {
  let value = initialValue;
  let version = initialVersion;
  let listener = () => {};
  return {
    uri: { toString: () => "file:///merman/playground.mmd" },
    getValue: () => value,
    getVersionId: () => version,
    isDisposed: () => false,
    onDidChangeContent(next) {
      listener = next;
      return { dispose: () => (listener = () => {}) };
    },
    change(nextValue) {
      value = nextValue;
      version += 1;
      listener();
    },
  };
}

function uncancelledToken() {
  return {
    isCancellationRequested: false,
    onCancellationRequested: () => ({ dispose() {} }),
  };
}

function cancellableToken() {
  let listener = () => {};
  return {
    token: {
      isCancellationRequested: false,
      onCancellationRequested(next) {
        listener = next;
        return { dispose: () => (listener = () => {}) };
      },
    },
    cancel() {
      this.token.isCancellationRequested = true;
      listener();
    },
  };
}

class FakeWorker {
  constructor() {
    this.listeners = new Map();
    this.messages = [];
    this.terminated = false;
  }

  addEventListener(type, listener) {
    const listeners = this.listeners.get(type) ?? [];
    listeners.push(listener);
    this.listeners.set(type, listeners);
  }

  removeEventListener(type, listener) {
    const listeners = this.listeners.get(type) ?? [];
    this.listeners.set(
      type,
      listeners.filter((candidate) => candidate !== listener),
    );
  }

  postMessage(message) {
    if (message.type === this.throwOnType) {
      throw new Error("postMessage rejected");
    }
    this.messages.push(message);
  }

  terminate() {
    this.terminated = true;
  }

  take(type) {
    const index = this.messages.findIndex((message) => message.type === type);
    assert.notEqual(index, -1, `missing worker message of type ${type}`);
    return this.messages.splice(index, 1)[0];
  }

  async takeEventually(type) {
    for (let attempt = 0; attempt < 10; attempt += 1) {
      if (this.messages.some((message) => message.type === type)) {
        return this.take(type);
      }
      await Promise.resolve();
    }
    return this.take(type);
  }

  respond(request, result, overrides = {}) {
    const snapshotIdentity = request.type === "query"
      ? {
          uri: request.uri,
          version: request.version,
          legendDigest: request.legendDigest,
        }
      : {};
    this.emit("message", {
      data: {
        protocol: TEST_WORKER_PROTOCOL,
        type: request.type === "query" ? "queryResult" : "result",
        requestId: request.requestId,
        ...snapshotIdentity,
        ...overrides,
        result,
      },
    });
  }

  fail(request, code, message, detail = null, nativeCode = null) {
    this.emit("message", {
      data: {
        protocol: TEST_WORKER_PROTOCOL,
        type: "error",
        requestId: request.requestId,
        code,
        message,
        detail,
        nativeCode,
      },
    });
  }

  emit(type, event) {
    for (const listener of this.listeners.get(type) ?? []) {
      listener(event);
    }
  }
}

class FakeWorkerScope {
  constructor() {
    this.listeners = [];
    this.messages = [];
    this.closed = false;
  }

  addEventListener(type, listener) {
    assert.equal(type, "message");
    this.listeners.push(listener);
  }

  postMessage(message) {
    this.messages.push(message);
  }

  close() {
    this.closed = true;
  }

  dispatch(data) {
    for (const listener of this.listeners) {
      listener({ data });
    }
  }

  async request(data) {
    this.dispatch(data);
    for (let attempt = 0; attempt < 20; attempt += 1) {
      const index = this.messages.findIndex(
        (message) => message.requestId === data.requestId,
      );
      if (index >= 0) {
        return this.messages.splice(index, 1)[0];
      }
      await Promise.resolve();
    }
    assert.fail(`worker did not respond to request ${data.requestId}`);
  }
}

async function initializeClient(worker, client) {
  const initializing = client.initialize();
  const request = worker.take("initialize");
  worker.emit("message", {
    data: {
      protocol: TEST_WORKER_PROTOCOL,
      type: "ready",
      requestId: request.requestId,
      transportApiVersion: TEST_TRANSPORT_API_VERSION,
      editorSchema: 1,
      legendDigest: TEST_LEGEND_DIGEST,
      legend: TEST_LEGEND,
    },
  });
  return initializing;
}

async function acknowledgeDocument(worker, pending) {
  await Promise.resolve();
  const message = worker.messages.find(
    (candidate) => candidate.type === "didOpen" || candidate.type === "didChange",
  );
  assert.ok(message, "missing document synchronization message");
  worker.messages.splice(worker.messages.indexOf(message), 1);
  worker.respond(message, null);
  await pending;
}

function fakeMonaco(capture = {}) {
  const disposable = () => ({ dispose() {} });
  return {
    Range: class Range {
      constructor(startLineNumber, startColumn, endLineNumber, endColumn) {
        this.startLineNumber = startLineNumber;
        this.startColumn = startColumn;
        this.endLineNumber = endLineNumber;
        this.endColumn = endColumn;
      }
    },
    MarkerSeverity: { Error: 8, Hint: 1, Info: 2, Warning: 4 },
    editor: { setModelMarkers() {} },
    languages: {
      CompletionItemInsertTextRule: { InsertAsSnippet: 4 },
      CompletionItemKind: { Class: 7, Keyword: 14, Snippet: 27, Variable: 12 },
      SymbolKind: {
        Class: 4,
        Event: 24,
        Function: 11,
        Module: 2,
        Namespace: 3,
        Object: 19,
        Package: 4,
        Property: 6,
        String: 15,
        Struct: 22,
        Variable: 12,
      },
      getLanguages: () => [],
      register: disposable,
      registerCodeActionProvider: disposable,
      registerCompletionItemProvider: disposable,
      registerDefinitionProvider: disposable,
      registerDocumentSemanticTokensProvider(_languageId, provider) {
        capture.semantic?.(provider);
        return disposable();
      },
      registerDocumentSymbolProvider: disposable,
      registerHoverProvider: disposable,
      registerReferenceProvider: disposable,
      registerRenameProvider(_languageId, provider) {
        capture.rename?.(provider);
        return disposable();
      },
      setLanguageConfiguration: disposable,
    },
  };
}
