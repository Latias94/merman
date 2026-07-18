import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";
import test from "node:test";
import vm from "node:vm";
import ts from "typescript";

const root = path.resolve(import.meta.dirname, "..");

function loadTypeScriptModule(relativePath) {
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
      module,
      exports: module.exports,
      require(specifier) {
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

test("semantic token encoding follows the Rust-provided legend", () => {
  const { encodeSemanticTokensForLegend } = loadTypeScriptModule(
    "src/lib/mermaid-language.ts",
  );
  const data = encodeSemanticTokensForLegend(
    [
      {
        line: 0,
        start: 3,
        length: 5,
        tokenType: "namespace",
        tokenModifier: "entity",
      },
    ],
    {
      tokenTypes: ["string", "namespace"],
      tokenModifiers: ["payload", "entity"],
    },
  );

  assert.deepEqual([...data], [0, 3, 5, 1, 2]);
});

test("semantic token encoding rejects unknown Rust token names", () => {
  const { encodeSemanticTokensForLegend } = loadTypeScriptModule(
    "src/lib/mermaid-language.ts",
  );
  assert.throws(
    () =>
      encodeSemanticTokensForLegend(
        [
          {
            line: 0,
            start: 0,
            length: 4,
            tokenType: "future-token",
            tokenModifier: "entity",
          },
        ],
        {
          tokenTypes: ["namespace"],
          tokenModifiers: ["entity"],
        },
      ),
    /unknown semantic token type/i,
  );
});

test("Monaco providers await the document client and advertise its immutable legend", async () => {
  const { registerMermaidLanguage } = loadTypeScriptModule(
    "src/lib/mermaid-language.ts",
  );
  let semanticProvider;
  const legend = {
    tokenTypes: ["string", "namespace"],
    tokenModifiers: ["payload", "entity"],
  };
  const client = fakeLanguageClient(({ query }) => {
    assert.equal(query.kind, "semanticTokens");
    return [
      {
        line: 0,
        start: 0,
        length: 4,
        tokenType: "namespace",
        tokenModifier: "entity",
      },
    ];
  });

  const registration = registerMermaidLanguage(
    fakeMonaco({
      semantic: (provider) => {
        semanticProvider = provider;
      },
    }),
    client,
    legend,
  );

  assert.deepEqual(semanticProvider.getLegend(), legend);
  const result = await semanticProvider.provideDocumentSemanticTokens(
    fakeModel("flowchart TD", 7),
    undefined,
    uncancelledToken(),
  );

  assert.deepEqual([...result.data], [0, 0, 4, 1, 2]);
  registration.dispose();
  assert.equal(client.disposed, true);
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
    { tokenTypes: ["namespace"], tokenModifiers: ["entity"] },
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

  const registration = registerMermaidLanguage(
    fakeMonaco({
      rename: (provider) => {
        renameProvider = provider;
      },
    }),
    client,
    { tokenTypes: ["namespace"], tokenModifiers: ["entity"] },
  );
  const result = await renameProvider.provideRenameEdits(
    model,
    { lineNumber: 2, column: 1 },
    "B",
    uncancelledToken(),
  );

  assert.equal(result.edits.length, 0);
  assert.match(result.rejectReason, /current document/i);
  registration.dispose();
});

test("worker client discards a response after the managed document version changes", async () => {
  const { createMermanLanguageWorkerClient, StaleDocumentError } =
    loadTypeScriptModule("src/editor/worker-client.ts");
  const worker = new FakeWorker();
  const client = createMermanLanguageWorkerClient(worker);
  await initializeClient(worker, client);
  await acknowledgeDocument(worker, client.openDocument(snapshot(1, "flowchart TD")));

  const query = client.query(snapshot(1, "flowchart TD"), {
    kind: "diagnostics",
  });
  const queryMessage = await worker.takeEventually("query");
  const change = client.changeDocument(snapshot(2, "flowchart TD\nA-->B"));
  worker.respond(queryMessage, { diagnostics: [] });

  await assert.rejects(query, (error) => error instanceof StaleDocumentError);
  await acknowledgeDocument(worker, change);
  client.dispose();
});

test("worker client cancellation rejects immediately and sends an explicit cancel", async () => {
  const { createMermanLanguageWorkerClient } = loadTypeScriptModule(
    "src/editor/worker-client.ts",
  );
  const worker = new FakeWorker();
  const client = createMermanLanguageWorkerClient(worker);
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
  assert.equal(worker.take("cancel").requestId, queryMessage.requestId);
  client.dispose();
});

test("worker client does not send a query canceled while awaiting didChange", async () => {
  const { createMermanLanguageWorkerClient } = loadTypeScriptModule(
    "src/editor/worker-client.ts",
  );
  const worker = new FakeWorker();
  const client = createMermanLanguageWorkerClient(worker);
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

test("worker client poisons every pending request on a malformed dedicated response", async () => {
  const { createMermanLanguageWorkerClient } = loadTypeScriptModule(
    "src/editor/worker-client.ts",
  );
  const worker = new FakeWorker();
  const client = createMermanLanguageWorkerClient(worker);
  await initializeClient(worker, client);
  await acknowledgeDocument(worker, client.openDocument(snapshot(1, "flowchart TD")));
  const query = client.query(snapshot(1, "flowchart TD"), {
    kind: "diagnostics",
  });
  await worker.takeEventually("query");

  worker.emit("message", {
    data: { protocol: 1, type: "result", requestId: "not-a-request-id" },
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
  const client = createMermanLanguageWorkerClient(worker);
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
  const client = createMermanLanguageWorkerClient(worker);
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

  respond(request, result) {
    this.emit("message", {
      data: {
        protocol: 1,
        type: "result",
        requestId: request.requestId,
        result,
      },
    });
  }

  fail(request, code, message) {
    this.emit("message", {
      data: {
        protocol: 1,
        type: "error",
        requestId: request.requestId,
        code,
        message,
      },
    });
  }

  emit(type, event) {
    for (const listener of this.listeners.get(type) ?? []) {
      listener(event);
    }
  }
}

async function initializeClient(worker, client) {
  const initializing = client.initialize();
  const request = worker.take("initialize");
  worker.emit("message", {
    data: {
      protocol: 1,
      type: "ready",
      requestId: request.requestId,
      nativeAbi: 2,
      editorSchema: 1,
      legend: { tokenTypes: ["namespace"], tokenModifiers: ["entity"] },
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
