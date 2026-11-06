import assert from "node:assert/strict";
import test from "node:test";
import type { CancellationToken, editor, languages } from "monaco-editor";

import {
  attachReadOnlyMermaidSyntax,
  READONLY_MERMAID_LANGUAGE_ID,
} from "./readonly-mermaid.ts";
import type { SyntaxDocumentSnapshot } from "./syntax-protocol.ts";
import {
  StaleSyntaxSnapshotError,
  type MermaidSyntaxWorkerClient,
  type MermaidSyntaxWorkerStartup,
} from "./syntax-worker-client.ts";
import { MERMAID_SYNTAX_TOKEN_LEGEND } from "./syntax-tokens.ts";

const uncancelled: CancellationToken = {
  isCancellationRequested: false,
  onCancellationRequested: () => ({ dispose() {} }),
};

test("read-only models share one provider without touching the main Mermaid model", async () => {
  const monaco = fakeMonaco();
  const first = fakePreview("first", "flowchart TD");
  const second = fakePreview("second", 'sequenceDiagram\nA->>B: "Hello"');
  const a = fakeWorker();
  const b = fakeWorker();
  const bindingA = attachReadOnlyMermaidSyntax(monaco.api, first.editor, () => a.startup);
  const bindingB = attachReadOnlyMermaidSyntax(monaco.api, second.editor, () => b.startup);

  assert.equal(monaco.registrations, 1);
  assert.equal(monaco.providers.size, 1);
  const provider = monaco.provider();
  assert.deepEqual(provider.getLegend(), MERMAID_SYNTAX_TOKEN_LEGEND);
  assert.deepEqual(await data(provider, first.model), [0, 0, 9, 3, 0]);
  assert.deepEqual(await data(provider, second.model), [0, 0, 9, 3, 0]);
  const main = fakePreview("main", "flowchart LR", "mermaid");
  assert.deepEqual(await data(provider, main.model), []);
  assert.equal(a.opens[0]?.uri, "inmemory://preview/first.mmd");
  assert.equal(b.opens[0]?.uri, "inmemory://preview/second.mmd");

  bindingA.dispose();
  assert.equal(first.listenerCount(), 0);
  assert.equal(a.disposals, 1);
  assert.equal(monaco.providers.size, 1);
  assert.deepEqual(await data(provider, first.model), []);
  assert.deepEqual(await data(provider, second.model), [0, 0, 9, 3, 0]);
  second.dispose();
  bindingB.dispose();
  assert.equal(b.disposals, 1);
  assert.equal(second.listenerCount(), 0);
  assert.equal(monaco.providers.size, 0);

  const reopened = attachReadOnlyMermaidSyntax(monaco.api, first.editor, () => fakeWorker().startup);
  assert.equal(monaco.registrations, 1);
  assert.equal(monaco.providers.size, 1);
  reopened.dispose();
});

test("initialization and rapid content changes preserve document order and the latest version", async () => {
  const monaco = fakeMonaco();
  const preview = fakePreview("changing", "flowchart TD");
  const ready = deferred<void>();
  const worker = fakeWorker(ready.promise);
  const binding = attachReadOnlyMermaidSyntax(monaco.api, preview.editor, () => worker.startup);
  preview.change('flowchart TD\nA["Old"]');
  preview.change('flowchart TD\nA["Latest"] --> B');
  assert.equal(worker.opens.length, 0);
  ready.resolve();

  assert.deepEqual(await data(monaco.provider(), preview.model), [0, 0, 9, 3, 0]);
  assert.equal(worker.opens[0]?.source, "flowchart TD");
  assert.deepEqual(worker.changes.map(({ version }) => version), [2, 3]);
  assert.equal(worker.changes.at(-1)?.source, 'flowchart TD\nA["Latest"] --> B');
  assert.deepEqual(worker.queries, [3]);
  binding.dispose();
});

test("model and language switches release old workers including pending initialization", async () => {
  const monaco = fakeMonaco();
  const preview = fakePreview("switching", "flowchart TD");
  const oldReady = deferred<void>();
  const workers = [fakeWorker(oldReady.promise)];
  const binding = attachReadOnlyMermaidSyntax(monaco.api, preview.editor, () => {
    if (workers.length === 1 && workers[0].disposals === 0) return workers[0].startup;
    const next = fakeWorker();
    workers.push(next);
    return next.startup;
  });
  const oldModel = preview.model;
  preview.replace("replacement", 'flowchart LR\nA["New"]');
  oldReady.resolve();
  assert.deepEqual(await data(monaco.provider(), oldModel), []);
  assert.deepEqual(await data(monaco.provider(), preview.model), [0, 0, 9, 3, 0]);
  assert.equal(workers[0].disposals, 1);
  assert.equal(workers[0].opens.length, 0);
  assert.equal(workers[1].opens[0]?.uri, "inmemory://preview/replacement.mmd");

  preview.language("json");
  assert.equal(workers[1].disposals, 1);
  preview.language("rust");
  preview.change('fn main() { println!("Hello"); }');
  assert.equal(workers.length, 2);
  assert.deepEqual(await data(monaco.provider(), preview.model), []);

  preview.language(READONLY_MERMAID_LANGUAGE_ID);
  preview.change("flowchart TB\nA --> B");
  assert.deepEqual(await data(monaco.provider(), preview.model), [0, 0, 9, 3, 0]);
  assert.equal(workers.length, 3);
  assert.equal(workers[2].changes.at(-1)?.source, "flowchart TB\nA --> B");
  preview.dispose();
  binding.dispose();
  assert(workers.every((worker) => worker.disposals === 1 && worker.failureListeners.size === 0));
  assert.equal(monaco.providers.size, 0);
});

test("late and cancelled highlights never color a newer document", async () => {
  const monaco = fakeMonaco();
  const preview = fakePreview("late", "flowchart TD");
  const worker = fakeWorker();
  const result = deferred<Uint32Array>();
  const requested = deferred<void>();
  worker.startup.client.highlights = async () => {
    requested.resolve();
    return result.promise;
  };
  const binding = attachReadOnlyMermaidSyntax(monaco.api, preview.editor, () => worker.startup);
  const provider = monaco.provider();
  const pending = data(provider, preview.model);
  await requested.promise;
  preview.change('flowchart TD\nA["New"]');
  result.resolve(new Uint32Array([0, 0, 9, 3, 0]));
  assert.deepEqual(await pending, []);
  assert.equal(worker.disposals, 0);

  worker.startup.client.highlights = async () => { throw new StaleSyntaxSnapshotError(); };
  assert.deepEqual(await data(provider, preview.model), []);
  assert.equal(worker.disposals, 0);
  worker.startup.client.highlights = async () => new Uint32Array([0, 0, 9, 3, 0]);
  assert.deepEqual(await data(provider, preview.model), [0, 0, 9, 3, 0]);
  assert.deepEqual(await data(provider, preview.model, { ...uncancelled, isCancellationRequested: true }), []);
  binding.dispose();
});

test("worker failures fall back to plain text without disabling another preview", async () => {
  for (const failure of ["construct", "initialize", "open", "change", "highlights", "event"] as const) {
    const monaco = fakeMonaco();
    const broken = fakePreview("broken", 'flowchart TD\nA["Still readable"]');
    const healthy = fakePreview("healthy", "flowchart LR\nA --> B");
    const worker = fakeWorker(failure === "initialize" ? Promise.reject(new Error("startup failed")) : undefined);
    const reject = async () => { throw new Error("worker failed"); };
    if (failure === "open") worker.startup.client.openDocument = reject;
    if (failure === "change") worker.startup.client.changeDocument = reject;
    if (failure === "highlights") worker.startup.client.highlights = reject;
    const brokenBinding = attachReadOnlyMermaidSyntax(monaco.api, broken.editor, () => {
      if (failure === "construct") throw new Error("Worker is unavailable");
      return worker.startup;
    });
    const healthyBinding = attachReadOnlyMermaidSyntax(monaco.api, healthy.editor, () => fakeWorker().startup);
    const provider = monaco.provider();
    if (failure === "change") broken.change('flowchart TD\nA["Changed, still readable"]');
    if (failure === "event") {
      await data(provider, broken.model);
      for (const listener of worker.failureListeners) listener(new Error("transport failed"));
    }
    assert.deepEqual(await data(provider, broken.model), [], failure);
    assert.match(broken.model.getValue(), /readable/);
    assert.deepEqual(await data(provider, healthy.model), [0, 0, 9, 3, 0]);
    brokenBinding.dispose();
    healthyBinding.dispose();
    assert.equal(worker.disposals, failure === "construct" ? 0 : 1);
    assert.equal(worker.failureListeners.size, 0);
    assert.equal(monaco.providers.size, 0);
  }
});

test("disposing during initialization does not open a document or retain listeners", async () => {
  const monaco = fakeMonaco();
  const preview = fakePreview("closed", "flowchart TD");
  const ready = deferred<void>();
  const worker = fakeWorker(ready.promise);
  const binding = attachReadOnlyMermaidSyntax(monaco.api, preview.editor, () => worker.startup);
  const provider = monaco.provider();
  const pending = data(provider, preview.model);
  binding.dispose();
  ready.resolve();
  assert.deepEqual(await pending, []);
  assert.equal(worker.opens.length, 0);
  assert.equal(worker.disposals, 1);
  assert.equal(worker.failureListeners.size, 0);
  assert.equal(preview.listenerCount(), 0);
  assert.equal(monaco.providers.size, 0);
});

async function data(
  provider: languages.DocumentSemanticTokensProvider,
  model: editor.ITextModel,
  token = uncancelled,
): Promise<number[]> {
  const result = await provider.provideDocumentSemanticTokens(model, null, token);
  assert(result && "data" in result);
  return [...result.data];
}

function fakeMonaco() {
  const ids: string[] = [];
  const providers = new Set<languages.DocumentSemanticTokensProvider>();
  return {
    providers,
    get registrations() { return ids.length; },
    provider() { assert.equal(providers.size, 1); return [...providers][0]; },
    api: {
      languages: {
        getLanguages: () => ids.map((id) => ({ id })),
        register({ id }: { id: string }) { ids.push(id); },
        registerDocumentSemanticTokensProvider(id: string, provider: languages.DocumentSemanticTokensProvider) {
          assert.equal(id, READONLY_MERMAID_LANGUAGE_ID);
          providers.add(provider);
          return { dispose: () => providers.delete(provider) };
        },
      },
    } as unknown as typeof import("monaco-editor"),
  };
}

function fakePreview(name: string, initialSource: string, initialLanguage = READONLY_MERMAID_LANGUAGE_ID) {
  let source = initialSource;
  let version = 1;
  let language = initialLanguage;
  const events = { model: new Set<() => void>(), language: new Set<() => void>(), content: new Set<() => void>(), dispose: new Set<() => void>() };
  const listen = (kind: keyof typeof events) => (listener: () => void) => {
    events[kind].add(listener);
    return { dispose: () => events[kind].delete(listener) };
  };
  const createModel = (id: string) => ({
    uri: { toString: () => `inmemory://preview/${id}.mmd` },
    getValue: () => source,
    getVersionId: () => version,
    getLanguageId: () => language,
    isDisposed: () => false,
  }) as unknown as editor.ITextModel;
  let model = createModel(name);
  return {
    get model() { return model; },
    editor: {
      getModel: () => model,
      onDidChangeModel: listen("model"),
      onDidChangeModelLanguage: listen("language"),
      onDidChangeModelContent: listen("content"),
      onDidDispose: listen("dispose"),
    } as unknown as editor.IStandaloneCodeEditor,
    change(next: string) { source = next; version += 1; for (const listener of events.content) listener(); },
    language(next: string) { language = next; for (const listener of events.language) listener(); },
    replace(id: string, next: string) {
      source = next;
      version = 1;
      model = createModel(id);
      for (const listener of events.model) listener();
    },
    dispose() { for (const listener of events.dispose) listener(); },
    listenerCount: () => Object.values(events).reduce((total, listeners) => total + listeners.size, 0),
  };
}

function fakeWorker(ready = Promise.resolve()) {
  let disposals = 0;
  const opens: SyntaxDocumentSnapshot[] = [];
  const changes: SyntaxDocumentSnapshot[] = [];
  const queries: number[] = [];
  const failureListeners = new Set<(error: Error) => void>();
  const client: MermaidSyntaxWorkerClient = {
    initialize: () => ready,
    openDocument: async (snapshot) => { opens.push(snapshot); },
    changeDocument: async (snapshot) => { changes.push(snapshot); },
    highlights: async ({ version }) => { queries.push(version); return new Uint32Array([0, 0, 9, 3, 0]); },
    onDidFail(listener) { failureListeners.add(listener); return { dispose: () => failureListeners.delete(listener) }; },
    dispose() { disposals = 1; },
  };
  return { startup: { ready, client } satisfies MermaidSyntaxWorkerStartup, opens, changes, queries, failureListeners, get disposals() { return disposals; } };
}

function deferred<Value>() {
  let resolve!: (value: Value | PromiseLike<Value>) => void;
  const promise = new Promise<Value>((done) => { resolve = done; });
  return { promise, resolve };
}
