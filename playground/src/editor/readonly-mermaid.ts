import type { CancellationToken, editor, IDisposable, languages } from "monaco-editor";

import {
  StaleSyntaxSnapshotError,
  type MermaidSyntaxWorkerStartup,
} from "./syntax-worker-client.ts";
import { MERMAID_SYNTAX_TOKEN_LEGEND } from "./syntax-tokens.ts";

export const READONLY_MERMAID_LANGUAGE_ID = "merman-readonly-mermaid";

type Monaco = typeof import("monaco-editor");
type StartSyntaxWorker = () => MermaidSyntaxWorkerStartup;

interface ModelSyntax extends IDisposable {
  change(): void;
  tokens(token: CancellationToken): Promise<languages.SemanticTokens>;
}

interface SyntaxRegistry extends IDisposable {
  readonly models: Map<editor.ITextModel, ModelSyntax>;
  readonly notify: () => void;
  owners: number;
}

const registries = new WeakMap<Monaco, SyntaxRegistry>();

export function ensureReadOnlyMermaidLanguage(monaco: Monaco): void {
  // Monaco language IDs live for the realm lifetime; reuse one ID for all previews.
  if (!monaco.languages.getLanguages().some(({ id }) => id === READONLY_MERMAID_LANGUAGE_ID)) {
    monaco.languages.register({ id: READONLY_MERMAID_LANGUAGE_ID });
  }
}

export function attachReadOnlyMermaidSyntax(
  monaco: Monaco,
  preview: editor.IStandaloneCodeEditor,
  startWorker: StartSyntaxWorker,
): IDisposable {
  const registry = acquireRegistry(monaco);
  let model: editor.ITextModel | null = null;
  let syntax: ModelSyntax | null = null;
  let disposed = false;

  const releaseModel = () => {
    if (model && registry.models.get(model) === syntax) registry.models.delete(model);
    syntax?.dispose();
    model = null;
    syntax = null;
  };
  const bindModel = () => {
    releaseModel();
    const next = preview.getModel();
    if (!next || next.isDisposed() || next.getLanguageId() !== READONLY_MERMAID_LANGUAGE_ID) return;
    model = next;
    syntax = createModelSyntax(next, startWorker, registry.notify);
    if (syntax) registry.models.set(next, syntax);
  };
  const binding: IDisposable = {
    dispose() {
      if (disposed) return;
      disposed = true;
      for (const listener of listeners) listener.dispose();
      releaseModel();
      registry.owners -= 1;
      if (registry.owners === 0) registry.dispose();
    },
  };
  const listeners = [
    preview.onDidChangeModel(bindModel),
    preview.onDidChangeModelLanguage(bindModel),
    preview.onDidChangeModelContent(() => syntax?.change()),
    preview.onDidDispose(() => binding.dispose()),
  ];
  bindModel();
  return binding;
}

function acquireRegistry(monaco: Monaco): SyntaxRegistry {
  ensureReadOnlyMermaidLanguage(monaco);
  const existing = registries.get(monaco);
  if (existing) {
    existing.owners += 1;
    return existing;
  }
  const models = new Map<editor.ITextModel, ModelSyntax>();
  const listeners = new Set<() => void>();
  const provider = monaco.languages.registerDocumentSemanticTokensProvider(
    READONLY_MERMAID_LANGUAGE_ID,
    {
      getLegend: () => ({
        tokenTypes: [...MERMAID_SYNTAX_TOKEN_LEGEND.tokenTypes],
        tokenModifiers: [...MERMAID_SYNTAX_TOKEN_LEGEND.tokenModifiers],
      }),
      onDidChange(listener) {
        listeners.add(listener);
        return { dispose: () => listeners.delete(listener) };
      },
      provideDocumentSemanticTokens(model, _lastResultId, token) {
        return models.get(model)?.tokens(token) ?? emptyTokens();
      },
      releaseDocumentSemanticTokens() {},
    },
  );
  const registry: SyntaxRegistry = {
    models,
    owners: 1,
    notify: () => { for (const listener of listeners) listener(); },
    dispose() {
      provider.dispose();
      listeners.clear();
      registries.delete(monaco);
    },
  };
  registries.set(monaco, registry);
  return registry;
}

function createModelSyntax(
  model: editor.ITextModel,
  startWorker: StartSyntaxWorker,
  notify: () => void,
): ModelSyntax | null {
  let startup: MermaidSyntaxWorkerStartup;
  try {
    startup = startWorker();
  } catch {
    return null;
  }
  const { client } = startup;
  let disposed = false;
  let failed = false;
  const active = () => !disposed && !failed && !model.isDisposed();
  const fail = () => {
    if (disposed || failed) return;
    failed = true;
    client.dispose();
    notify();
  };
  const snapshot = () => ({
    uri: model.uri.toString(),
    version: model.getVersionId(),
    source: model.getValue(),
  });
  const opening = snapshot();
  let version = opening.version;
  let synchronization = startup.ready
    .then(async () => { if (active()) await client.openDocument(opening); })
    .catch(fail);
  const failureListener = client.onDidFail(fail);
  void synchronization.then(() => { if (active()) notify(); });

  return {
    change() {
      if (!active() || model.getVersionId() <= version) return;
      const next = snapshot();
      version = next.version;
      synchronization = synchronization
        .then(async () => { if (active()) await client.changeDocument(next); })
        .catch(fail);
    },
    async tokens(token) {
      if (!active() || token.isCancellationRequested) return emptyTokens();
      const identity = { uri: model.uri.toString(), version: model.getVersionId() };
      try {
        await synchronization;
        if (!active() || token.isCancellationRequested || model.getVersionId() !== identity.version) {
          return emptyTokens();
        }
        const data = await client.highlights(identity);
        return active() && !token.isCancellationRequested && model.getVersionId() === identity.version
          ? { data }
          : emptyTokens();
      } catch (error) {
        if (!(error instanceof StaleSyntaxSnapshotError)) fail();
        return emptyTokens();
      }
    },
    dispose() {
      if (disposed) return;
      disposed = true;
      failureListener.dispose();
      client.dispose();
    },
  };
}

function emptyTokens(): languages.SemanticTokens {
  return { data: new Uint32Array() };
}
