import assert from "node:assert/strict";
import test from "node:test";
import type { CancellationToken, Position, editor, languages } from "monaco-editor";
import type {
  EditorDocumentIdentity,
  EditorWorkerQuery,
  EditorWorkerQueryResult,
} from "../editor/protocol.ts";
import type {
  EditorLanguageIdentity,
  MermanLanguageWorkerClient,
} from "../editor/worker-client.ts";
import type {
  MermaidSyntaxWorkerClient,
} from "../editor/syntax-worker-client.ts";
import { MERMAID_SYNTAX_TOKEN_LEGEND } from "../editor/syntax-tokens.ts";
import {
  MERMAID_DOCUMENT_URI,
  registerMermaidLanguage,
  type MermaidLanguageRequestRejection,
} from "./mermaid-language.ts";

const IDENTITY: EditorLanguageIdentity = Object.freeze({
  completionTriggerCharacters: Object.freeze([" ", "\n", "-", ":"]),
  transportApiVersion: 5,
});

test("Tree-sitter supplies Monaco tokens while semantic updates flush on demand", async () => {
  const providers: ProviderCapture = {};
  const semantic = fakeSemanticClient();
  const syntax = fakeSyntaxClient(new Uint32Array([0, 0, 9, 3, 0]));
  const model = reactiveModel("flowchart TD", 1);
  const registration = await registerMermaidLanguage(
    fakeMonaco(providers),
    startups(semantic, syntax),
  );
  const binding = await registration.bindModel(model.model);
  await Promise.resolve();

  assert.deepEqual(providers.tokens?.getLegend(), {
    tokenTypes: [...MERMAID_SYNTAX_TOKEN_LEGEND.tokenTypes],
    tokenModifiers: [],
  });
  const tokens = await providers.tokens?.provideDocumentSemanticTokens(
    model.model,
    null,
    uncancelledToken(),
  );
  assert(tokens && "data" in tokens);
  assert.deepEqual([...tokens.data], [0, 0, 9, 3, 0]);
  assert.deepEqual(syntax.highlightVersions, [1]);

  model.change("flowchart TD\nA --> B");
  await Promise.resolve();
  assert.equal(syntax.changedSnapshots.at(-1)?.version, 2);
  assert.equal(semantic.changedSnapshots.length, 0);

  await providers.completions?.provideCompletionItems(
    model.model,
    monacoPosition(2, 1),
    { triggerKind: 0 as languages.CompletionTriggerKind },
    uncancelledToken(),
  );
  assert.equal(semantic.changedSnapshots.at(-1)?.version, 2);
  assert(semantic.queryKinds.includes("completions"));

  binding.dispose();
  registration.dispose();
});

test("syntax failure leaves Merman semantic features available", async () => {
  const providers: ProviderCapture = {};
  const semantic = fakeSemanticClient();
  const syntax = fakeSyntaxClient();
  syntax.highlights = async () => {
    throw new Error("syntax crashed");
  };
  const syntaxFailures: Error[] = [];
  const registration = await registerMermaidLanguage(
    fakeMonaco(providers),
    startups(semantic, syntax),
    { onSyntaxUnavailable: (error) => syntaxFailures.push(error) },
  );
  const model = reactiveModel("flowchart TD", 1);
  const binding = await registration.bindModel(model.model);

  const tokens = await providers.tokens?.provideDocumentSemanticTokens(
    model.model,
    null,
    uncancelledToken(),
  );
  assert(tokens && "data" in tokens);
  assert.equal(tokens.data.length, 0);
  assert.match(syntaxFailures[0]?.message ?? "", /syntax crashed/);
  await providers.hover?.provideHover(
    model.model,
    monacoPosition(1, 1),
    uncancelledToken(),
  );
  assert(semantic.queryKinds.includes("hover"));

  binding.dispose();
  registration.dispose();
});

test("semantic startup failure leaves Tree-sitter highlighting available", async () => {
  const providers: ProviderCapture = {};
  const semantic = fakeSemanticClient();
  const syntax = fakeSyntaxClient(new Uint32Array([0, 0, 4, 3, 0]));
  const semanticFailures: Error[] = [];
  const registration = await registerMermaidLanguage(
    fakeMonaco(providers),
    {
      semantic: { client: semantic, ready: Promise.reject(new Error("semantic failed")) },
      syntax: { client: syntax, ready: Promise.resolve() },
    },
    { onSemanticUnavailable: (error) => semanticFailures.push(error) },
  );
  const model = reactiveModel("flowchart TD", 1);
  const binding = await registration.bindModel(model.model);

  const tokens = await providers.tokens?.provideDocumentSemanticTokens(
    model.model,
    null,
    uncancelledToken(),
  );
  assert(tokens && "data" in tokens);
  assert.deepEqual([...tokens.data], [0, 0, 4, 3, 0]);
  assert.match(semanticFailures[0]?.message ?? "", /semantic failed/);
  const hover = await providers.hover?.provideHover(
    model.model,
    monacoPosition(1, 1),
    uncancelledToken(),
  );
  assert.equal(hover, null);

  binding.dispose();
  registration.dispose();
});

test("pending semantic readiness does not block Tree-sitter highlighting", async () => {
  const providers: ProviderCapture = {};
  const semantic = fakeSemanticClient();
  const syntax = fakeSyntaxClient(new Uint32Array([0, 0, 9, 3, 0]));
  const registration = await settleWithin(
    registerMermaidLanguage(fakeMonaco(providers), {
      semantic: { client: semantic, ready: new Promise(() => {}) },
      syntax: { client: syntax, ready: Promise.resolve() },
    }),
    "language registration",
  );
  const model = reactiveModel("flowchart TD", 1);
  const binding = await settleWithin(
    registration.bindModel(model.model),
    "model binding",
  );
  const tokens = await settleWithin(
    Promise.resolve(
      providers.tokens!.provideDocumentSemanticTokens(
        model.model,
        null,
        uncancelledToken(),
      ),
    ),
    "syntax highlighting",
  );

  assert(tokens && "data" in tokens);
  assert.deepEqual([...tokens.data], [0, 0, 9, 3, 0]);

  binding.dispose();
  registration.dispose();
});

test("pending semantic document open does not block Tree-sitter highlighting", async () => {
  const providers: ProviderCapture = {};
  const semantic = fakeSemanticClient();
  semantic.openDocument = () => new Promise<void>(() => {});
  const syntax = fakeSyntaxClient(new Uint32Array([0, 0, 9, 3, 0]));
  const registration = await registerMermaidLanguage(
    fakeMonaco(providers),
    startups(semantic, syntax),
  );
  const model = reactiveModel("flowchart TD", 1);
  const binding = await settleWithin(
    registration.bindModel(model.model),
    "model binding",
  );
  const tokens = await settleWithin(
    Promise.resolve(
      providers.tokens!.provideDocumentSemanticTokens(
        model.model,
        null,
        uncancelledToken(),
      ),
    ),
    "syntax highlighting",
  );

  assert(tokens && "data" in tokens);
  assert.deepEqual([...tokens.data], [0, 0, 9, 3, 0]);

  binding.dispose();
  registration.dispose();
});

test("disposing a pending binding keeps the registration one-shot", async () => {
  let resolveSemantic!: (identity: EditorLanguageIdentity) => void;
  let resolveSyntax!: () => void;
  const semanticReady = new Promise<EditorLanguageIdentity>((resolve) => {
    resolveSemantic = resolve;
  });
  const syntaxReady = new Promise<void>((resolve) => {
    resolveSyntax = resolve;
  });
  const semantic = fakeSemanticClient();
  const syntax = fakeSyntaxClient();
  let semanticOpens = 0;
  let syntaxOpens = 0;
  semantic.openDocument = async () => {
    semanticOpens += 1;
  };
  syntax.openDocument = async () => {
    syntaxOpens += 1;
  };
  const registration = await registerMermaidLanguage(fakeMonaco({}), {
    semantic: { client: semantic, ready: semanticReady },
    syntax: { client: syntax, ready: syntaxReady },
  });
  const binding = await registration.bindModel(
    reactiveModel("flowchart TD", 1).model,
  );

  binding.dispose();
  resolveSemantic(IDENTITY);
  resolveSyntax();
  await Promise.all([semanticReady, syntaxReady]);
  await Promise.resolve();

  assert.equal(semanticOpens, 0);
  assert.equal(syntaxOpens, 0);
  await assert.rejects(
    registration.bindModel(reactiveModel("flowchart LR", 1).model),
    /owns one model lifetime/,
  );
  registration.dispose();
});

test("rename still rejects edits targeting an unmanaged URI", async () => {
  const providers: ProviderCapture = {};
  const semantic = fakeSemanticClient((query) => {
    if (query.kind !== "rename") return resultForQuery(query);
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
  const rejections: MermaidLanguageRequestRejection[] = [];
  const registration = await registerMermaidLanguage(
    fakeMonaco(providers),
    startups(semantic, fakeSyntaxClient()),
    { onRequestRejected: (value) => rejections.push(value) },
  );
  const model = reactiveModel("flowchart TD\nA", 1);
  const binding = await registration.bindModel(model.model);
  const result = await providers.rename?.provideRenameEdits(
    model.model,
    monacoPosition(2, 1),
    "B",
    uncancelledToken(),
  );

  assert.equal(result?.edits.length, 0);
  assert.match(rejections[0]?.message ?? "", /current document/);
  binding.dispose();
  registration.dispose();
});

interface ProviderCapture {
  completions?: languages.CompletionItemProvider;
  hover?: languages.HoverProvider;
  rename?: languages.RenameProvider;
  tokens?: languages.DocumentSemanticTokensProvider;
}

interface FakeSemanticClient extends MermanLanguageWorkerClient {
  readonly changedSnapshots: { source: string; version: number }[];
  readonly queryKinds: EditorWorkerQuery["kind"][];
}

interface FakeSyntaxClient extends MermaidSyntaxWorkerClient {
  readonly changedSnapshots: { source: string; version: number }[];
  readonly highlightVersions: number[];
  highlights(document: EditorDocumentIdentity): Promise<Uint32Array>;
}

function startups(semantic: FakeSemanticClient, syntax: FakeSyntaxClient) {
  return {
    semantic: { client: semantic, ready: Promise.resolve(IDENTITY) },
    syntax: { client: syntax, ready: Promise.resolve() },
  };
}

function fakeSemanticClient(
  runQuery: (query: EditorWorkerQuery) => unknown = resultForQuery,
): FakeSemanticClient {
  return {
    changedSnapshots: [],
    queryKinds: [],
    async initialize() {
      return IDENTITY;
    },
    onDidFail() {
      return { dispose() {} };
    },
    async openDocument() {},
    async changeDocument(document) {
      this.changedSnapshots.push({ source: document.source, version: document.version });
    },
    async query<Query extends EditorWorkerQuery>(
      _identity: EditorDocumentIdentity,
      query: Query,
    ) {
      this.queryKinds.push(query.kind);
      return runQuery(query) as EditorWorkerQueryResult<Query>;
    },
    dispose() {},
  };
}

function fakeSyntaxClient(tokens = new Uint32Array()): FakeSyntaxClient {
  return {
    changedSnapshots: [],
    highlightVersions: [],
    async initialize() {},
    onDidFail() {
      return { dispose() {} };
    },
    async openDocument() {},
    async changeDocument(document) {
      this.changedSnapshots.push({ source: document.source, version: document.version });
    },
    async highlights(document) {
      this.highlightVersions.push(document.version);
      return tokens;
    },
    dispose() {},
  };
}

function resultForQuery(query: EditorWorkerQuery): unknown {
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
  }
}

function reactiveModel(initialSource: string, initialVersion: number) {
  let source = initialSource;
  let version = initialVersion;
  let listener = () => {};
  const model = {
    uri: { toString: () => MERMAID_DOCUMENT_URI },
    getValue: () => source,
    getVersionId: () => version,
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
  };
}

function uncancelledToken(): CancellationToken {
  return {
    isCancellationRequested: false,
    onCancellationRequested: () => ({ dispose() {} }),
  };
}

function monacoPosition(lineNumber: number, column: number): Position {
  return { lineNumber, column } as Position;
}

async function settleWithin<Value>(
  promise: Promise<Value>,
  label: string,
): Promise<Value> {
  let timeout: ReturnType<typeof setTimeout> | undefined;
  try {
    return await Promise.race([
      promise,
      new Promise<never>((_resolve, reject) => {
        timeout = setTimeout(
          () => reject(new Error(`${label} did not settle independently.`)),
          100,
        );
      }),
    ]);
  } finally {
    if (timeout !== undefined) clearTimeout(timeout);
  }
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
    registerCodeActionProvider: disposable,
    registerDefinitionProvider: disposable,
    registerDocumentSymbolProvider: disposable,
    registerReferenceProvider: disposable,
    registerCompletionItemProvider(_id: string, provider: languages.CompletionItemProvider) {
      capture.completions = provider;
      return disposable();
    },
    registerDocumentSemanticTokensProvider(
      _id: string,
      provider: languages.DocumentSemanticTokensProvider,
    ) {
      capture.tokens = provider;
      return disposable();
    },
    registerHoverProvider(_id: string, provider: languages.HoverProvider) {
      capture.hover = provider;
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
    MarkerTag: { Deprecated: 2 },
    MarkerSeverity: { Error: 8, Hint: 1, Info: 2, Warning: 4 },
    editor: { setModelMarkers() {} },
    languages,
  } as unknown as typeof import("monaco-editor");
}
