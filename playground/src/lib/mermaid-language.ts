import type { editor, IDisposable, languages } from "monaco-editor";
import type {
  EditorCodeAction,
  EditorCompletionItem,
  EditorDiagnostic,
  EditorDocumentSymbol,
  EditorLocation,
  EditorRange,
  EditorSemanticToken,
  EditorSemanticTokenLegend,
  EditorSymbolKind,
  EditorWorkspaceEdit,
} from "@mermanjs/web";
import type {
  EditorDocumentSnapshot,
  EditorWorkerQuery,
} from "@/src/editor/protocol";
import type {
  EditorCancellationToken,
  MermanLanguageWorkerClient,
} from "@/src/editor/worker-client";

export const MERMAID_LANGUAGE_ID = "mermaid";
export const MERMAID_DOCUMENT_URI = "file:///merman/playground.mmd";

const MARKER_OWNER = "merman";
const DIAGNOSTIC_DELAY_MS = 180;

export type MermaidSemanticTokenLegend = EditorSemanticTokenLegend;

export interface MermaidLanguageRegistration extends IDisposable {
  bindModel(model: editor.ITextModel): Promise<IDisposable>;
}

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
  legend: MermaidSemanticTokenLegend
): MermaidLanguageRegistration {
  const disposables: IDisposable[] = [];
  const modelBindings = new Set<IDisposable>();
  const semanticListeners = new Set<() => void>();
  let managedModel: editor.ITextModel | null = null;
  let disposed = false;

  if (
    !monaco.languages
      .getLanguages()
      .some((language) => language.id === MERMAID_LANGUAGE_ID)
  ) {
    // Monaco 0.55 keeps language IDs for the realm lifetime and returns no handle.
    monaco.languages.register({ id: MERMAID_LANGUAGE_ID });
  }
  disposables.push(
    monaco.languages.setLanguageConfiguration(
      MERMAID_LANGUAGE_ID,
      mermaidLanguageConfig
    )
  );

  disposables.push(
    monaco.languages.registerCompletionItemProvider(MERMAID_LANGUAGE_ID, {
      triggerCharacters: [" ", "\n", "-", "@", ":"],
      async provideCompletionItems(model, position, _context, token) {
        const completions = await queryOr(
          client,
          model,
          {
            kind: "completions",
            position: toEditorPosition(position),
          },
          token,
          null
        );
        if (!completions) return { suggestions: [] };
        return {
          incomplete: completions.is_incomplete,
          suggestions: completions.items.map((item) =>
            toEditorCompletionItem(monaco, item, position)
          ),
        };
      },
    })
  );

  disposables.push(
    monaco.languages.registerHoverProvider(MERMAID_LANGUAGE_ID, {
      async provideHover(model, position, token) {
        const hover = await queryOr(
          client,
          model,
          { kind: "hover", position: toEditorPosition(position) },
          token,
          null
        );
        if (!hover) return null;
        return {
          range: hover.range ? toMonacoRange(monaco, hover.range) : undefined,
          contents: [{ value: hover.contents.value }],
        };
      },
    })
  );

  disposables.push(
    monaco.languages.registerCodeActionProvider(MERMAID_LANGUAGE_ID, {
      async provideCodeActions(model, _range, context, token) {
        const actions = await queryOr(
          client,
          model,
          { kind: "codeActions" },
          token,
          []
        );
        return {
          actions: actions
            .filter((action) =>
              action.diagnostics.some((diagnostic) =>
                context.markers.some((marker) =>
                  markerMatchesDiagnostic(monaco, model, marker, diagnostic)
                )
              )
            )
            .map((action) => toMonacoCodeAction(monaco, model, action)),
          dispose() {},
        };
      },
    })
  );

  disposables.push(
    monaco.languages.registerDocumentSymbolProvider(MERMAID_LANGUAGE_ID, {
      async provideDocumentSymbols(model, token) {
        const symbols = await queryOr(
          client,
          model,
          { kind: "documentSymbols" },
          token,
          []
        );
        return symbols.map((symbol) =>
          toMonacoDocumentSymbol(monaco, symbol)
        );
      },
    })
  );

  disposables.push(
    monaco.languages.registerDefinitionProvider(MERMAID_LANGUAGE_ID, {
      async provideDefinition(model, position, token) {
        const location = await queryOr(
          client,
          model,
          { kind: "definition", position: toEditorPosition(position) },
          token,
          null
        );
        return location
          ? toMonacoLocation(monaco, model, location)
          : null;
      },
    })
  );

  disposables.push(
    monaco.languages.registerReferenceProvider(MERMAID_LANGUAGE_ID, {
      async provideReferences(model, position, context, token) {
        const locations = await queryOr(
          client,
          model,
          {
            kind: "references",
            position: toEditorPosition(position),
            includeDeclaration: context.includeDeclaration,
          },
          token,
          []
        );
        return locations.map((location) =>
          toMonacoLocation(monaco, model, location)
        );
      },
    })
  );

  disposables.push(
    monaco.languages.registerRenameProvider(MERMAID_LANGUAGE_ID, {
      async resolveRenameLocation(model, position, token) {
        const prepare = await queryOr(
          client,
          model,
          { kind: "prepareRename", position: toEditorPosition(position) },
          token,
          null
        );
        return prepare
          ? {
              range: toMonacoRange(monaco, prepare.range),
              text: prepare.placeholder,
            }
          : null;
      },
      async provideRenameEdits(model, position, newName, token) {
        const edit = await queryOr(
          client,
          model,
          {
            kind: "rename",
            position: toEditorPosition(position),
            newName,
          },
          token,
          null
        );
        if (!edit) {
          return { edits: [], rejectReason: "No Mermaid symbol at cursor." };
        }
        try {
          return toMonacoWorkspaceEdit(monaco, model, edit);
        } catch (error) {
          if (error instanceof UnmanagedDocumentEditError) {
            return { edits: [], rejectReason: error.message };
          }
          throw error;
        }
      },
    })
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
          const tokens = await queryOr(
            client,
            model,
            { kind: "semanticTokens" },
            token,
            []
          );
          return {
            data: encodeSemanticTokensForLegend(tokens, legend),
            resultId: undefined,
          };
        },
        releaseDocumentSemanticTokens() {},
      }
    )
  );

  return {
    async bindModel(model) {
      if (disposed) throw new Error("Mermaid language registration is disposed.");
      if (managedModel) {
        throw new Error("Mermaid language registration already owns a model.");
      }
      const snapshot = snapshotForModel(model);
      if (snapshot.uri !== MERMAID_DOCUMENT_URI) {
        throw new Error(
          `Mermaid editor model must use ${MERMAID_DOCUMENT_URI}; received ${snapshot.uri}.`
        );
      }
      managedModel = model;
      await client.openDocument(snapshot);
      if (disposed || model.isDisposed()) {
        throw new Error("Mermaid editor model was disposed while opening.");
      }

      let diagnosticTimer: ReturnType<typeof setTimeout> | null = null;
      let bindingDisposed = false;
      const publishDiagnostics = async () => {
        const current = snapshotForModel(model);
        const result = await queryOr(
          client,
          model,
          { kind: "diagnostics" },
          undefined,
          null
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
          void publishDiagnostics().catch(reportEditorWorkerFailure);
        }, DIAGNOSTIC_DELAY_MS);
      };
      const contentListener = model.onDidChangeContent(() => {
        try {
          void client
            .changeDocument(snapshotForModel(model))
            .catch(reportEditorWorkerFailure);
        } catch (error) {
          reportEditorWorkerFailure(error);
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
      for (const listener of semanticListeners) listener();
      void publishDiagnostics().catch(reportEditorWorkerFailure);
      return binding;
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

export function encodeSemanticTokensForLegend(
  tokens: EditorSemanticToken[],
  legend: MermaidSemanticTokenLegend
): Uint32Array {
  const data: number[] = [];
  let previousLine = 0;
  let previousStart = 0;
  const sorted = [...tokens].sort(
    (left, right) =>
      left.line - right.line ||
      left.start - right.start ||
      left.length - right.length
  );

  for (const token of sorted) {
    const tokenType = legend.tokenTypes.indexOf(token.tokenType);
    const modifierIndex = legend.tokenModifiers.indexOf(token.tokenModifier);
    if (tokenType < 0) {
      throw new SemanticTokenContractError(
        `Unknown semantic token type from Rust: ${token.tokenType}.`
      );
    }
    if (modifierIndex < 0) {
      throw new SemanticTokenContractError(
        `Unknown semantic token modifier from Rust: ${token.tokenModifier}.`
      );
    }
    if (
      !Number.isSafeInteger(token.line) ||
      !Number.isSafeInteger(token.start) ||
      !Number.isSafeInteger(token.length) ||
      token.line < 0 ||
      token.start < 0 ||
      token.length <= 0
    ) {
      throw new SemanticTokenContractError("Rust returned an invalid semantic token range.");
    }
    const deltaLine = token.line - previousLine;
    const deltaStart = deltaLine === 0 ? token.start - previousStart : token.start;
    if (deltaLine < 0 || deltaStart < 0) {
      throw new SemanticTokenContractError(
        "Rust returned semantic tokens that cannot be delta encoded."
      );
    }

    data.push(deltaLine, deltaStart, token.length, tokenType, 1 << modifierIndex);
    previousLine = token.line;
    previousStart = token.start;
  }

  return new Uint32Array(data);
}

async function queryOr<Query extends EditorWorkerQuery, Fallback>(
  client: MermanLanguageWorkerClient,
  model: editor.ITextModel,
  query: Query,
  token: EditorCancellationToken | undefined,
  fallback: Fallback
) {
  try {
    return await client.query(snapshotForModel(model), query, token);
  } catch (error) {
    if (isExpectedDiscard(error)) return fallback;
    throw error;
  }
}

function snapshotForModel(model: editor.ITextModel): EditorDocumentSnapshot {
  return {
    uri: model.uri.toString(),
    version: model.getVersionId(),
    source: model.getValue(),
  };
}

function toEditorPosition(position: {
  lineNumber: number;
  column: number;
}): { line: number; character: number } {
  return { line: position.lineNumber - 1, character: position.column - 1 };
}

function toEditorCompletionItem(
  monaco: typeof import("monaco-editor"),
  item: EditorCompletionItem,
  position: { lineNumber: number; column: number }
): languages.CompletionItem {
  const fallbackRange = new monaco.Range(
    position.lineNumber,
    position.column,
    position.lineNumber,
    position.column
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
  kind: EditorCompletionItem["kind"]
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
  action: EditorCodeAction
): languages.CodeAction {
  return {
    title: action.title,
    kind: action.kind,
    diagnostics: action.diagnostics.map((diagnostic) =>
      toMarkerData(monaco, model, diagnostic)
    ),
    edit: toMonacoWorkspaceEdit(monaco, model, action.edit),
    isPreferred: action.isPreferred,
  };
}

function toMonacoWorkspaceEdit(
  monaco: typeof import("monaco-editor"),
  model: editor.ITextModel,
  edit: EditorWorkspaceEdit
): languages.WorkspaceEdit {
  const managedUri = model.uri.toString();
  const entries = Object.entries(edit.changes);
  const unmanaged = entries.find(([uri]) => uri !== managedUri);
  if (unmanaged) {
    throw new UnmanagedDocumentEditError(
      `Rename is limited to the current document; received an edit for ${unmanaged[0]}.`
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
      }))
    ),
  };
}

function toMonacoLocation(
  monaco: typeof import("monaco-editor"),
  model: editor.ITextModel,
  location: EditorLocation
): languages.Location {
  if (location.uri !== model.uri.toString()) {
    throw new UnmanagedDocumentEditError(
      `Navigation is limited to the current document; received ${location.uri}.`
    );
  }
  return { uri: model.uri, range: toMonacoRange(monaco, location.range) };
}

function toMonacoDocumentSymbol(
  monaco: typeof import("monaco-editor"),
  symbol: EditorDocumentSymbol
): languages.DocumentSymbol {
  return {
    name: symbol.name,
    detail: symbol.detail ?? "",
    kind: symbolKind(monaco, symbol.kind),
    tags: [],
    range: toMonacoRange(monaco, symbol.range),
    selectionRange: toMonacoRange(monaco, symbol.selectionRange),
    children: symbol.children.map((child) =>
      toMonacoDocumentSymbol(monaco, child)
    ),
  };
}

function updateMermaidEditorMarkers(
  monaco: typeof import("monaco-editor"),
  model: editor.ITextModel,
  diagnostics: EditorDiagnostic[]
): void {
  monaco.editor.setModelMarkers(
    model,
    MARKER_OWNER,
    diagnostics.map((diagnostic) => toMarkerData(monaco, model, diagnostic))
  );
}

function clearMermaidMarkers(
  monaco: typeof import("monaco-editor"),
  model: editor.ITextModel
): void {
  monaco.editor.setModelMarkers(model, MARKER_OWNER, []);
}

function toMarkerData(
  monaco: typeof import("monaco-editor"),
  model: editor.ITextModel,
  diagnostic: EditorDiagnostic
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
    code:
      typeof diagnostic.code === "number"
        ? String(diagnostic.code)
        : diagnostic.code,
    relatedInformation: relatedRanges.map(({ related, range: relatedRange }) => ({
      resource: model.uri,
      message: related.message,
      startLineNumber: relatedRange.startLineNumber,
      startColumn: relatedRange.startColumn,
      endLineNumber: relatedRange.endLineNumber,
      endColumn: relatedRange.endColumn,
    })),
  };
}

function markerMatchesDiagnostic(
  monaco: typeof import("monaco-editor"),
  model: editor.ITextModel,
  marker: editor.IMarkerData,
  diagnostic: EditorDiagnostic
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
  range: EditorRange
): InstanceType<typeof monaco.Range> {
  return new monaco.Range(
    range.start.line + 1,
    range.start.character + 1,
    range.end.line + 1,
    Math.max(range.end.character + 1, 1)
  );
}

function toMonacoDisplayRange(
  monaco: typeof import("monaco-editor"),
  range: EditorRange
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
      monacoRange.endColumn + 1
    );
  }
  return monacoRange;
}

function diagnosticSeverity(
  monaco: typeof import("monaco-editor"),
  severity: EditorDiagnostic["severity"]
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
  kind: EditorSymbolKind
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
    (error.name === "AbortError" || error.name === "StaleDocumentError")
  );
}

function reportEditorWorkerFailure(error: unknown): void {
  if (!isExpectedDiscard(error)) {
    console.error("Merman editor language worker failed", error);
  }
}

class UnmanagedDocumentEditError extends Error {}
class SemanticTokenContractError extends Error {}
