import assert from "node:assert/strict";
import test from "node:test";
import type {
  CancellationToken,
  Position,
  Range,
  editor,
  languages,
} from "monaco-editor";
import type {
  EditorDocumentIdentity,
  EditorWorkerQuery,
  EditorWorkerQueryResult,
} from "../editor/protocol.ts";
import type {
  EditorLanguageIdentity,
  MermanLanguageWorkerClient,
} from "../editor/worker-client.ts";
import { EditorWorkerProtocolError } from "../editor/worker-client.ts";
import {
  MERMAID_DOCUMENT_URI,
  registerMermaidLanguage,
  type MermaidLanguageRequestRejection,
} from "./mermaid-language.ts";

const IDENTITY: EditorLanguageIdentity = Object.freeze({
  legend: Object.freeze({
    tokenTypes: Object.freeze(["string", "namespace"]),
    tokenModifiers: Object.freeze(["payload", "entity"]),
  }),
  legendDigest: "sha256:test-generated-token-descriptor",
  transportApiVersion: 3,
});

test("Monaco publishes planner-packed tokens without rereading source", async () => {
  const providers: ProviderCapture = {};
  const packed = new Uint32Array([0, 0, 4, 1, 2]);
  const queries: EditorWorkerQuery["kind"][] = [];
  const client = fakeLanguageClient((query) => {
    queries.push(query.kind);
    return resultForQuery(query, packed);
  });
  const model = reactiveModel("flowchart TD", 7);
  const registration = registerMermaidLanguage(
    fakeMonaco(providers),
    client,
    IDENTITY,
  );
  const binding = await registration.bindModel(model.model);
  await Promise.resolve();
  const readsAfterOpen = model.getValueCalls();

  assert.equal(readsAfterOpen, 1);
  assert.deepEqual(providers.semantic?.getLegend(), IDENTITY.legend);
  const result = await providers.semantic?.provideDocumentSemanticTokens(
    model.model,
    null,
    uncancelledToken(),
  );
  assert(result);
  assert("data" in result);
  assert.equal(result.data, packed);
  assert.equal(model.getValueCalls(), readsAfterOpen);
  assert(queries.includes("diagnostics"));
  assert(queries.includes("semanticTokens"));

  binding.dispose();
  registration.dispose();
  assert.equal(client.disposed, true);
});

test("every Monaco query path uses URI/version while source reads stay change-owned", async () => {
  const providers: ProviderCapture = {};
  const identities: EditorDocumentIdentity[] = [];
  const client = fakeLanguageClient((query, identity) => {
    identities.push(identity);
    return resultForQuery(query, new Uint32Array());
  });
  const model = reactiveModel("flowchart TD\nA --> B", 1);
  const registration = registerMermaidLanguage(
    fakeMonaco(providers),
    client,
    IDENTITY,
  );
  const binding = await registration.bindModel(model.model);
  await Promise.resolve();
  const readsAfterOpen = model.getValueCalls();
  const position = monacoPosition(2, 1);
  const token = uncancelledToken();

  await providers.completions?.provideCompletionItems(
    model.model,
    position,
    { triggerKind: 0 as languages.CompletionTriggerKind },
    token,
  );
  await providers.hover?.provideHover(model.model, position, token);
  await providers.codeActions?.provideCodeActions(
    model.model,
    monacoRange(2, 1, 2, 2),
    { markers: [], trigger: 1 as languages.CodeActionTriggerType },
    token,
  );
  await providers.symbols?.provideDocumentSymbols(model.model, token);
  await providers.definition?.provideDefinition(model.model, position, token);
  await providers.references?.provideReferences(
    model.model,
    position,
    { includeDeclaration: true },
    token,
  );
  assert(providers.rename?.resolveRenameLocation);
  await providers.rename.resolveRenameLocation(model.model, position, token);
  await providers.rename?.provideRenameEdits(model.model, position, "B", token);
  await providers.semantic?.provideDocumentSemanticTokens(
    model.model,
    null,
    token,
  );

  assert.equal(model.getValueCalls(), readsAfterOpen);
  assert.deepEqual(
    new Set(identities.map(({ version }) => version)),
    new Set([1]),
  );
  assert.deepEqual(
    new Set(client.queryKinds),
    new Set([
      "codeActions",
      "completions",
      "definition",
      "diagnostics",
      "documentSymbols",
      "hover",
      "prepareRename",
      "references",
      "rename",
      "semanticTokens",
    ]),
  );

  model.change("flowchart TD\nA --> C");
  await Promise.resolve();
  assert.equal(model.getValueCalls(), readsAfterOpen + 1);
  assert.equal(client.changedSnapshots.at(-1)?.source, "flowchart TD\nA --> C");

  binding.dispose();
  registration.dispose();
});

test("a source change is forwarded while didOpen is still pending", async () => {
  let resolveOpen!: () => void;
  const changes: { source: string; version: number }[] = [];
  const client = fakeLanguageClient((query) =>
    resultForQuery(query, new Uint32Array()),
  );
  client.openDocument = () =>
    new Promise<void>((resolve) => {
      resolveOpen = resolve;
    });
  client.changeDocument = async (document) => {
    changes.push({ source: document.source, version: document.version });
  };
  const model = reactiveModel("flowchart TD", 1);
  const registration = registerMermaidLanguage(
    fakeMonaco({}),
    client,
    IDENTITY,
  );

  const bindingPromise = registration.bindModel(model.model);
  model.change("flowchart TD\nA --> B");
  await Promise.resolve();

  assert.deepEqual(changes, [{ version: 2, source: "flowchart TD\nA --> B" }]);
  resolveOpen();
  const binding = await bindingPromise;
  binding.dispose();
  registration.dispose();
});

test("rename rejects workspace edits targeting an unmanaged URI", async () => {
  const providers: ProviderCapture = {};
  let rejection: MermaidLanguageRequestRejection | undefined;
  const client = fakeLanguageClient((query) => {
    if (query.kind !== "rename")
      return resultForQuery(query, new Uint32Array());
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
  const model = reactiveModel("flowchart TD\nA", 3);
  const registration = registerMermaidLanguage(
    fakeMonaco(providers),
    client,
    IDENTITY,
    { onRequestRejected: (value) => (rejection = value) },
  );

  const result = await providers.rename?.provideRenameEdits(
    model.model,
    monacoPosition(2, 1),
    "B",
    uncancelledToken(),
  );

  assert(result);
  assert.equal(result.edits.length, 0);
  assert.match(result.rejectReason ?? "", /current document/i);
  assert.equal(
    rejection?.message,
    "Rename is limited to the current document; received an edit for file:///elsewhere.mmd.",
  );
  registration.dispose();
});

test("request-local query failure falls back without disconnecting language tools", async () => {
  const providers: ProviderCapture = {};
  let hoverAttempts = 0;
  let unavailableCalls = 0;
  const client = fakeLanguageClient((query) => {
    if (query.kind === "hover" && hoverAttempts++ === 0) {
      throw new EditorWorkerProtocolError(
        "Hover failed for this request.",
        "QUERY_FAILED",
      );
    }
    return resultForQuery(query, new Uint32Array());
  });
  const model = reactiveModel("flowchart TD\nA --> B", 1);
  const registration = registerMermaidLanguage(
    fakeMonaco(providers),
    client,
    IDENTITY,
    { onUnavailable: () => (unavailableCalls += 1) },
  );
  const binding = await registration.bindModel(model.model);
  await Promise.resolve();

  const position = monacoPosition(2, 1);
  assert.equal(
    await providers.hover?.provideHover(
      model.model,
      position,
      uncancelledToken(),
    ),
    null,
  );
  assert.equal(unavailableCalls, 0);
  assert.equal(
    await providers.hover?.provideHover(
      model.model,
      position,
      uncancelledToken(),
    ),
    null,
  );
  assert.equal(hoverAttempts, 2);
  assert.equal(client.disposed, false);

  binding.dispose();
  registration.dispose();
});

interface ProviderCapture {
  codeActions?: languages.CodeActionProvider;
  completions?: languages.CompletionItemProvider;
  definition?: languages.DefinitionProvider;
  hover?: languages.HoverProvider;
  references?: languages.ReferenceProvider;
  rename?: languages.RenameProvider;
  semantic?: languages.DocumentSemanticTokensProvider;
  symbols?: languages.DocumentSymbolProvider;
}

interface FakeLanguageClient extends MermanLanguageWorkerClient {
  changedSnapshots: { source: string; version: number }[];
  disposed: boolean;
  queryKinds: EditorWorkerQuery["kind"][];
}

function fakeLanguageClient(
  runQuery: (
    query: EditorWorkerQuery,
    identity: EditorDocumentIdentity,
  ) => unknown,
): FakeLanguageClient {
  return {
    changedSnapshots: [],
    disposed: false,
    queryKinds: [],
    async initialize() {
      return IDENTITY;
    },
    onDidFail() {
      return { dispose() {} };
    },
    async openDocument() {},
    async changeDocument(document) {
      this.changedSnapshots.push({
        source: document.source,
        version: document.version,
      });
    },
    async query<Query extends EditorWorkerQuery>(
      identity: EditorDocumentIdentity,
      query: Query,
    ) {
      this.queryKinds.push(query.kind);
      return runQuery(query, identity) as EditorWorkerQueryResult<Query>;
    },
    dispose() {
      this.disposed = true;
    },
  };
}

function resultForQuery(
  query: EditorWorkerQuery,
  semanticTokens: Uint32Array,
): unknown {
  switch (query.kind) {
    case "diagnostics":
      return {
        version: 1,
        valid: true,
        summary: { errors: 0, warnings: 0, infos: 0, hints: 0 },
        source: { kind: "diagram", language: "mermaid" },
        diagnostics: [],
      };
    case "diagramDetection":
      return {
        status: "unavailable",
        validity: "unknown",
        diagramType: null,
        syntaxId: null,
        effectiveLayoutId: null,
      };
    case "codeActions":
    case "documentSymbols":
    case "references":
      return [];
    case "completions":
      return { is_incomplete: false, items: [] };
    case "hover":
    case "definition":
    case "prepareRename":
      return null;
    case "rename":
      return { changes: {} };
    case "semanticTokens":
      return semanticTokens;
  }
}

function reactiveModel(initialSource: string, initialVersion: number) {
  let source = initialSource;
  let version = initialVersion;
  let listener = () => {};
  let valueReads = 0;
  const model = {
    uri: { toString: () => MERMAID_DOCUMENT_URI },
    getValue() {
      valueReads += 1;
      return source;
    },
    getVersionId: () => version,
    getLineCount: () => source.split("\n").length,
    getLineContent: (lineNumber: number) =>
      source.split("\n")[lineNumber - 1] ?? "",
    getLineMaxColumn: (lineNumber: number) =>
      (source.split("\n")[lineNumber - 1] ?? "").length + 1,
    isDisposed: () => false,
    onDidChangeContent(next: () => void) {
      listener = next;
      return { dispose: () => (listener = () => {}) };
    },
  } as unknown as editor.ITextModel;
  return {
    model,
    change(nextSource: string) {
      source = nextSource;
      version += 1;
      listener();
    },
    getValueCalls: () => valueReads,
  };
}

function uncancelledToken(): CancellationToken {
  return {
    isCancellationRequested: false,
    onCancellationRequested: () => ({ dispose() {} }),
  };
}

function monacoPosition(lineNumber: number, column: number): Position {
  return { lineNumber, column } as unknown as Position;
}

function monacoRange(
  startLineNumber: number,
  startColumn: number,
  endLineNumber: number,
  endColumn: number,
): Range {
  return {
    startLineNumber,
    startColumn,
    endLineNumber,
    endColumn,
  } as unknown as Range;
}

function fakeMonaco(capture: ProviderCapture) {
  const disposable = () => ({ dispose() {} });
  const languages = {
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
    registerCodeActionProvider(
      _id: string,
      provider: languages.CodeActionProvider,
    ) {
      capture.codeActions = provider;
      return disposable();
    },
    registerCompletionItemProvider(
      _id: string,
      provider: languages.CompletionItemProvider,
    ) {
      capture.completions = provider;
      return disposable();
    },
    registerDefinitionProvider(
      _id: string,
      provider: languages.DefinitionProvider,
    ) {
      capture.definition = provider;
      return disposable();
    },
    registerDocumentSemanticTokensProvider(
      _id: string,
      provider: languages.DocumentSemanticTokensProvider,
    ) {
      capture.semantic = provider;
      return disposable();
    },
    registerDocumentSymbolProvider(
      _id: string,
      provider: languages.DocumentSymbolProvider,
    ) {
      capture.symbols = provider;
      return disposable();
    },
    registerHoverProvider(_id: string, provider: languages.HoverProvider) {
      capture.hover = provider;
      return disposable();
    },
    registerReferenceProvider(
      _id: string,
      provider: languages.ReferenceProvider,
    ) {
      capture.references = provider;
      return disposable();
    },
    registerRenameProvider(_id: string, provider: languages.RenameProvider) {
      capture.rename = provider;
      return disposable();
    },
    setLanguageConfiguration: disposable,
  };
  return {
    Range: class Range {
      readonly startLineNumber: number;
      readonly startColumn: number;
      readonly endLineNumber: number;
      readonly endColumn: number;

      constructor(
        startLineNumber: number,
        startColumn: number,
        endLineNumber: number,
        endColumn: number,
      ) {
        this.startLineNumber = startLineNumber;
        this.startColumn = startColumn;
        this.endLineNumber = endLineNumber;
        this.endColumn = endColumn;
      }
    },
    MarkerSeverity: { Error: 8, Hint: 1, Info: 2, Warning: 4 },
    editor: { setModelMarkers() {} },
    languages,
  } as unknown as typeof import("monaco-editor");
}
