import assert from "node:assert/strict";
import test from "node:test";
import type {
  BrowserEditorSession,
  RuntimeCatalog,
} from "@mermanjs/web";
import {
  EDITOR_SCHEMA_VERSION,
  EDITOR_WORKER_PROTOCOL,
  type EditorWorkerQuery,
  type EditorWorkerResponse,
} from "./protocol.ts";
import {
  createEditorWorkerRuntime,
  type EditorWorkerRuntimeBindings,
  type EditorWorkerRuntimePort,
} from "./worker-runtime.ts";

const URI = "file:///merman/runtime-test.mmd";
const COMPLETION_TRIGGER_CHARACTERS = [
  " ",
  "\n",
  "-",
  ">",
  "%",
  "[",
  "(",
  "{",
  "/",
  "\\",
  "@",
  ":",
];

class RuntimePort implements EditorWorkerRuntimePort {
  closeCalls = 0;
  readonly messages: EditorWorkerResponse[] = [];
  readonly transfers: (ArrayBuffer[] | undefined)[] = [];

  close(): void {
    this.closeCalls += 1;
  }

  postMessage(message: EditorWorkerResponse, transfer?: ArrayBuffer[]): void {
    this.messages.push(message);
    this.transfers.push(transfer);
  }

  take(requestId: number): EditorWorkerResponse {
    const index = this.messages.findIndex(
      (message) => message.requestId === requestId,
    );
    assert.notEqual(index, -1, `missing response for request ${requestId}`);
    this.transfers.splice(index, 1);
    return this.messages.splice(index, 1)[0];
  }
}

test("runtime owns one native session and projects all semantic query kinds", async () => {
  const port = new RuntimePort();
  const calls: string[] = [];
  const session = createSession(calls);
  const runtime = createEditorWorkerRuntime(port, bindings(session, calls));

  await runtime.receive(request(1, "initialize"));
  assert.deepEqual(port.take(1), {
    protocol: EDITOR_WORKER_PROTOCOL,
    type: "ready",
    requestId: 1,
    completionTriggerCharacters: COMPLETION_TRIGGER_CHARACTERS,
    transportApiVersion: 5,
    editorSchema: EDITOR_SCHEMA_VERSION,
  });

  await runtime.receive({
    ...request(2, "didOpen"),
    document: snapshot(1, "flowchart TD\n  A --> B"),
  });
  assert.equal(port.take(2).type, "result");
  await runtime.receive({
    ...request(3, "didChange"),
    document: snapshot(7, "flowchart TD\n  A --> C"),
  });
  assert.equal(port.take(3).type, "result");

  const queries: readonly EditorWorkerQuery[] = [
    { kind: "diagnostics" },
    { kind: "diagramDetection" },
    { kind: "codeActions" },
    { kind: "completions", position: position() },
    { kind: "hover", position: position() },
    { kind: "documentSymbols" },
    { kind: "definition", position: position() },
    { kind: "references", position: position(), includeDeclaration: true },
    { kind: "prepareRename", position: position() },
    { kind: "rename", position: position(), newName: "Renamed" },
  ];

  for (const [index, query] of queries.entries()) {
    const requestId = 10 + index;
    await runtime.receive(queryRequest(requestId, query, 7));
    const response = port.take(requestId);
    assert.equal(response.type, "queryResult");
  }

  assert.deepEqual(calls, [
    "init",
    "create:1:flowchart TD\n  A --> B",
    "update:7:flowchart TD\n  A --> C",
    "diagnostics",
    "diagramDetection",
    "codeActions",
    "completions",
    "hover",
    "documentSymbols",
    "definition",
    "references",
    "prepareRename",
    "rename",
  ]);

  await runtime.receive({ protocol: EDITOR_WORKER_PROTOCOL, type: "dispose" });
  assert.equal(calls.at(-1), "dispose");
  assert.equal(port.closeCalls, 1);
});

test("initialization rejects an unsupported Web transport API", async () => {
  const port = new RuntimePort();
  const runtimeBindings = bindings(createSession([]), []);
  const runtime = createEditorWorkerRuntime(port, {
    ...runtimeBindings,
    runtimeCatalog: () =>
      ({
        transport_api_version: 6,
        capabilities: { capability_ids: ["editor"] },
      }) as RuntimeCatalog,
    transportApiVersion: () => 6,
  });

  await runtime.receive(request(1, "initialize"));
  const response = port.take(1);
  assert.equal(response.type, "error");
  assert.equal(response.code, "PROTOCOL_MISMATCH");
    assert.match(response.message, /incompatible with 5/u);
});

test("invalid rename remains request-local and later diagnostics still work", async () => {
  const port = new RuntimePort();
  const session = createSession([]);
  session.rename = () => {
    throw {
      version: 1,
      ok: false,
      code: 1,
      code_name: "MERMAN_INVALID_ARGUMENT",
      kind: "generic",
      capability_id: null,
      message: "Rename target must be a valid Mermaid identifier.",
    };
  };
  const runtime = createEditorWorkerRuntime(port, bindings(session, []));
  await openRuntime(runtime, port);

  await runtime.receive(
    queryRequest(3, {
      kind: "rename",
      position: position(),
      newName: "bad name",
    }),
  );
  const failure = port.take(3);
  assert.equal(failure.type, "error");
  assert.equal(failure.code, "OPERATION_REJECTED");
  assert.equal(failure.nativeCode, "MERMAN_INVALID_ARGUMENT");

  await runtime.receive(queryRequest(4, { kind: "diagnostics" }));
  assert.equal(port.take(4).type, "queryResult");
  assert.equal(port.closeCalls, 0);
});

test("malformed messages fail closed and release the native session", async (t) => {
  await t.test("correlated malformed request", async () => {
    const port = new RuntimePort();
    const calls: string[] = [];
    const runtime = createEditorWorkerRuntime(
      port,
      bindings(createSession(calls), calls),
    );
    await openRuntime(runtime, port);

    await runtime.receive({
      protocol: EDITOR_WORKER_PROTOCOL,
      requestId: 9,
      type: "query",
      uri: URI,
      version: 1,
      removedTokenContract: true,
      query: { kind: "references", position: position() },
    });

    const response = port.take(9);
    assert.equal(response.type, "error");
    assert.equal(response.code, "PROTOCOL_MISMATCH");
    assert.equal(port.closeCalls, 1);
    assert.equal(calls.at(-1), "dispose");
  });

  await t.test("uncorrelated decode failure", async () => {
    const port = new RuntimePort();
    const runtime = createEditorWorkerRuntime(
      port,
      bindings(createSession([]), []),
    );
    await runtime.receive({ type: "unknown" });
    assert.deepEqual(port.messages, []);
    assert.equal(port.closeCalls, 1);
  });

  await t.test("messageerror", () => {
    const port = new RuntimePort();
    const runtime = createEditorWorkerRuntime(
      port,
      bindings(createSession([]), []),
    );
    runtime.receiveMessageError();
    runtime.receiveMessageError();
    assert.equal(port.closeCalls, 1);
  });
});

async function openRuntime(
  runtime: ReturnType<typeof createEditorWorkerRuntime>,
  port: RuntimePort,
): Promise<void> {
  await runtime.receive(request(1, "initialize"));
  port.take(1);
  await runtime.receive({
    ...request(2, "didOpen"),
    document: snapshot(1, "flowchart TD"),
  });
  port.take(2);
}

function bindings(
  session: BrowserEditorSession,
  calls: string[],
): EditorWorkerRuntimeBindings {
  return {
    createEditorSession(source, version) {
      calls.push(`create:${version}:${source}`);
      return session;
    },
    async initMerman() {
      calls.push("init");
    },
    editorCompletionTriggerCharacters: () => [
      ...COMPLETION_TRIGGER_CHARACTERS,
    ],
    runtimeCatalog: () =>
      ({
        transport_api_version: 5,
        capabilities: { capability_ids: ["editor"] },
      }) as RuntimeCatalog,
    transportApiVersion: () => 5,
  };
}

function createSession(calls: string[]): BrowserEditorSession {
  let version = 1;
  return {
    uri: URI,
    get version() {
      return version;
    },
    update(source: string, nextVersion: number) {
      calls.push(`update:${nextVersion}:${source}`);
      version = nextVersion;
    },
    diagnostics() {
      calls.push("diagnostics");
      return diagnosticsResult();
    },
    diagramDetection() {
      calls.push("diagramDetection");
      return {
        status: "unavailable",
        validity: "unknown",
        diagramType: null,
        syntaxId: null,
        effectiveLayoutId: null,
      };
    },
    codeActions() {
      calls.push("codeActions");
      return [];
    },
    completions() {
      calls.push("completions");
      return { is_incomplete: false, items: [] };
    },
    hover() {
      calls.push("hover");
      return null;
    },
    documentSymbols() {
      calls.push("documentSymbols");
      return [];
    },
    searchDocumentSymbols() {
      return [];
    },
    definition() {
      calls.push("definition");
      return null;
    },
    references() {
      calls.push("references");
      return [];
    },
    prepareRename() {
      calls.push("prepareRename");
      return null;
    },
    rename() {
      calls.push("rename");
      return null;
    },
    dispose() {
      calls.push("dispose");
    },
  } as unknown as BrowserEditorSession;
}

function diagnosticsResult() {
  return {
    version: EDITOR_SCHEMA_VERSION,
    valid: true,
    summary: { errors: 0, warnings: 0, infos: 0, hints: 0 },
    source: { kind: "diagram" as const, language: "mermaid" },
    diagnostics: [],
  };
}

function request(
  requestId: number,
  type: "didChange" | "didOpen" | "initialize",
) {
  return { protocol: EDITOR_WORKER_PROTOCOL, requestId, type };
}

function queryRequest(
  requestId: number,
  query: EditorWorkerQuery,
  version = 1,
) {
  return {
    protocol: EDITOR_WORKER_PROTOCOL,
    requestId,
    type: "query",
    uri: URI,
    version,
    query,
  };
}

function snapshot(version: number, source: string) {
  return { uri: URI, version, source };
}

function position() {
  return { line: 0, character: 0 };
}
