import type { editor, IDisposable, languages } from "monaco-editor";
import type {
  EditorCodeAction,
  EditorCompletionItem,
  EditorDiagnostic,
  EditorDocumentSymbol,
  EditorLocation,
  EditorRange,
  EditorSymbolKind,
  EditorWorkspaceEdit,
} from "@mermanjs/web";
import type {
  EditorDocumentIdentity,
  EditorDocumentSnapshot,
  EditorWorkerQuery,
} from "@/src/editor/protocol";
import {
  EditorWorkerProtocolError,
  type EditorCancellationToken,
  type EditorLanguageIdentity,
  type MermanLanguageWorkerClient,
} from "../editor/worker-client.ts";

export const MERMAID_LANGUAGE_ID = "mermaid";
export const MERMAID_DOCUMENT_URI = "file:///merman/playground.mmd";

const MARKER_OWNER = "merman";
const DIAGNOSTIC_DELAY_MS = 180;

export type MermaidSemanticTokenLegend = EditorLanguageIdentity["legend"];

export interface MermaidLanguageRegistration extends IDisposable {
  bindModel(model: editor.ITextModel): Promise<IDisposable>;
}

export interface MermaidLanguageRequestRejection {
  readonly detail: string | null;
  readonly message: string;
  readonly nativeCode: string | null;
  readonly operation: "rename";
}

export interface MermaidLanguageCallbacks {
  readonly onRequestRejected?: (
    rejection: MermaidLanguageRequestRejection,
  ) => void;
  readonly onUnavailable?: (error: Error) => void;
}

const configuredMonacoInstances = new WeakSet<object>();

const mermaidLanguageConfig: languages.LanguageConfiguration = {
  comments: { lineComment: "%%" },
  brackets: [
    ["{", "}"],
    ["[", "]"],
    ["(", ")"],
  ],
  autoClosingPairs: [
    { open: "{", close: "}" },
    { open: "[", close: "]" },
    { open: "(", close: ")" },
    { open: '"', close: '"' },
    { open: "'", close: "'" },
  ],
};

export function registerMermaidLanguage(
  monaco: typeof import("monaco-editor"),
  client: MermanLanguageWorkerClient,
  identity: EditorLanguageIdentity,
  callbacks: MermaidLanguageCallbacks = {},
): MermaidLanguageRegistration {
  ensureMermaidLanguageRegistered(monaco);
  const disposables: IDisposable[] = [];
  const modelBindings = new Set<IDisposable>();
  const semanticListeners = new Set<() => void>();
  let managedModel: editor.ITextModel | null = null;
  let disposed = false;
  let unavailable = false;
  const notifyUnavailable = (error: unknown) => {
    if (disposed || unavailable || isExpectedDiscard(error)) return;
    unavailable = true;
    if (managedModel && !managedModel.isDisposed()) {
      clearMermaidMarkers(monaco, managedModel);
    }
    const failure = error instanceof Error ? error : new Error(String(error));
    console.error("Merman editor language worker failed", failure);
    callbacks.onUnavailable?.(failure);
  };
  const rejectRename = (
    message: string,
    detail: string | null = null,
    nativeCode: string | null = null,
  ) => {
    callbacks.onRequestRejected?.({
      detail,
      message,
      nativeCode,
      operation: "rename",
    });
    const nativeCodeSuffix = nativeCode ? ` (${nativeCode})` : "";
    return { edits: [], rejectReason: `${message}${nativeCodeSuffix}` };
  };
  const query = <Query extends EditorWorkerQuery, Fallback>(
    model: editor.ITextModel,
    request: Query,
    token: EditorCancellationToken | undefined,
    fallback: Fallback,
  ) => queryOr(client, model, request, token, fallback, notifyUnavailable);
  const legend: languages.SemanticTokensLegend = {
    tokenTypes: [...identity.legend.tokenTypes],
    tokenModifiers: [...identity.legend.tokenModifiers],
  };

  disposables.push(
    monaco.languages.registerCompletionItemProvider(MERMAID_LANGUAGE_ID, {
      triggerCharacters: [...identity.completionTriggerCharacters],
      async provideCompletionItems(model, position, _context, token) {
        const completions = await query(
          model,
          {
            kind: "completions",
            position: toEditorPosition(position),
          },
          token,
          null,
        );
        if (!completions) return { suggestions: [] };
        return {
          incomplete: completions.is_incomplete,
          suggestions: completions.items.map((item) =>
            toEditorCompletionItem(monaco, item, position),
          ),
        };
      },
    }),
  );

  disposables.push(
    monaco.languages.registerHoverProvider(MERMAID_LANGUAGE_ID, {
      async provideHover(model, position, token) {
        const hover = await query(
          model,
          { kind: "hover", position: toEditorPosition(position) },
          token,
          null,
        );
        if (!hover) return null;
        return {
          range: hover.range ? toMonacoRange(monaco, hover.range) : undefined,
          contents: [{ value: hover.contents.value }],
        };
      },
    }),
  );

  disposables.push(
    monaco.languages.registerCodeActionProvider(MERMAID_LANGUAGE_ID, {
      async provideCodeActions(model, _range, context, token) {
        const actions = await query(model, { kind: "codeActions" }, token, []);
        return {
          actions: actions
            .filter((action) =>
              action.diagnostics.some((diagnostic) =>
                context.markers.some((marker) =>
                  markerMatchesDiagnostic(monaco, model, marker, diagnostic),
                ),
              ),
            )
            .map((action) => toMonacoCodeAction(monaco, model, action)),
          dispose() {},
        };
      },
    }),
  );

  disposables.push(
    monaco.languages.registerDocumentSymbolProvider(MERMAID_LANGUAGE_ID, {
      async provideDocumentSymbols(model, token) {
        const symbols = await query(
          model,
          { kind: "documentSymbols" },
          token,
          [],
        );
        return symbols.map((symbol) => toMonacoDocumentSymbol(monaco, symbol));
      },
    }),
  );

  disposables.push(
    monaco.languages.registerDefinitionProvider(MERMAID_LANGUAGE_ID, {
      async provideDefinition(model, position, token) {
        const location = await query(
          model,
          { kind: "definition", position: toEditorPosition(position) },
          token,
          null,
        );
        return location ? toMonacoLocation(monaco, model, location) : null;
      },
    }),
  );

  disposables.push(
    monaco.languages.registerReferenceProvider(MERMAID_LANGUAGE_ID, {
      async provideReferences(model, position, context, token) {
        const locations = await query(
          model,
          {
            kind: "references",
            position: toEditorPosition(position),
            includeDeclaration: context.includeDeclaration,
          },
          token,
          [],
        );
        return locations.map((location) =>
          toMonacoLocation(monaco, model, location),
        );
      },
    }),
  );

  disposables.push(
    monaco.languages.registerRenameProvider(MERMAID_LANGUAGE_ID, {
      async resolveRenameLocation(model, position, token) {
        const prepare = await query(
          model,
          { kind: "prepareRename", position: toEditorPosition(position) },
          token,
          null,
        );
        return prepare
          ? {
              range: toMonacoRange(monaco, prepare.range),
              text: prepare.placeholder,
            }
          : null;
      },
      async provideRenameEdits(model, position, newName, token) {
        let edit: EditorWorkspaceEdit | null;
        try {
          edit = await client.query(
            identityForModel(model),
            {
              kind: "rename",
              position: toEditorPosition(position),
              newName,
            },
            token,
          );
        } catch (error) {
          if (isExpectedDiscard(error)) {
            return { edits: [], rejectReason: "Rename request was canceled." };
          }
          if (isRequestLocalWorkerError(error)) {
            return rejectRename(error.message, error.detail, error.nativeCode);
          }
          notifyUnavailable(error);
          return {
            edits: [],
            rejectReason: "Mermaid language tools are unavailable.",
          };
        }
        if (!edit) {
          return rejectRename("No Mermaid symbol at cursor.");
        }
        try {
          return toMonacoWorkspaceEdit(monaco, model, edit);
        } catch (error) {
          if (error instanceof UnmanagedDocumentEditError) {
            return rejectRename(error.message);
          }
          throw error;
        }
      },
    }),
  );

  disposables.push(
    monaco.languages.registerDocumentSemanticTokensProvider(
      MERMAID_LANGUAGE_ID,
      {
        getLegend: () => legend,
        onDidChange(listener) {
          semanticListeners.add(listener);
          return { dispose: () => semanticListeners.delete(listener) };
        },
        async provideDocumentSemanticTokens(model, _lastResultId, token) {
          const tokens = await query(
            model,
            { kind: "semanticTokens" },
            token,
            new Uint32Array(),
          );
          return {
            data: tokens,
            resultId: undefined,
          };
        },
        releaseDocumentSemanticTokens() {},
      },
    ),
  );

  return {
    async bindModel(model) {
      if (disposed)
        throw new Error("Mermaid language registration is disposed.");
      if (managedModel) {
        throw new Error("Mermaid language registration already owns a model.");
      }
      const snapshot = snapshotForModel(model);
      if (snapshot.uri !== MERMAID_DOCUMENT_URI) {
        throw new Error(
          `Mermaid editor model must use ${MERMAID_DOCUMENT_URI}; received ${snapshot.uri}.`,
        );
      }
      managedModel = model;
      let opening: Promise<void>;
      try {
        opening = client.openDocument(snapshot);
      } catch (error) {
        managedModel = null;
        throw error;
      }

      let diagnosticTimer: ReturnType<typeof setTimeout> | null = null;
      let bindingDisposed = false;
      const publishDiagnostics = async () => {
        const current = identityForModel(model);
        const result = await query(
          model,
          { kind: "diagnostics" },
          undefined,
          null,
        );
        if (
          result &&
          !bindingDisposed &&
          !model.isDisposed() &&
          model.getVersionId() === current.version
        ) {
          updateMermaidEditorMarkers(monaco, model, result.diagnostics);
        }
      };
      const scheduleDiagnostics = () => {
        if (diagnosticTimer !== null) clearTimeout(diagnosticTimer);
        diagnosticTimer = setTimeout(() => {
          diagnosticTimer = null;
          void publishDiagnostics().catch(notifyUnavailable);
        }, DIAGNOSTIC_DELAY_MS);
      };
      const contentListener = model.onDidChangeContent(() => {
        try {
          void client
            .changeDocument(snapshotForModel(model))
            .catch(notifyUnavailable);
        } catch (error) {
          notifyUnavailable(error);
        }
        for (const listener of semanticListeners) listener();
        scheduleDiagnostics();
      });
      const binding: IDisposable = {
        dispose() {
          if (bindingDisposed) return;
          bindingDisposed = true;
          contentListener.dispose();
          if (diagnosticTimer !== null) clearTimeout(diagnosticTimer);
          if (!model.isDisposed()) clearMermaidMarkers(monaco, model);
          if (managedModel === model) managedModel = null;
          modelBindings.delete(binding);
        },
      };
      modelBindings.add(binding);
      try {
        await opening;
        if (disposed || bindingDisposed || model.isDisposed()) {
          throw new Error("Mermaid editor model was disposed while opening.");
        }
        for (const listener of semanticListeners) listener();
        void publishDiagnostics().catch(notifyUnavailable);
        return binding;
      } catch (error) {
        binding.dispose();
        throw error;
      }
    },
    dispose() {
      if (disposed) return;
      disposed = true;
      for (const binding of [...modelBindings]) binding.dispose();
      for (const disposable of disposables.reverse()) disposable.dispose();
      semanticListeners.clear();
      client.dispose();
    },
  };
}

export function ensureMermaidLanguageRegistered(
  monaco: typeof import("monaco-editor"),
): void {
  if (configuredMonacoInstances.has(monaco)) return;
  if (
    !monaco.languages
      .getLanguages()
      .some((language) => language.id === MERMAID_LANGUAGE_ID)
  ) {
    // Monaco keeps language IDs for the realm lifetime and returns no handle.
    monaco.languages.register({ id: MERMAID_LANGUAGE_ID });
  }
  monaco.languages.setLanguageConfiguration(
    MERMAID_LANGUAGE_ID,
    mermaidLanguageConfig,
  );
  configuredMonacoInstances.add(monaco);
}

async function queryOr<Query extends EditorWorkerQuery, Fallback>(
  client: MermanLanguageWorkerClient,
  model: editor.ITextModel,
  query: Query,
  token: EditorCancellationToken | undefined,
  fallback: Fallback,
  onFailure: (error: unknown) => void,
) {
  try {
    return await client.query(identityForModel(model), query, token);
  } catch (error) {
    if (isExpectedDiscard(error)) return fallback;
    if (isRequestLocalWorkerError(error)) return fallback;
    onFailure(error);
    return fallback;
  }
}

function isRequestLocalWorkerError(
  error: unknown,
): error is EditorWorkerProtocolError {
  return (
    error instanceof EditorWorkerProtocolError &&
    (error.code === "OPERATION_REJECTED" || error.code === "QUERY_FAILED")
  );
}

function snapshotForModel(model: editor.ITextModel): EditorDocumentSnapshot {
  return {
    ...identityForModel(model),
    source: model.getValue(),
  };
}

function identityForModel(model: editor.ITextModel): EditorDocumentIdentity {
  return {
    uri: model.uri.toString(),
    version: model.getVersionId(),
  };
}

function toEditorPosition(position: { lineNumber: number; column: number }): {
  line: number;
  character: number;
} {
  return { line: position.lineNumber - 1, character: position.column - 1 };
}

function toEditorCompletionItem(
  monaco: typeof import("monaco-editor"),
  item: EditorCompletionItem,
  position: { lineNumber: number; column: number },
): languages.CompletionItem {
  const fallbackRange = new monaco.Range(
    position.lineNumber,
    position.column,
    position.lineNumber,
    position.column,
  );
  return {
    label: item.label_details
      ? {
          label: item.label,
          detail: item.label_details.detail ?? undefined,
          description: item.label_details.description ?? undefined,
        }
      : item.label,
    kind: completionKind(monaco, item.kind),
    insertText: item.text_edit?.new_text ?? item.insert_text ?? item.label,
    insertTextRules:
      item.insert_text_format === "snippet"
        ? monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet
        : undefined,
    detail: item.detail ?? undefined,
    range: item.text_edit?.range
      ? toMonacoRange(monaco, item.text_edit.range)
      : fallbackRange,
  };
}

function completionKind(
  monaco: typeof import("monaco-editor"),
  kind: EditorCompletionItem["kind"],
): languages.CompletionItemKind {
  switch (kind) {
    case "class":
      return monaco.languages.CompletionItemKind.Class;
    case "snippet":
      return monaco.languages.CompletionItemKind.Snippet;
    case "variable":
      return monaco.languages.CompletionItemKind.Variable;
    case "keyword":
      return monaco.languages.CompletionItemKind.Keyword;
  }
}

function toMonacoCodeAction(
  monaco: typeof import("monaco-editor"),
  model: editor.ITextModel,
  action: EditorCodeAction,
): languages.CodeAction {
  return {
    title: action.title,
    kind: action.kind,
    diagnostics: action.diagnostics.map((diagnostic) =>
      toMarkerData(monaco, model, diagnostic),
    ),
    edit: toMonacoWorkspaceEdit(monaco, model, action.edit),
    isPreferred: action.isPreferred,
  };
}

function toMonacoWorkspaceEdit(
  monaco: typeof import("monaco-editor"),
  model: editor.ITextModel,
  edit: EditorWorkspaceEdit,
): languages.WorkspaceEdit {
  const managedUri = model.uri.toString();
  const entries = Object.entries(edit.changes);
  const unmanaged = entries.find(([uri]) => uri !== managedUri);
  if (unmanaged) {
    throw new UnmanagedDocumentEditError(
      `Rename is limited to the current document; received an edit for ${unmanaged[0]}.`,
    );
  }
  return {
    edits: entries.flatMap(([, edits]) =>
      edits.map((textEdit) => ({
        resource: model.uri,
        versionId: model.getVersionId(),
        textEdit: {
          range: toMonacoRange(monaco, textEdit.range),
          text: textEdit.newText,
        },
      })),
    ),
  };
}

function toMonacoLocation(
  monaco: typeof import("monaco-editor"),
  model: editor.ITextModel,
  location: EditorLocation,
): languages.Location {
  if (location.uri !== model.uri.toString()) {
    throw new UnmanagedDocumentEditError(
      `Navigation is limited to the current document; received ${location.uri}.`,
    );
  }
  return { uri: model.uri, range: toMonacoRange(monaco, location.range) };
}

function toMonacoDocumentSymbol(
  monaco: typeof import("monaco-editor"),
  symbol: EditorDocumentSymbol,
): languages.DocumentSymbol {
  return {
    name: symbol.name,
    detail: symbol.detail ?? "",
    kind: symbolKind(monaco, symbol.kind),
    tags: [],
    range: toMonacoRange(monaco, symbol.range),
    selectionRange: toMonacoRange(monaco, symbol.selectionRange),
    children: symbol.children.map((child) =>
      toMonacoDocumentSymbol(monaco, child),
    ),
  };
}

function updateMermaidEditorMarkers(
  monaco: typeof import("monaco-editor"),
  model: editor.ITextModel,
  diagnostics: EditorDiagnostic[],
): void {
  monaco.editor.setModelMarkers(
    model,
    MARKER_OWNER,
    diagnostics.map((diagnostic) => toMarkerData(monaco, model, diagnostic)),
  );
}

function clearMermaidMarkers(
  monaco: typeof import("monaco-editor"),
  model: editor.ITextModel,
): void {
  monaco.editor.setModelMarkers(model, MARKER_OWNER, []);
}

function toMarkerData(
  monaco: typeof import("monaco-editor"),
  model: editor.ITextModel,
  diagnostic: EditorDiagnostic,
): editor.IMarkerData {
  const range = toMonacoDisplayRange(monaco, diagnostic.range);
  const relatedRanges = diagnostic.related.map((related) => ({
    related,
    range: toMonacoDisplayRange(monaco, related.range),
  }));
  return {
    startLineNumber: range.startLineNumber,
    startColumn: range.startColumn,
    endLineNumber: range.endLineNumber,
    endColumn: range.endColumn,
    severity: diagnosticSeverity(monaco, diagnostic.severity),
    message: diagnostic.message,
    source: diagnostic.source || "Merman",
    tags: diagnostic.tags?.map((tag) => diagnosticMarkerTag(monaco, tag)),
    code:
      typeof diagnostic.code === "number"
        ? String(diagnostic.code)
        : diagnostic.code,
    relatedInformation: relatedRanges.map(
      ({ related, range: relatedRange }) => ({
        resource: model.uri,
        message: related.message,
        startLineNumber: relatedRange.startLineNumber,
        startColumn: relatedRange.startColumn,
        endLineNumber: relatedRange.endLineNumber,
        endColumn: relatedRange.endColumn,
      }),
    ),
  };
}

function diagnosticMarkerTag(
  monaco: typeof import("monaco-editor"),
  tag: NonNullable<EditorDiagnostic["tags"]>[number],
): import("monaco-editor").MarkerTag {
  switch (tag) {
    case "deprecated":
      return monaco.MarkerTag.Deprecated;
    default:
      throw new Error(`unsupported diagnostic tag: ${tag}`);
  }
}

function markerMatchesDiagnostic(
  monaco: typeof import("monaco-editor"),
  model: editor.ITextModel,
  marker: editor.IMarkerData,
  diagnostic: EditorDiagnostic,
): boolean {
  const expected = toMarkerData(monaco, model, diagnostic);
  return (
    marker.message === expected.message &&
    marker.source === expected.source &&
    normalizeMarkerCode(marker.code) === normalizeMarkerCode(expected.code) &&
    marker.startLineNumber === expected.startLineNumber &&
    marker.startColumn === expected.startColumn &&
    marker.endLineNumber === expected.endLineNumber &&
    marker.endColumn === expected.endColumn
  );
}

function normalizeMarkerCode(code: editor.IMarkerData["code"]): string {
  if (code === undefined) return "";
  return typeof code === "object" ? String(code.value) : String(code);
}

function toMonacoRange(
  monaco: typeof import("monaco-editor"),
  range: EditorRange,
): InstanceType<typeof monaco.Range> {
  return new monaco.Range(
    range.start.line + 1,
    range.start.character + 1,
    range.end.line + 1,
    Math.max(range.end.character + 1, 1),
  );
}

function toMonacoDisplayRange(
  monaco: typeof import("monaco-editor"),
  range: EditorRange,
): InstanceType<typeof monaco.Range> {
  const monacoRange = toMonacoRange(monaco, range);
  if (
    monacoRange.startLineNumber === monacoRange.endLineNumber &&
    monacoRange.startColumn === monacoRange.endColumn
  ) {
    return new monaco.Range(
      monacoRange.startLineNumber,
      monacoRange.startColumn,
      monacoRange.endLineNumber,
      monacoRange.endColumn + 1,
    );
  }
  return monacoRange;
}

function diagnosticSeverity(
  monaco: typeof import("monaco-editor"),
  severity: EditorDiagnostic["severity"],
): (typeof monaco.MarkerSeverity)[keyof typeof monaco.MarkerSeverity] {
  switch (severity) {
    case "hint":
      return monaco.MarkerSeverity.Hint;
    case "info":
      return monaco.MarkerSeverity.Info;
    case "warning":
      return monaco.MarkerSeverity.Warning;
    case "error":
      return monaco.MarkerSeverity.Error;
  }
}

function symbolKind(
  monaco: typeof import("monaco-editor"),
  kind: EditorSymbolKind,
): languages.SymbolKind {
  switch (kind) {
    case "class":
      return monaco.languages.SymbolKind.Class;
    case "event":
      return monaco.languages.SymbolKind.Event;
    case "function":
      return monaco.languages.SymbolKind.Function;
    case "module":
      return monaco.languages.SymbolKind.Module;
    case "namespace":
      return monaco.languages.SymbolKind.Namespace;
    case "object":
      return monaco.languages.SymbolKind.Object;
    case "package":
      return monaco.languages.SymbolKind.Package;
    case "property":
      return monaco.languages.SymbolKind.Property;
    case "string":
      return monaco.languages.SymbolKind.String;
    case "struct":
      return monaco.languages.SymbolKind.Struct;
    case "variable":
      return monaco.languages.SymbolKind.Variable;
  }
}

function isExpectedDiscard(error: unknown): boolean {
  return (
    error instanceof Error &&
    (error.name === "AbortError" || error.name === "StaleLanguageSnapshotError")
  );
}

class UnmanagedDocumentEditError extends Error {}
